from __future__ import annotations

from typing import Iterator, Protocol

from provmem.types import Turn


class Adapter(Protocol):
    agent: str

    def iter_turns(self, path: str) -> Iterator[Turn]:
        ...
