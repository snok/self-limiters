use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::mpsc::{RecvError, SendError};
use tokio::task::JoinError;

// Exception to return in place of redis::RedisError
// PyErr instances are raised as Python exceptions by pyo3, while
// native rust errors result in panics.
create_exception!(timely, RedisError, PyException);

// Exception to raise when we've slept for too long.
// Useful to catch forever-growing queues, etc.
create_exception!(timely, MaxSleepExceededError, PyException);

/// Enum containing all handled errors.
#[derive(Debug)]
pub enum TLError {
    MaxSleepExceeded(String),
    Redis(String),
    RuntimeError(String),
    ValueError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<TLError> for PyErr {
    fn from(e: TLError) -> PyErr {
        match e {
            TLError::MaxSleepExceeded(e) => MaxSleepExceededError::new_err(e),
            TLError::Redis(e) => RedisError::new_err(e),
            TLError::RuntimeError(e) => PyRuntimeError::new_err(e),
            TLError::ValueError(e) => PyValueError::new_err(e),
        }
    }
}

impl From<RedisLibError> for TLError {
    fn from(e: RedisLibError) -> Self {
        TLError::Redis(e.to_string())
    }
}

impl From<ParseIntError> for TLError {
    fn from(e: ParseIntError) -> Self {
        TLError::RuntimeError(e.to_string())
    }
}

impl From<JoinError> for TLError {
    fn from(e: JoinError) -> Self {
        TLError::RuntimeError(e.to_string())
    }
}

impl<T> From<SendError<T>> for TLError {
    fn from(e: SendError<T>) -> Self {
        TLError::RuntimeError(e.to_string())
    }
}

impl From<RecvError> for TLError {
    fn from(e: RecvError) -> Self {
        TLError::RuntimeError(e.to_string())
    }
}

impl From<FromUtf8Error> for TLError {
    fn from(e: FromUtf8Error) -> Self {
        TLError::RuntimeError(e.to_string())
    }
}
