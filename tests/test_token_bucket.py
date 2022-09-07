import asyncio
import logging
import re
from asyncio.exceptions import TimeoutError
from functools import partial
from uuid import uuid4

import pytest

from timely import TokenBucket

from .conftest import run

logger = logging.getLogger(__name__)


def tokenbucket_factory(**kwargs) -> partial:
    """
    Provide an almost initialized token bucket with defaults.

    This makes it much easier to init token buckets with slightly different configurations in tests.
    """

    defaults = {
        'name': uuid4().hex[:6],
        'capacity': 1,
        'refill_frequency': 1.0,
        'refill_amount': 1,
        'redis_url': 'redis://127.0.0.1:6389',
    }
    return partial(TokenBucket, **(defaults | kwargs))


async def test_token_bucket_runtimes():
    n = 3

    # Ensure n tasks never complete in less than n/(refill_frequency * refill_amount)
    coro = asyncio.wait_for(
        timeout=n - 0.05,
        fut=asyncio.gather(
            *[asyncio.create_task(run(tokenbucket_factory(name='runtimes', capacity=1), duration=0)) for i in range(n)]
        ),
    )

    with pytest.raises(TimeoutError):
        await coro

    # Queue n tasks run no slower than ~0.1 seconds.
    await asyncio.wait_for(
        timeout=n + 0.05,
        fut=asyncio.gather(*[asyncio.create_task(run(tokenbucket_factory(capacity=1), duration=0)) for _ in range(n)]),
    )


async def test_sleep_is_non_blocking():
    async def _sleep(duration: float) -> None:
        await asyncio.sleep(duration)

    tasks = [
        # Create task for token bucket to sleep 1 second
        # And create other tasks to normal asyncio sleep for 1 second
        asyncio.create_task(_sleep(1)),
        asyncio.create_task(run(tokenbucket_factory(), 0)),
        asyncio.create_task(_sleep(1)),
        asyncio.create_task(run(tokenbucket_factory(), 0)),
    ]

    # Both tasks should complete in ~1 second if thing are working correctly
    await asyncio.wait_for(timeout=1.1, fut=asyncio.gather(*tasks))


def test_class_attributes():
    """
    Check attributes are accessible, and check defaults.
    """
    tb = tokenbucket_factory(name='test', capacity=1)()
    assert tb.name
    assert tb.capacity == 1
    assert tb.refill_frequency == 1.0
    assert tb.refill_amount == 1
    assert tb.id


def test_repr():
    tb = tokenbucket_factory(name='test', capacity=1)()
    assert re.match(r'Token bucket instance .{10} for queue __timely-test-queue', str(tb))  # noqa: W605