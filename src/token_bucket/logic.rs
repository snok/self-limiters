use std::num::NonZeroUsize;
use std::time::Duration;

use log::{debug, info};
use pyo3::PyErr;
use redis::{AsyncCommands, Client, LposOptions};
use redlock::{Lock, RedLock};

use crate::token_bucket::data::{Data, MIN_BUFFER};
use crate::token_bucket::error::TokenBucketError;
use crate::token_bucket::utils::{
    create_node_key, minimum_time_until_slot, nodes_to_fetch, now_millis, set_scheduled, sleep_for,
    was_scheduled, TBResult,
};
use crate::token_bucket::ThreadState;
use crate::utils::open_client_connection;

async fn sleep_based_on_position(position: &i64, ts: &ThreadState) -> TBResult<()> {
    let sleep_duration = Duration::from_millis(minimum_time_until_slot(
        position,
        &(ts.capacity as i64),
        &ts.frequency,
        &ts.amount,
    ) as u64);
    sleep_for(sleep_duration, ts.max_sleep).await
}

pub(crate) async fn wait_for_slot(ts: ThreadState) -> Result<(), PyErr> {
    // Connect to redis
    let mut connection = open_client_connection::<&Client, TokenBucketError>(&ts.client).await?;

    // Enter queue
    // Note: The position received here is *not* an indication of where we are
    // relative to the next slot, since the queue is rpop'ed from. Instead,
    // it is an indication of how close we are to being assigned a slot.
    let mut position: i64 = connection
        .rpush(&ts.queue_key, &ts.id)
        .await
        .map_err(|e| PyErr::from(TokenBucketError::from(e)))?;

    // Since there's a ~0% chance we've already been assigned a slot, take a chill pill, then check
    sleep_based_on_position(&position, &ts).await?;

    // Create node key
    let node_key = create_node_key(&ts.name, &ts.id);

    loop {
        // Check for slot
        let slot: Option<u64> = connection
            .get(&node_key)
            .await
            .map_err(|e| PyErr::from(TokenBucketError::from(e)))?;

        // When slot is found, sleep until it's due
        if slot.is_some() {
            let sleep_duration = Duration::from_millis((slot.unwrap() - now_millis()) as u64);
            sleep_for(sleep_duration, ts.max_sleep).await?;
            debug!("w {} Breaking", &ts.id);
            break;
        };

        // Re-check position
        position = connection
            .lpos(&ts.queue_key, &ts.id, LposOptions::default())
            .await
            .unwrap_or(0);

        // Nap time
        sleep_based_on_position(&position, &ts).await?;
    }
    Ok(())
}

pub(crate) async fn schedule(ts: ThreadState) -> TBResult<()> {
    // Open redis connection
    let mut connection = open_client_connection::<&Client, TokenBucketError>(&ts.client).await?;

    // Try to acquire scheduler lock
    let url = format!("redis://{}", ts.client.get_connection_info().addr);
    let redlock = RedLock::new(vec![url]);

    let (lock, expiry): (Lock, u64) = loop {
        match redlock.lock(ts.name.as_bytes(), 1000) {
            Some(l) => {
                info!("{} won lock", ts.id);
                let expiry = now_millis() + l.validity_time.to_owned() as u64;
                break (l, expiry - 50);
            }
            None => {
                if was_scheduled(&ts.id, &mut connection).await? {
                    // Another scheduler has done the job for us.
                    return Ok(());
                } else {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                };
            }
        }
    };

    // Fetch data
    let mut data = Data::get(&ts.data_key, &ts.frequency, ts.amount, &mut connection).await?;

    loop {
        // Check that we still own the lock
        if now_millis() >= expiry - MIN_BUFFER {
            break;
        }

        // Refresh slot, token count, etc.
        data = data.update_bucket(ts.frequency, ts.amount, ts.capacity);

        // Figure out how many nodes, n, to fetch
        let number_of_nodes_to_fetch = nodes_to_fetch(data.tokens_left_for_slot as u32, ts.amount);

        // Fetch between 0-n nodes
        let size = NonZeroUsize::new(number_of_nodes_to_fetch as usize);
        // debug!("s Fetching {:?} nodes", size);
        let maybe_nodes = connection
            .rpop::<&String, Option<Vec<String>>>(&ts.queue_key, size)
            .await?;

        let nodes = match maybe_nodes {
            Some(nodes) => nodes,
            None => {
                if was_scheduled(&ts.id, &mut connection).await? {
                    break;
                } else {
                    // info!("s Sticking around since I havent been scheduled");
                    continue;
                };
            }
        };

        // Assign slots
        for node in nodes {
            // Generate the appropriate key for this particular node
            let node_key = create_node_key(&ts.name, &node);

            // Get the node's slot
            let slot_value = &data.slot;

            // There are three operations here that we could run concurrently,
            // but since we're in the scheduler process it didn't seem worth
            // the added complexity.
            connection.set(&node_key, slot_value).await?;
            set_scheduled(&node, &mut connection).await?;
            data.tokens_left_for_slot -= 1;
            debug!("Assigned slot {} to node `{}`", slot_value, node);
        }
    }

    data.set(&ts.data_key, &mut connection).await?;
    info!("Releasing lock");
    redlock.unlock(&lock);
    Ok(())
}
