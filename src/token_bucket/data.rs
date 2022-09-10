use crate::token_bucket::utils::{now_millis, TBResult};
use redis::aio::Connection;
use redis::AsyncCommands;

/// Data is the state stored by our scheduler after/between each run.
#[derive(Debug)]
pub(crate) struct Data {
    pub slot: u64,
    pub tokens_left_for_slot: i32,
}

const SEPARATOR: &str = "///";

// If we're closer to the next slot than this buffer, roll over to the next slot.
// 20ms is a bit arbitrary, but seems ok.
pub(crate) const MIN_BUFFER: u64 = 20;

impl Data {
    /// Create new instance with default values.a
    /// When a bucket does not exist, we set tokens as 0
    /// and last slot as now, which just means "start now".
    pub fn new(frequency: &f32, amount: u32) -> Self {
        let now = now_millis();
        let start = now + (frequency * 1000.0) as u64;

        Self {
            slot: start as u64,
            tokens_left_for_slot: amount as i32,
        }
    }

    /// Convert to bytes we can send to Redis. This
    /// and the deserialize methods apply some (possibly questionable)
    /// custom parsing logic. This can probably be improved.
    fn serialize(self) -> Vec<u8> {
        format!("{}{}{}", self.slot, SEPARATOR, self.tokens_left_for_slot).into_bytes()
    }

    /// Deserialize bytes from Redis, using the format specified above.
    fn deserialize(bytes: Vec<u8>) -> Data {
        let s = String::from_utf8_lossy(&bytes);
        let v: Vec<&str> = s.split(SEPARATOR).collect();
        Data {
            slot: v[0].parse::<u64>().unwrap(),
            tokens_left_for_slot: v[1].parse::<i32>().unwrap(),
        }
    }

    /// Retrieve data instance from Redis.
    pub async fn get(
        data_key: &String,
        frequency: &f32,
        amount: u32,
        connection: &mut Connection,
    ) -> TBResult<Data> {
        match connection.get::<&String, Option<Vec<u8>>>(data_key).await? {
            Some(bytes) => Ok(Data::deserialize(bytes)),
            None => Ok(Self::new(frequency, amount)),
        }
    }

    /// Write data to Redis.
    pub async fn set(self, data_key: &String, connection: &mut Connection) -> TBResult<()> {
        Ok(connection.set(data_key, self.serialize()).await?)
    }

    /// Figure out how many nodes to fetch from the queue, and which slot to start scheduling
    /// them in. If the capacity of the bucket is 4 tokens, then we can assign the same slot
    /// to up to 4 nodes. If we finish scheduling slot `n` after processing 2 nodes, that means
    /// we might still be able to fetch and assign 2 more nodes to that slot the next time a
    /// scheduler runs. The only thing that would prevent us from assigning 2 more nodes to
    /// that slot, is if it's not far enough into the future anymore.
    pub fn update_bucket(mut self, frequency: f32, amount: u32, capacity: u32) -> Self {
        let now = now_millis();

        // If the slot is in the past, skip to n+1
        if self.slot < (now + MIN_BUFFER) {
            // info!("-> next slot, as the current slot is in the past");

            let slot_diff = ((self.slot - now) as f64 / (frequency * 1000.0) as f64) as u16;
            // println!("Slot diff was {}", slot_diff);

            self.slot = now + (frequency * 1000.0) as u64;

            // Add token if bucket is not full
            self.tokens_left_for_slot += slot_diff as i32;

            if self.tokens_left_for_slot > capacity as i32 {
                // info!("Reducing tokens to capacity");
                self.tokens_left_for_slot = capacity as i32;
            };
        }
        // Check if the current slot is fully consumed
        else if self.tokens_left_for_slot == 0 {
            // info!("Moving to next slot, as the current slot exhausted");
            self.slot += (frequency * 1000.0) as u64;
            self.tokens_left_for_slot += amount as i32;
        };

        self
    }
}
