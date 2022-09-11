[![codecov](https://codecov.io/gh/sondrelg/timely/branch/main/graph/badge.svg?token=Q4YJPOFC1F)](https://codecov.io/gh/sondrelg/timely)

# Timely

> This is currently a work in progress.

Timely provides two types of rate limiters, as a way to police your own processes.

Rate-limited APIs often enforce the rate limits by penalizing excessive use.
It's generally in everyone's interest that this doesn't happen.

This package contains one implementation for concurrency-based time limits
([semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming))),
and one implementation for time-based rate limits
([token bucket](https://en.wikipedia.org/wiki/Token_bucket)).

Both implementations are async, FIFO, and queues are distributed using [Redis](https://redis.io).

## Installation

```bash
pip install timely
```

## The semaphore implementation

The semaphore implementation is useful when you need to limit a process
to `n` actions at the same time.

In terms of fairness, the semaphore implementation skews towards
FIFO, but is opportunistic. A worker will not be allowed to run until
there is capacity assigned to them, specifically; but the order of
execution is not guaranteed to be exactly FIFO.

The flow goes roughly like this:

<img width=800 heigh=800 src="docs/semaphore.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>Enter queue (<a href="https://redis.io/commands/rpush/">RPUSH</a>*) and get the queue position.

*<small>When using RPUSH, <a href="https://redis.io/commands/lpos/">LPOS</a> will find the first index at O(1),
and the second at O(2), and so on. By using this combination, we're prioritizing quicker access at lower indexes.</small>
</li>
<li>
How long to sleep for depends on our position in the queue. If the capacity of the semaphore is 10, and we are position 15,
then we need 5 places to be freed up before it's our turn. In this case, we sleep 5 * `sleep_duration`, which is 100ms
by default and can be specified when instantiating the Semaphore class.
</li>
<li>
Check our position (<a href="https://redis.io/commands/lpos/">LPOS</a>) and raise an error if the position exceeds
the maximum allowed position (none, by default).
</li>
<li>
After running the rate limited code, we need to clean up the queue entry we added.
We could use a distributed lock to check our position and pop the right index, but that seems needlessly inefficient. Instead, we
just pop the first index, since this will free up capacity in the semaphore for the next process.

There is a potential issue where capacity will never be freed if a process crashes before running aexit. To remedy
this slightly, we specify that the queue should expire after a short period of inactivity.
</li>
</ol>
</details>

### Usage

The utility is implemented as a context manager in Python. Here is an example of a semaphore which will allow 10 concurrent requests:

```python
from timely import Semaphore


# Instantiate a semaphore that will allow 10 concurrent requests
concurrency_limited_queue = Semaphore(
    name="unique-resource-name",
    capacity=10,
    redis_url="redis://localhost:6379"
)

while True:
    async with concurrency_limited_queue:
        client.get(...)
```

## The token bucket implementation

The token bucket implementation is useful when you need to limit a process
to `n` actions per `m` seconds.

The token bucket algorithm, in short, is meant to work by having a token added every `m` seconds. When there
is a token in the bucket, a queueing node can consume it and continue.

This implementation, unlike the semaphore algorithm, does not rely on continuous polling to see whether there is
capacity or not. Instead, we let a scheduler process work out *when* a token would have been added for each node
in the queue, and let the nodes sleep until it's time to consume their token. The aim is to minimize i/o and maximize
sleep, as sleeping asynchronously frees up the event loop to do other things.

The code flow goes:

<img width=800 heigh=800 src="docs/token_bucket.png"></img>

By implementing parts of the logic in a [Lua](https://www.lua.org/) script, we're able to get great
performance, while minimising i/o. Since redis is single-threaded it also reduces complexity greatly.

### Usage

This is also implemented as a context manager in Python and can be used roughly as follows:

```python
from timely import TokenBucket

# Instantiate a bucket that will allow 10 requests per minute
time_limited_queue = TokenBucket(
    name="unique-resource-name",
    capacity=10,
    refill_frequency=60,
    refill_amount=10,
    redis_url="redis://localhost:6379"
)

while True:
    async with time_limited_queue:
        # Perform the rate-limited work immediately
        client.get(...)
```


## Benchmarks

When testing locally:

- processing 100 nodes with the semaphore implementation takes ~13ms
- processing 100 nodes with the token bucket implementation takes ~7ms

<img src="https://slack-imgs.com/?c=1&o1=ro&url=https%3A%2F%2Fmedia4.giphy.com%2Fmedia%2FzCv1NuGumldXa%2Fgiphy.gif%3Fcid%3D6104955e8s1fovp9mroo6e9uj176fvl3o5earbfq5lkzjt03%26rid%3Dgiphy.gif%26ct%3Dg"/>
