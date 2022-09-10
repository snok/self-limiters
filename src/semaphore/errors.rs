use crate::errors::RedisError;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use redis::RedisError as RedisLibError;
use std::sync::mpsc::{RecvError, SendError};
use tokio::task::JoinError;

// Exception to raise when max position is exceeded
create_exception!(timely, MaxPositionExceededError, PyException);

/// Enum containing all handled errors.
#[derive(Debug)]
pub(crate) enum SemaphoreError {
    MaxPositionExceeded(String),
    Redis(String),
    RuntimeError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<SemaphoreError> for PyErr {
    fn from(e: SemaphoreError) -> PyErr {
        match e {
            SemaphoreError::MaxPositionExceeded(e) => MaxPositionExceededError::new_err(e),
            SemaphoreError::Redis(e) => RedisError::new_err(e),
            SemaphoreError::RuntimeError(e) => PyRuntimeError::new_err(e),
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
        SemaphoreError::RuntimeError(e.to_string())
    }
}

impl<T> From<SendError<T>> for SemaphoreError {
    fn from(e: SendError<T>) -> Self {
        SemaphoreError::RuntimeError(e.to_string())
    }
}

impl From<RecvError> for SemaphoreError {
    fn from(e: RecvError) -> Self {
        SemaphoreError::RuntimeError(e.to_string())
    }
}
