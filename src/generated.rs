/// This file is generated with a build script.
///
/// Do not make changes to this file. Instead edit the Lua scripts directly.

pub const SEMAPHORE_SCRIPT: &str = "\
--- Script called from the Semaphore implementation.
---
--- Lua scripts are run atomically by default, and since redis
--- is single threaded, there are no race conditions to worry about.
---
--- The script checks if a list exists for the Semaphore, and
--- creates one of length `capacity` if it doesn't.
---
--- keys:
--- * key: The key to use for the list
--- * existskey: The key to use for the string we use to check if the lists exists
---
--- args:
--- * capacity: The capacity of the semaphore (i.e., the length of the list)
---
--- returns:
--- * 1 if created, else 0 (but the return value isn't used; only useful for debugging)

redis.replicate_commands()

-- Init config variables
local key = tostring(KEYS[1])
local existskey = tostring(KEYS[2])
local capacity = tonumber(ARGV[1])

-- Check if list exists
-- Note, we cannot use EXISTS or LLEN below, as we need
-- to know if a list exists, but has capacity zero.
local does_not_exist = redis.call('SETNX', string.format(existskey, key), 1)

-- Create the list if none exists
if does_not_exist == 1 then
    -- Add '1' as an argument equal to the capacity of the semaphore
    -- If capacity is 5 here, we generate `{RPUSH, 1, 1, 1, 1, 1}`.
    local args = { 'RPUSH', key }
    for _ = 1, capacity do
        table.insert(args, 1)
    end
    redis.call(unpack(args))
    return true
end
return false
";
pub const TOKEN_BUCKET_SCRIPT: &str = "\
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
local now = tonumber(redis_time[1]) * 1000 + (tonumber(redis_time[2]) / 1000)

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
";
