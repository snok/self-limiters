import asyncio
import logging
import sys
from datetime import datetime
from uuid import uuid4

from maturin import import_hook
from timely import RedisSemaphore

log_format = '[%(asctime)s] [%(levelname)s] - %(message)s'
logging.basicConfig(level='INFO', format=log_format)

# install the import hook with default settings
import_hook.install()
# or you can specify bindings
import_hook.install(bindings='pyo3')
# and build in release mode instead of the default debug mode
import_hook.install(release=True)


# now you can start importing your Rust module
# from timely import TokenBucket  # noqa
#


async def _consume(name):
    async with RedisSemaphore(
        name=name,
        capacity=5,
        redis_url='redis://127.0.0.1:6389',
    ):
        await asyncio.sleep(0.1)


async def main():
    name = uuid4().hex[:6]
    await asyncio.gather(*[asyncio.create_task(_consume(name)) for _ in range(100)])


start = datetime.now()
asyncio.run(main())
print(datetime.now() - start)  # noqa
sys.exit(0)
