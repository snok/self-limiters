import asyncio
import logging
import re
from datetime import datetime
from uuid import uuid4

import pytest
from self_limiters import MaxSleepExceededError

from .conftest import delta_to_seconds, run, tokenbucket_factory

logger = logging.getLogger(__name__)


@pytest.mark.parametrize(
    'n, frequency, timeout',
    [
        (10, 0.1, 1),
        (10, 0.01, 0.1),
        (2, 1, 2),
    ],
)
async def test_token_bucket_runtimes(n, frequency, timeout):
    # Ensure n tasks never complete in less than n/(refill_frequency * refill_amount)
    name = f'runtimes-{uuid4()}'
    tasks = [
        asyncio.create_task(run(tokenbucket_factory(name=name, capacity=1, refill_frequency=frequency), duration=0))
        for _ in range(n)
    ]

    before = datetime.now()
    await asyncio.gather(*tasks)
    assert timeout <= delta_to_seconds(datetime.now() - before) <= timeout * 1.5


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

    with pytest.raises(
        AttributeError, match="attribute 'refill_amount' of 'self_limiters.TokenBucket' objects is not writable"
    ):
        tb.refill_amount = 'test2'


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
        ({'capacity': -1}, OverflowError),
        ({'capacity': None}, TypeError),
        ({'capacity': 'test'}, TypeError),
        ({'refill_frequency': 2.2}, None),
        ({'refill_frequency': 'test'}, TypeError),
        ({'refill_frequency': None}, TypeError),
        ({'refill_frequency': -1}, ValueError),
        ({'refill_amount': 1}, None),
        ({'refill_amount': -1}, OverflowError),
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


async def test_max_sleep():
    name = uuid4().hex[:6]
    e = 'Received wake up time in [0-9] seconds, which is greater or equal to the specified max sleep of 1 seconds'
    with pytest.raises(MaxSleepExceededError, match=e):
        await asyncio.gather(
            *[asyncio.create_task(run(tokenbucket_factory(name=name, max_sleep=1), 0)) for _ in range(10)]
        )
