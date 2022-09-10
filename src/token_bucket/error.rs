use crate::errors::RedisError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::mpsc::{RecvError, SendError};
use tokio::task::JoinError;

// Exception to raise when max sleep time is exceeded
create_exception!(timely, MaxSleepExceededError, PyException);

/// Enum containing all handled errors.
#[derive(Debug)]
pub enum TokenBucketError {
    MaxSleepExceeded(String),
    Redis(String),
    RuntimeError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<TokenBucketError> for PyErr {
    fn from(e: TokenBucketError) -> PyErr {
        match e {
            TokenBucketError::MaxSleepExceeded(e) => MaxSleepExceededError::new_err(e),
            TokenBucketError::Redis(e) => RedisError::new_err(e),
            TokenBucketError::RuntimeError(e) => PyRuntimeError::new_err(e),
        }
    }
}

impl From<RedisLibError> for TokenBucketError {
    fn from(e: RedisLibError) -> Self {
        TokenBucketError::Redis(e.to_string())
    }
}

impl From<ParseIntError> for TokenBucketError {
    fn from(e: ParseIntError) -> Self {
        TokenBucketError::RuntimeError(e.to_string())
    }
}

impl From<JoinError> for TokenBucketError {
    fn from(e: JoinError) -> Self {
        TokenBucketError::RuntimeError(e.to_string())
    }
}

impl<T> From<SendError<T>> for TokenBucketError {
    fn from(e: SendError<T>) -> Self {
        TokenBucketError::RuntimeError(e.to_string())
    }
}

impl From<RecvError> for TokenBucketError {
    fn from(e: RecvError) -> Self {
        TokenBucketError::RuntimeError(e.to_string())
    }
}

impl From<FromUtf8Error> for TokenBucketError {
    fn from(e: FromUtf8Error) -> Self {
        TokenBucketError::RuntimeError(e.to_string())
    }
}
