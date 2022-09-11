use crate::token_bucket::error::TokenBucketError;
use redis::Script;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) type TBResult<T> = Result<T, TokenBucketError>;

pub async fn sleep_for(sleep_duration: Duration, max_sleep: Duration) -> TBResult<()> {
    if max_sleep.as_secs_f32() > 0.0 && sleep_duration > max_sleep {
        return Err(TokenBucketError::MaxSleepExceeded(format!(
            "Sleep duration {} exceeds max sleep {}",
            sleep_duration.as_secs(),
            max_sleep.as_secs()
        )));
    }
    let ms = sleep_duration.as_millis();

    if ms < 5 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    } else {
        tokio::time::sleep(sleep_duration).await;
    }

    Ok(())
}

pub(crate) fn now_millis() -> u64 {
    // Beware: This will fail with an overflow error in 500 thousand years
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub(crate) fn get_script() -> Script {
    let path = Path::new("src/schedule.lua");
    let mut file = File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    Script::new(&content)
}
