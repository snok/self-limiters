extern crate redis;

use std::num::NonZeroUsize;
use std::sync::mpsc::channel;

use crate::semaphore::errors::SemaphoreError;
use log::debug;
use nanoid::nanoid;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::{parse_redis_url, AsyncCommands, Client, LposOptions};
use tokio::task::JoinHandle;

pub(crate) mod errors;
pub(crate) mod utils;

use crate::semaphore::utils::{
    estimate_appropriate_sleep_duration, open_client_connection, receive_shared_state,
    send_shared_state, SemResult,
};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
pub(crate) struct ThreadState {
    queue_key: Vec<u8>,
    capacity: u32,
    max_position: u32,
    sleep_duration: f32,
    identifier: Vec<u8>,
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
            identifier: slf.id.clone(),
        }
    }

    /// Enter queue and return when the Semaphore has capacity.
    async fn wait_for_slot(self) -> SemResult<()> {
        // Open redis connection
        let mut connection = open_client_connection(&self.client).await?;

        // Enter queue and get the current position
        let mut position = connection.rpush(&self.queue_key, &self.identifier).await?;
        debug!("Entered queue in position {}", position);

        loop {
            // If our position is within the Semaphore's capacity, return
            if position < self.capacity {
                debug!("Position is less than capacity. Returning.");
                break;
            }

            // If the position exceeds the maximum tolerated position, throw an error
            if self.max_position > 0 && position > self.max_position {
                debug!("Position is greater than max position. Returning.");
                return Err(SemaphoreError::MaxPositionExceeded(format!(
                    "Position {} exceeds the max position ({}).",
                    position, self.max_position
                )));
            }

            // Otherwise, sleep for a bit and check again
            let sleep_duration = estimate_appropriate_sleep_duration(
                &position,
                &self.capacity,
                &self.sleep_duration,
            );
            debug!(
                "Position {} is greater than capacity ({}). Sleeping",
                position, self.capacity
            );
            tokio::time::sleep(sleep_duration).await;

            // Retrieve position again
            position = connection
                .lpos::<&Vec<u8>, &Vec<u8>, Option<u32>>(
                    &self.queue_key,
                    &self.identifier,
                    LposOptions::default(),
                )
                .await?
                .unwrap_or(1);
            debug!("Position is now {}", position);
        }
        Ok(())
    }

    /// Pop from the queue, to add capacity back to the
    /// semaphore, and refresh expiry for the queue.
    async fn clean_up(self) -> SemResult<()> {
        struct S {
            client: Client,
            queue_key: Vec<u8>,
        }

        let (s1, r1) = channel();
        s1.send(S {
            client: self.client.to_owned(),
            queue_key: self.queue_key.to_owned(),
        })
        .unwrap();

        let (s2, r2) = channel();
        s2.send(S {
            client: self.client.to_owned(),
            queue_key: self.queue_key.to_owned(),
        })
        .unwrap();

        let task1: JoinHandle<SemResult<()>> = tokio::task::spawn(async move {
            let slf = r1.recv()?;
            let mut con = open_client_connection(&slf.client).await?;
            con.expire(&slf.queue_key, 30_usize).await?;
            Ok(())
        });

        let task2: JoinHandle<SemResult<()>> = tokio::task::spawn(async move {
            let slf = r2.recv()?;
            let mut con = open_client_connection(&slf.client).await?;
            con.lpop(&slf.queue_key, NonZeroUsize::new(1_usize)).await?;
            Ok(())
        });

        task1.await??;
        task2.await??;
        Ok(())
    }
}

#[pyclass(name = "Semaphore", dict)]
pub struct Semaphore {
    #[pyo3(get, set)]
    capacity: u32,
    #[pyo3(get, set)]
    max_position: u32,
    #[pyo3(get, set)]
    sleep_duration: f32,
    #[pyo3(get, set)]
    queue_key: Vec<u8>,
    #[pyo3(get, set)]
    id: Vec<u8>,
    client: Client,
}

#[pymethods]
impl Semaphore {
    /// Create a new class instance.
    #[new]
    fn new(
        name: &str,
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
        let queue_key = format!("__timely-{}-queue", name).as_bytes().to_vec();
        Ok(Self {
            queue_key,
            capacity,
            client,
            sleep_duration: sleep_duration.unwrap_or(0.1),
            max_position: max_position.unwrap_or(0),
            id: nanoid!(10).into_bytes(),
        })
    }

    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state(ThreadState::from(&slf))?;
        future_into_py(py, async move {
            let shared_state = receive_shared_state(receiver)?;
            Ok(shared_state.wait_for_slot().await?)
        })
    }

    #[args(_a = "*")]
    fn __aexit__<'a>(slf: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state(ThreadState::from(&slf))?;
        future_into_py(py, async move {
            let shared_state = receive_shared_state(receiver)?;
            shared_state.clean_up().await?;
            Ok(())
        })
    }

    /// Create a string representation of the class. Without
    /// this, the class is printed as builtin.Semaphore, which
    /// is pretty confusing.
    fn __repr__(&self) -> String {
        format!(
            "Semaphore instance {} for queue {}",
            String::from_utf8_lossy(&self.id),
            String::from_utf8_lossy(&self.queue_key)
        )
    }
}
