use std::time::{SystemTime, UNIX_EPOCH};

use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use log::info;
use redis::parse_redis_url;

use crate::errors::SLError;

pub(crate) type SLResult<T> = Result<T, SLError>;
pub(crate) const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub(crate) const REDIS_KEY_PREFIX: &str = "__self-limiters:";

pub(crate) fn now_millis() -> SLResult<u64> {
    // Beware: This will overflow in 500 thousand years
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

pub(crate) fn create_connection_manager(redis_url: Option<&str>) -> SLResult<RedisConnectionManager> {
    match parse_redis_url(redis_url.unwrap_or(REDIS_DEFAULT_URL)) {
        Some(url) => match RedisConnectionManager::new(url) {
            Ok(manager) => Ok(manager),
            Err(e) => Err(SLError::Redis(format!(
                "Failed to open redis connection manager: {}",
                e
            ))),
        },
        None => Err(SLError::Redis(String::from("Failed to parse redis url"))),
    }
}

pub(crate) fn create_connection_pool(
    manager: RedisConnectionManager,
    max_size: u32,
) -> SLResult<Pool<RedisConnectionManager>> {
    let future = async move { Pool::builder().max_size(max_size).build(manager).await.unwrap() };
    let res = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future);
    info!("Created connection pool of max {} connections", max_size);
    Ok(res)
}
