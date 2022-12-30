use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use log::{debug, info};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{AsyncCommands, Script};

use crate::errors::SLError;
use crate::generated::SEMAPHORE_SCRIPT;
use crate::utils::{create_connection_manager, create_connection_pool, now_millis, SLResult, REDIS_KEY_PREFIX};

struct ThreadState {
    open_connection_pool: Pool<RedisConnectionManager>,
    return_connection_pool: Pool<RedisConnectionManager>,
    name: String,
    expiry: usize,
    capacity: u32,
    max_sleep: f32,
}

impl ThreadState {
    fn from(slf: &Semaphore) -> Self {
        Self {
            open_connection_pool: slf.open_connection_pool.clone(),
            return_connection_pool: slf.return_connection_pool.clone(),
            name: slf.name.clone(),
            expiry: slf.expiry,
            capacity: slf.capacity,
            max_sleep: slf.max_sleep,
        }
    }

    /// Key (re)use in Lua scripts to determine if Semaphore exists or not
    fn exists_key(&self) -> String {
        format!("{}-exists", self.name)
    }
}

async fn create_and_acquire_semaphore(ts: ThreadState) -> SLResult<()> {
    // Connect to redis
    let mut connection = ts.open_connection_pool.get().await?;

    // Define queue if it doesn't already exist
    if Script::new(SEMAPHORE_SCRIPT)
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

async fn release_semaphore(ts: ThreadState) -> SLResult<()> {
    // Connect to redis
    let mut connection = ts.return_connection_pool.get().await?;

    // Push capacity back to the semaphore
    // We don't care about this being atomic
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
pub(crate) struct Semaphore {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    max_sleep: f32,
    #[pyo3(get)]
    expiry: usize,
    open_connection_pool: Pool<RedisConnectionManager>,
    return_connection_pool: Pool<RedisConnectionManager>,
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
        let open_manager = create_connection_manager(redis_url)?;
        let return_manager = create_connection_manager(redis_url)?;

        // Create connection pool
        let open_pool = create_connection_pool(open_manager, connection_pool_size.unwrap_or(15))?;
        let return_pool = create_connection_pool(return_manager, connection_pool_size.unwrap_or(15))?;

        Ok(Self {
            capacity,
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            max_sleep: max_sleep.unwrap_or(0.0),
            expiry: expiry.unwrap_or(30),
            open_connection_pool: open_pool,
            return_connection_pool: return_pool,
        })
    }

    fn __aenter__<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        let ts = ThreadState::from(self);
        future_into_py(py, async { Ok(create_and_acquire_semaphore(ts).await?) })
    }

    #[args(_a = "*")]
    fn __aexit__<'p>(&self, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        let ts = ThreadState::from(self);
        future_into_py(py, async { Ok(release_semaphore(ts).await?) })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
