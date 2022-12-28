use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use log::debug;
use std::time::Duration;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::{PyAny, PyResult, Python};
use pyo3_asyncio::tokio::future_into_py;
use redis::Script;
use tokio::sync::Mutex;

use crate::errors::SLError;
use crate::utils::{create_connection_manager, create_connection_pool, now_millis, SLResult, REDIS_KEY_PREFIX};

/// Pure rust DTO for the data we need to pass to our thread
/// We could pass the token bucket itself, but this seemed simpler.
pub struct ThreadState {
    pub(crate) capacity: u32,
    pub(crate) frequency: f32,
    pub(crate) amount: u32,
    pub(crate) max_sleep: f32,
    pub(crate) connection_pool: Pool<RedisConnectionManager>,
    pub(crate) name: String,
}

impl ThreadState {
    fn from(slf: &PyRef<TokenBucket>) -> Self {
        Self {
            capacity: slf.capacity,
            frequency: slf.refill_frequency,
            amount: slf.refill_amount,
            max_sleep: slf.max_sleep,
            connection_pool: slf.connection_pool.clone(),
            name: slf.name.clone(),
        }
    }
}

async fn schedule_and_sleep(m: Mutex<ThreadState>) -> SLResult<()> {
    let ts = m.lock().await;

    // Connect to redis
    let mut connection = ts.connection_pool.get().await?;

    // Retrieve slot
    let slot: u64 = Script::new(
        r"
        --- Script called from the Semaphore implementation.
        --- partially modelled after https://github.com/Tinche/aiosteady
        ---
        --- Lua scripts are run atomically by default, and since redis
        --- is single threaded, there are no race conditions to worry about.
        ---
        --- This script does three things, in order:
        --- 1. Retrieves token bucket state, which means the last slot assigned,
        ---    and how many tokens are left to be assigned for that slot
        --- 2. Works out whether we need to move to the next slot, or consume another
        ---    token from the current one.
        --- 3. Saves the token bucket state and returns the slot.
        ---
        --- The token bucket implementation is forward looking, so we're really just handing
        --- out the next time there would be tokens in the bucket, and letting the client
        --- sleep until then. This would be terrible in a sync application, but for an async
        --- python task runner or web-server, it's actually very handy. There is the issue
        --- of processes sleeping for an unreasonably long time, but there is a max-sleep
        --- setting in both implementations to offset this.
        ---
        --- keys:
        --- * key: The key name to use for the semaphore
        ---
        --- args:
        --- * capacity: The max capacity of the bucket
        --- * refill_rate: How often tokens are added to the bucket, (NOTE) in *milliseconds*
        ---                The rate is in milliseconds since we cannot use floats for the `now` variable.
        ---                This deviates from the rest of the package code, where the rate is specified in seconds.
        --- * refill_amount: How many tokens are added at each interval
        ---
        --- returns:
        --- * The assigned slot, as a millisecond timestamp

        redis.replicate_commands()

        -- Init config variables
        local data_key = KEYS[1]
        local capacity = tonumber(ARGV[1])
        local refill_rate = tonumber(ARGV[2])
        local refill_amount = tonumber(ARGV[3])

        -- Get current time (ms timestamp)
        local redis_time = redis.call('TIME') -- Array of [seconds, microseconds]
        local now = tonumber(redis_time[1]) * 1000 + (tonumber(redis_time[2]) / 1000)  -- Millisecond timestamp

        -- Instantiate default bucket values
        -- These are used if no state is retrieved below; i.e., they
        -- are the values we use for creating a new bucket.
        local tokens = refill_amount
        local slot = now + refill_rate

        -- Retrieve (possibly) stored state
        local data = redis.call('GET', data_key)

        if data ~= false then
            for a, b in string.gmatch(data, '(%S+) (%S+)') do
                slot = tonumber(a)
                tokens = tonumber(b)
            end

            -- Quickly validate our state

            -- If the slot is in the past, we need to increment the slot
            -- value, and add tokens to the bucket equal to the slots skipped
            if slot < now + 20 then
                tokens = tokens + (slot - now) / refill_rate
                slot = slot + refill_rate

                -- If we skipped 3 slots, but the capacity is 1,
                -- trim the tokens left.
                if tokens > capacity then
                    tokens = capacity
                end
            end

            -- If the current slot has no more tokens to assign,
            -- move to the next slot.
            if tokens <= 0 then
                slot = slot + refill_rate
                tokens = refill_amount
            end
        end

        -- Consume a token
        tokens = tokens - 1

        -- Save state and set expiry
        redis.call('SETEX', data_key, 30, string.format('%d %d', slot, tokens))

        return slot
        ",
    )
    .key(&ts.name)
    .arg(ts.capacity) // capacity
    .arg(ts.frequency * 1000.0) // refill rate in ms
    .arg(ts.amount) // refill amount
    .invoke_async(&mut *connection)
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

    debug!("Retrieved slot. Sleeping for {}.", sleep_duration.as_secs_f32());
    tokio::time::sleep(sleep_duration).await;

    Ok(())
}

/// Async context manager useful for controlling client traffic
/// in situations where you need to limit traffic to `n` requests per `m` unit of time.
/// For example, when you can only send 1 request per minute.
#[pyclass(frozen)]
#[pyo3(name = "TokenBucket")]
#[pyo3(module = "self_limiters")]
pub struct TokenBucket {
    #[pyo3(get)]
    capacity: u32,
    #[pyo3(get)]
    refill_frequency: f32,
    #[pyo3(get)]
    refill_amount: u32,
    #[pyo3(get)]
    name: String,
    max_sleep: f32,
    connection_pool: Pool<RedisConnectionManager>,
}

#[pymethods]
impl TokenBucket {
    /// Create a new class instance.
    #[new]
    fn new(
        name: String,
        capacity: u32,
        refill_frequency: f32,
        refill_amount: u32,
        redis_url: Option<&str>,
        max_sleep: Option<f32>,
        connection_pool_size: Option<u32>,
    ) -> PyResult<Self> {
        debug!("Creating new TokenBucket instance");

        if refill_frequency <= 0.0 {
            return Err(PyValueError::new_err("Refill frequency must be greater than 0"));
        }
        // Create redis connection manager
        let manager = create_connection_manager(redis_url)?;

        // Create connection pool
        let pool = create_connection_pool(manager, connection_pool_size.unwrap_or(15))?;

        Ok(Self {
            capacity,
            refill_amount,
            refill_frequency,
            max_sleep: max_sleep.unwrap_or(0.0),
            name: format!("{}{}", REDIS_KEY_PREFIX, name),
            connection_pool: pool,
        })
    }

    /// Spawn a scheduler thread to schedule wake-up times for nodes,
    /// and let the main thread wait for assignment of wake-up time
    /// then sleep until ready.
    fn __aenter__<'p>(slf: PyRef<Self>, py: Python<'p>) -> PyResult<&'p PyAny> {
        let m = Mutex::new(ThreadState::from(&slf));
        future_into_py(py, async { Ok(schedule_and_sleep(m).await?) })
    }

    /// Do nothing on aexit.
    #[args(_a = "*")]
    fn __aexit__<'p>(_s: PyRef<Self>, py: Python<'p>, _a: &'p PyTuple) -> PyResult<&'p PyAny> {
        future_into_py(py, async { Ok(()) })
    }

    fn __repr__(&self) -> String {
        format!("Token bucket instance for queue {}", &self.name)
    }
}
