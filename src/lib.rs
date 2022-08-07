use pyo3::prelude::*;

mod semaphore;

use semaphore::RedisSemaphore;

#[pymodule]
fn timely(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();
    Ok(())
}
