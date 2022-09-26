use crate::MaxSleepExceededError;
use crate::_errors::SLError;
use crate::_utils::{
    get_script, get_script_sha, now_millis, receive_shared_state, redis_get, redis_set,
    send_shared_state, Redis, REDIS_KEY_PREFIX,
};
use log::{debug, info};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use std::pin::Pin;

mod redis {
    // Import the Python redis.NoScriptError exception
    pyo3::import_exception!(redis, NoScriptError);
}

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) redis_url: String,
    pub(crate) name: String,
    pub(crate) capacity: u32,
    pub(crate) max_sleep: u32,
    pub(crate) redis: Redis,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>, redis: Redis) -> Self {
        Self {
            name: slf.name.clone(),
            capacity: slf.capacity,
            redis_url: slf.redis_url.clone(),
            max_sleep: slf.max_sleep,
            redis,
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
        // Create struct containing an redis.asyncio.Redis class instance
        let redis_instance: Py<PyAny> = PyModule::from_code(
            py,
            &format!(
                "from redis.asyncio import Redis\nr = Redis.from_url('{}')",
                slf.redis_url
            ),
            "",
            "",
        )?
        .getattr("r")?
        .into();

        let receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(
            &slf,
            Redis {
                redis: redis_instance,
            },
        ))?;

        future_into_py(py, async {
            // Collect thread state struct

            // Call Lua script, using redis class instance, to define
            // a queue for the semaphore if it doesn't already exist.
            // This operation is idempotent, so can be safely called
            // by every consumer of the semaphore.

            // Read script content from file
            let script_content = get_script("src/scripts/create_semaphore.lua");

            // Create script hash
            let script_sha = Python::with_gil(|py| get_script_sha(py, &script_content));

            println!("Script SHA is {}", script_sha);

            Python::with_gil(|py| {
                let ts = receive_shared_state::<ThreadState, SLError>(receiver).unwrap();
                let locals = pyo3_asyncio::TaskLocals::with_running_loop(py)?.copy_context(py)?;
                redis_set(&ts.redis.redis, py, "test", 1)?;
                let r = redis_get(&ts.redis.redis, py, &locals, "test").await?;
                // println!("Saved value was {:?}", r);
                Ok::<(), PyErr>(())
            })?;

            // // Try to execute Redis script.
            // // If the script has been called before, all we need to do is call `EVALSHA`
            // // with the hash from above. Otherwise, we will need to call `EVAL` with the script
            // // contents.
            // // If the script *hasn't* been run before, we will receive a NoScriptError exception.
            // match redis.evalsha(&script_sha, 1, vec![&ts.capacity.to_string()]) {
            //     Ok(_) => (),
            //     Err(err) => {
            //         // If it hasn't been run before, fall back to `EVAL`.
            //         if err.is_instance_of::<redis::NoScriptError>(py) {
            //             redis.eval(&script_content, 1, vec![key, &ts.capacity.to_string()])?;
            //         } else {
            //             return err;
            //         }
            //     }
            // }
            //
            // // We now know that the semaphore queue exists, so we proceed to call `BLPOP`
            // // which will async block (i.e., non-blocking) until there is something in the
            // // queue to pop.
            // let result = match redis.blpop(&ts.name, ts.max_sleep) {
            //     Some(_) => {
            //         debug!("Acquired semaphore");
            //         Ok(())
            //     }
            //     None => Err(MaxSleepExceededError::new_err(
            //         "Max sleep exceeded when waiting for Semaphore".to_string(),
            //     )),
            // };
            // result
            Ok(())
        })
    }

    #[args(_a = "*")]
    fn __aexit__<'a>(slf: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        // let receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(&slf))?;
        future_into_py(py, async {
            // let ts = receive_shared_state::<ThreadState, SLError>(receiver)?;

            // let redis = Python::with_gil(|py| Redis::new(py, &ts.redis_url));

            // // Define queue if it doesn't exist
            // let script_content = get_script("src/scripts/release_semaphore.lua");
            // let script_sha = get_script_sha(py, &script_content);
            //

            debug!("Released semaphore");
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
