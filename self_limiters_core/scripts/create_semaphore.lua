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
---
--- args:
--- * capacity: The capacity of the semaphore (i.e., the length of the list)
---
--- returns:
--- * 1 if created, else 0 (but the return value isn't used; only useful for debugging)

redis.replicate_commands()

-- Init config variables
local key = tostring(KEYS[1])
local capacity = tonumber(ARGV[1])

-- Check if list exists
-- SETNX does in one call what we would otherwise do in two
--
-- One thing to note about this call is that we would rather not do this. It would
-- be much more intuitive to call EXISTS on the list key and create the list if it
-- did not exist. Unfortunately, if you create a list with 3 elements, and you pop
-- all three elements (popping == acquiring the semaphore), the list stops "existing"
-- once empty. In other words, EXISTS is not viable, so this is a workaround.
-- If you have better suggestions for how to achieve this, please submit a PR.
local does_not_exist = redis.call("SETNX", string.format("(%s)-exists", key), 1)

-- Create the list if none exists
if does_not_exist == 1 then
    -- Add "1" as an argument equal to the capacity of the semaphore
    -- In other words, if we passed capacity 5 here, this should
    -- generate `{RPUSH, 1, 1, 1, 1, 1}`.
    -- The values we push to the list are arbitrary.
    local args = {"RPUSH", key}
    for _=1,capacity do
        table.insert(args, 1)
    end
    redis.call(unpack(args))
    return true
end
return false
