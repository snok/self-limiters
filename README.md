<br>
<p align="center">
<a href="https://github.com/snok/self-limiters"><img src="docs/logo.svg" width="250px"></a>
<br>
<b>Distributed async rate limiters for clients</b>
<br><br>
<a href="https://pypi.org/project/self-limiters/"><img alt="PyPI" src="https://img.shields.io/pypi/v/self-limiters?label=Release&style=flat-square"></a>
<a href="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml"><img alt="test status" src="https://github.com/sondrelg/self-limiters/actions/workflows/publish.yml/badge.svg"></a>
<a href="https://codecov.io/gh/sondrelg/self-limiters/"><img alt="coverage" src="https://codecov.io/gh/sondrelg/self-limiters/branch/main/graph/badge.svg?token=Q4YJPOFC1F"></a>
</p>
<br>

A self-limiting source produces traffic which never exceeds some upper bound.
This is important when interacting with rate limited resources,
or to prevent burstiness.
`self-limiters` provides a way to police your outgoing traffic
with respect to:

- Concurrency, using a distributed [semaphore](https://en.wikipedia.org/wiki/semaphore_(programming))
- Time, using a distributed [token bucket](https://en.wikipedia.org/wiki/Token_bucket)

To use this package, you'll need to be running an `async` stack,
and have redis available on Python 3.8 or above.

## Installation

```bash
pip install self-limiters
```

## Performance considerations

Some parts of the package logic are implemented using Lua scripts, to run
_on_ the redis instance. This makes it possible to do the same work in one
request (from the client), that would otherwise take 4. One benefit of this
is that it eliminates the latency for each request saved. However, the biggest
benefit is while the lua script is running, our python app event-loop is
freed up to do other things.

The flow of the semaphore implementation is:
- Run [initial script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/create_semaphore.lua) to create semaphore if needed
- Run [`BLPOP`](https://redis.io/commands/blpop/) to wait for the semaphore to return
- Run [script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/release_semaphore.lua) to "release" the semaphore by adding back capacity

All of these are non-blocking.

The flow for the token bucket implementation is:
- Run initial script to retrieve a wake-up time
- Sleep asynchronously until the wake-up time

Both of these are also non-blocking.

In other words, the limiters' impact on the application event-loop should be negligible.

## The semaphore implementation

The semaphore implementation is useful when you need to limit a process
to `n` concurrent actions. For example if you have 10 web servers, and
you're interacting with an API that will only tolerate 5 concurrent
requests before locking you out.

In terms of fairness, the semaphore implementation skews towards FIFO,
but is opportunistic. A worker will not be allowed to run until there
is capacity assigned to them, specifically; but the order of execution
is not guaranteed to be exactly FIFO.

The flow goes roughly like this:

<img width=800 heigh=800 src="docs/semaphore.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>

The [Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/create_semaphore.lua) will call [`SETNX`](https://redis.io/commands/setnx/) on the name of the
queue plus a postfix. If the returned value is 1 it means the queue we will use for our
semaphore does not exist yet and needs to be created.

(It might strike you as weird to have a separate entry for indicating whether the list
should be created or not. It would be great if we could use [`EXISTS`](https://redis.io/commands/exists/)
on the list directly instead, but a list is deleted when all elements are popped, so I don't see
another way of achieving this. Contributions are welcome if you do.)
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
and set an expiry on the queue and the value we called `SETNX` on.

The expires are a half measure for dealing with dropped capacity. If a node holding the semaphore dies,
the capacity might never be returned. If, however, there is no one using the semaphore for the duration of the
expiry value, all values will be cleared, and the semaphore will be recreated at full capacity next time it's used.
The expiry is 30 seconds at the time of writing, but could be made configurable.
</li>
</ol>
</details>

### Usage

The utility is implemented as a context manager in Python. Here is an example of a semaphore which will allow 10 concurrent requests:

```python
from self_limiters import semaphore


# Instantiate a semaphore that will allow 10 concurrent requests
concurrency_limited_queue = semaphore(
    name="unique-resource-name",
    capacity=10,
    redis_url="redis://localhost:6379"
)

while True:
    async with concurrency_limited_queue:
        client.get(...)
```

## The token bucket implementation

The token bucket implementation is useful when you need to limit a process to a
certain number of actions per unit of time. For example, 1 request per minute.

The implementation is forward-looking. It works out the time there *would have been*
capacity in the bucket for a given client and returns that time. From there we can
asynchronously sleep until it's time to perform our rate limited action.

The code flow goes:

<img width=800 heigh=800 src="docs/token_bucket.png"></img>

<details>
<summary><b>Flow breakdown</b></summary>
<ol>
<li>

The [Lua script](https://github.com/sondrelg/self-limiters/blob/main/src/scripts/schedule.lua)
first [`GET`](https://redis.io/commands/get/)s the state of the bucket. That means, the last slot
that was scheduled and the number of tokens left for that slot. With a capacity of 1,
having a `tokens_left_for_slot` variable makes no sense, but if there's capacity of 2 or more,
it is possible that we will need to schedule multiple clients on the same slot.

The script then works out whether to decrement the `tokens_left_for_slot` value, or to
increment the slot value wrt. the frequency variable.

Finally, we store the bucket state again using [`SETEX`](https://redis.io/commands/setex/).
This allows us to store the state and set expiry at the same time. The default expiry
is 30 at the time of writing, but could be made configurable.

One thing to note, is that this would not work if it wasn't for the fact that redis is single threaded,
so Lua scripts on Redis are FIFO. Without this we would need locks and a lot more logic.
</li>
<li>

Then we just sleep!
</li>
</ol>
</details>



### Usage

This is also implemented as a context manager in Python and can be used roughly as follows:

```python
from self_limiters import TokenBucket

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


## Contributing

### Debugging Lua scripts

Assuming you have a redis server running at `:6389` you can debug
a lua script by calling `redis-cli -u redis://127.0.0.1:6389 --ldb --eval src/semaphore/rpushnx.lua x 1`.

Just type `help` in the debugger for options.
