use log::{debug, info};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;

use crate::MaxSleepExceededError;
use crate::_errors::SLError;
use crate::_utils::{get_redis, get_script, get_script_sha, now_millis, receive_shared_state, Redis, REDIS_KEY_PREFIX, run_script, send_shared_state};

mod redis {
    pyo3::import_exception!(redis, NoScriptError);
}

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) redis_url: String,
    pub(crate) name: String,
    pub(crate) capacity: u32,
    pub(crate) max_sleep: u32,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>) -> Self {
        Self {
            name: slf.name.clone(),
            capacity: slf.capacity,
            redis_url: slf.redis_url.clone(),
            max_sleep: slf.max_sleep,
        }
    }
}

/// Async context manager useful for enforcing police client traffic
/// when dealing with a concurrency-based external rate limit. For example,
/// when you can only have 2 active requests simultaneously.
#[pyclass]
#[pyo3(name = "Semaphore")]
#[pyo3(module = "self_limiters")]
pub struct Semaphore {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    max_sleep: u32,
    redis_url: String,
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        max_sleep: Option<u32>,
        redis_url: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            capacity,
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            max_sleep: max_sleep.unwrap_or(0),
            redis_url: redis_url.unwrap_or("redis://127.0.0.1:6379").to_string(),
        })
    }

    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(&slf))?;

        future_into_py(py, async {
            // Retrieve thread state struct
            let ts = receive_shared_state::<ThreadState, SLError>(receiver).unwrap();

            let redis = Python::with_gil(|py| {
                get_redis(py, &ts.redis_url)
            });

            // Define queue if it doesn't already exist
            let script_content = get_script("src/scripts/create_semaphore.lua");
            let script_sha = get_script_sha(py, &script_content);

            // Try to execute Redis script. If it hasn't been run before,
            // we will receive a returned NoScriptError exception.
            match redis.evalsha(&script_sha, 1, vec![capacity]) {
                Ok(_) => (),
                Err(err) => {
                    // If it has not been run before, run it using `EVAL` instead.
                    if err.is_instance_of::<redis::NoScriptError>(py) {
                        redis.eval(&script_content, 1, vec![key, capacity])?;
                    } else {
                        return err;
                    }
                }
            }

            // Wait for our turn - this waits non-blockingly until we're free to proceed
            let start = now_millis();
            redis.blpop(&ts.name, ts.max_sleep);

            // Raise an exception if we waited too long
            if ts.max_sleep != 0 && (now_millis() - start) > (ts.max_sleep as f64 * 1000.0) as u64 {
                return Err(MaxSleepExceededError::new_err(
                    "Max sleep exceeded when waiting for Semaphore".to_string(),
                ));
            };

            debug!("Acquired semaphore");
            Ok(())
        })
    }

    #[args(_a = "*")]
    fn __aexit__<'a>(slf: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(&slf))?;
        future_into_py(py, async {
            let ts = receive_shared_state::<ThreadState, SLError>(receiver)?;

            let redis = Python::with_gil(|py| {
                get_redis(py, &ts.redis_url)
            });

            // Define queue if it doesn't exist
            let script_content = get_script("src/scripts/release_semaphore.lua");
            let script_sha = get_script_sha(py, &script_content);
            run_script(py, &script_content, &script_sha, &ts.name, &ts.capacity, redis);

            debug!("Released semaphore");
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
