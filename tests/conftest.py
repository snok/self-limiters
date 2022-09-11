import asyncio
import logging
from functools import partial
from uuid import uuid4

import pytest as pytest
import uvloop

from timely import Semaphore, TokenBucket

uvloop.install()

logger = logging.getLogger(__name__)


@pytest.fixture(autouse=True, scope='session')
def _setup_logging():
    log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
    logging.basicConfig(level='DEBUG', format=log_format)


def semaphore_factory(**kwargs) -> partial:
    """
    Provide an almost initialized semaphore with defaults.

    This makes it easy to init semaphores with slightly different configurations in tests.
    """

    defaults = {'name': uuid4().hex[:6], 'capacity': 1, 'redis_url': 'redis://127.0.0.1:6389'}
    return partial(Semaphore, **{**defaults, **kwargs})


def tokenbucket_factory(**kwargs) -> partial:
    """
    Provide an almost initialized token bucket with defaults.

    This makes it easy to init token buckets with slightly different configurations in tests.
    """

    defaults = {
        'name': uuid4().hex[:6],
        'capacity': 1,
        'refill_frequency': 1.0,
        'refill_amount': 1,
        'redis_url': 'redis://127.0.0.1:6389',
    }
    return partial(TokenBucket, **{**defaults, **kwargs})


async def run(pt: partial, duration: float) -> None:
    async with pt():
        logger.info(f'Sleeping {duration}')
        await asyncio.sleep(duration)
