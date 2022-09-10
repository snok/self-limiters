[![codecov](https://codecov.io/gh/sondrelg/timely/branch/main/graph/badge.svg?token=Q4YJPOFC1F)](https://codecov.io/gh/sondrelg/timely)

# Timely

> This is currently a work in progress.

Timely provides a way to rate-limit your Python processes.

It is not uncommon for rate-limited APIs to punish clients when
they exceed a limit, so it's usually a good idea for clients to police
themselves in these cases.

This package contains one implementation for concurrency-based time limits
([semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming))),
and one implementation for time-based rate limits
([token bucket](https://en.wikipedia.org/wiki/Token_bucket)).

Both implementations `async`, fair, and queues are distributed using [Redis](https://redis.io).

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

It is implemented as a context manager in Python. Here is an example of a semaphore which will allow 10 concurrent requests:

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
sleeping, as sleeping asynchronously will free up the event loop to do other things.

By implementing this in Rust, we're able to get around the [GIL](https://realpython.com/python-gil/), and
structure some logic in a separate thread. See [pyo3's](https://pyo3.rs/) section on
[parallelism](https://pyo3.rs/v0.16.4/parallelism.html) for more details.

The code flow goes:

<img width=800 heigh=800 src="docs/token_bucket.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>Enter queue (<a href="https://redis.io/commands/rpush/">RPUSH</a>) and get the queue position*.

Since the scheduler is spawned at the same time as our main thread, and the scheduler cannot queue our node until
we've entered the queue, there is zero percent likelihood that we would find an assigned slot if we checked right away.
Knowing this, we take the opportunity to sleep a little before proceeding. More info about the sleep in the next point.<br>

*<small>The "position" here is not the absolute position of the node in the queue. The scheduler process pops IDs from
the queue when assigning wake-up times (slots), so the position returned when entering the queue is rather an indication
of how close our node is to have a slot assigned to it.
</small>
</li>
<li>
How long to sleep for depends on our position in the queue. As mentioned in the last point, our position is not
the position _in_ the overall queue, but <= our real position. Knowing this we can create a lower-bound answer
to how long we can wait, at minimum, until it's our time to consume a token.

To give an example: if our position is 2, we know that our real position is 2 or more. If tokens are refilled by
1 token at a frequency of once per second, we know we can sleep at least 2 seconds, minus a small margin.
So we that's what we do.
</li>
<li>
If we fail to acquire the lock it should mean the lock is owned. In this case we can safely exit.

The image isn't exactly accurate here though, as we will poll to check that our ID has been scheduled before we actually exit.

We could also fail because Redis is unavailable. We don't currently have handling for this, and assume the above has happened.
</li>
<li>
The lock has an expiration of 1000ms. Between each loop of scheduling we make sure that expiration, minus a buffer, has not been reached.
If it has, we save the bucket state and release the lock.

If we didn't do this, another thread would acquire the lock, and we would start double-scheduling slots.
</li>
<li>
We fetch an amount of nodes, equal to the tokens in the bucket. There's no guarantee that there will be
any node IDs to fetch in the queue; the result will contain `0..n` IDs.
</li>
<li>
It's possible, though unlikely, that the scheduler will finish scheduling all nodes before the main thread
has had time to enter the queue. In a situation where there's only 1 node in the bucket, this could
lead to deadlocks. To prevent this, we make sure, in the scheduler thread, that we've scheduled ourselves before exiting.
</li>
</ol>
</details>

It is implemented as a context manager in Python and can be used roughly as follows:

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
