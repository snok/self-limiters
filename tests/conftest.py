import asyncio
import logging
from functools import partial
from pathlib import Path
from typing import TYPE_CHECKING
from uuid import uuid4

from self_limiters import Semaphore, TokenBucket

if TYPE_CHECKING:
    from datetime import timedelta

logger = logging.getLogger(__name__)

REPO_ROOT = Path(__file__).parent.parent


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


def delta_to_seconds(t: 'timedelta') -> float:
    return t.seconds + t.microseconds / 1_000_000


async def run(pt: partial, duration: float) -> None:
    async with pt():
        logger.info(f'Sleeping {duration}')
        await asyncio.sleep(duration)
