extern crate redis;

use std::num::NonZeroUsize;

use crate::semaphore::errors::SemaphoreError;
use log::{debug, info};
use nanoid::nanoid;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3_asyncio::tokio::future_into_py;
use redis::aio::Connection;
use redis::{parse_redis_url, AsyncCommands, Client, LposOptions};

pub(crate) mod errors;
pub(crate) mod utils;

use crate::semaphore::utils::{
    estimate_appropriate_sleep_duration, open_client_connection, receive_shared_state,
    send_shared_state,
};

// Struct for the data we need to pass to our async thread
pub struct SharedState {
    queue_key: Vec<u8>,
    capacity: u32,
    client: Client,
    sleep_duration: f32,
    max_position: u32,
    identifier: Vec<u8>,
}

impl SharedState {
    fn from(slf: &PyRef<Semaphore>) -> SharedState {
        SharedState {
            queue_key: slf.queue_key.clone(),
            capacity: slf.capacity,
            max_position: slf.max_position,
            sleep_duration: slf.sleep_duration,
            client: slf.client.clone(),
            identifier: slf._id.clone(),
        }
    }

    async fn wait_for_slot(self, connection: &mut Connection) -> Result<(), SemaphoreError> {
        // Enter a queue and get the current position
        let mut position = connection.rpush(&self.queue_key, &self.identifier).await?;
        debug!("Entered queue in position {}", position);

        loop {
            // If our position is within the Semaphore's capacity, return
            if position <= self.capacity {
                debug!("Position is less than capacity. Returning.");
                break;
            }

            // If the position is beyond the maximum tolerated position, throw an error
            if self.max_position > 0 && position > self.max_position {
                debug!("Position is greater than max position. Returning.");
                return Err(SemaphoreError::MaxPositionExceeded(format!(
                    "Position {} exceeds the max position ({}).",
                    position, self.max_position
                )));
            }

            // Sleep for a bit
            let sleep_duration = estimate_appropriate_sleep_duration(
                &position,
                &self.capacity,
                &self.sleep_duration,
            )
            .await;
            debug!(
                "Position {} is greater than capacity ({}). Sleeping",
                position, self.capacity
            );
            tokio::time::sleep(sleep_duration).await;

            // Then retrieve the position again
            position = match connection
                .lpos::<&Vec<u8>, &Vec<u8>, Option<u32>>(
                    &self.queue_key,
                    &self.identifier,
                    LposOptions::default(),
                )
                .await?
            {
                Some(position) => position + 1,
                // There's a chance our ID was popped from the queue.
                // This can only happen if we're within the capacity,
                // so setting position to 1 here is the way to handle thiself.
                None => 1,
            };
            debug!("Position is now {}", position);
        }
        Ok(())
    }

    async fn clean_up(self, connection: &mut Connection) -> Result<(), SemaphoreError> {
        info!("Leaving queue");
        connection
            .lpop(&self.queue_key, NonZeroUsize::new(1_usize))
            .await?;
        connection.expire(&self.queue_key, 30_usize).await?;
        Ok(())
    }
}

#[pyclass()]
pub struct Semaphore {
    capacity: u32,
    max_position: u32,
    sleep_duration: f32,
    client: Client,
    queue_key: Vec<u8>,
    _id: Vec<u8>,
}

#[pymethods]
impl Semaphore {
    #[new]
    fn new(
        name: &str,
        capacity: u32,
        redis_url: Option<&str>,
        sleep_duration: Option<f32>,
        max_position: Option<u32>,
    ) -> PyResult<Semaphore> {
        debug!("Creating new Semaphore instance");
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

        Ok(Semaphore {
            queue_key,
            capacity,
            client,
            sleep_duration: sleep_duration.unwrap_or(0.1),
            max_position: max_position.unwrap_or(0),
            _id: nanoid!(10).into_bytes(),
        })
    }

    fn __aenter__<'a>(slf: PyRef<'_, Self>, py: Python<'a>) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state(&slf)?;
        future_into_py(py, async move {
            let shared_state = receive_shared_state(receiver)?;
            let mut connection = open_client_connection(&shared_state.client).await?;
            Ok(shared_state.wait_for_slot(&mut connection).await?)
        })
    }

    #[args(_a = "*")]
    fn __aexit__<'a>(slf: PyRef<'_, Self>, py: Python<'a>, _a: &'a PyTuple) -> PyResult<&'a PyAny> {
        let receiver = send_shared_state(&slf)?;
        future_into_py(py, async move {
            let shared_state = receive_shared_state(receiver)?;
            let mut connection = open_client_connection(&shared_state.client).await?;
            Ok(shared_state.clean_up(&mut connection).await?)
        })
    }
}
