extern crate core;

use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::errors::{MaxSleepExceededError, RedisError};
use crate::semaphore::Semaphore;

mod errors;
mod semaphore;
mod token_bucket;
mod utils;

#[pymodule]
fn self_limiters(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();
    m.add("MaxSleepExceededError", py.get_type::<MaxSleepExceededError>())?;
    m.add("RedisError", py.get_type::<RedisError>())?;
    m.add_class::<Semaphore>()?;
    m.add_class::<TokenBucket>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::utils::*;

    #[tokio::test]
    async fn test_now_millis() -> SLResult<()> {
        let now = now_millis()?;
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(now + 30 <= now_millis()?);
        assert!(now + 33 >= now_millis()?);
        Ok(())
    }

    #[test]
    fn test_create_connection_manager() {
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
                create_connection_manager(Some(&format!("{}{}", good_url, port_postfix))).unwrap();
            }
        }

        // None is also allowed, and we will try to connect to the default address
        create_connection_manager(None).unwrap();

        // Make sure these bad URLs fail
        for bad_url in &["", "1", "127.0.0.1:6379", "test://127.0.0.1:6379"] {
            if create_connection_manager(Some(bad_url)).is_ok() {
                panic!("Should fail")
            }
        }
    }
}
