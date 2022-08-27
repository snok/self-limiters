# Timely

> This is currently a work in progress.

Timely provides a way to rate-limit your Python processes. The
rate-limits are stored in Redis, which makes it a good fit when you're
looking to enforce limits across multiple processes.

The package contains an implementation for concurrency-based time limits
([semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming))),
and an implementation for time-based rate limits
([token bucket](https://en.wikipedia.org/wiki/Token_bucket)).

Both implementations are:

- Async
- Distributed
- Fair (FIFO)
- Performant

The package is written in Rust for improved parallelism.

## Installation

```bash
pip install timely
```

## The semaphore implementation

The semaphore implementation limits concurrent operations.
It's useful when you, for example, need to make sure that you're
at most sending `n` requests to a third-party API at the same time.

The flow goes roughly like this:

<img width=800 heigh=800 src="docs/semaphore.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>Enter queue with <a href="https://redis.io/commands/rpush/">RPUSH</a> which returns the position of the node.
When using RPUSH, <a href="https://redis.io/commands/lpos/">LPOS</a> will find the first index at O(1),
and the second at O(2), and so on. By using this combination, we're prioritizing quicker access at lower indexes.
</li>
<li>
How long to sleep for depends on our position in the queue. If we're position number 10 million, we can sleep for
longer than if we're the next one up. Sleep duration is 100ms times the number of nodes ahead in the queue by
default, but the duration is configurable.
</li>
<li>
Check position with <a href="https://redis.io/commands/lpos/">LPOS</a> and raise an error if the position exceeds
the maximum allowed position, which is none by default.
</li>
<li>
When the client has finished and aexit is called, we need to clean up the queue entry we added. We could use a
distributed lock to check our position and pop the right index, but that seems needlessly inefficient. Instead, we
just pop the first index, since this will free up capacity in the semaphore for the next process.
<br><br>
We also have an unsolved issue of capacity never being freed if a process crashes before running aexit. By adding
a queue expiry we give ourselves a chance to reset the queue after a period of inactivity.
<br><br>
We run these calls in parallel.
</li>
</ol>
</details>

It is implemented as a context manager in Python. Here is an example of a semaphore which will allow 10 concurrent requests:

```python
from timely import Semaphore


async with Semaphore(
        name="unique-resource-name",
        capacity=10,
        redis_url="redis://localhost:6379"
):
    client.get(...)
```

## The token bucket implementation

The token bucket algorithm provides time-based rate limiting. By implementing
it in Rust, we're able to get around the [GIL](https://realpython.com/python-gil/) and
spawn an entirely separate process/thread to perform the work needed to assign tokens
to queued nodes (see [pyo3's](https://pyo3.rs/) section on [parallelism](https://pyo3.rs/v0.16.4/parallelism.html)
for more details).

The code flow is as follows:

<img width=800 heigh=800 src="docs/token_bucket.png"></img>

It is implemented as a context manager in Python and can be used roughly as follows:

```python
from timely import RedisTokenBucket

# Instantiate a bucket that will allow 10 requests per minute
rate_limited_queue = RedisTokenBucket(
    capacity=10,
    refill_frequency=60,
    refill_amount=10,
    redis_url="redis://localhost:6379"
)

while True:
    async with rate_limited_queue:
        # Perform the rate-limited work immediately
        ...
```

## todos

- [ ] Finish first draft of token bucket implementation
- [ ] Create [benchmarks](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [ ] Add python tests
- [ ] Set up build and publish pipeline
