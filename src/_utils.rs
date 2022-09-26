use crate::_errors::SLError;
use pyo3::prelude::PyModule;
use pyo3::types::{IntoPyDict, PyString, PyTuple};
use pyo3::{Py, PyAny, PyObject, PyResult, Python};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};
use pyo3_asyncio::TaskLocals;

pub type TLResult<T> = Result<T, SLError>;

const REDIS_DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub const REDIS_KEY_PREFIX: &str = "__self-limiters:";

/// Open a channel and send some data
pub fn send_shared_state<T, E: From<std::sync::mpsc::SendError<T>>>(
    ts: T,
) -> Result<Receiver<T>, E> {
    let (sender, receiver) = channel();
    sender.send(ts)?;
    Ok(receiver)
}

/// Read data from channel
pub fn receive_shared_state<T, E: From<std::sync::mpsc::RecvError>>(
    receiver: Receiver<T>,
) -> Result<T, E> {
    Ok(receiver.recv()?)
}

pub fn get_script(path: &str) -> String {
    let path = Path::new(path);
    let mut file = File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
}

pub fn get_script_sha(py: Python<'_>, content: &str) -> String {
    let sha1 = PyModule::import(py, "hashlib")
        .unwrap()
        .getattr("sha1")
        .unwrap();

    let encoded_content = PyString::new(py, &content)
        .getattr("encode")
        .unwrap()
        .call1(("utf-8",))
        .unwrap();

    let result = sha1
        .call1((encoded_content,))
        .unwrap()
        .getattr("hexdigest")
        .unwrap()
        .call0()
        .unwrap();

    result.to_string()
}

pub fn now_millis() -> u64 {
    // Beware: This will fail with an overflow error in 500 thousand years
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Debug)]
pub struct Redis {
    pub(crate) redis: Py<PyAny>,
}

pub fn redis_set(redis: &Py<PyAny>, py: Python, key: &str, value: i32) -> PyResult<PyObject> {
    let r = redis.getattr(py, "set")?;
    r.call1(py, (key, value))
}

pub async fn redis_get(redis: &Py<PyAny>, py: Python, locals: &TaskLocals, key: &str) -> PyResult<Option<i32>> {
    let r = redis.getattr(py, "get")?;
    let result:u32 = pyo3_asyncio::into_future_with_locals(
        locals, r.call1(py, (key,))?.extract(py)?
    )?.await?.extract(py)?;
    println!("{:?}", &result);

    // if result.is_none(py) {
    //     Ok(None)
    // } else {
    //     // Parse i32 from string
    //     let int_result = u32::from_str(&result.to_string()).unwrap();
    //     Ok(Some(int_result))
    // }
    Ok(None)
}

impl Redis {
    // pub fn blpop(self, key: &str, max_sleep: u32) -> Option<()> {
    //     let f = self.redis.getattr("blpop").unwrap();
    //     f.call1((key, max_sleep)).unwrap();
    //     Some(())
    // }

    // pub fn evalsha(self, script_sha: &str, keys: i32, arguments: Vec<&str>) -> PyResult<PyAny> {
    //     self.redis
    //         .getattr("evalsha")
    //         .unwrap()
    //         .call1((script_sha, keys, arguments))
    //         .unwrap()
    //         .extract()
    // }
    //
    // pub fn eval(self, script_content: &str, keys: i32, arguments: Vec<&str>) -> PyResult<PyAny> {
    //     self.redis
    //         .getattr("eval")
    //         .unwrap()
    //         .call1((script_content, keys, arguments))
    //         .unwrap()
    //         .extract()
    // }
}
