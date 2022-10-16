extern crate redis;

use crate::errors::SLError;
use log::{debug, info};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{AsyncCommands, Client, Script};
use std::sync::mpsc::Receiver;

use crate::utils::{now_millis, send_shared_state, validate_redis_url, SLResult, REDIS_KEY_PREFIX};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) client: Client,
    pub(crate) name: String,
    pub(crate) capacity: u32,
    pub(crate) max_sleep: f32,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>) -> Self {
        Self {
            client: slf.client.clone(),
            name: slf.name.clone(),
            capacity: slf.capacity,
            max_sleep: slf.max_sleep,
        }
    }
}

/// Async context manager useful for controlling client traffic
/// in situations where you need to limit traffic to `n` requests concurrently.
/// For example, when you can only have 2 active requests simultaneously.
#[pyclass(frozen)]
#[pyo3(name = "Semaphore")]
#[pyo3(module = "self_limiters")]
pub struct Semaphore {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    max_sleep: f32,
    client: Client,
}

async fn create_and_acquire_semaphore(receiver: Receiver<ThreadState>) -> SLResult<()> {
    // Retrieve thread state struct
    let ts = receiver.recv()?;

    // Connect to redis
    let mut connection = ts.client.get_async_connection().await?;

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
        ---
        --- args:
        --- * capacity: The capacity of the semaphore (i.e., the length of the list)
        ---
        --- returns:
        --- * 1 if created, else 0 (but the return value isn't used; only useful for debugging)

        redis.replicate_commands()

        -- Init config variables
        local key = tostring(KEYS[1])
        local capacity = tonumber(ARGV[1])

        -- Check if list exists
        -- SETNX does in one call what we would otherwise do in two
        --
        -- One thing to note about this call is that we would rather not do this. It would
        -- be much more intuitive to call EXISTS on the list key and create the list if it
        -- did not exist. Unfortunately, if you create a list with 3 elements, and you pop
        -- all three elements (popping == acquiring the semaphore), the list stops 'existing'
        -- once empty. In other words, EXISTS is not viable, so this is a workaround.
        -- If you have better suggestions for how to achieve this, please submit a PR.
        local does_not_exist = redis.call('SETNX', string.format('(%s)-exists', key), 1)

        -- Create the list if none exists
        if does_not_exist == 1 then
            -- Add '1' as an argument equal to the capacity of the semaphore
            -- In other words, if we passed capacity 5 here, this should
            -- generate `{RPUSH, 1, 1, 1, 1, 1}`.
            -- The values we push to the list are arbitrary.
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
    .arg(ts.capacity)
    .invoke_async(&mut connection)
    .await?
    {
        info!("Created new semaphore queue with a capacity of {}", &ts.capacity);
    }

    // Wait for our turn - this waits non-blockingly until we're free to proceed
    let start = now_millis()?;
    connection.blpop(&ts.name, ts.max_sleep as usize).await?;

    // Raise an exception if we waited too long
    if ts.max_sleep > 0.0 && (now_millis()? - start) > (ts.max_sleep * 1000.0) as u64 {
        return Err(SLError::MaxSleepExceeded(
            "Max sleep exceeded when waiting for Semaphore".to_string(),
        ));
    };

    debug!("Acquired semaphore");
    Ok(())
}

async fn release_semaphore(receiver: Receiver<ThreadState>) -> SLResult<()> {
    let ts = receiver.recv()?;

    // Connect to redis
    let mut connection = ts.client.get_async_connection().await?;

    // Push capacity back to the semaphore
    // *We don't care about this being atomic
    redis::pipe()
        .lpush(&ts.name, 1)
        .expire(&ts.name, 30)
        .expire(format!("{}-exists", &ts.name), 30)
        .query_async(&mut connection)
        .await?;

    debug!("Released semaphore");
    Ok(())
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(name: String, capacity: u32, max_sleep: Option<f32>, redis_url: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            capacity,
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            max_sleep: max_sleep.unwrap_or(0.0),
            client: validate_redis_url(redis_url)?,
        })
    }

    fn __aenter__<'p>(slf: PyRef<Self>, py: Python<'p>) -> PyResult<&'p PyAny> {
        let receiver = send_shared_state(ThreadState::from(&slf))?;

        future_into_py(py, async { Ok(create_and_acquire_semaphore(receiver).await?) })
    }

    /// Return capacity to the Semaphore on exit.
    #[args(_a = "*")]
    fn __aexit__<'p>(slf: PyRef<Self>, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        let receiver = send_shared_state(ThreadState::from(&slf))?;
        future_into_py(py, async { Ok(release_semaphore(receiver).await?) })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
