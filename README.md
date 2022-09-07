# Timely

> This is currently a work in progress.

Timely provides a way to rate-limit your Python processes. The
rate-limits are stored in Redis, which makes it a good fit when you're
looking to enforce limits across multiple processes.

The package contains an implementation for concurrency-based time limits
([semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming))),
and an implementation for time-based rate limits
([token bucket](https://en.wikipedia.org/wiki/Token_bucket)).

Both implementations are written are asynchronous and distributed. The package is written in Rust for improved parallelism.

## Installation

```bash
pip install timely
```

## The semaphore implementation

The semaphore implementation limits concurrent operations.
It's useful when you, for example, need to make sure that you're
at most sending `n` requests to a third-party API at the same time.

In terms of fairness, the semaphore implementation skews towards
FIFO, but is opportunistic. If a new node enters the queue when the
semaphore is below capacity, we immediately return.

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

The token bucket algorithm provides time-based rate limiting. By implementing
it in Rust, we're able to get around the [GIL](https://realpython.com/python-gil/) and
spawn an entirely separate process/thread to perform the work needed to assign tokens
to queued nodes (see [pyo3's](https://pyo3.rs/) section on [parallelism](https://pyo3.rs/v0.16.4/parallelism.html)
for more details).

The code flow goes:

<img width=800 heigh=800 src="docs/token_bucket.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>Enter queue with <a href="https://redis.io/commands/rpush/">RPUSH</a> which returns the position of the node.
<br><br>
The position is not really the position wrt. which token we can expect to be assigned, since the queue is consumed
by the scheduler and trimmed using <a href="https://redis.io/commands/rpop/">RPOP</a>. Instead, it's an indication
of how close our node is to have a slot assigned to it.
<br><br>
Since the scheduler is spawned at the same time as our main thread, and the scheduler cannot queue our node until
we've entered the queue, there is zero percent likelihood that we would find an assigned slot if we checked right away.
Knowing this, we take the opportunity to sleep a little before proceeding. More info about the sleep in the next point.<br>
</li>
<li>
How long to sleep for depends on our position in the queue. As mentioned in the last point, our position is not
the position _in_ the overall queue, but <= our real position. Knowing this we can create a lower-bound answer
to how long we can at least wait until it's our time to consume a token. To give an example, if our position
is 2, we know that our real position is 2 or more. If tokens are refilled by 1 a frequency of once per second,
we know we can sleep at least 2 seconds, minus a small margin. So we do just that. This should also minimise
overall i/o.
</li>
<li>
If we fail to acquire the lock it should mean the lock is owned. In this case we can safely exit.
<br><br>
The image isn't exactly accurate here though, as we will poll to check that our ID has been scheduled before we actually exit.
<br><br>
We could also fail because Redis is unavailable. We don't currently have handling for this, and assume the above has happened.
</li>
<li>
The lock has an expiration of 1000ms. Between each loop of scheduling we make sure that expiration, minus a buffer, has not been reached.
If it has, we save the bucket state and release the lock.
<br><br>
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

## todos

- [x] Finish first draft of token bucket implementation
- [ ] Create [benchmarks](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [x] Add python tests
- [ ] Set up build and publish pipeline
- [x] Try to prune a few dependencies (chrono, ..)
