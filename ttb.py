import asyncio
import logging
import sys
from datetime import datetime
from uuid import uuid4

from maturin import import_hook

log_format = '%(asctime)s - %(message)s'  # [%(asctime)s] [%(levelname)s] -
logging.basicConfig(level='DEBUG', format=log_format)

# Build and install
import_hook.install(bindings='pyo3')

from tl import TokenBucket  # noqa


async def _consume(name):
    tb = TokenBucket(
        name=name,
        capacity=1,
        refill_frequency=0.1,
        refill_amount=1,
        redis_url='redis://127.0.0.1:6389',
    )
    async with tb:
        await asyncio.sleep(0.1)


async def main():
    name = uuid4().hex[:6]
    await asyncio.gather(*[asyncio.create_task(_consume(name)) for _ in range(99)])


start = datetime.now()
asyncio.run(main())
print(datetime.now() - start)  # noqa
sys.exit(0)
