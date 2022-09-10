import asyncio
import logging
from typing import TYPE_CHECKING

import pytest as pytest
import uvloop

if TYPE_CHECKING:
    from functools import partial

uvloop.install()

logger = logging.getLogger(__name__)


@pytest.fixture(autouse=True, scope='session')
def _setup_logging():
    log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
    logging.basicConfig(level='DEBUG', format=log_format)


async def run(pt: 'partial', duration: float) -> None:
    async with pt():
        logger.info(f'Sleeping {duration}')
        await asyncio.sleep(duration)
