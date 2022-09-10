use crate::token_bucket::error::TokenBucketError;
use crate::token_bucket::ThreadState;
use crate::TokenBucket;
use pyo3::PyRef;
use redis::aio::Connection;
use std::sync::mpsc::{channel, Receiver};

use redis::AsyncCommands;
use redis::Client;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) type TBResult<T> = Result<T, TokenBucketError>;

// Return a lower-bound answer to how long (at least) a node will need to wait before
// it's slot is due. We do this to reduce unnecessary i/o.
// In a bucket that fills up one token per minute, position 10 in the
// queue will be due in 10 minutes or more. That means there's no reason
// to check Redis for assigned slots for the next close-to-10-minutes.
// Note, we say "10 minutes or more", since the position a node receives when entering
// the queue isn't actually it's position in the absolute queue. It's its position in the
// redis list representing the unscheduled nodes in the queue. A node could be told it is
// position 1 in the queue when there are actually 100 nodes scheduled before it.
// Position 1 only represent the lowest possible position it can have.
pub fn minimum_time_until_slot(
    position: &i64,
    capacity: &i64,
    frequency: &f32,
    amount: &u32,
) -> f32 {
    let val = ((((position + 1 - capacity) as f32) / *amount as f32) * frequency).ceil();

    if val < 0.0 {
        0.0
    } else {
        val
    }
}

pub async fn sleep_for(sleep_duration: Duration, max_sleep: Duration) -> TBResult<()> {
    if max_sleep.as_secs_f32() > 0.0 && sleep_duration > max_sleep {
        return Err(TokenBucketError::MaxSleepExceeded(format!(
            "Sleep duration {} exceeds max sleep {}",
            sleep_duration.as_secs(),
            max_sleep.as_secs()
        )));
    }
    let ms = sleep_duration.as_millis();

    if ms < 5 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    } else {
        tokio::time::sleep(sleep_duration).await;
    }

    Ok(())
}

// Open a channel and send some data
pub fn send_shared_state(slf: &PyRef<TokenBucket>) -> TBResult<Receiver<ThreadState>> {
    let (sender, receiver) = channel();
    match sender.send(ThreadState::from(slf)) {
        Ok(_) => Ok(receiver),
        Err(e) => Err(TokenBucketError::ChannelError(format!(
            "Error sending shared state: {}",
            e
        ))),
    }
}

// Read data from channel
pub fn receive_shared_state(receiver: Receiver<ThreadState>) -> TBResult<ThreadState> {
    match receiver.recv() {
        Ok(s) => Ok(s),
        Err(e) => Err(TokenBucketError::ChannelError(format!(
            "Error receiving shared state: {}",
            e
        ))),
    }
}

pub async fn open_client_connection(client: &Client) -> TBResult<Connection> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(TokenBucketError::Redis(format!(
            "Failed to connect to redis: {}",
            e
        ))),
    }
}

pub(crate) fn create_node_key(name: &str, id: &str) -> Vec<u8> {
    format!("__timely-{}-node-{}", name, id).as_bytes().to_vec()
}

pub(crate) fn nodes_to_fetch(tokens_left_for_slot: u32, amount: u32) -> u32 {
    if tokens_left_for_slot == 0 {
        amount
    } else {
        tokens_left_for_slot
    }
}

fn get_scheduler_key(id: &str) -> String {
    format!("__timely-{}-scheduled", id)
}

pub(crate) async fn was_scheduled(id: &str, connection: &mut Connection) -> TBResult<bool> {
    match connection
        .get::<String, Option<u8>>(get_scheduler_key(id))
        .await?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub(crate) async fn set_scheduled(id: &str, connection: &mut Connection) -> TBResult<()> {
    let key = get_scheduler_key(id);
    connection.set(&key, 1_u8).await?;
    connection.expire(&key, 15).await?;
    Ok(())
}

pub(crate) fn now_millis() -> u64 {
    // Beware: This will fail with an overflow error in 500 thousand years
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
