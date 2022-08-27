use std::time::Duration;

use crossbeam_channel::{bounded, Receiver};
use redis::aio::Connection;
use redis::Client;

use crate::semaphore::errors::SemaphoreError;
use crate::semaphore::ThreadState;

pub(crate) type SemResult<T> = Result<T, SemaphoreError>;

/// Calculate appropriate sleep duration for a given node.
/// Sleep longer when nodes are further back in the queue,
/// and generally try to sleeps as long as possible, to minimise i/o.
pub(crate) fn estimate_appropriate_sleep_duration(
    position: &u32,
    capacity: &u32,
    duration: &f32,
) -> Duration {
    Duration::from_millis(((position - capacity) as f32 * duration) as u64)
}

/// Open a channel and send some data
pub(crate) fn send_shared_state(ts: ThreadState) -> SemResult<Receiver<ThreadState>> {
    let (sender, receiver) = bounded(1);
    sender.send(ts)?;
    Ok(receiver)
}

/// Read data from channel
pub(crate) fn receive_shared_state(receiver: Receiver<ThreadState>) -> SemResult<ThreadState> {
    Ok(receiver.recv()?)
}

/// Open Redis connection
pub(crate) async fn open_client_connection(client: &Client) -> SemResult<Connection> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(SemaphoreError::Redis(format!(
            "Failed to connect to redis: {}",
            e
        ))),
    }
}
