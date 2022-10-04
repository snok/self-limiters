from types import TracebackType
from typing import Optional

class TokenBucket:
    def __init__(
        self,
        name: str,
        capacity: int,
        refill_frequency: float,
        refill_amount: int,
        redis_url: Optional[str] = None,  # will be set as "redis://127.0.0.1:6379" if None
        max_sleep: Optional[float] = None,  # will be set to 0.0 if None. In seconds.
    ) -> None: ...

    capacity: int
    name: str
    refill_frequency: float
    refill_amount: int

    async def __aenter__(self) -> None: ...
    async def __aexit__(
        self, exc_type: type[BaseException] | None, exc_val: BaseException | None, exc_tb: TracebackType | None
    ) -> None: ...

class Semaphore:
    def __init__(
        self,
        name: str,
        capacity: int,
        max_sleep: Optional[float] = None,  # Set to 0.0 when None is passed. In seconds.
        redis_url: Optional[str] = None,  # will be set as "redis://127.0.0.1:6379" if None
    ) -> None: ...

    capacity: int
    name: bytes
    max_sleep: int

    async def __aenter__(self) -> None: ...
    async def __aexit__(
        self, exc_type: type[BaseException] | None, exc_val: BaseException | None, exc_tb: TracebackType | None
    ) -> None: ...

__all__: list[str]

class RedisError(Exception):
    """
    Raised when the downstream redis library raises any exception.
    """

    pass

class MaxSleepExceededError(Exception):
    """
    Raised when we've slept for longer than the `max_sleep` specified limit.
    """

    pass
