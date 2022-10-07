use std::io::Error;
use std::sync::mpsc::RecvError;
use std::time::SystemTimeError;

use redis::RedisError as RedisLibError;

/// Enum containing all handled errors.
/// This enables us to use the `?` operator on function calls to utilities
/// that raise any of the mapped errors below, to automatically raise the
/// appropriate mapped Python error.
#[derive(Debug)]
pub enum SLError {
    MaxSleepExceeded(String),
    Redis(String),
    RuntimeError(String),
    ValueError(String),
}

// redis::RedisError could be raised any time we perform a call to redis
impl From<RedisLibError> for SLError {
    fn from(e: RedisLibError) -> Self {
        Self::Redis(e.to_string())
    }
}

// std::io::Error could be raised when we read our Lua scripts
impl From<Error> for SLError {
    fn from(e: Error) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

// SystemTimeError could be raised when calling SystemTime.now()
impl From<SystemTimeError> for SLError {
    fn from(e: SystemTimeError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

// RecvError could be raised when we read data from a channel
impl From<RecvError> for SLError {
    fn from(e: RecvError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}
