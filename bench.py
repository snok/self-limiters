from __future__ import annotations

import asyncio
from datetime import datetime, timedelta
from functools import partial
from statistics import median
from typing import Optional, Union
from uuid import uuid4

import typer
from self_limiters import Semaphore, TokenBucket


async def run(t: Union[Semaphore, TokenBucket], sleep: float):
    """Run context manager."""
    async with t:
        await asyncio.sleep(sleep)


async def run_n(*, n: int, t: Union[Semaphore, TokenBucket], sleep: float):
    """Call run `n` times for the same limiter."""
    tasks = [asyncio.create_task(run(t, sleep)) for _ in range(n)]
    start = datetime.now()
    await asyncio.gather(*tasks)
    return datetime.now() - start


async def run_iterations(t: partial, sleep: float, count: int, iterations: int):
    """Call run_n for `iterations` permutations of a limiter."""
    return [await run_n(n=count, t=t(name=uuid4().hex[:6]), sleep=sleep) for _ in range(iterations)]


def main(
    type: str,
    count: int = 10,
    iterations: int = 10,
    target: Optional[float] = None,
    capacity: int = 1,
    max_sleep: float = 0.0,
    redis_url: str = 'redis://127.0.0.1:6389',
    sleep: float = 0.0,
):
    """
    Runs a simple benchmark using the library limiters.

    Can be run like:

        python bench.py s \
            --count 100 \
            --iterations 12 \
            --target 4

    :param type: Which of the limiters to use. Semaphore or TokenBucket.
    :param count: How many context managers to run for a single limiter.
    :param iterations: How many limiters to run.
    :param target: What maximum time to allow, in ms.
    :param capacity: The limiter capacity.
    :param max_sleep: The limiter max sleep.
    :param redis_url: Redis connection string.
    :param sleep: How long to sleep before exiting context manager closure.
    :return: Nothing.
    """
    t: partial
    if type.startswith('s'):
        typer.echo('Testing semaphore...')
        t = partial(Semaphore, capacity=capacity, max_sleep=max_sleep, redis_url=redis_url, connection_pool_size=30)
        offset = 0.0
    elif type.startswith('t'):
        typer.echo('Testing token bucket...')
        t = partial(
            TokenBucket,
            refill_frequency=0.01,
            refill_amount=1,
            capacity=capacity,
            max_sleep=max_sleep,
            redis_url=redis_url,
            connection_pool_size=30,
        )
        offset = 0.01 * count
    else:
        typer.echo(f'type must be \'semaphore\' or \'token_bucket\', not {type}', color=True, err=True)
        return exit(1)

    times: list[timedelta] = asyncio.run(run_iterations(t, sleep, count, iterations))

    seconds = [t.seconds + t.microseconds / 1_000_000 - offset for t in times]

    # discard first iterations
    n = len(seconds) // 10
    print(f'Discarding {n} first samples')
    seconds = seconds[n:]

    avg = sum(seconds) / iterations / count * 1000
    med = median(seconds) / count * 1000
    print(f'Average was {avg :.2f}ms per run')
    print(f'Median was {med:.2f}ms per run')

    if target:
        assert med <= target, f'Median time of {med}ms was not above target of {target}ms'
        print(f'Median time was below target of {target}ms')


if __name__ == '__main__':
    typer.run(main)
