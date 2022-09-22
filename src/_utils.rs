use crate::_errors::SLError;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};
use pyo3::prelude::PyModule;
use pyo3::{Py, PyAny, PyResult, Python};
use pyo3::types::{IntoPyDict, PyString, PyTuple};
use std::str::FromStr;

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
        .unwrap()
        .getattr("hexdigest")
        .unwrap();
    let result = sha1.call1((encoded_content,)).unwrap();
    result.to_string()
}

pub fn run_script(py: Python<'_>, script_content: &str, script_sha: &str, key: &str, capacity: &u32, redis: Redis) {
    let func: Py<PyAny> = PyModule::from_code(py, "", "scripts/run_script.py", "")
        .unwrap()
        .getattr("run_script")
        .unwrap()
        .into();

    let kwargs = [
        ("script_content", script_content),
        ("script_sha", script_sha),
        ("key", key),
        ("capacity", capacity),
        ("redis", redis.redis),
    ].into_py_dict(py);
    func.call(py, (), Some(kwargs))?;
}

pub fn now_millis() -> u64 {
    // Beware: This will fail with an overflow error in 500 thousand years
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Debug, Copy, Clone)]
pub struct Redis<'a> {
    redis: &'a PyAny,
}

impl Redis<'_> {
    pub fn set(self, key: &str, value: i32) {
        self.redis.getattr("set").unwrap().call1((key, value)).unwrap();
    }

    pub fn get(self, key: &str) -> Option<i32> {
        let result: Option<Vec<u8>> = self.redis
            .getattr("get")
            .unwrap()
            .call1((key, ))
            .unwrap()
            .extract()
            .unwrap();

        if result.is_some() {
            // Unpack Option
            let temp = result.unwrap();
            // Parse string from vector of bytes (Vec<u8>)
            let decoded_result = String::from_utf8_lossy(&temp);
            // Parse i32 from string
            let int_result = i32::from_str(&decoded_result).unwrap();
            Some(int_result)
        } else {
            None
        }
    }
    pub fn blpop(self, key: &str, max_sleep: u32) {
        self.redis
            .getattr("blpop")
            .unwrap()
            .call1((key, max_sleep))
            .unwrap();
    }
    pub fn evalsha<'a>(self, script_sha: &'a str, keys: i32, arguments: Vec<&str>) -> PyResult<&'a PyAny> {
        self.redis
            .getattr("evalsha")
            .unwrap()
            .call1((script_sha, keys, arguments))
            .unwrap()?
    }
    pub fn eval<'a>(self, script_content: &'a str, keys: i32, arguments: Vec<&str>) -> PyResult<&'a PyAny> {
        self.redis
            .getattr("eval")
            .unwrap()
            .call1((script_content, keys, arguments))
            .unwrap()?
    }
}

pub fn get_redis<'a>(py: Python<'a>, redis_url: &'a str) -> Redis<'a> {
    Redis {
        redis: PyModule::import(py, "redis")
            .unwrap()
            // access .Redis
            .getattr("Redis")
            .unwrap()
            // access .from_url
            .getattr("from_url")
            .unwrap()
            // Pass in redis URL (this is not how we'll do it, but this works for a demo)
            .call1((redis_url,))
            .unwrap()
    }
}