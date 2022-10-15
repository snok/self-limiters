extern crate core;

use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::errors::{MaxSleepExceededError, RedisError};
use crate::semaphore::Semaphore;
mod semaphore;

mod _tests;
mod errors;
mod token_bucket;
mod utils;

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
