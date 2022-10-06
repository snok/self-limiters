import asyncio
from datetime import datetime, timedelta
from functools import partial
from pprint import pprint
from statistics import median
from uuid import uuid4

import typer
from self_limiters import Semaphore, TokenBucket


async def run(t: Semaphore | TokenBucket, sleep: float):
    """Run context manager."""
    async with t:
        await asyncio.sleep(sleep)


async def run_n(*, n: int, t: Semaphore | TokenBucket, sleep: float):
    """Call run `n` times for the same limiter."""
    tasks = [asyncio.create_task(run(t, sleep)) for _ in range(n)]
    start = datetime.now()
    await asyncio.gather(*tasks)
    return datetime.now() - start


async def run_iterations(t: partial[Semaphore | TokenBucket], sleep: float, count: int, iterations: int):
    """Call run_n for `iterations` permutations of a limiter."""
    return [await run_n(n=count, t=t(name=uuid4().hex[:6]), sleep=sleep) for _ in range(iterations)]


def main(
    type: str,
    count: int = 10,
    iterations: int = 10,
    capacity: int = 1,
    max_sleep: float = 0.0,
    redis_url: str = 'redis://127.0.0.1:6389',
    sleep: float = 0.0,
):
    t: partial[TokenBucket | Semaphore]
    if type.startswith('s'):
        typer.echo('Testing semaphore...')
        t = partial(Semaphore, capacity=capacity, max_sleep=max_sleep, redis_url=redis_url)
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

    avg = sum(seconds) / iterations
    med = median(seconds)
    print(f'Average was {avg / count * 1000:.1f}ms per run')
    print(f'Median was {med / count * 1000:.1f}ms per run')
    pprint(seconds)


if __name__ == '__main__':
    typer.run(main)
