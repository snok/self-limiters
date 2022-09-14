--- Script called from the Semaphore implementation.
---
--- Lua scripts are run atomically by default, and since redis
--- is single threaded, there are no race conditions to worry about.
---
--- The script releases the semaphore (pushes back the popped entry
--- to our list) and sets and expiry on all related keys.
---
--- keys:
--- * key: The key to use for the list
---
--- returns:
--- * Nothing

redis.replicate_commands()

-- Init config variables
local key = tostring(KEYS[1])

-- Add back capacity to the queue
redis.call("LPUSH", key, 1)

-- Then set expiry for the queue
redis.call("EXPIRE", key, 30)

-- Then set expiry for the key we use to check if the queue exists
-- See comments in the other semaphore script for a detailed explanation
-- of this value.
redis.call("EXPIRE", string.format("(%s)-exists", key), 30)
