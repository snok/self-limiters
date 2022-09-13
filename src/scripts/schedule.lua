-- Single threaded token bucket implementation,
-- modeled after github.com/WeTransfer/prorate

redis.replicate_commands()

-- args:
-- * key name
-- * bucket capacity
-- * refill rate
-- * refill amount

-- returns:
-- * the assigned slot

-- Init config variables
local data_key = KEYS[1]
local capacity = tonumber(ARGV[1])  -- max tokens
local refill_rate = tonumber(ARGV[2])  -- how often to refill token, in ms
local refill_amount = tonumber(ARGV[3])

-- Get current time (ms timestamp)
local redis_time = redis.call("TIME") -- Array of [seconds, microseconds]
local now = tonumber(redis_time[1]) * 1000 + (tonumber(redis_time[2]) / 1000)

-- Instantiate default bucket values
local tokens = refill_amount
local slot = now + refill_rate

-- Retrieve (possibly) stored state
local data = redis.call("GET", data_key)

if data ~= false then
    for a, b in string.gmatch(data, "(%S+) (%S+)") do
        slot = tonumber(a)
        tokens = tonumber(b)
    end

    -- Quickly validate our state
    if slot < now + 20 then
        tokens = tokens + (slot - now) / refill_rate
        slot = slot + refill_rate
    end

    if tokens <= 0 then
        slot = slot + refill_rate
        tokens = refill_amount
    end

    if tokens > capacity then
        tokens = capacity
    end
end

-- Consume a token
tokens = tokens - 1

-- Save state
redis.call("SETEX", data_key, 30, string.format("%d %d", slot, tokens))

-- Return
return slot
