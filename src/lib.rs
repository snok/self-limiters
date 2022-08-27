use crate::semaphore::errors::{MaxPositionExceededError, RedisError};
use crate::semaphore::Semaphore;
use pyo3::prelude::*;

mod semaphore;

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
    m.add_class::<Semaphore>()?;
    Ok(())
}
