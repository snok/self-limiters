#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use redis::{AsyncCommands, Client};

    use crate::_utils::*;
    use crate::semaphore::ThreadState as SemaphoreThreadState;
    use crate::token_bucket::ThreadState as TokenBucketThreadState;

    #[test]
    fn test_send_and_receive_via_channel_semaphore_threaded() -> SLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389")?;
        let name = String::from("test");
        let capacity = 1;
        let max_sleep = 0.0;

        // Send and receive w/o thread
        let receiver = send_shared_state(SemaphoreThreadState {
            client,
            name: name.to_owned(),
            capacity: capacity.to_owned(),
            max_sleep: max_sleep.to_owned(),
        })?;
        let copied_ts = thread::spawn(move || receive_shared_state(receiver).unwrap())
            .join()
            .unwrap();
        assert_eq!(copied_ts.name, name);
        assert_eq!(copied_ts.capacity, capacity);
        Ok(())
    }

    #[test]
    fn test_send_and_receive_via_channel_token_bucket_threaded() -> SLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389")?;
        let name = String::from("test");
        let capacity = 1;
        let max_sleep = 10.0;
        let frequency = 0.1;
        let amount = 1;

        // Send and receive w/o thread
        let receiver = send_shared_state(TokenBucketThreadState {
            client,
            name: name.to_owned(),
            capacity: capacity.to_owned(),
            max_sleep: max_sleep.to_owned(),
            frequency: frequency.to_owned(),
            amount: amount.to_owned(),
        })
        .unwrap();
        thread::spawn(move || {
            let copied_ts = receive_shared_state(receiver).unwrap();
            assert_eq!(copied_ts.name, name);
            assert_eq!(copied_ts.capacity, capacity);
            assert_eq!(copied_ts.max_sleep, max_sleep);
            assert_eq!(copied_ts.frequency, frequency);
            assert_eq!(copied_ts.amount, amount);
        });
        Ok(())
    }

    #[tokio::test]
    async fn test_open_client_connection() -> SLResult<()> {
        let client = Client::open("redis://127.0.0.1:6389")?;
        let mut connection = open_client_connection(&client).await?;
        connection.set("test", 2).await?;
        let result: i32 = connection.get("test").await?;
        assert_eq!(result, 2);
        Ok(())
    }

    #[test]
    fn test_get_script() -> SLResult<()> {
        get_script("src/scripts/schedule.lua")?;
        get_script("src/scripts/create_semaphore.lua")?;
        get_script("src/scripts/release_semaphore.lua")?;
        Ok(())
    }

    #[tokio::test]
    async fn test_now_millis() -> SLResult<()> {
        let now = now_millis()?;
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(now + 30 <= now_millis()?);
        assert!(now + 33 >= now_millis()?);
        Ok(())
    }

    #[test]
    fn test_validate_redis_urls() {
        // Make sure these normal URLs pass parsing
        for good_url in &[
            "redis://127.0.0.1",
            "redis://username:@127.0.0.1",
            "redis://username:password@127.0.0.1",
            "redis://:password@127.0.0.1",
            "redis+unix:///127.0.0.1",
            "unix:///127.0.0.1",
        ] {
            for port_postfix in &[":6379", ":1234", ""] {
                validate_redis_url(Some(&format!("{}{}", good_url, port_postfix))).unwrap();
            }
        }

        // None is also allowed, and we will try to connect to the default address
        validate_redis_url(None).unwrap();

        // Make sure these bad URLs fail
        for bad_url in &["", "1", "127.0.0.1:6379", "test://127.0.0.1:6379"] {
            if let Ok(_) = validate_redis_url(Some(bad_url)) {
                panic!("Should fail")
            }
        }
    }
}
