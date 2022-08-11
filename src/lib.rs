use pyo3::prelude::*;

mod semaphore;

use crate::semaphore::{MaxPositionExceededError, RedisError, RedisSemaphore};

#[pymodule]
fn timely(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();
    // Add semaphore exceptions
    m.add(
        "MaxPositionExceeded",
        py.get_type::<MaxPositionExceededError>(),
    )?;
    m.add("RedisError", py.get_type::<RedisError>())?;
    // Add semaphore context manager
    m.add_class::<RedisSemaphore>()?;
    Ok(())
}
