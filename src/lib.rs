use pyo3::prelude::*;

use token_bucket::TokenBucket;

use crate::semaphore::errors::{MaxPositionExceededError, RedisError};
use crate::semaphore::Semaphore;
use crate::token_bucket::error::MaxSleepExceededError;

mod semaphore;
mod token_bucket;

#[pymodule]
fn timely(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    // Add semaphore resources
    m.add(
        "MaxPositionExceeded",
        py.get_type::<MaxPositionExceededError>(),
    )?;
    m.add("RedisError", py.get_type::<RedisError>())?;
    m.add_class::<Semaphore>()?;

    // Add token bucket resources
    m.add(
        "MaxSleepExceededError",
        py.get_type::<MaxSleepExceededError>(),
    )?;
    m.add_class::<TokenBucket>()?;

    Ok(())
}
