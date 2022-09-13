-- Single threaded script for checking if a list exists,
-- and creating one of length `capacity` if it doesn't.

redis.replicate_commands()


-- keys:
-- * list key

-- args:
-- * capacity

-- returns:
-- * 1 if created, else 0


-- Init config variables
local key = tostring(KEYS[1])
local capacity = tonumber(ARGV[1])

-- Check if list exists
local exists = redis.call("SETNX", string.format("(%s)-exists", key), 1)

-- Create the list if it doesn't
if exists == 1 then
    local args = {"RPUSH", key}
    for _=1,capacity do
        table.insert(args, 1)
    end
    redis.call(unpack(args))
    return true
end
return false
