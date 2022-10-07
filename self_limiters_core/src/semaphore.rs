use std::sync::mpsc::Receiver;

use log::{debug, info};
use redis::{AsyncCommands, Client};

use crate::errors::SLError;
use crate::utils::{get_script, now_millis, SLResult};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the Semaphore itself, but this seemed simpler.
#[derive(Debug, Clone)]
pub struct ThreadState {
    pub client: Client,
    pub name: String,
    pub capacity: u32,
    pub max_sleep: f32,
}

pub async fn create_and_acquire_semaphore(receiver: Receiver<ThreadState>) -> SLResult<()> {
    // Retrieve thread state struct
    let ts = receiver.recv()?;

    // Connect to redis
    let mut connection = ts.client.get_async_connection().await?;

    // Define queue if it doesn't already exist
    if get_script("scripts/create_semaphore.lua")?
        .key(&ts.name)
        .arg(ts.capacity)
        .invoke_async(&mut connection)
        .await?
    {
        info!(
            "Created new semaphore queue with a capacity of {}",
            &ts.capacity
        );
    }

    // Wait for our turn - this waits non-blockingly until we're free to proceed
    let start = now_millis()?;
    connection
        .blpop::<&str, Option<()>>(&ts.name, ts.max_sleep as usize)
        .await?;

    // Raise an exception if we waited too long
    if ts.max_sleep > 0.0 && (now_millis()? - start) > (ts.max_sleep * 1000.0) as u64 {
        return Err(SLError::MaxSleepExceeded(
            "Max sleep exceeded when waiting for Semaphore".to_string(),
        ));
    };

    debug!("Acquired semaphore");
    Ok(())
}

pub async fn release_semaphore(receiver: Receiver<ThreadState>) -> SLResult<()> {
    let ts = receiver.recv()?;

    // Connect to redis
    let mut connection = ts.client.get_async_connection().await?;

    // Define queue if it doesn't exist
    get_script("scripts/release_semaphore.lua")?
        .key(&ts.name)
        .invoke_async(&mut connection)
        .await?;

    debug!("Released semaphore");
    Ok(())
}
