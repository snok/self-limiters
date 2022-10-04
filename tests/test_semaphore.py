import asyncio
import logging
import re
from asyncio.exceptions import TimeoutError
from datetime import datetime, timedelta
from uuid import uuid4

import pytest
from self_limiters import MaxSleepExceededError, Semaphore

from .conftest import run, semaphore_factory

logger = logging.getLogger(__name__)


async def test_semaphore_runtimes():
    n = 10
    sleep = 0.4

    # Ensure n tasks never completed in less the time it would take for n nodes to finish sleeping
    name = f'runtimes-{uuid4()}'
    coro = asyncio.wait_for(
        timeout=n * sleep,
        fut=asyncio.gather(
            *[asyncio.create_task(run(semaphore_factory(name=name, capacity=1), duration=sleep)) for _ in range(n)]
        ),
    )

    with pytest.raises(TimeoutError):
        await coro

    # Queue n tasks run no slower than ~0.1 seconds.
    await asyncio.wait_for(
        timeout=n * sleep * 1.05,
        fut=asyncio.gather(
            *[asyncio.create_task(run(semaphore_factory(capacity=1), duration=sleep)) for _ in range(n)]
        ),
    )


async def test_sleep_is_non_blocking():
    async def _sleep(duration: float) -> None:
        await asyncio.sleep(duration)

    tasks = [
        # Create task for semaphore to sleep 1 second
        asyncio.create_task(run(semaphore_factory(), 0)),
        # And create another task to normal asyncio sleep for 1 second
        asyncio.create_task(_sleep(1)),
    ]

    # Both tasks should complete in ~1 second if thing are working correctly
    await asyncio.wait_for(asyncio.gather(*tasks), 1.05)


def test_class_attributes():
    """
    Check attributes are accessible, and check defaults.
    """
    semaphore = Semaphore(name='test', capacity=1)
    assert semaphore.name
    assert semaphore.capacity == 1


def test_repr():
    semaphore = Semaphore(name='test', capacity=1)
    assert re.match(r'Semaphore instance for queue __self-limiters:test', str(semaphore))  # noqa: W605


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
            semaphore_factory(**config)()
    else:
        semaphore_factory(**config)()


async def r(sleep, **kw):
    async with semaphore_factory(**kw)():
        await asyncio.sleep(sleep)


@pytest.mark.parametrize(
    'n,duration',
    [
        (100, 1),
        (1000, 4),
    ],
)
async def test_high_load(n, duration):
    # This should pass, and should run under a threshold
    name = uuid4().hex[:6]
    start = datetime.now()
    await asyncio.gather(*[asyncio.create_task(r(0, name=name)) for _ in range(n)])
    assert datetime.now() - start < timedelta(seconds=duration)


async def test_max_sleep():
    name = uuid4().hex[:6]
    with pytest.raises(MaxSleepExceededError, match='Max sleep exceeded when waiting for Semaphore'):
        await asyncio.gather(*[asyncio.create_task(r(1, name=name, max_sleep=1)) for _ in range(3)])
