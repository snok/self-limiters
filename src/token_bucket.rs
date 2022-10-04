use std::time::Duration;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyAny, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use redis::Client;

use crate::RedisError;
use crate::_errors::SLError;
use crate::_utils::{
    get_script, now_millis, open_client_connection, receive_shared_state, send_shared_state,
    validate_redis_url, SLResult, REDIS_KEY_PREFIX,
};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the token bucket itself, but this seemed simpler.
pub(crate) struct ThreadState {
    pub(crate) capacity: u32,
    pub(crate) frequency: f32,
    pub(crate) amount: u32,
    pub(crate) max_sleep: f64,
    pub(crate) client: Client,
    pub(crate) name: String,
}

impl ThreadState {
    fn from(slf: &PyRef<TokenBucket>) -> Self {
        Self {
            capacity: slf.capacity,
            frequency: slf.refill_frequency,
            amount: slf.refill_amount,
            max_sleep: slf.max_sleep,
            client: slf.client.clone(),
            name: slf.name.clone(),
        }
    }
}

pub async fn sleep_for(sleep_duration: Duration, max_sleep: Duration) -> SLResult<()> {
    if max_sleep.as_secs_f32() > 0.0 && sleep_duration > max_sleep {
        return Err(SLError::MaxSleepExceeded(format!(
            "Received wake up time in {} seconds, which is \
            greater or equal to the specified max sleep of {} seconds",
            sleep_duration.as_secs(),
            max_sleep.as_secs()
        )));
    }

    tokio::time::sleep(sleep_duration).await;
    Ok(())
}

/// Async context manager useful for controlling client traffic
/// in situations where you need to limit traffic to `n` requests per `m` unit of time.
/// For example, when you can only send 1 request per minute.
#[pyclass]
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
    max_sleep: f64,
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
        max_sleep: Option<f64>,
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
    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState>(ThreadState::from(&slf))?;

        future_into_py(py, async {
            let ts = receive_shared_state(receiver)?;

            // Connect to redis
            let mut connection = open_client_connection(&ts.client).await?;

            // Retrieve slot
            let slot: u64 = get_script("src/scripts/schedule.lua")?
                .key(&ts.name)
                .arg(ts.capacity) // capacity
                .arg((ts.frequency * 1000.0) as u64) // refill rate in ms
                .arg(ts.amount) // refill amount
                .invoke_async(&mut connection)
                .await
                .map_err(|e| RedisError::new_err(e.to_string()))?;

            let now = now_millis()?;
            let sleep_duration = {
                if slot <= now {
                    Duration::from_millis(0)
                } else {
                    Duration::from_millis((slot - now) as u64)
                }
            };
            sleep_for(sleep_duration, Duration::from_secs_f64(ts.max_sleep)).await?;
            Ok(())
        })
    }

    /// Do nothing on aexit.
    #[args(_a = "*")]
    fn __aexit__<'a>(_s: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        future_into_py(py, async { Ok(()) })
    }

    fn __repr__(&self) -> String {
        format!("Token bucket instance for queue {}", &self.name)
    }
}
