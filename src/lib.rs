use pyo3::prelude::*;

use token_bucket::TokenBucket;

use _errors::{MaxSleepExceededError, RedisError};
use semaphore::Semaphore;

pub mod _errors;
pub mod _tests;
pub mod _utils;
pub mod semaphore;
pub mod token_bucket;

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
