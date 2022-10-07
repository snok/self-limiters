use std::sync::mpsc::SendError;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use self_limiters_core::errors::SLError;

// Raised when redis::RedisError is raised by the redis crate.
create_exception!(self_limiters, RedisError, PyException);

// Raised when we've slept for too long. Useful for catching forever-growing queues.
create_exception!(self_limiters, MaxSleepExceededError, PyException);

#[derive(Debug)]
pub enum PSLError {
    MaxSleepExceeded(String),
    Redis(String),
    RuntimeError(String),
    ValueError(String),
}

// Map relevant error types to appropriate Python exceptions
impl From<PSLError> for PyErr {
    fn from(e: PSLError) -> Self {
        match e {
            PSLError::MaxSleepExceeded(e) => MaxSleepExceededError::new_err(e),
            PSLError::Redis(e) => RedisError::new_err(e),
            PSLError::RuntimeError(e) => PyRuntimeError::new_err(e),
            PSLError::ValueError(e) => PyValueError::new_err(e),
        }
    }
}

// Map PSLError to SLError
impl From<SLError> for PSLError {
    fn from(e: SLError) -> Self {
        match e {
            SLError::MaxSleepExceeded(e) => PSLError::MaxSleepExceeded(e),
            SLError::Redis(e) => PSLError::Redis(e),
            SLError::RuntimeError(e) => PSLError::RuntimeError(e),
            SLError::ValueError(e) => PSLError::ValueError(e),
        }
    }
}

// SendError could be raised when we pass data to a channel
impl<T> From<SendError<T>> for PSLError {
    fn from(e: SendError<T>) -> Self {
        Self::RuntimeError(e.to_string())
    }
}
