use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::errors::RedisError;
use crate::semaphore::errors::MaxPositionExceededError;
use crate::semaphore::Semaphore;
use crate::token_bucket::error::MaxSleepExceededError;
mod semaphore;

mod errors;
mod token_bucket;
mod utils;

#[pymodule]
fn timely(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    // Add semaphore resources
    m.add(
        "MaxPositionExceeded",
        py.get_type::<MaxPositionExceededError>(),
    )?;
    m.add("RedisError", py.get_type::<RedisError>())?;
    m.add_class::<Semaphore>()?;

    // Add token bucket resources
    m.add(
        "MaxSleepExceededError",
        py.get_type::<MaxSleepExceededError>(),
    )?;
    m.add_class::<TokenBucket>()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::semaphore::errors::SemaphoreError;
    use crate::semaphore::ThreadState as SemaphoreThreadState;
    use crate::token_bucket::error::TokenBucketError;
    use crate::token_bucket::ThreadState as TokenBucketThreadState;
    use crate::utils::{open_client_connection, receive_shared_state, send_shared_state};
    use redis::{AsyncCommands, Client};
    use std::thread;
    use std::time::Duration;

    use super::token_bucket::utils::TBResult;
    use crate::semaphore::utils::SemResult;

    #[test]
    fn send_and_receive_via_channel_semaphore_threaded() -> SemResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let queue_key = String::from("test");
        let id = String::from("test");
        let capacity = 1;
        let max_position = 10;
        let sleep_duration = 0.1;

        // Send and receive w/o thread
        let receiver =
            send_shared_state::<SemaphoreThreadState, SemaphoreError>(SemaphoreThreadState {
                client: client,
                queue_key: queue_key.to_owned(),
                id: id.to_owned(),
                capacity: capacity.to_owned(),
                max_position: max_position.to_owned(),
                sleep_duration: sleep_duration.to_owned(),
            })
            .unwrap();
        let copied_ts = thread::spawn(move || {
            receive_shared_state::<SemaphoreThreadState, SemaphoreError>(receiver).unwrap()
        })
        .join()
        .unwrap();
        assert_eq!(copied_ts.queue_key, queue_key);
        assert_eq!(copied_ts.id, id);
        assert_eq!(copied_ts.capacity, capacity);
        assert_eq!(copied_ts.max_position, max_position);
        assert_eq!(copied_ts.sleep_duration, sleep_duration);
        Ok(())
    }

    #[test]
    fn send_and_receive_via_channel_token_bucket_threaded() -> TBResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let name = String::from("test");
        let capacity = 1;
        let max_sleep = Duration::from_millis(10);
        let frequency = 0.1;
        let amount = 1;

        // Send and receive w/o thread
        let receiver =
            send_shared_state::<TokenBucketThreadState, TokenBucketError>(TokenBucketThreadState {
                client,
                name: name.to_owned(),
                capacity: capacity.to_owned(),
                max_sleep: max_sleep.to_owned(),
                frequency: frequency.to_owned(),
                amount: amount.to_owned(),
            })
            .unwrap();
        thread::spawn(move || {
            let copied_ts =
                receive_shared_state::<TokenBucketThreadState, TokenBucketError>(receiver).unwrap();
            assert_eq!(copied_ts.name, name);
            assert_eq!(copied_ts.capacity, capacity);
            assert_eq!(copied_ts.max_sleep, max_sleep);
            assert_eq!(copied_ts.frequency, frequency);
            assert_eq!(copied_ts.amount, amount);
        });
        Ok(())
    }

    #[tokio::test]
    async fn open_client_connection_token_bucket() -> TBResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let mut connection = open_client_connection::<&Client, TokenBucketError>(&client).await?;
        connection.set("test", 1).await?;
        let result: i32 = connection.get("test").await?;
        assert_eq!(result, 1);
        Ok(())
    }

    #[tokio::test]
    async fn open_client_connection_semaphore() -> SemResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let mut connection = open_client_connection::<Client, SemaphoreError>(&client).await?;
        connection.set("test", 2).await?;
        let result: i32 = connection.get("test").await?;
        assert_eq!(result, 2);
        Ok(())
    }
}
