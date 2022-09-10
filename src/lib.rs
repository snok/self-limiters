use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::semaphore::errors::{MaxPositionExceededError, RedisError};
use crate::semaphore::Semaphore;
use crate::token_bucket::error::MaxSleepExceededError;

mod semaphore;

mod token_bucket;

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
    use redis::Client;

    use super::token_bucket::data::Data;
    use super::token_bucket::utils::{
        minimum_time_until_slot, now_millis, open_client_connection, set_scheduled, was_scheduled,
        TBResult,
    };

    #[tokio::test]
    async fn test_scheduling() -> TBResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").expect("Failed to connect to Redis");
        let mut connection = open_client_connection(&client).await?;
        let mut scheduled = was_scheduled("test", &mut connection).await?;
        assert_eq!(scheduled, false);
        set_scheduled("test", &mut connection).await?;
        scheduled = was_scheduled("test", &mut connection).await?;
        assert_eq!(scheduled, true);
        Ok(())
    }

    #[test]
    fn minimum_time_until_slot_works() {
        assert_eq!(minimum_time_until_slot(&0, &3, &1.0, &3), 0.0);
        assert_eq!(minimum_time_until_slot(&1, &3, &1.0, &3), 0.0);
        assert_eq!(minimum_time_until_slot(&2, &3, &1.0, &3), 0.0);
        assert_eq!(minimum_time_until_slot(&3, &3, &1.0, &3), 1.0);
        assert_eq!(minimum_time_until_slot(&4, &3, &1.0, &3), 1.0);
        assert_eq!(minimum_time_until_slot(&5, &3, &1.0, &3), 1.0);
        assert_eq!(minimum_time_until_slot(&100, &3, &1.0, &3), 33.0);
    }

    /// Make sure the serialization/deserialization actually works.
    #[tokio::test]
    async fn test_write_and_read_data() -> TBResult<()> {
        let client = Client::open("redis://127.0.0.1:6389").expect("Failed to connect to Redis");
        let mut connection = open_client_connection(&client).await?;

        let data = Data::new(&0.5, 1);

        // Copy all values
        let slot = data.slot.to_owned();
        let tokens_left_for_slot = data.tokens_left_for_slot.to_owned();

        data.set(&"test-data-readwrite".to_string(), &mut connection)
            .await
            .unwrap();

        let stored_data =
            Data::get(&"test-data-readwrite".to_string(), &0.5, 1, &mut connection).await?;

        assert_eq!(slot, stored_data.slot);
        assert_eq!(tokens_left_for_slot, stored_data.tokens_left_for_slot);
        Ok(())
    }

    #[tokio::test]
    async fn test_update_bucket() -> TBResult<()> {
        let mut data = Data::new(&0.05, 1);
        let mut now = now_millis() + 50;

        for _ in 0..100 {
            assert_eq!(&data.tokens_left_for_slot, &1);
            assert_eq!(now, data.slot);

            data.tokens_left_for_slot -= 1;
            data = data.update_bucket(0.05, 1, 1);
            now += 50;
        }
        Ok(())
    }
}
