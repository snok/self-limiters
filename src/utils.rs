use redis::aio::Connection;
use redis::{Client, RedisError as RedisLibError};
use std::sync::mpsc::{channel, Receiver};

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
pub(crate) async fn open_client_connection<T, E: From<RedisLibError>>(
    client: &Client,
) -> Result<Connection, E> {
    match client.get_async_connection().await {
        Ok(connection) => Ok(connection),
        Err(e) => Err(E::from(e)),
    }
}
