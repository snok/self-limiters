use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::convert::Infallible;
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::mpsc::{RecvError, SendError};
use tokio::task::JoinError;

// Raised when redis::RedisError is raised downstream.
create_exception!(self_limiters, RedisError, PyException);

// Raised when we've slept for too long.
// Useful to catch forever-growing queues.
create_exception!(self_limiters, MaxSleepExceededError, PyException);

/// Enum containing all handled errors.
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

impl From<ParseIntError> for SLError {
    fn from(e: ParseIntError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

impl From<JoinError> for SLError {
    fn from(e: JoinError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

impl<T> From<SendError<T>> for SLError {
    fn from(e: SendError<T>) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

impl From<RecvError> for SLError {
    fn from(e: RecvError) -> Self {
        Self::RuntimeError(e.to_string())
    }
}

impl From<FromUtf8Error> for SLError {
    fn from(e: FromUtf8Error) -> Self {
        Self::RuntimeError(e.to_string())
    }
}
