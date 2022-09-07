import asyncio
import logging
from typing import TYPE_CHECKING

import pytest as pytest
import uvloop
from maturin import import_hook

if TYPE_CHECKING:
    from functools import partial


uvloop.install()

logger = logging.getLogger(__name__)


@pytest.fixture(autouse=True, scope='session')
def _install_package():
    import_hook.install(bindings='pyo3')


@pytest.fixture(autouse=True, scope='session')
def _setup_logging():
    log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
    logging.basicConfig(level='DEBUG', format=log_format)


async def run(pt: 'partial', duration: float) -> None:
    async with pt():
        logger.info(f'Sleeping {duration}')
        await asyncio.sleep(duration)
