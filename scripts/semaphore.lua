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
