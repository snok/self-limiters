use crate::_errors::SLError;
use pyo3::prelude::PyModule;
use pyo3::types::{IntoPyDict, PyString, PyTuple};
use pyo3::{Py, PyAny, PyResult, Python};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

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

impl Redis {
    //
    // pub fn set(self, key: &str, value: i32) {
    //     self.redis
    //         .getattr("set")
    //         .unwrap()
    //         .call1((key, value))
    //         .unwrap();
    // }
    //
    // pub fn get(self, key: &str) -> Option<i32> {
    //     let result: Option<Vec<u8>> = self
    //         .redis
    //         .getattr("get")
    //         .unwrap()
    //         .call1((key,))
    //         .unwrap()
    //         .extract()
    //         .unwrap();
    //
    //     if result.is_some() {
    //         // Unpack Option
    //         let temp = result.unwrap();
    //         // Parse string from vector of bytes (Vec<u8>)
    //         let decoded_result = String::from_utf8_lossy(&temp);
    //         // Parse i32 from string
    //         let int_result = i32::from_str(&decoded_result).unwrap();
    //         Some(int_result)
    //     } else {
    //         None
    //     }
    // }
    //
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
