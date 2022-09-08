use crate::token_bucket::ThreadState;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use std::num::ParseIntError;
use std::sync::mpsc::{RecvError, SendError};
use tokio::task::JoinError;

// Exception to raise when max sleep time is exceeded
create_exception!(timely, MaxSleepExceededError, PyException);

// Exception to return in place of redis::RedisError
// PyErr instances are raised as Python exceptions by pyo3, while
// native rust errors result in panics.
create_exception!(timely, RedisError, PyException);

/// Enum containing all handled errors.
#[derive(Debug)]
pub enum TokenBucketError {
    MaxSleepExceeded(String),
    Redis(String),
    ChannelError(String),
    ParseIntError(String),
    JoinError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<TokenBucketError> for PyErr {
    fn from(e: TokenBucketError) -> PyErr {
        match e {
            TokenBucketError::MaxSleepExceeded(e) => MaxSleepExceededError::new_err(e),
            TokenBucketError::Redis(e) => RedisError::new_err(e),
            TokenBucketError::ChannelError(e) => PyRuntimeError::new_err(e),
            TokenBucketError::ParseIntError(e) => PyRuntimeError::new_err(e),
            TokenBucketError::JoinError(e) => PyRuntimeError::new_err(e),
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
        TokenBucketError::ParseIntError(e.to_string())
    }
}

impl From<JoinError> for TokenBucketError {
    fn from(e: JoinError) -> Self {
        TokenBucketError::JoinError(e.to_string())
    }
}

impl From<SendError<&ThreadState>> for TokenBucketError {
    fn from(e: SendError<&ThreadState>) -> Self {
        TokenBucketError::ChannelError(e.to_string())
    }
}

impl From<SendError<ThreadState>> for TokenBucketError {
    fn from(e: SendError<ThreadState>) -> Self {
        TokenBucketError::ChannelError(e.to_string())
    }
}

impl From<RecvError> for TokenBucketError {
    fn from(e: RecvError) -> Self {
        TokenBucketError::ChannelError(e.to_string())
    }
}
