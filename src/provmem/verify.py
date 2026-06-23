from __future__ import annotations

import os
from typing import Callable

from provmem.adapters.antigravity import AntigravityAdapter
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.adapters.codex import CodexAdapter
from provmem.crypto import verify_sig
from provmem.types import Fact

_ADAPTERS = {
    "claude_code": ClaudeCodeAdapter(),
    "codex": CodexAdapter(),
    "antigravity": AntigravityAdapter(),
}


def default_adapter_for(agent: str):
    return _ADAPTERS[agent]


def verify_fact(fact: Fact, adapter_for: Callable[[str], object] = default_adapter_for) -> str:
    """Re-derive the source turn and byte-compare. Returns one status string."""
    src = fact.source
    if not verify_sig(fact.pubkey, fact.content_hash, fact.signature):
        return "bad-signature"
    if not os.path.exists(src.native_path):
        return "source-missing"
    adapter = adapter_for(src.agent)
    turn = next((t for t in adapter.iter_turns(src.native_path)
                 if t.turn_id == src.turn_id), None)
    if turn is None:
        return "turn-missing"
    if turn.turn_sha256 != src.turn_sha256:
        return "source-modified"
    start, end = src.span
    actual = turn.text.encode()[start:end].decode(errors="replace")
    return "ok" if actual == fact.quote else "span-mismatch"
