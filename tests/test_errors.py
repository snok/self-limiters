import asyncio
import logging
from uuid import uuid4

import pytest
from redis.asyncio.client import Redis
from self_limiters import RedisError

from .conftest import run, semaphore_factory, tokenbucket_factory

logger = logging.getLogger(__name__)


@pytest.mark.parametrize('limiter', [semaphore_factory(redis_url='test'), tokenbucket_factory(redis_url='test')])
async def test_redis_error_on_bad_connection_string(limiter):
    """
    Redis errors should propagate as self_limiters.RedisError.
    """
    with pytest.raises(RedisError):
        await run(limiter, 0)


async def test_redis_error():
    """
    Trigger the equivalent of a runtime error in Redis,
    to ensure that these types of redis errors are also
    propagated correctly.
    """
    name = f'error-test-{uuid4()}'
    queue_name = f'__self-limiters:{name}'

    async def corrupt_queue():
        # Wait 0.1 seconds then destroy the queue
        await asyncio.sleep(0.3)
        r = Redis.from_url('redis://127.0.0.1:6389')
        await r.delete(queue_name)
        await r.set(queue_name, 'test')

    tasks = [asyncio.create_task(corrupt_queue())]
    tasks += [asyncio.create_task(run(semaphore_factory(name=name), 0.1)) for _ in range(10)]

    with pytest.raises(RedisError):
        await asyncio.gather(*tasks)
