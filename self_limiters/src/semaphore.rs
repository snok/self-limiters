use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::Client;

use crate::errors::PSLError;
use self_limiters_core::semaphore::{create_and_acquire_semaphore, release_semaphore, ThreadState};

use crate::utils::{send_shared_state, validate_redis_url, REDIS_KEY_PREFIX};

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
        let ts = ThreadState {
            client: slf.client.clone(),
            name: slf.name.clone(),
            capacity: slf.capacity,
            max_sleep: slf.max_sleep,
        };
        let receiver = send_shared_state(ts)?;

        future_into_py(py, async {
            Ok(create_and_acquire_semaphore(receiver)
                .await
                .map_err(PSLError::from)?)
        })
    }

    /// Return capacity to the Semaphore on exit.
    #[args(_a = "*")]
    fn __aexit__<'p>(slf: PyRef<Self>, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        let ts = ThreadState {
            client: slf.client.clone(),
            name: slf.name.clone(),
            capacity: slf.capacity,
            max_sleep: slf.max_sleep,
        };
        let receiver = send_shared_state(ts)?;
        future_into_py(py, async {
            Ok(release_semaphore(receiver).await.map_err(PSLError::from)?)
        })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
