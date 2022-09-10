extern crate redis;

use nanoid::nanoid;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{parse_redis_url, Client};

use crate::semaphore::errors::SemaphoreError;
use crate::semaphore::logic::{clean_up, wait_for_slot};
use crate::utils::{receive_shared_state, send_shared_state};

pub(crate) mod errors;
pub(crate) mod logic;
pub(crate) mod utils;

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub(crate) struct ThreadState {
    queue_key: String,
    capacity: u32,
    max_position: u32,
    sleep_duration: f32,
    id: String,
    client: Client,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>) -> ThreadState {
        ThreadState {
            queue_key: slf.queue_key.clone(),
            capacity: slf.capacity,
            max_position: slf.max_position,
            sleep_duration: slf.sleep_duration,
            client: slf.client.clone(),
            id: slf.id.clone(),
        }
    }
}

#[pyclass]
#[pyo3(name = "Semaphore")]
#[pyo3(module = "timely")]
pub struct Semaphore {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    max_position: u32,
    #[pyo3(get)]
    sleep_duration: f32,
    #[pyo3(get)]
    queue_key: String,
    #[pyo3(get)]
    id: String,
    client: Client,
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        redis_url: Option<&str>,
        sleep_duration: Option<f32>,
        max_position: Option<u32>,
    ) -> PyResult<Self> {
        let url = match parse_redis_url(redis_url.unwrap_or("redis://127.0.0.1:6379")) {
            Some(url) => url,
            None => {
                return Err(PyErr::from(SemaphoreError::Redis(String::from(
                    "Failed to parse redis url",
                ))));
            }
        };
        let client = Client::open(url).expect("Failed to connect to Redis");
        let queue_key = format!("__timely-{}-queue", name);
        Ok(Self {
            queue_key,
            capacity,
            client,
            sleep_duration: sleep_duration.unwrap_or(0.1),
            max_position: max_position.unwrap_or(0),
            id: nanoid!(10),
        })
    }

    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, SemaphoreError>(ThreadState::from(&slf))?;
        future_into_py(py, async {
            let shared_state = receive_shared_state::<ThreadState, SemaphoreError>(receiver)?;
            wait_for_slot(shared_state).await?;
            Ok(())
        })
    }

    #[args(_a = "*")]
    fn __aexit__<'a>(slf: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, SemaphoreError>(ThreadState::from(&slf))?;
        future_into_py(py, async {
            let shared_state = receive_shared_state::<ThreadState, SemaphoreError>(receiver)?;
            clean_up(shared_state).await?;
            Ok(())
        })
    }

    /// Create a string representation of the class. Without
    /// this, the class is printed as builtin.Semaphore, which
    /// is pretty confusing.
    fn __repr__(&self) -> String {
        format!(
            "Semaphore instance {} for queue {}",
            &self.id, &self.queue_key
        )
    }
}
