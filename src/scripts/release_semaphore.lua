-- Single threaded script for adding capacity back to
-- the Semaphore queue, and setting queue expiry.

redis.replicate_commands()

-- keys:
-- * list key

-- returns:
-- * 1 if created, else 0

-- Init config variables
local key = tostring(KEYS[1])


-- Add back capacity to the queue if the queue exists
redis.call("LPUSH", key, 1)

-- Then set expiry
redis.call("EXPIRE", key, 30)

-- Then set expiry
redis.call("EXPIRE", string.format("(%s)-exists", key), 30)

return key
