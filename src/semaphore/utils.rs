use crate::semaphore::errors::SemaphoreError;
use crate::semaphore::ThreadState;
use crossbeam_channel::{bounded, Receiver};
use redis::aio::Connection;
use redis::Client;
use std::time::Duration;

// Calculate appropriate sleep duration for a given node. Sleep longer when nodes are
// further back in the queue. Sleeps as long as possible, to minimise i/o.
pub(crate) fn estimate_appropriate_sleep_duration(
    position: &u32,
    capacity: &u32,
    duration: &f32,
) -> Duration {
    Duration::from_millis(((position - capacity) as f32 * duration) as u64)
}

// Open a channel and send some data
pub(crate) fn send_shared_state(ts: ThreadState) -> Result<Receiver<ThreadState>, SemaphoreError> {
    let (sender, receiver) = bounded(1);
    sender.send(ts)?;
    Ok(receiver)
}

// Read data from channel
pub(crate) fn receive_shared_state(
    receiver: Receiver<ThreadState>,
) -> Result<ThreadState, SemaphoreError> {
    Ok(receiver.recv()?)
}

pub(crate) async fn open_client_connection(client: &Client) -> Result<Connection, SemaphoreError> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(SemaphoreError::Redis(format!(
            "Failed to connect to redis: {}",
            e
        ))),
    }
}
