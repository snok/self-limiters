use std::sync::mpsc::{channel, Receiver};

use redis::{parse_redis_url, Client};

use crate::errors::PSLError;

pub const REDIS_KEY_PREFIX: &str = "__self-limiters:";

const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";

/// Open a channel and send some data
pub fn send_shared_state<T>(ts: T) -> Result<Receiver<T>, PSLError> {
    let (sender, receiver) = channel();
    sender.send(ts)?;
    Ok(receiver)
}

pub fn validate_redis_url(redis_url: Option<&str>) -> Result<Client, PSLError> {
    let url = match parse_redis_url(redis_url.unwrap_or(REDIS_DEFAULT_URL)) {
        Some(url) => url,
        None => {
            return Err(PSLError::Redis(String::from("Failed to parse redis url")));
        }
    };

    let client = match Client::open(url) {
        Ok(client) => client,
        Err(e) => {
            return Err(PSLError::Redis(format!(
                "Failed to open redis client: {}",
                e
            )));
        }
    };

    Ok(client)
}
