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
        max_sleep: Optional[float] = None,  # will be set to 0 if None
    ) -> None: ...

    name: str
    capacity: int
    refill_frequency: float
    refill_amount: int
    redis_url: str

    async def __aenter__(self) -> None: ...
    async def __aexit__(
        self, exc_type: type[BaseException] | None, exc_val: BaseException | None, exc_tb: TracebackType | None
    ) -> None: ...

class Semaphore:
    def __init__(
        self,
        name: str,
        capacity: int,
        redis_url: Optional[str] = None,  # will be set as "redis://127.0.0.1:6379" if None
        sleep_duration: Optional[float] = None,  # will be set as 0.1 if None
        max_position: Optional[int] = None,  # will be set to 0 if None
    ) -> None: ...

    capacity: int
    id: bytes
    queue_key: bytes
    sleep_duration: float
    max_position: int

    async def __aenter__(self) -> None: ...
    async def __aexit__(
        self, exc_type: type[BaseException] | None, exc_val: BaseException | None, exc_tb: TracebackType | None
    ) -> None: ...

__all__: list[str]

class RedisError(Exception):
    pass
