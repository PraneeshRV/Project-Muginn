from __future__ import annotations

import json
import os
from typing import Iterator

from provmem.crypto import sha256_text
from provmem.types import Turn


def _flatten(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(
            b.get("text", "")
            for b in content
            if isinstance(b, dict) and b.get("type") == "text"
        )
    return ""


class ClaudeCodeAdapter:
    """Deterministic parse of a Claude Code session .jsonl. Hashes per-turn."""

    agent = "claude_code"

    def iter_turns(self, path: str) -> Iterator[Turn]:
        session_id = os.path.splitext(os.path.basename(path))[0]
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                text = _flatten(obj.get("message", {}).get("content", ""))
                if not text:
                    continue
                yield Turn(
                    agent=self.agent,
                    session_id=session_id,
                    turn_id=str(obj.get("uuid", "")),
                    role=obj.get("type", ""),
                    text=text,
                    native_path=path,
                    turn_sha256=sha256_text(text),
                )
