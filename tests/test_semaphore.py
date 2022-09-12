import asyncio
import logging
import re
from asyncio.exceptions import TimeoutError

import pytest

from timely import Semaphore

from .conftest import run, semaphore_factory

logger = logging.getLogger(__name__)


async def test_semaphore_runtimes():
    n = 10
    sleep = 0.4

    # Ensure n tasks never completed in less the time it would take for n nodes to finish sleeping
    coro = asyncio.wait_for(
        timeout=n * sleep - 0.05,
        fut=asyncio.gather(
            *[
                asyncio.create_task(run(semaphore_factory(name='runtimes', capacity=1), duration=sleep))
                for i in range(n)
            ]
        ),
    )

    with pytest.raises(TimeoutError):
        await coro

    # Queue n tasks run no slower than ~0.1 seconds.
    await asyncio.wait_for(
        timeout=n * sleep * 1.1,
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
    assert semaphore.queue_key
    assert semaphore.capacity == 1


def test_repr():
    semaphore = Semaphore(name='test', capacity=1)
    assert re.match(r'Semaphore instance for queue __timely-test', str(semaphore))  # noqa: W605
