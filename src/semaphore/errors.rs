use crate::semaphore::ThreadState;
use crossbeam_channel::{RecvError, SendError};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use tokio::task::JoinError;

// Exception to raise when max position is exceeded
create_exception!(timely, MaxPositionExceededError, PyException);

// Exception to return in place of redis::RedisError
// PyErr instances are raised as Python exceptions by pyo3, while
// native rust errors result in panics.
create_exception!(timely, RedisError, PyException);

#[derive(Debug)]
pub(crate) enum SemaphoreError {
    MaxPositionExceeded(String),
    Redis(String),
    ChannelError(String),
    JoinError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<SemaphoreError> for PyErr {
    fn from(e: SemaphoreError) -> PyErr {
        match e {
            SemaphoreError::MaxPositionExceeded(e) => MaxPositionExceededError::new_err(e),
            SemaphoreError::Redis(e) => RedisError::new_err(e),
            SemaphoreError::ChannelError(e) => PyRuntimeError::new_err(e),
            SemaphoreError::JoinError(e) => PyRuntimeError::new_err(e),
        }
    }
}

impl From<RedisLibError> for SemaphoreError {
    fn from(e: RedisLibError) -> Self {
        SemaphoreError::Redis(e.to_string())
    }
}

impl From<JoinError> for SemaphoreError {
    fn from(e: JoinError) -> Self {
        SemaphoreError::JoinError(e.to_string())
    }
}

impl From<SendError<&ThreadState>> for SemaphoreError {
    fn from(e: SendError<&ThreadState>) -> Self {
        SemaphoreError::ChannelError(e.to_string())
    }
}

impl From<SendError<ThreadState>> for SemaphoreError {
    fn from(e: SendError<ThreadState>) -> Self {
        SemaphoreError::ChannelError(e.to_string())
    }
}

impl From<RecvError> for SemaphoreError {
    fn from(e: RecvError) -> Self {
        SemaphoreError::ChannelError(e.to_string())
    }
}
