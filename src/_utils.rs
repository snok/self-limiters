use crate::_errors::TLError;
use redis::aio::Connection;
use redis::{parse_redis_url, Client, Script};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

pub type TLResult<T> = Result<T, TLError>;

const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub(crate) const REDIS_KEY_PREFIX: &str = "__self-limiters-";

/// Open a channel and send some data
pub(crate) fn send_shared_state<T, E: From<std::sync::mpsc::SendError<T>>>(
    ts: T,
) -> Result<Receiver<T>, E> {
    let (sender, receiver) = channel();
    sender.send(ts)?;
    Ok(receiver)
}

/// Read data from channel
pub(crate) fn receive_shared_state<T, E: From<std::sync::mpsc::RecvError>>(
    receiver: Receiver<T>,
) -> Result<T, E> {
    Ok(receiver.recv()?)
}

/// Open Redis connection
pub(crate) async fn open_client_connection(client: &Client) -> Result<Connection, TLError> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(TLError::Redis(e.to_string())),
    }
}

pub(crate) fn get_script(path: &str) -> Script {
    let path = Path::new(path);
    let mut file = File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    Script::new(&content)
}

pub(crate) fn now_millis() -> u64 {
    // Beware: This will fail with an overflow error in 500 thousand years
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub(crate) fn validate_redis_url(redis_url: Option<&str>) -> TLResult<Client> {
    let url = match parse_redis_url(redis_url.unwrap_or(REDIS_DEFAULT_URL)) {
        Some(url) => url,
        None => {
            return Err(TLError::Redis(String::from("Failed to parse redis url")));
        }
    };
    let client = match Client::open(url) {
        Ok(client) => client,
        Err(e) => {
            return Err(TLError::Redis(format!("Failed to connect to redis: {}", e)));
        }
    };
    Ok(client)
}
