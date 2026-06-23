from __future__ import annotations

from dataclasses import asdict, dataclass, field


@dataclass(frozen=True)
class Turn:
    """One canonical, decoded transcript turn. ``turn_sha256`` hashes THIS turn's
    text (not the whole file), so appending later turns never invalidates a fact."""

    agent: str
    session_id: str
    turn_id: str
    role: str
    text: str
    native_path: str
    turn_sha256: str


@dataclass(frozen=True)
class FactSource:
    """A re-checkable citation. ``span`` is (start, end) byte offsets into the
    source turn's UTF-8 text: ``quote == turn.text.encode()[start:end].decode()``."""

    agent: str
    native_path: str
    session_id: str
    turn_id: str
    span: tuple[int, int]
    turn_sha256: str


@dataclass
class Fact:
    fact_id: str
    quote: str
    source: FactSource
    content_hash: str
    signature: str
    pubkey: str
    prev_fact_id: str
    topic_key: str = ""
    superseded_by: str = ""
    stale: bool = False
    tags: list[str] = field(default_factory=list)
    created_at: str = ""

    def to_dict(self) -> dict:
        d = asdict(self)
        d["source"]["span"] = list(self.source.span)
        return d
