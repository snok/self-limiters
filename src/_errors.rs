use std::io::Error;
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::mpsc::{RecvError, SendError};

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use tokio::task::JoinError;

// Raised when redis::RedisError is raised by the redis crate.
create_exception!(self_limiters, RedisError, PyException);

// Raised when we've slept for too long. Useful for catching forever-growing queues.
create_exception!(self_limiters, MaxSleepExceededError, PyException);

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

// Map relevant error types to appropriate Python exceptions
impl From<SLError> for PyErr {
    fn from(e: SLError) -> Self {
        match e {
            SLError::MaxSleepExceeded(e) => MaxSleepExceededError::new_err(e),
            SLError::Redis(e) => RedisError::new_err(e),
            SLError::RuntimeError(e) => PyRuntimeError::new_err(e),
            SLError::ValueError(e) => PyValueError::new_err(e),
        }
    }
}

// redis::RedisError could be raised any time we perform a call to redis
impl From<RedisLibError> for SLError {
    fn from(e: RedisLibError) -> Self {
        Self::Redis(e.to_string())
    }
}

// SendError could be raised when we pass data to a channel
impl<T> From<SendError<T>> for SLError {
    fn from(e: SendError<T>) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

// RecvError could be raised when we read data from a channel
impl From<RecvError> for SLError {
    fn from(e: RecvError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

// std::io::Error could be raised when we read our Lua scripts
impl From<Error> for SLError {
    fn from(e: Error) -> Self {
        Self::RuntimeError(e.to_string())
    }
}
