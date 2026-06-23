from __future__ import annotations

import json
from typing import Iterator

from provmem.crypto import sha256_text
from provmem.types import Turn

# Assumed shape: {"sessionId": str, "messages": [{"id","role","text"}]}.
# Verify against a real Antigravity chat file; adjust the loop if needed.


class AntigravityAdapter:
    agent = "antigravity"

    def iter_turns(self, path: str) -> Iterator[Turn]:
        with open(path, "r", encoding="utf-8") as fh:
            doc = json.load(fh)
        session_id = str(doc.get("sessionId", ""))
        for m in doc.get("messages", []):
            text = m.get("text", "")
            if not text:
                continue
            yield Turn(
                agent=self.agent,
                session_id=session_id,
                turn_id=str(m.get("id", "")),
                role=m.get("role", ""),
                text=text,
                native_path=path,
                turn_sha256=sha256_text(text),
            )
