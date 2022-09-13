import asyncio
import logging
from uuid import uuid4

import pytest
from redis.asyncio.client import Redis
from tl import RedisError

from .conftest import run, semaphore_factory, tokenbucket_factory

logger = logging.getLogger(__name__)


async def test_redis_error_on_init():
    """
    Make sure redis errors in the redis crate are propagated as tl.RedisError.

    Test failure to parse connection string.
    """
    with pytest.raises(RedisError):
        await run(semaphore_factory(redis_url='test'), 0)

    with pytest.raises(RedisError):
        await run(tokenbucket_factory(redis_url='test'), 0)


async def test_redis_error():
    """
    Make sure redis errors in the redis crate are propagated as tl.RedisError.

    Test failed redis operation in the main semaphore flow.
    """
    name = f'error-test-{uuid4()}'
    queue_name = f'__traffic-lights-{name}'

    async def corrupt_queue():
        # Wait 0.1 seconds then corrupt the queue
        await asyncio.sleep(0.3)
        r = Redis.from_url('redis://127.0.0.1:6389')
        await r.delete(queue_name)
        await r.set(queue_name, 'test')

    tasks = [asyncio.create_task(corrupt_queue())]
    tasks += [asyncio.create_task(run(semaphore_factory(name=name), 0.1)) for _ in range(10)]

    with pytest.raises(RedisError):
        await asyncio.gather(*tasks)
