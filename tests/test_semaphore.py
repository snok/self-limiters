from asyncio import TimeoutError, create_task, gather, sleep, wait_for
from functools import partial
from uuid import uuid4

import pytest


def semaphore_factory(**kwargs) -> partial:
    """
    Provide an almost initialized semaphore with defaults.

    This makes it much easier to init semaphores with slightly different configurations in tests.
    """
    from timely import RedisSemaphore

    defaults = {'name': uuid4().hex[:6], 'capacity': 1, 'redis_url': 'redis://127.0.0.1:6389'}
    return partial(RedisSemaphore, **(defaults | kwargs))


async def run(pt: partial, duration: float) -> None:
    async with pt():
        await sleep(duration)


async def test_semaphore_runtimes():
    # Ensure 6 tasks never completes in less than 0.1 seconds.
    with pytest.raises(TimeoutError):
        await wait_for(gather(*[create_task(run(semaphore_factory(capacity=1), 0.1)) for _ in range(2)]), 0.1)

    # Queue 6 tasks run no slower than ~0.1 seconds.
    await wait_for(gather(*[create_task(run(semaphore_factory(capacity=1), 0.1)) for _ in range(2)]), 0.15)


async def test_semaphore_capacity_isnt_exceeded():
    t = 3
    with pytest.raises(TimeoutError):
        await wait_for(gather(*[create_task(run(semaphore_factory(capacity=1), t)) for _ in range(2)]), t)


async def test_sleep_is_non_blocking():
    async def _sleep(duration: float) -> None:
        await sleep(duration)

    tasks = [
        # Create task for semaphore to sleep 1 second
        create_task(run(semaphore_factory(), 1)),
        # And create another task to normal asyncio sleep for 1 second
        create_task(_sleep(1)),
    ]

    # Both tasks should complete in ~1 second if thing are working correctly
    await wait_for(gather(*tasks), 1.05)
