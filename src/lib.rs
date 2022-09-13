extern crate core;

use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::_errors::{MaxSleepExceededError, RedisError};
use crate::semaphore::Semaphore;
mod semaphore;

mod _errors;
mod _utils;
mod token_bucket;

#[pymodule]
fn tl(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    // Add exceptions
    m.add(
        "MaxSleepExceededError",
        py.get_type::<MaxSleepExceededError>(),
    )?;
    m.add("RedisError", py.get_type::<RedisError>())?;

    // Add semaphore resources
    m.add_class::<Semaphore>()?;

    // Add token bucket resources
    m.add_class::<TokenBucket>()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::_errors::TLError;
    use crate::_utils::{
        open_client_connection, receive_shared_state, send_shared_state, TLResult,
    };
    use crate::semaphore::ThreadState as SemaphoreThreadState;
    use crate::token_bucket::ThreadState as TokenBucketThreadState;
    use redis::{AsyncCommands, Client};
    use std::thread;
    use std::time::Duration;

    #[tokio::test]
    async fn test_open_client_connection() -> TLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let mut connection = open_client_connection(&client).await?;
        connection.set("test", 2).await?;
        let result: i32 = connection.get("test").await?;
        assert_eq!(result, 2);
        Ok(())
    }

    #[test]
    fn test_send_and_receive_via_channel_semaphore_threaded() -> TLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let name = String::from("test");
        let capacity = 1;
        let max_sleep = 0;

        // Send and receive w/o thread
        let receiver = send_shared_state::<SemaphoreThreadState, TLError>(SemaphoreThreadState {
            client,
            name: name.to_owned(),
            capacity: capacity.to_owned(),
            max_sleep: max_sleep.to_owned(),
        })
        .unwrap();
        let copied_ts = thread::spawn(move || {
            receive_shared_state::<SemaphoreThreadState, TLError>(receiver).unwrap()
        })
        .join()
        .unwrap();
        assert_eq!(copied_ts.name, name);
        assert_eq!(copied_ts.capacity, capacity);
        Ok(())
    }

    #[test]
    fn test_send_and_receive_via_channel_token_bucket_threaded() -> TLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").unwrap();
        let name = String::from("test");
        let capacity = 1;
        let max_sleep = Duration::from_millis(10);
        let frequency = 0.1;
        let amount = 1;

        // Send and receive w/o thread
        let receiver =
            send_shared_state::<TokenBucketThreadState, TLError>(TokenBucketThreadState {
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
                receive_shared_state::<TokenBucketThreadState, TLError>(receiver).unwrap();
            assert_eq!(copied_ts.name, name);
            assert_eq!(copied_ts.capacity, capacity);
            assert_eq!(copied_ts.max_sleep, max_sleep);
            assert_eq!(copied_ts.frequency, frequency);
            assert_eq!(copied_ts.amount, amount);
        });
        Ok(())
    }
}
