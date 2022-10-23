import asyncio
import logging
import re
from datetime import datetime
from uuid import uuid4

import pytest
from redis.asyncio.client import Monitor, Redis
from self_limiters import MaxSleepExceededError, Semaphore

from .conftest import delta_to_seconds, run, semaphore_factory

logger = logging.getLogger(__name__)


@pytest.mark.parametrize(
    'n, capacity, sleep, timeout',
    [
        (10, 1, 0.1, 1),
        (10, 2, 0.1, 0.5),
        (10, 10, 0.1, 0.1),
        (5, 1, 0.1, 0.5),
    ],
)
async def test_semaphore_runtimes(n, capacity, sleep, timeout):
    """
    Make sure that the runtime of multiple Semaphore instances conform to our expectations.

    The runtime should never fall below the expected lower bound. If we run 6 instances for
    a Semaphore with a capacity of 5, where each instance sleeps 1 second, then it should
    always take 1 >= seconds to run those.
    """
    name = f'runtimes-{uuid4()}'
    tasks = [
        asyncio.create_task(run(semaphore_factory(name=name, capacity=capacity), duration=sleep)) for _ in range(n)
    ]

    before = datetime.now()
    await asyncio.gather(*tasks)
    assert timeout <= delta_to_seconds(datetime.now() - before) <= timeout * 1.2


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
    """Check attributes are readable, but immutable."""
    semaphore = Semaphore(name='test', capacity=1)

    assert semaphore.name == '__self-limiters:test'
    assert semaphore.capacity == 1
    assert semaphore.max_sleep == 0

    with pytest.raises(AttributeError, match="attribute 'name' of 'self_limiters.Semaphore' objects is not writable"):
        semaphore.name = 'test2'


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


@pytest.mark.filterwarnings('ignore::RuntimeWarning')
async def test_max_sleep():
    name = uuid4().hex[:6]
    with pytest.raises(MaxSleepExceededError, match='Max sleep exceeded when waiting for Semaphore'):
        await asyncio.gather(
            *[asyncio.create_task(run(semaphore_factory(name=name, max_sleep=1), 1)) for _ in range(3)]
        )


async def test_redis_instructions():
    r = Redis.from_url('redis://127.0.0.1:6389')
    name = uuid4().hex

    m: Monitor
    async with r.monitor() as m:
        await m.connect()
        await run(semaphore_factory(name=name, expiry=1), 0)

        # We expect the eval to generate 7 calls
        commands = [
            # EVALSHA
            str(await m.connection.read_response()),
            # SETNX
            str(await m.connection.read_response()),
            # RPUSH
            str(await m.connection.read_response()),
            # BLPOP
            str(await m.connection.read_response()),
            # PING - from connection manager
            str(await m.connection.read_response()),
            # LPUSH
            str(await m.connection.read_response()),
            # EXPIRE
            str(await m.connection.read_response()),
            # EXPIRE
            str(await m.connection.read_response()),
        ]

        # Make sure there are no other commands generated
        with pytest.raises(asyncio.TimeoutError):
            # This will time out if there are no other commands
            await asyncio.wait_for(timeout=1, fut=m.connection.read_response())

        # Make sure each command conforms to our expectations
        assert 'EVALSHA' in commands[0]
        assert 'SETNX' in commands[1]
        assert f'__self-limiters:{name}-exists' in commands[1]
        assert 'RPUSH' in commands[2]
        assert f'__self-limiters:{name}' in commands[2]
        assert 'BLPOP' in commands[3]
        assert 'LPUSH' in commands[5]
        assert 'EXPIRE' in commands[6]
        assert f'__self-limiters:{name}' in commands[6]
        assert 'EXPIRE' in commands[7]
        assert f'__self-limiters:{name}-exists' in commands[7]
