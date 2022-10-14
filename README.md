<br>
<br>
<p align="center">
<a href="https://github.com/sondrelg/self-limiters"><img src="docs/logo.svg" width="280px"></a>
<br>
<b>distributed async client rate limiters</b>
<br><br>
</p>
<br>
<a href="https://pypi.org/project/self-limiters/"><img alt="PyPI" src="https://img.shields.io/pypi/v/self-limiters.svg"></a>
<a href="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml"><img alt="test status" src="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml/badge.svg"></a>
<a href="https://codecov.io/gh/sondrelg/self-limiters/"><img alt="coverage" src="https://codecov.io/gh/sondrelg/self-limiters/branch/main/graph/badge.svg?token=Q4YJPOFC1F"></a>
<a href="https://codecov.io/gh/sondrelg/self-limiters/"><img alt="python version" src="https://img.shields.io/badge/python-3.8%2B-blue"></a>
<br>
<br>

This package implements an async distributed [semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming)),
and the [token bucket algorithm](https://en.wikipedia.org/wiki/Token_bucket).
Both implementations are FIFO, and require redis.

Between them, the two implementations make it possible to regulate traffic with respect to:

- **Concurrency-based limits** (5 active requests at the same time), or

- **Time-based limits** (5 requests every 10 seconds)

This was written for rate-limiting, but the semaphore and token bucket
implementations can be used for anything.

# Installation

```shell
pip install self-limiters
```

# Usage

Both implementations are written as async context managers.

## Semaphore

The `Semaphore` context manager is used as follows:

```python
from self_limiters import Semaphore


# allows 10 concurrent requests
concurrency_limited_queue = Semaphore(  # <-- init
    name="foo",
    capacity=10,
    max_sleep=15,
    redis_url="redis://localhost:6379"
)

async def main():
    async with concurrency_limited_queue:  # <-- usage
        client.get(...)
```

A `MaxSleepExceededError` is raised after `max_sleep` seconds, if `max_sleep` is specified.

## Token bucket

The `TokenBucket` context manager is used like this:

```python
from self_limiters import TokenBucket

# allows 10 requests per minute
time_limited_queue = TokenBucket(  # <-- init
    name="unique-resource-name",
    capacity=10,
    refill_frequency=60,
    refill_amount=10,  #
    max_sleep=600,
    redis_url="redis://localhost:6379"
)

async def main():
    async with time_limited_queue:  # <-- usage
        client.get(...)
```

If `max_sleep` is set and the sleep time exceeds max-sleep a `MaxSleepExceededError` is raised.

## Decorators

The package doesn't ship any decorators, but if you want to apply these to a function,
you can roll your own, like this:

```python
from self_limiters import Semaphore


def limit(name, capacity):
    # Initialize semaphore or token bucket
    limiter = Semaphore(name=name, capacity=capacity, redis_url="redis://127.0.0.1:6389")

    def middle(f):
        async def inner(*args, **kwargs):
            # Use context manager within the decorator
            async with limiter:
                return await f(*args, **kwargs)
        return inner
    return middle


# The decorator will then ensure we sleep until ready
@limit(name="my-user-api", capacity=50)
def retrieve_user(id: UUID) -> User:
    ...
```

# Implementation breakdown

The package was written for performance, and it seems that the most performant
way to implement these algorithms is by leveraging [Lua](http://www.lua.org/about.html)
scripts. I initially wrote this in pure [Rust](https://www.rust-lang.org/), but Lua scripts
present a couple of really great benefits:

- [Lua](http://www.lua.org/about.html) scripts are executed on a redis instance,
  and lets us implement close to the entire implementation logic in a single script.
  This means our client can make 1 request to redis to run the script instead of
  1 request per redis call needed. The time saved by reducing the number of requests
  is huge.

- The initial rust implementation (pre-lua scripts) had to use the
  [redlock](https://redis.com/redis-best-practices/communication-patterns/redlock/)
  algorithm to ensure fairness. With Lua scripts (since redis instance are
  single threaded), our implementations are FIFO out of the box.

  This makes our implementation a lot faster, since we no longer need locks, and it
  simultaneously makes the code much, much simpler.

With Lua scripts, this is how our flows ended up being:

### The semaphore implementation

1. Run [lua script](https://github.com/sondrelg/self-limiters/blob/main/src/semaphore.rs#L59:L109) to create a list, which will be the foundation of our semaphore. This is skipped if it has already been created.
2. Run [`BLPOP`](https://redis.io/commands/blpop/) to non-blockingly wait until the semaphore has capacity. When it does, we pop from the list.
3. Then run another [lua script](https://github.com/sondrelg/self-limiters/blob/main/src/semaphore.rs#L143:L174) to "release" the semaphore by adding back the capacity we popped.

So in total we make 3 calls to redis (we would have made 6 without the scripts),
which are all non-blocking.

### The token bucket implementation

Here, things are *even* simpler. The steps are:

1. Run [lua script](https://github.com/sondrelg/self-limiters/blob/main/src/token_bucket.rs#L69:L159) to retrieve a wake-up time.
2. Sleep until then.

We make 1 call instead of 3, and both of these are also non-blocking.

In other words, the limiters' impact on an application event-loop should be completely negligible.

# Benchmarks

When testing on Github actions we get sub-millisecond runtimes:

- processing 100 nodes with the semaphore implementation takes ~0.6ms per instance
- processing 100 nodes with the token bucket implementation takes ~0.03ms per instance

<img src="https://slack-imgs.com/?c=1&o1=ro&url=https%3A%2F%2Fmedia4.giphy.com%2Fmedia%2FzCv1NuGumldXa%2Fgiphy.gif%3Fcid%3D6104955e8s1fovp9mroo6e9uj176fvl3o5earbfq5lkzjt03%26rid%3Dgiphy.gif%26ct%3Dg"/>

The majority of this should also still be waiting for i/o.


# Implementation details

This section breaks down how both implementations are written, in detail.
It assumes you know how a Python (async) context manager works. If this
is new to you, take a look [here](https://www.geeksforgeeks.org/context-manager-in-python/)
first!

## The semaphore implementation

The semaphore implementation is useful when you need to limit a process
to `n` concurrent actions. For example if you have 10 web servers, and
you're interacting with an API that will only tolerate 5 concurrent
requests before locking you out.

The flow can be broken down as follows:
<p align="center">
  <img width=700 heigh=600 src="docs/semaphore.png"></img>
</p>
<ul>
<li>

The [create_semaphore Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/create_semaphore.lua)
calls [`SETNX`](https://redis.io/commands/setnx/) on the key of the queue plus a postfix (if the `name`
specified in the class instantiation is "my-queue", then the queue name will be `__self-limiters:my-queue`
and setnx will be called for `__self-limiters:my-queue-exists`). If the returned value is 1 it means the
queue we will use for our semaphore does not exist yet and needs to be created.

(It might strike you as weird to maintain a separate value, just to indicate whether a list exists,
when we could just check the list itself. It would be nice if we could use
[`EXISTS`](https://redis.io/commands/exists/) on the list directly, but unfortunately a list is considered
not to exist when all elements are popped (i.e., when a semaphore is fully acquired), so I don't see
another way of doing this. Contributions are very welcome if you do!
</li>
<li>

Then if the queue needs to be created we call [`RPUSH`](https://redis.io/commands/rpush/) with the number of arguments
equal to the `capacity` value used when initializing the semaphore instance. For a semaphore with
a capacity of 5, we call `RPUSH 1 1 1 1 1`, where the values are completely arbitrary.
</li>
<li>

Once the list/queue has been created, we call [`BLPOP`](https://redis.io/commands/blpop/) to block until it's
our turn. `BLPOP` is FIFO by default. We also make sure to specify the `max_sleep` based on the initialized
semaphore instance setting. If nothing was passed we allow sleeping forever.
</li>
<li>

On `__aexit__` we call another script to [`RPUSH`](https://redis.io/commands/rpush/) a `1` value back into the queue
(i.e., release the semaphore) and set an expiry on the queue and the string value we called `SETNX` on.

The expires are a half measure for dealing with dropped capacity. If a node holding the semaphore dies,
the capacity might never be returned. If, however, there is no one using the semaphore for the duration of the
expiry value, all values will be cleared, and the semaphore will be recreated at full capacity next time it's used.
The expiry is 30 seconds at the time of writing, but could be made configurable.
</li>
</ul>

## The token bucket implementation

The token bucket implementation is useful when you need to limit a process by
time. For example, to 1 request per minute, or 500 every 10 seconds.

The implementation is forward-looking. It works out the time there *would have been*
capacity in the bucket for a given client and returns that time. From there we can
asynchronously sleep until it's time to perform our rate limited action.

The flow can be broken down as follows:

<p align="center">
<img align=center width=700 src="docs/token_bucket.png"></img>
</p>

<ul>
<li>

Call the [schedule Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/schedule.lua)
which first [`GET`](https://redis.io/commands/get/)s the *state* of the bucket.

The bucket state contains the last slot scheduled and the number of tokens left for that slot.
With a capacity of 1, having a `tokens_left_for_slot` variable makes no sense, but if there's
capacity of 2 or more, it is possible that we will need to schedule multiple clients to the
same slot.

The script then works out whether to decrement the `tokens_left_for_slot` value, or to
increment the slot value wrt. the frequency variable.

Finally, we store the bucket state again using [`SETEX`](https://redis.io/commands/setex/).
This allows us to store the state and set expiry at the same time. The default expiry
is 30 at the time of writing, but could be made configurable.

One thing to note, is that this would not work if it wasn't for the fact that redis is single threaded,
so Lua scripts on Redis are FIFO. Without this we would need locks and a lot more logic.
</li>
<li>

Then we just sleep. Simple!
</li>
</ul>


# Contributing

Please do!

Feedback on the implementation, issues, and PRs are welcome.
See `CONTRIBUTING.md` for more details.
