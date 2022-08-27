from typing import Any, Optional

# class TokenBucket:
#     name: str
#     capacity: int
#     refill_frequency: float
#     refill_amount: int
#     redis_url: str
#     _id: str
#
#     async def __aenter__(self) -> None: ...
#     async def __aexit__(
#         self, exc_type: type[BaseException] | None, exc_val: BaseException | None, exc_tb: TracebackType | None
#     ) -> None: ...

class Semaphore:
    def __init__(
        self,
        name: str,
        capacity: int,
        redis_url: str,
        sleep_duration: Optional[float] = None,
        max_position: Optional[int] = None,
    ) -> None: ...

    queue_key: str
    client: Any
    identifier: str
    sleep_duration: float
    max_position: int
