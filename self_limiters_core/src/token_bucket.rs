use std::sync::mpsc::Receiver;
use std::time::Duration;

use log::debug;
use redis::Client;
use tokio::time::sleep;

use crate::errors::SLError;
use crate::utils::{get_script, now_millis, SLResult};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the token bucket itself, but this seemed simpler.
pub struct ThreadState {
    pub capacity: u32,
    pub frequency: f32,
    pub amount: u32,
    pub max_sleep: f32,
    pub client: Client,
    pub name: String,
}

pub async fn schedule_and_sleep(receiver: Receiver<ThreadState>) -> SLResult<()> {
    // Receive class state
    let ts = receiver.recv()?;

    // Connect to redis
    let mut connection = ts.client.get_async_connection().await?;

    // Retrieve slot
    let slot: u64 = get_script("scripts/schedule.lua")?
        .key(&ts.name)
        .arg(ts.capacity) // capacity
        .arg(ts.frequency * 1000.0) // refill rate in ms
        .arg(ts.amount) // refill amount
        .invoke_async(&mut connection)
        .await?;

    let now = now_millis()?;
    let sleep_duration = {
        // This might happen at very low refill frequencies.
        // Current handling isn't robust enough to ensure
        // exactly uniform traffic when this happens. Might be
        // something worth looking at more in the future, if needed.
        if slot <= now {
            Duration::from_millis(0)
        } else {
            Duration::from_millis(slot - now)
        }
    };

    if ts.max_sleep > 0.0 && sleep_duration > Duration::from_secs_f32(ts.max_sleep) {
        return Err(SLError::MaxSleepExceeded(format!(
            "Received wake up time in {} seconds, which is \
            greater or equal to the specified max sleep of {} seconds",
            sleep_duration.as_secs(),
            ts.max_sleep
        )));
    }

    debug!(
        "Retrieved slot. Sleeping for {}.",
        sleep_duration.as_secs_f32()
    );
    sleep(sleep_duration).await;

    Ok(())
}
