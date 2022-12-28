pub extern crate redis;

use crate::errors::SLError;
use bb8_redis::bb8::Pool;
use log::{debug, info};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{AsyncCommands, Script};

use bb8_redis::RedisConnectionManager;
use tokio::sync::Mutex;

use crate::utils::{create_connection_manager, create_connection_pool, now_millis, SLResult, REDIS_KEY_PREFIX};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) connection_pool: Pool<RedisConnectionManager>,
    pub(crate) name: String,
    pub(crate) expiry: usize,
    pub(crate) capacity: u32,
    pub(crate) max_sleep: f32,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>) -> Self {
        Self {
            connection_pool: slf.connection_pool.clone(),
            name: slf.name.clone(),
            expiry: slf.expiry,
            capacity: slf.capacity,
            max_sleep: slf.max_sleep,
        }
    }

    /// Key (re)use in Lua scripts to determine if Semaphore exists or not
    pub(crate) fn exists_key(&self) -> String {
        format!("{}-exists", self.name)
    }
}

async fn create_and_acquire_semaphore(m: Mutex<ThreadState>) -> SLResult<()> {
    let ts = m.lock().await;

    // Connect to redis
    let mut connection = ts.connection_pool.get().await?;

    // Define queue if it doesn't already exist
    if Script::new(
        r"
        --- Script called from the Semaphore implementation.
        ---
        --- Lua scripts are run atomically by default, and since redis
        --- is single threaded, there are no race conditions to worry about.
        ---
        --- The script checks if a list exists for the Semaphore, and
        --- creates one of length `capacity` if it doesn't.
        ---
        --- keys:
        --- * key: The key to use for the list
        --- * existskey: The key to use for the string we use to check if the lists exists
        ---
        --- args:
        --- * capacity: The capacity of the semaphore (i.e., the length of the list)
        ---
        --- returns:
        --- * 1 if created, else 0 (but the return value isn't used; only useful for debugging)

        redis.replicate_commands()

        -- Init config variables
        local key = tostring(KEYS[1])
        local existskey = tostring(KEYS[2])
        local capacity = tonumber(ARGV[1])

        -- Check if list exists
        -- Note, we cannot use EXISTS or LLEN below, as we need
        -- to know if a list exists, but has capacity zero.
        local does_not_exist = redis.call('SETNX', string.format(existskey, key), 1)

        -- Create the list if none exists
        if does_not_exist == 1 then
          -- Add '1' as an argument equal to the capacity of the semaphore
          -- If capacity is 5 here, we generate `{RPUSH, 1, 1, 1, 1, 1}`.
          local args = {'RPUSH', key}
          for _=1,capacity do
            table.insert(args, 1)
          end
          redis.call(unpack(args))
          return true
        end
        return false",
    )
    .key(&ts.name)
    .key(&ts.exists_key())
    .arg(ts.capacity)
    .invoke_async(&mut *connection)
    .await?
    {
        info!("Created new semaphore queue with a capacity of {}", &ts.capacity);
    } else {
        debug!("Skipped creating new semaphore queue, since one exists already")
    }

    // Wait for our turn - this waits non-blockingly until we're free to proceed
    let start = now_millis()?;
    connection.blpop(&ts.name, ts.max_sleep as usize).await?;

    // Raise an exception if we waited too long
    if ts.max_sleep > 0.0 && (now_millis()? - start) > (ts.max_sleep * 1000.0) as u64 {
        return Err(SLError::MaxSleepExceeded(
            "Max sleep exceeded waiting for Semaphore".to_string(),
        ));
    };

    debug!("Acquired semaphore");
    Ok(())
}

async fn release_semaphore(m: Mutex<ThreadState>) -> SLResult<()> {
    let ts = m.lock().await;

    // Connect to redis
    let mut connection = ts.connection_pool.get().await?;

    // Push capacity back to the semaphore
    // *We don't care about this being atomic
    redis::pipe()
        .lpush(&ts.name, 1)
        .expire(&ts.name, ts.expiry)
        .expire(&ts.exists_key(), ts.expiry)
        .query_async(&mut *connection)
        .await?;

    debug!("Released semaphore");
    Ok(())
}

/// Async context manager useful for controlling client traffic
/// in situations where you need to limit traffic to `n` requests concurrently.
/// For example, when you can only have 2 active requests simultaneously.
#[pyclass(frozen)]
#[pyo3(name = "Semaphore")]
#[pyo3(module = "self_limiters")]
pub struct Semaphore {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    max_sleep: f32,
    #[pyo3(get)]
    expiry: usize,
    connection_pool: Pool<RedisConnectionManager>,
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        max_sleep: Option<f32>,
        expiry: Option<usize>,
        redis_url: Option<&str>,
        connection_pool_size: Option<u32>,
    ) -> PyResult<Self> {
        debug!("Creating new Semaphore instance");

        // Create redis connection manager
        let manager = create_connection_manager(redis_url)?;

        // Create connection pool
        let pool = create_connection_pool(manager, connection_pool_size.unwrap_or(15))?;

        Ok(Self {
            capacity,
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            max_sleep: max_sleep.unwrap_or(0.0),
            expiry: expiry.unwrap_or(30),
            connection_pool: pool,
        })
    }

    fn __aenter__<'p>(slf: PyRef<Self>, py: Python<'p>) -> PyResult<&'p PyAny> {
        let m = Mutex::new(ThreadState::from(&slf));
        future_into_py(py, async { Ok(create_and_acquire_semaphore(m).await?) })
    }

    #[args(_a = "*")]
    fn __aexit__<'p>(slf: PyRef<Self>, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        let m = Mutex::new(ThreadState::from(&slf));
        future_into_py(py, async { Ok(release_semaphore(m).await?) })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
