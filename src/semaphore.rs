extern crate redis;

use crate::_errors::SLError;
use crate::{MaxSleepExceededError, RedisError};
use log::{debug, info};
use pyo3::basic::getattr;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{AsyncCommands, Client};
use std::str::FromStr;

use crate::_utils::{
    get_script, now_millis, open_client_connection, receive_shared_state, send_shared_state,
    validate_redis_url, REDIS_KEY_PREFIX,
};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) client: Client,
    pub(crate) name: String,
    pub(crate) capacity: u32,
    pub(crate) max_sleep: u32,
}

impl ThreadState {
    fn from(slf: &PyRef<Semaphore>) -> Self {
        Self {
            name: slf.name.clone(),
            capacity: slf.capacity,
            client: slf.client.clone(),
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
    client: Client,
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
            client: validate_redis_url(redis_url)?,
        })
    }

    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(&slf))?;

        let redis_receiver = send_shared_state::<ThreadState, SLError>(ThreadState::from(&slf))?;

        println!("Trying redis");
        let _ = Python::with_gil(|py| {
            let rs = receive_shared_state::<ThreadState, SLError>(redis_receiver).unwrap();
            // Import redis
            let redis = PyModule::import(py, "redis")
                .unwrap()
                // access .Redis
                .getattr("Redis")
                .unwrap()
                // access .from_url
                .getattr("from_url")
                .unwrap()
                // Pass in redis URL (this is not how we'll do it, but this works for a demo)
                .call1(PyTuple::new(
                    py,
                    vec![format!(
                        "redis://{}",
                        rs.client.get_connection_info().addr.to_string()
                    )],
                ))
                .unwrap();
            // Set value
            let _ = redis.getattr("set").unwrap().call1(("key", 1)).unwrap();
            // Get value
            let result: Option<Vec<u8>> = redis
                .getattr("get")
                .unwrap()
                .call1(("key",))
                .unwrap()
                .extract()
                .unwrap();

            println!("{:?}", redis);
            if result.is_some() {
                // Unpack Option
                let temp = result.unwrap();
                // Parse string from vector of bytes (Vec<u8>)
                let decoded_result = String::from_utf8_lossy(&temp);
                // Parse i32 from string
                let int_result = i32::from_str(&decoded_result).unwrap();
                println!("{:?}", int_result);
            }
        });

        future_into_py(py, async {
            // Retrieve thread state struct
            let ts = receive_shared_state::<ThreadState, SLError>(receiver)?;

            // Connect to redis
            let mut connection = open_client_connection(&ts.client).await?;

            // Define queue if it doesn't already exist
            if get_script("src/scripts/create_semaphore.lua")
                .key(&ts.name)
                .arg(ts.capacity)
                .invoke_async(&mut connection)
                .await
                .map_err(|e| RedisError::new_err(e.to_string()))?
            {
                info!(
                    "Created new semaphore queue with a capacity of {}",
                    &ts.capacity
                );
            }

            // Wait for our turn - this waits non-blockingly until we're free to proceed
            let start = now_millis();
            connection
                .blpop::<&str, Option<()>>(&ts.name, ts.max_sleep as usize)
                .await
                .map_err(|e| RedisError::new_err(e.to_string()))?;

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

            // Connect to redis
            let mut connection = open_client_connection(&ts.client).await?;

            // Define queue if it doesn't exist
            get_script("src/scripts/release_semaphore.lua")
                .key(&ts.name)
                .invoke_async(&mut connection)
                .await
                .map_err(|e| RedisError::new_err(e.to_string()))?;

            debug!("Released semaphore");
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!("Semaphore instance for queue {}", &self.name)
    }
}
