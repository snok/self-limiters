import asyncio
import logging
import re
from asyncio.exceptions import TimeoutError
from datetime import datetime, timedelta
from uuid import uuid4

import pytest
from self_limiters import MaxSleepExceededError

from .conftest import run, tokenbucket_factory

logger = logging.getLogger(__name__)


async def test_token_bucket_runtimes():
    n = 10
    frequency = 0.2

    # Ensure n tasks never complete in less than n/(refill_frequency * refill_amount)
    name = f'runtimes-{uuid4()}'
    coro = asyncio.wait_for(
        timeout=(frequency * n),
        fut=asyncio.gather(
            *[
                asyncio.create_task(
                    run(tokenbucket_factory(name=name, capacity=1, refill_frequency=frequency), duration=0)
                )
                for i in range(n)
            ]
        ),
    )

    with pytest.raises(TimeoutError):
        await coro

    # Queue n tasks run no slower than ~0.1 seconds.
    await asyncio.wait_for(
        timeout=(frequency * n) + 0.05,
        fut=asyncio.gather(
            *[
                asyncio.create_task(run(tokenbucket_factory(capacity=1, refill_frequency=frequency), duration=0))
                for _ in range(n)
            ]
        ),
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


def test_repr():
    tb = tokenbucket_factory(name='test', capacity=1)()
    assert re.match(r'Token bucket instance for queue __self-limiters:test', str(tb))  # noqa: W605


@pytest.mark.parametrize(
    'config,e',
    [
        ({'name': ''}, None),
        ({'name': None}, TypeError),
        ({'name': 1}, TypeError),
        ({'name': True}, TypeError),
        ({'capacity': 2}, None),
        ({'capacity': 2.2}, TypeError),
        ({'capacity': None}, TypeError),
        ({'capacity': 'test'}, TypeError),
        ({'refill_frequency': 2.2}, None),
        ({'refill_frequency': 'test'}, TypeError),
        ({'refill_frequency': None}, TypeError),
        ({'refill_frequency': -1}, ValueError),
        ({'refill_amount': 1}, None),
        ({'refill_amount': -1}, ValueError),
        ({'refill_amount': 'test'}, TypeError),
        ({'refill_amount': None}, TypeError),
        ({'redis_url': 'redis://a.b'}, None),
        ({'redis_url': 1}, TypeError),
        ({'redis_url': True}, TypeError),
        ({'max_sleep': 20}, None),
        ({'max_sleep': 0}, None),
        ({'max_sleep': 'test'}, TypeError),
        ({'max_sleep': None}, None),
    ],
)
def test_init_types(config, e):
    if e:
        with pytest.raises(e):
            tokenbucket_factory(**config)()
    else:
        tokenbucket_factory(**config)()


async def r(**kw):
    async with tokenbucket_factory(**kw)():
        pass


@pytest.mark.parametrize(
    'n,duration',
    [
        (100, 1.1),
        (1000, 11),
    ],
)
async def test_high_load(n, duration):
    # This should pass, and should run under a threshold
    name = uuid4().hex[:6]
    start = datetime.now()
    await asyncio.gather(*[asyncio.create_task(r(refill_frequency=0.01, name=name)) for _ in range(n)])
    assert datetime.now() - start < timedelta(seconds=duration)


async def test_max_sleep():
    name = uuid4().hex[:6]
    with pytest.raises(
        MaxSleepExceededError,
        match=(
            'Received wake up time in [0-9] seconds, which is greater or equal to the specified max sleep of 1 seconds'
        ),
    ):
        await asyncio.gather(*[asyncio.create_task(r(name=name, max_sleep=1)) for _ in range(10)])
