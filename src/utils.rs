use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

use redis::{parse_redis_url, Client};

use crate::errors::SLError;

pub type SLResult<T> = Result<T, SLError>;
pub const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub const REDIS_KEY_PREFIX: &str = "__self-limiters:";

pub fn send_shared_state<T>(ts: T) -> SLResult<Receiver<T>> {
    let (sender, receiver) = channel();
    sender.send(ts)?;
    Ok(receiver)
}

pub fn now_millis() -> SLResult<u64> {
    // Beware: This will overflow in 500 thousand years
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

pub fn validate_redis_url(redis_url: Option<&str>) -> SLResult<Client> {
    let url = match parse_redis_url(redis_url.unwrap_or(REDIS_DEFAULT_URL)) {
        Some(url) => url,
        None => return Err(SLError::Redis(String::from("Failed to parse redis url"))),
    };
    let client = match Client::open(url) {
        Ok(client) => client,
        Err(e) => return Err(SLError::Redis(format!("Failed to open redis client: {}", e))),
    };
    Ok(client)
}
