import asyncio
import logging
from uuid import uuid4

import pytest
from redis.asyncio.client import Redis

from timely import RedisError

from .conftest import run, semaphore_factory, tokenbucket_factory

logger = logging.getLogger(__name__)


async def test_redis_error_on_init():
    """
    Make sure redis errors in the redis crate are propagated as timely.RedisError.

    Test failure to parse connection string.
    """
    with pytest.raises(RedisError):
        await run(semaphore_factory(redis_url='test'), 0)

    with pytest.raises(RedisError):
        await run(tokenbucket_factory(redis_url='test'), 0)


async def test_error(caplog):
    """
    Make sure redis errors in the redis crate are propagated as timely.RedisError.

    Test failed redis operation in the main semaphore flow.
    """
    name = f'error-test-{uuid4()}'

    async def corrupt_queue():
        # Wait 0.1 seconds then corrupt the queue
        await asyncio.sleep(0.1)
        r = Redis.from_url('redis://127.0.0.1:6389')
        await r.set(f'__timely-{name}-queue', 'test')

    tasks = [asyncio.create_task(corrupt_queue())]
    tasks += [asyncio.create_task(run(semaphore_factory(name=name), 0.1)) for i in range(10)]

    with pytest.raises(RedisError, match='WRONGTYPE: Operation against a key holding the wrong kind of value'):
        await asyncio.gather(*tasks)
