use crate::semaphore::errors::SemaphoreError;
use crate::semaphore::SharedState;
use crate::Semaphore;
use crossbeam_channel::{bounded, Receiver};
use pyo3::PyRef;
use redis::aio::Connection;
use redis::Client;
use std::time::Duration;

// Calculate appropriate sleep duration for a given node. Sleep longer when nodes are
// further back in the queue. Sleeps as long as possible, to minimise i/o.
pub async fn estimate_appropriate_sleep_duration(
    position: &u32,
    capacity: &u32,
    duration: &f32,
) -> Duration {
    Duration::from_millis(((position - capacity) as f32 * duration) as u64)
}

// Open a channel and send some data
pub fn send_shared_state(slf: &PyRef<Semaphore>) -> Result<Receiver<SharedState>, SemaphoreError> {
    let (sender, receiver) = bounded(1);
    match sender.send(SharedState::from(slf)) {
        Ok(_) => Ok(receiver),
        Err(e) => Err(SemaphoreError::ChannelError(format!(
            "Error sending shared state: {}",
            e
        ))),
    }
}

// Read data from channel
pub fn receive_shared_state(
    receiver: Receiver<SharedState>,
) -> Result<SharedState, SemaphoreError> {
    match receiver.recv() {
        Ok(s) => Ok(s),
        Err(e) => Err(SemaphoreError::ChannelError(format!(
            "Error receiving shared state: {}",
            e
        ))),
    }
}

pub async fn open_client_connection(client: &Client) -> Result<Connection, SemaphoreError> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(SemaphoreError::Redis(format!(
            "Failed to connect to redis: {}",
            e
        ))),
    }
}
