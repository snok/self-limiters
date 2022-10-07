use log::error;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use redis::Script;

use crate::errors::SLError;

// Result which returns SLError which is convertible to PyErr
pub type SLResult<T> = Result<T, SLError>;

pub fn get_script(path: &str) -> SLResult<Script> {
    let path = Path::new(path);
    let mut file = match File::open(path) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to open file {}", path.to_str().unwrap());
            return Err(SLError::RuntimeError(e.to_string()));
        }
    };
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(Script::new(&content))
}

pub fn now_millis() -> SLResult<u64> {
    // Beware: This will overflow in 500 thousand years
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}
