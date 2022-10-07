import asyncio
import sys
from datetime import datetime, timedelta
from functools import partial
from statistics import median
from uuid import uuid4

import typer

from self_limiters import Semaphore, TokenBucket

# {type}_{iterations}_{count}
SEMAPHORE_10x10 = 0.231233
SEMAPHORE_50x10 = 0.0313455
SEMAPHORE_15x40 = 1.54161
SEMAPHORE_40x100 = 9.970499
SEMAPHORE_50x25 = 0.07681850000000001
SEMAPHORE_50x100 = 0.3069965
SEMAPHORE_60x20 = 2.617198
TOKEN_BUCKET_TIME_10x10 = 0.3


async def consume(t: Semaphore | TokenBucket, sleep: int):
    async with t:
        await asyncio.sleep(sleep)


async def run_tasks(t, sleep, count):
    """Run `count` tasks to consume the same limiter."""
    tasks = [asyncio.create_task(consume(t, sleep)) for _ in range(count)]
    start = datetime.now()
    await asyncio.gather(*tasks)
    return datetime.now() - start


async def run_iterations(t: partial[Semaphore | TokenBucket], sleep: float, count: int, iterations: int):
    """Run `iterations` permutations of one type of limiter."""
    return [await run_tasks(t(name=uuid4().hex[:6]), sleep, count) for _ in range(iterations)]


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
        n = 'SEMAPHORE'
        t = partial(Semaphore, capacity=capacity, max_sleep=max_sleep, redis_url=redis_url)
    elif type.startswith('t'):
        typer.echo('Testing token bucket...')
        n = 'TOKEN_BUCKET'
        t = partial(
            TokenBucket,
            refill_frequency=0.1,
            refill_amount=1,
            capacity=capacity,
            max_sleep=max_sleep,
            redis_url=redis_url,
        )
    else:
        typer.echo(f'type must be \'semaphore\' or \'token_bucket\', not {type}', color=True, err=True)
        return exit(1)

    times: list[timedelta] = asyncio.run(run_iterations(t, sleep, count, iterations))

    seconds = median([t.seconds + t.microseconds / 1_000_000 for t in times])

    established_time_name = f'{n}_{iterations}x{count}'
    established_time = getattr(sys.modules[__name__], established_time_name, None)

    if not established_time:
        typer.echo(f"Hardcode benchmark time: '{established_time_name} = {seconds}'")
        return exit(1)

    if (lt := seconds < established_time * 0.95) or (gt := seconds > established_time * 1.05):
        if lt:
            typer.echo(
                f'There was a {(established_time / seconds - 1) * 100 :.2f}% performance improvement\n'
                f'Hardcode the new time: \'{established_time_name} = {seconds}\' in bench.py'
            )

        elif gt:
            typer.echo(
                f'Error: Runtime increased by {(seconds / established_time - 1) * 100 :.2f}%, from'
                f' {established_time:.2f} to {seconds:.2f} '
            )

        return exit(1)

    print('All good!')


if __name__ == '__main__':
    typer.run(main)
