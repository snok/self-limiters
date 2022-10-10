extern crate redis;

use crate::_errors::SLError;
use log::{debug, info};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{AsyncCommands, Client};
use std::sync::mpsc::Receiver;

use crate::_utils::{
    get_script, now_millis, send_shared_state, validate_redis_url, SLResult, REDIS_KEY_PREFIX,
};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub(crate) struct ThreadState {
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
#[pyclass]
// #[pyclass(frozen)]  # <-- TODO: Add in when on pyo3 v0.17
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
    if get_script("src/scripts/create_semaphore.lua")?
        .key(&ts.name)
        .arg(ts.capacity)
        .invoke_async(&mut connection)
        .await?
    {
        info!(
            "Created new semaphore queue with a capacity of {}",
            &ts.capacity
        );
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

    // Define queue if it doesn't exist
    get_script("src/scripts/release_semaphore.lua")?
        .key(&ts.name)
        .invoke_async(&mut connection)
        .await?;

    debug!("Released semaphore");
    Ok(())
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        max_sleep: Option<f32>,
        redis_url: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            capacity,
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            max_sleep: max_sleep.unwrap_or(0.0),
            client: validate_redis_url(redis_url)?,
        })
    }

    fn __aenter__<'p>(slf: PyRef<Self>, py: Python<'p>) -> PyResult<&'p PyAny> {
        let receiver = send_shared_state(ThreadState::from(&slf))?;

        future_into_py(py, async {
            Ok(create_and_acquire_semaphore(receiver).await?)
        })
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
