use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyAny, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use redis::Client;

use self_limiters_core::token_bucket::{schedule_and_sleep, ThreadState};

use crate::errors::PSLError;
use crate::utils::{send_shared_state, validate_redis_url, REDIS_KEY_PREFIX};

/// Async context manager useful for controlling client traffic
/// in situations where you need to limit traffic to `n` requests per `m` unit of time.
/// For example, when you can only send 1 request per minute.
#[pyclass]
// #[pyclass(frozen)]  # <-- TODO: Add in when on pyo3 v0.17
#[pyo3(name = "TokenBucket")]
#[pyo3(module = "self_limiters")]
pub struct TokenBucket {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    refill_frequency: f32,
    #[pyo3(get)]
    refill_amount: u32,
    #[pyo3(get)]
    name: String,
    max_sleep: f32,
    client: Client,
}

#[pymethods]
impl TokenBucket {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        refill_frequency: f32,
        refill_amount: u32,
        redis_url: Option<&str>,
        max_sleep: Option<f32>,
    ) -> PyResult<Self> {
        if refill_frequency <= 0.0 {
            return Err(PyValueError::new_err(
                "Refill frequency must be greater than 0",
            ));
        }
        Ok(Self {
            capacity,
            refill_amount,
            refill_frequency,
            max_sleep: max_sleep.unwrap_or(0.0),
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            client: validate_redis_url(redis_url)?,
        })
    }

    /// Spawn a scheduler thread to schedule wake-up times for nodes,
    /// and let the main thread wait for assignment of wake-up time
    /// then sleep until ready.
    fn __aenter__<'p>(slf: PyRef<Self>, py: Python<'p>) -> PyResult<&'p PyAny> {
        let ts = ThreadState {
            capacity: slf.capacity,
            frequency: slf.refill_frequency,
            amount: slf.refill_amount,
            max_sleep: slf.max_sleep,
            client: slf.client.clone(),
            name: slf.name.clone(),
        };
        let receiver = send_shared_state(ts)?;
        future_into_py(py, async {
            Ok(schedule_and_sleep(receiver).await.map_err(PSLError::from)?)
        })
    }

    /// Do nothing on aexit.
    #[args(_a = "*")]
    fn __aexit__<'p>(_s: PyRef<Self>, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        future_into_py(py, async { Ok(()) })
    }

    fn __repr__(&self) -> String {
        format!("Token bucket instance for queue {}", &self.name)
    }
}
