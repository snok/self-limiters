use std::time::Duration;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyAny, PyErr, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use redis::{parse_redis_url, Client};

use crate::token_bucket::utils::{now_millis, sleep_for};
use crate::utils::{get_script, open_client_connection, receive_shared_state, send_shared_state};
use crate::RedisError;
use error::TokenBucketError;

pub(crate) mod error;
pub(crate) mod utils;

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the token bucket itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) client: Client,
    pub(crate) name: String,
    pub(crate) capacity: u32,
    pub(crate) frequency: f32,
    pub(crate) amount: u32,
    pub(crate) max_sleep: Duration,
}

impl ThreadState {
    fn from(slf: &PyRef<TokenBucket>) -> ThreadState {
        ThreadState {
            capacity: slf.capacity,
            frequency: slf.refill_frequency,
            amount: slf.refill_amount,
            client: slf.client.clone(),
            max_sleep: slf.max_sleep,
            name: slf.name.clone(),
        }
    }
}

#[pyclass]
#[pyo3(name = "TokenBucket")]
#[pyo3(module = "timely")]
pub struct TokenBucket {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    refill_frequency: f32,
    #[pyo3(get)]
    refill_amount: u32,
    #[pyo3(get)]
    name: String,
    max_sleep: Duration,
    client: Client,
}

#[pymethods]
impl TokenBucket {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: i64,
        refill_frequency: f32,
        refill_amount: i64,
        redis_url: Option<&str>,
        max_sleep: Option<f64>,
    ) -> PyResult<Self> {
        let url = match parse_redis_url(redis_url.unwrap_or("redis://127.0.0.1:6379")) {
            Some(url) => url,
            None => {
                return Err(PyErr::from(TokenBucketError::Redis(String::from(
                    "Failed to parse redis url",
                ))));
            }
        };
        let client = match Client::open(url) {
            Ok(client) => client,
            Err(e) => {
                return Err(PyErr::from(TokenBucketError::Redis(format!(
                    "Failed to connect to redis: {}",
                    e
                ))));
            }
        };

        if refill_frequency <= 0.0 {
            return Err(PyValueError::new_err(
                "Refill frequency must be greater than 0",
            ));
        }
        if refill_amount <= 0 {
            return Err(PyValueError::new_err(
                "Refill amount must be greater than 0",
            ));
        }
        Ok(Self {
            capacity: capacity as u32,
            client,
            refill_amount: refill_amount as u32,
            refill_frequency,
            max_sleep: Duration::from_millis((max_sleep.unwrap_or(0.0)) as u64),
            name: format!("__timely-{}", name),
        })
    }

    /// Spawn a scheduler thread to schedule wake-up times for nodes,
    /// and let the main thread wait for assignment of wake-up time
    /// then sleep until ready.
    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, TokenBucketError>(ThreadState::from(&slf))?;

        future_into_py(py, async {
            let ts = receive_shared_state::<ThreadState, TokenBucketError>(receiver)?;

            // Connect to redis
            let mut connection =
                open_client_connection::<&Client, TokenBucketError>(&ts.client).await?;

            // Retrieve slot
            let slot: u64 = get_script("src/schedule.lua")
                .key(&ts.name)
                .arg(ts.capacity) // capacity
                .arg((ts.frequency * 1000.0) as u64) // refill rate in ms
                .arg(ts.amount) // refill amount
                .invoke_async(&mut connection)
                .await
                .map_err(|e| RedisError::new_err(e.to_string()))?;

            let now = now_millis();
            let sleep_duration = {
                if slot <= now {
                    Duration::from_millis(0)
                } else {
                    Duration::from_millis((slot - now) as u64)
                }
            };
            sleep_for(sleep_duration, ts.max_sleep).await?;
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
