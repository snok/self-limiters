use crate::_errors::SLError;
use redis::aio::Connection;
use redis::{parse_redis_url, Client, Script};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

// Result which returns SLError which is convertible to PyErr
pub type SLResult<T> = Result<T, SLError>;

const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub const REDIS_KEY_PREFIX: &str = "__self-limiters:";

/// Open a channel and send some data
pub fn send_shared_state<T>(ts: T) -> SLResult<Receiver<T>> {
    let (sender, receiver) = channel();
    sender.send(ts)?;
    Ok(receiver)
}

/// Read data from channel
pub fn receive_shared_state<T>(receiver: Receiver<T>) -> SLResult<T> {
    Ok(receiver.recv()?)
}

/// Open Redis connection
pub async fn open_client_connection(client: &Client) -> SLResult<Connection> {
    Ok(client.get_async_connection().await?)
}

pub fn get_script(path: &str) -> SLResult<Script> {
    let path = Path::new(path);
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(Script::new(&content))
}

pub fn now_millis() -> SLResult<u64> {
    // Beware: This will overflow in 500 thousand years
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

pub fn validate_redis_url(redis_url: Option<&str>) -> SLResult<Client> {
    let url = match parse_redis_url(redis_url.unwrap_or(REDIS_DEFAULT_URL)) {
        Some(url) => url,
        None => {
            return Err(SLError::Redis(String::from("Failed to parse redis url")));
        }
    };

    let client = match Client::open(url) {
        Ok(client) => client,
        Err(e) => {
            return Err(SLError::Redis(format!(
                "Failed to open redis client: {}",
                e
            )));
        }
    };

    Ok(client)
}
