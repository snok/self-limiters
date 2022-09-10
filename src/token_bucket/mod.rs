use std::thread;
use std::time::Duration;

use nanoid::nanoid;

use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyAny, PyErr, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use redis::{parse_redis_url, Client};

use crate::utils::{receive_shared_state, send_shared_state};
use error::TokenBucketError;
use logic::{schedule, wait_for_slot};

pub(crate) mod data;
pub(crate) mod error;
pub(crate) mod logic;
pub(crate) mod utils;

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the token bucket itself, but this seemed simpler.
pub struct ThreadState {
    client: Client,
    name: String,
    queue_key: String,
    data_key: String,
    id: String,
    capacity: u32,
    max_sleep: Duration,
    frequency: f32,
    amount: u32,
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
            id: slf.id.clone(),
            data_key: slf.data_key.clone(),
            queue_key: slf.queue_key.clone(),
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
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    queue_key: String,
    max_sleep: Duration,
    client: Client,
    data_key: String,
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
        let url = match parse_redis_url(redis_url.unwrap_or("redis://127.0.0.1:6379")) {
            Some(url) => url,
            None => {
                return Err(PyErr::from(TokenBucketError::Redis(String::from(
                    "Failed to parse redis url",
                ))));
            }
        };
        let client = Client::open(url).expect("Failed to connect to Redis");
        let id = nanoid!(10);
        let data_key = format!("__timely-{}-data", name);
        let queue_key = format!("__timely-{}-queue", name);
        Ok(Self {
            capacity,
            client,
            refill_amount,
            refill_frequency,
            max_sleep: Duration::from_millis((max_sleep.unwrap_or(0.0)) as u64),
            id,
            name,
            data_key,
            queue_key,
        })
    }

    /// Spawn a scheduler thread to schedule wake-up times for nodes,
    /// and let the main thread wait for assignment of wake-up time
    /// then sleep until ready.
    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        // Spawn a thread to run scheduling
        let r1 = send_shared_state::<ThreadState, TokenBucketError>(ThreadState::from(&slf))?;

        py.allow_threads(move || {
            thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let shared_state =
                            receive_shared_state::<ThreadState, TokenBucketError>(r1).unwrap();
                        schedule(shared_state).await.unwrap();
                    });
            });
        });

        // Return future for the python event loop
        let r2 = send_shared_state::<ThreadState, TokenBucketError>(ThreadState::from(&slf))?;
        future_into_py(py, async {
            let shared_state = receive_shared_state::<ThreadState, TokenBucketError>(r2)?;
            wait_for_slot(shared_state).await?;
            Ok(())
        })
    }

    /// Do nothing on aexit.
    #[args(_a = "*")]
    fn __aexit__<'a>(_s: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        future_into_py(py, async { Ok(()) })
    }

    /// Create a string representation of the class. Without
    /// this, the class is printed as builtin.TokenBucket, which
    /// is pretty confusing.
    fn __repr__(&self) -> String {
        format!(
            "Token bucket instance {} for queue {}",
            &self.id, &self.queue_key
        )
    }
}
