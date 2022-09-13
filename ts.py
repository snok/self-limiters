import asyncio
import logging
import sys
from datetime import datetime
from uuid import uuid4

from maturin import import_hook

log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
logging.basicConfig(level='DEBUG', format=log_format)

# Build and install
import_hook.install(bindings='pyo3')

from self_limiters import Semaphore  # noqa


async def _consume(name):
    async with Semaphore(
        name=name,
        capacity=1,
        max_sleep=0,
        redis_url='redis://127.0.0.1:6389',
    ):
        # await asyncio.sleep(0.1)
        pass


async def main():
    name = uuid4().hex[:6]
    await asyncio.gather(*[asyncio.create_task(_consume(name)) for _ in range(100)])


start = datetime.now()
asyncio.run(main())
print(datetime.now() - start)  # noqa
sys.exit(0)
