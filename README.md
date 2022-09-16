<br>
<br>
<p align="center">
<a href="https://github.com/sondrelg/self-limiters"><img src="docs/logo.svg" width="250px"></a>
<br>
<b>distributed async rate limiters for clients</b>
<br><br>
</p>
<hr>
<a href="https://pypi.org/project/self-limiters/"><img alt="PyPI" src="https://img.shields.io/pypi/v/self-limiters.svg"></a>
<a href="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml"><img alt="test status" src="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml/badge.svg"></a>
<a href="https://codecov.io/gh/sondrelg/self-limiters/"><img alt="coverage" src="https://codecov.io/gh/sondrelg/self-limiters/branch/main/graph/badge.svg?token=Q4YJPOFC1F"></a>
<a href="https://codecov.io/gh/sondrelg/self-limiters/"><img alt="python version" src="https://img.shields.io/badge/python-3.8%2B-blue"></a>
<br>
<br>

This library implements an async distributed [semaphore](https://en.wikipedia.org/wiki/Semaphore_(programming)),
as well as the [token bucket algorithm](https://en.wikipedia.org/wiki/Token_bucket).
Both implementations are FIFO, and use redis as the backend.

Between them, the two implementations make it possible to regulate traffic with respect to:

- **Concurrency based limits** (max 5 active requests at the time, across all servers), or
- **Time based limits** (max 5 requests every 10 seconds, across all servers)

The motivation for implementing these was to help with rate-limiting,
but the semaphore and token bucket implementations can be used for anything.

# Installation

```bash
pip install self-limiters
```

# Usage

Both implementations are written as async context managers,
and are used the same way.

## Semaphore

A semaphore can be initialized and used like this:

```python
from self_limiters import Semaphore


# allow 10 concurrent requests
concurrency_limited_queue = Semaphore(  # <-- init
    name="foo",
    capacity=10,
    max_sleep=15,
    redis_url="redis://localhost:6379"
)

async def main():
    async with concurrency_limited_queue:  # <-- use
        client.get(...)
```

A `MaxSleepExceededError` is raised after `max_sleep` seconds, if a value was specified. By default,
the value is zero, which means wait forever.

## Token bucket

A token bucket is used like this:

```python
from self_limiters import TokenBucket

# allow 10 requests per minute
time_limited_queue = TokenBucket(  # <-- init
    name="unique-resource-name",
    capacity=10,
    refill_frequency=60,
    refill_amount=10,  #
    max_sleep=600,
    redis_url="redis://localhost:6379"
)

async def main():
    async with time_limited_queue:  # <-- use
        client.get(...)
```

The token bucket implementation immediately estimates when a new entry to the queue will
have its turn. A `MaxSleepExceededError` is raised if the time estimated exceeds the specified max sleep.
If `max_sleep` is unspecified, we'll let the queue grow forever.

## Decorators

The package doesn't ship any decorators, but if you want to apply these to a function,
you can roll your own, like this:

```python
from functools import wraps

from self_limiters import Semaphore


def limit(name, capacity):
    # Initialize semaphore or token bucket
    limiter = Semaphore(name=name, capacity=capacity, ...)

    async def middle(f):
        @wraps(f)
        async def inner(*args, **kwargs):
            # Use context manager within the decorator
            async with limiter:
                return f(*args, **kwargs)
        return inner
    return middle


# The decorator will then ensure we sleep until ready
@limit(name="my-user-api", capacity=50)
def retrieve_user(id: UUID) -> User:
    ...
```

# Performance considerations

Some parts of the package logic are implemented using Lua scripts, to run
_on_ the redis instance. This makes it possible to do the same work in one
request (from the client), that would otherwise need `n` requests. Lua scripts
seems to present the most efficient way to run the calls each implementation
requires us to, and client calls to Lua scripts are non-blocking.

For example, the flow of the semaphore implementation is:
- Run our [initial Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/create_semaphore.lua) to create the semaphore in Redis, if needed
- Run [`BLPOP`](https://redis.io/commands/blpop/) to wait until we acquire the semaphore
- Run [the cleanup script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/release_semaphore.lua) to "release" the semaphore by adding back capacity

Each step is one call by the client, and all of these are non-blocking.

For the second implementation, the token bucket, the breakdown is even simpler.
- Run the [initial Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/schedule.lua) to retrieve a wake-up time
- Sleep asynchronously until the wake-up time

Both of these are also non-blocking.

In other words, the limiters' impact on the application event-loop should be negligible.

# Implementation details

This section assumes you know how a Python (async) context manager works.
If this is new to you, take a look [here](https://www.geeksforgeeks.org/context-manager-in-python/) first!

## The semaphore implementation

The semaphore implementation is useful when you need to limit a process
to `n` concurrent actions. For example if you have 10 web servers, and
you're interacting with an API that will only tolerate 5 concurrent
requests before locking you out.

In terms of fairness, the semaphore implementation skews towards FIFO,
but is opportunistic. A worker will not be allowed to run until there
is capacity assigned to them, specifically; but the order of execution
is not guaranteed to be exactly FIFO.

The flow can be broken down as follows:
<p align="center">
<img width=700 heigh=700 src="docs/semaphore.png"></img>
</p>
<ul>
<li>

An [initial Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/create_semaphore.lua)
will call [`SETNX`](https://redis.io/commands/setnx/) on the name of the queue plus a postfix (if the `name`
specified in the class instantiation is "my-queue", then the queue name will be `__self-limiters:my-queue`).
If the returned value is 1 it means the queue we will use for our semaphore does not exist yet and needs to be created.

(It might strike you as weird to maintain a separate string, just to indicate the existence of a list,
when we could just check the list itself. It would actually be great if we could use
[`EXISTS`](https://redis.io/commands/exists/) on the list directly, but a list is deleted when all elements are
popped (i.e., when a semaphore is fully acquired), so I don't see another way of doing this.
Contributions are welcome if you do.)
</li>
<li>

If the queue needs to be created we call [`RPUSH`](https://redis.io/commands/rpush/) with the number of arguments
equal to the `capacity` value used when initializing the semaphore instance.
</li>
<li>

Once the queue has been created, we call [`BLPOP`](https://redis.io/commands/blpop/) to block until it's
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

A [Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/schedule.lua)
[`GET`](https://redis.io/commands/get/)s the state of the bucket.

The bucket has state for the last slot scheduled and the number of tokens left for that slot.
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

# Benchmarks

When testing locally:

- processing 100 nodes with the semaphore implementation takes ~6ms
- processing 100 nodes with the token bucket implementation takes ~4ms

<img src="https://slack-imgs.com/?c=1&o1=ro&url=https%3A%2F%2Fmedia4.giphy.com%2Fmedia%2FzCv1NuGumldXa%2Fgiphy.gif%3Fcid%3D6104955e8s1fovp9mroo6e9uj176fvl3o5earbfq5lkzjt03%26rid%3Dgiphy.gif%26ct%3Dg"/>

# Contributing

Please do!
