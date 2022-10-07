use pyo3::prelude::*;

use errors::{MaxSleepExceededError, RedisError};
use semaphore::Semaphore;
use token_bucket::TokenBucket;

pub mod errors;
pub mod semaphore;
#[cfg(test)]
mod tests;
pub mod token_bucket;
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
