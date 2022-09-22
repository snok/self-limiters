from redis.exceptions import NoScriptError
from redis import Redis


def run_script(script_content: str, script_sha: str, key: str, capacity: int, redis: Redis) -> bool:
    """Run Lua script on Redis instance."""
    try:
        return await redis.evalsha(script_sha, 1, [key, capacity])
    except NoScriptError:
        return await redis.eval(script_content, 1, [key, capacity])
