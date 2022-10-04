extern crate core;

use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::_errors::{MaxSleepExceededError, RedisError};
use crate::semaphore::Semaphore;
mod semaphore;

mod _errors;
mod _utils;
mod _tests;
mod token_bucket;

#[pymodule]
fn self_limiters(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();
    m.add(
        "MaxSleepExceededError",
        py.get_type::<MaxSleepExceededError>(),
    )?;
    m.add("RedisError", py.get_type::<RedisError>())?;
    m.add_class::<Semaphore>()?;
    m.add_class::<TokenBucket>()?;

    Ok(())
}
