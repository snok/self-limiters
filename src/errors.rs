use pyo3::create_exception;
use pyo3::exceptions::PyException;

// Exception to return in place of redis::RedisError
// PyErr instances are raised as Python exceptions by pyo3, while
// native rust errors result in panics.
create_exception!(timely, RedisError, PyException);
