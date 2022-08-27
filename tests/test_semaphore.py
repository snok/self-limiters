import asyncio
import logging
import re
from functools import partial
from uuid import uuid4

import pytest

from timely import Semaphore

logger = logging.getLogger(__name__)


def semaphore_factory(**kwargs) -> partial:
    """
    Provide an almost initialized semaphore with defaults.

    This makes it much easier to init semaphores with slightly different configurations in tests.
    """

    defaults = {'name': uuid4().hex[:6], 'capacity': 1, 'redis_url': 'redis://127.0.0.1:6389'}
    return partial(Semaphore, **(defaults | kwargs))


async def run(pt: partial, duration: float) -> None:
    async with pt():
        logger.info(f'Sleeping {duration}')
        await asyncio.sleep(duration)


async def test_semaphore_runtimes():
    n = 3
    sleep = 2

    # Ensure n tasks never completed in less the time it would take for n nodes to finish sleeping
    with pytest.raises(TimeoutError):
        await asyncio.wait_for(
            timeout=n * sleep,
            fut=asyncio.gather(
                *[
                    asyncio.create_task(run(semaphore_factory(name='runtimes', capacity=1), duration=sleep))
                    for i in range(n)
                ]
            ),
        )

    # Queue n tasks run no slower than ~0.1 seconds.
    await asyncio.wait_for(
        asyncio.gather(*[asyncio.create_task(run(semaphore_factory(capacity=1), duration=sleep)) for _ in range(n)]),
        n * sleep * 1.1,
    )


async def test_sleep_is_non_blocking():
    async def _sleep(duration: float) -> None:
        await asyncio.sleep(duration)

    tasks = [
        # Create task for semaphore to sleep 1 second
        asyncio.create_task(run(semaphore_factory(), 1)),
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
    assert semaphore.queue_key
    assert semaphore.capacity == 1
    assert round(semaphore.sleep_duration, 1) == 0.1
    assert semaphore.max_position == 0


def test_repr():
    semaphore = Semaphore(name='test', capacity=1)
    assert re.match(r'Semaphore instance .{10} for queue __timely-test-queue', str(semaphore))  # noqa: W605
