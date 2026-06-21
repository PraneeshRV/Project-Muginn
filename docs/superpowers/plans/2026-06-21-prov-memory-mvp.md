# prov-memory — MASTER Implementation Plan (rev2)

> **For the executing agent:** Build this top-to-bottom. Each task is self-contained: exact files, complete final code (copy verbatim — do not improvise), the test, the command to run, and the expected output. After each task, run the command, confirm the expected output, then commit with the given message. Do **not** add "Co-Authored-By" lines to commits. Steps use checkbox (`- [ ]`) syntax.
>
> **Golden rules:** (1) Type code exactly as shown. (2) Run the test command; it must show the expected output before you commit. (3) If a test fails, fix YOUR typo — the code here is verified-consistent; do not redesign. (4) Never call any network/cloud API; this tool is strictly local.

**Goal:** A local-first, cross-agent memory tool. Every fact is a verbatim quote from a coding-agent transcript with a byte-verifiable citation to its exact source turn, plus temporal staleness so superseded facts stop surfacing. Optional Ed25519 signing for cross-trust transfer.

**Architecture:** Per-agent **adapters** parse native transcripts → canonical `Turn`s (each hashed individually). A **selector** picks salient byte-spans. The **store** byte-verifies each span, signs it, hash-chains it per session, derives a `topic_key`, marks older same-topic facts stale, and persists to SQLite+FTS5. **recall** renders markdown cards (hiding stale); **verify** re-derives the source turn and byte-compares. Safety lives in the verifier: a quote not physically present in a real transcript turn is rejected.

**Tech stack:** Python 3.11+, `cryptography` (Ed25519), stdlib `sqlite3`+FTS5, `pytest`. `fastmcp` only for the optional MCP server (Task 11). NO cloud embeddings; recall is FTS5 keyword search. Local vector search is an explicit post-MVP stretch (Task 14).

**Honest framing (read once):** For a single-user local tool the signing is *not* a security guarantee against yourself — it is there so memory can later be transferred across trust boundaries. The real day-one value is **grounded citations + staleness detection**. Do not market this as "tamper-proof." See `docs/superpowers/specs/2026-06-21-prov-memory-design.md` (rev2 note) for why.

---

## Canonical contracts (every task obeys these — do not rename)

- `Turn(agent, session_id, turn_id, role, text, native_path, turn_sha256)` — `text` is one decoded transcript turn; `turn_sha256 = sha256(text)` (per-turn, NOT per-file, so appending later turns never invalidates earlier facts).
- A **span** is `(start, end)` byte offsets into `Turn.text` UTF-8: `quote = turn.text.encode()[start:end].decode()`.
- `FactSource(agent, native_path, session_id, turn_id, span, turn_sha256)`.
- `Fact(fact_id, quote, source, content_hash, signature, pubkey, prev_fact_id, topic_key, superseded_by, stale, tags, created_at)`.
- `content_hash = sha256(canonical_json({"quote": quote, "source": source_dict}))` (hex). `source_dict` is `asdict(source)` with `span` forced to a `[start, end]` list.
- `fact_id = sha256(content_hash + pubkey_hex)` (hex).
- `signature = Ed25519_sign(priv, content_hash)` (hex).
- `prev_fact_id` = previous stored `fact_id` for the same `session_id`, else `""`.
- `topic_key` = first 4 alphanumeric tokens of the quote, lowercased, joined by `-`.
- verify statuses (exact strings): `ok`, `bad-signature`, `source-missing`, `turn-missing`, `source-modified`, `span-mismatch`.

**Directory layout produced by this plan:**
```
prov-memory/
  pyproject.toml  LICENSE  NOTICE  .gitignore  README.md
  src/provmem/{__init__,types,crypto,select,store,verify,render,ingest,cli,mcp_server}.py
  src/provmem/adapters/{__init__,base,claude_code,codex,antigravity}.py
  tests/{__init__,conftest}.py  tests/test_*.py  tests/fixtures/...
  eval/recall_eval.py  eval/fixtures/labeled.jsonl
  docs/superpowers/{specs,plans}/...   research/...   (already committed)
```

> **Starting state note:** Some `src/` and `tests/` files may already exist on disk from an earlier session. For a clean run, discard them first: `git stash -u` (reversible) or `git clean -fdx src tests pyproject.toml`. Then build strictly from this plan.

---

### Task 0: Scaffold

**Files:** Create `pyproject.toml`, `.gitignore`, `NOTICE`, `LICENSE`, `src/provmem/__init__.py`, `tests/__init__.py`, `tests/conftest.py`.

- [ ] **Step 1:** `pyproject.toml`
```toml
[project]
name = "provmem"
version = "0.1.0"
description = "Local-first cross-agent memory with byte-verifiable span provenance and staleness detection"
requires-python = ">=3.11"
dependencies = ["cryptography>=42"]

[project.optional-dependencies]
dev = ["pytest>=8"]
mcp = ["fastmcp>=2"]

[project.scripts]
provmem = "provmem.cli:main"

[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[tool.setuptools.packages.find]
where = ["src"]

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
```

- [ ] **Step 2:** `.gitignore`
```
__pycache__/
*.pyc
.venv/
*.db
*.sqlite
*.egg-info/
build/
dist/
```

- [ ] **Step 3:** `NOTICE`
```
prov-memory
Copyright 2026 Praneesh RV
Licensed under the Apache License, Version 2.0.
```

- [ ] **Step 4:** `LICENSE` — write the standard Apache License 2.0 text. Fetch it: `curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE` (offline fallback: paste the canonical Apache-2.0 text). Do not hand-edit it.

- [ ] **Step 5:** Empty `src/provmem/__init__.py` and `tests/__init__.py`. Then `tests/conftest.py`:
```python
import pathlib

FIXTURES = pathlib.Path(__file__).parent / "fixtures"
```

- [ ] **Step 6:** Install + confirm discovery.
Run: `pip install -e ".[dev]" --break-system-packages -q && pytest -q`
Expected: exit code 5, message `no tests ran`.

- [ ] **Step 7:** Commit
```bash
git add -A && git commit -m "chore: scaffold provmem package"
```

---

### Task 1: Types

**Files:** Create `src/provmem/types.py`, `tests/test_types.py`.

- [ ] **Step 1:** `src/provmem/types.py`
```python
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
```

- [ ] **Step 2:** `tests/test_types.py`
```python
from provmem.types import Fact, FactSource, Turn


def test_turn_holds_canonical_text():
    t = Turn("claude_code", "s1", "t1", "assistant", "hello world", "/x.jsonl", "sha")
    assert t.text.encode()[0:5].decode() == "hello"


def test_factsource_span_is_byte_pair():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    assert src.span == (0, 5)


def test_fact_to_dict_roundtrip():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    f = Fact("f", "hello", src, "ch", "sig", "pk", "", tags=["x"], created_at="2026")
    d = f.to_dict()
    assert d["source"]["span"] == [0, 5]
    assert d["quote"] == "hello"
    assert d["stale"] is False
```

- [ ] **Step 3:** Run `pytest tests/test_types.py -q` → Expected: `3 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: canonical Turn/FactSource/Fact types"`

---

### Task 2: Crypto

**Files:** Create `src/provmem/crypto.py`, `tests/test_crypto.py`.

- [ ] **Step 1:** `src/provmem/crypto.py`
```python
from __future__ import annotations

import hashlib
import json

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)


def canonical_json(obj) -> str:
    """Deterministic JSON: sorted keys, no whitespace. Stable for hashing."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def new_keypair() -> tuple[str, str]:
    priv = Ed25519PrivateKey.generate()
    return priv.private_bytes_raw().hex(), priv.public_key().public_bytes_raw().hex()


def sign(priv_hex: str, message: str) -> str:
    priv = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(priv_hex))
    return priv.sign(message.encode()).hex()


def verify_sig(pub_hex: str, message: str, signature_hex: str) -> bool:
    try:
        pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(pub_hex))
        pub.verify(bytes.fromhex(signature_hex), message.encode())
        return True
    except (InvalidSignature, ValueError):
        return False


def content_hash(quote: str, source_dict: dict) -> str:
    """Bind the quote to its full source citation (including turn_sha256)."""
    payload = canonical_json({"quote": quote, "source": source_dict})
    return hashlib.sha256(payload.encode()).hexdigest()


def fact_id(content_hash_hex: str, pubkey_hex: str) -> str:
    return hashlib.sha256((content_hash_hex + pubkey_hex).encode()).hexdigest()
```

- [ ] **Step 2:** `tests/test_crypto.py`
```python
from provmem.crypto import (
    canonical_json,
    content_hash,
    fact_id,
    new_keypair,
    sign,
    verify_sig,
)


def test_canonical_json_is_stable():
    assert canonical_json({"b": 1, "a": 2}) == canonical_json({"a": 2, "b": 1})


def test_sign_then_verify_roundtrip():
    priv, pub = new_keypair()
    ch = content_hash("hello", {"span": [0, 5]})
    assert verify_sig(pub, ch, sign(priv, ch)) is True


def test_verify_rejects_tampered_hash():
    priv, pub = new_keypair()
    sig = sign(priv, content_hash("hello", {"span": [0, 5]}))
    assert verify_sig(pub, content_hash("HELLO", {"span": [0, 5]}), sig) is False


def test_fact_id_changes_with_pubkey():
    ch = content_hash("hello", {"span": [0, 5]})
    assert fact_id(ch, "pkA") != fact_id(ch, "pkB")
```

- [ ] **Step 3:** Run `pytest tests/test_crypto.py -q` → Expected: `4 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: Ed25519 signing, content hashing, fact identity"`

---

### Task 3: Adapter base + Claude Code adapter (priority 1)

**Files:** Create `src/provmem/adapters/__init__.py` (empty), `src/provmem/adapters/base.py`, `src/provmem/adapters/claude_code.py`, `tests/fixtures/claude_code/sample.jsonl`.

**Background:** Claude Code stores a session as `~/.claude/projects/<slug>/<uuid>.jsonl`, one JSON object per line, each with `uuid`, `type` (`user`/`assistant`), `message.content` (string OR list of `{"type":"text","text":...}` blocks).

- [ ] **Step 1:** `src/provmem/adapters/base.py`
```python
from __future__ import annotations

from typing import Iterator, Protocol

from provmem.types import Turn


class Adapter(Protocol):
    agent: str

    def iter_turns(self, path: str) -> Iterator[Turn]:
        ...
```

- [ ] **Step 2:** `src/provmem/adapters/claude_code.py`
```python
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
```

- [ ] **Step 3:** `tests/fixtures/claude_code/sample.jsonl` (exact 3 lines)
```
{"uuid":"u1","type":"user","message":{"content":"fix the auth bug"}}
{"uuid":"a1","type":"assistant","message":{"content":[{"type":"text","text":"Decision: use Ed25519 because it is fast and small."}]}}
{"uuid":"u2","type":"user","message":{"content":"sounds good"}}
```

- [ ] **Step 4:** Tests go in a shared `tests/test_adapters.py` (created here, extended in Tasks 4–5).
```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import sha256_text
from tests.conftest import FIXTURES


def test_claude_parses_turns():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    turns = list(ClaudeCodeAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["u1", "a1", "u2"]
    assert turns[1].role == "assistant"
    assert "Ed25519" in turns[1].text
    assert turns[1].agent == "claude_code"
    assert turns[1].turn_sha256 == sha256_text(turns[1].text)


def test_claude_is_deterministic():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    a = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    b = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    assert a == b
```

- [ ] **Step 5:** Run `pytest tests/test_adapters.py -q` → Expected: `2 passed`.
- [ ] **Step 6:** Commit `git add -A && git commit -m "feat: Claude Code transcript adapter"`

---

### Task 4: Codex adapter (priority 2)

**Files:** Create `src/provmem/adapters/codex.py`, `tests/fixtures/codex/rollout.jsonl`; append tests to `tests/test_adapters.py`.

**Background:** Codex CLI rollout `.jsonl` under `~/.codex/sessions/`. Message events: `{"type":"message","role":...,"id":...,"content":[{"type":"input_text"|"output_text","text":...}]}`. Skip non-message events.

- [ ] **Step 1:** `src/provmem/adapters/codex.py`
```python
from __future__ import annotations

import json
import os
from typing import Iterator

from provmem.crypto import sha256_text
from provmem.types import Turn


def _flatten(content) -> str:
    if isinstance(content, list):
        return "".join(
            b.get("text", "")
            for b in content
            if isinstance(b, dict) and b.get("type") in ("input_text", "output_text")
        )
    if isinstance(content, str):
        return content
    return ""


class CodexAdapter:
    """Parse a Codex rollout .jsonl. Only 'message' events become Turns."""

    agent = "codex"

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
                if obj.get("type") != "message":
                    continue
                text = _flatten(obj.get("content", ""))
                if not text:
                    continue
                yield Turn(
                    agent=self.agent,
                    session_id=session_id,
                    turn_id=str(obj.get("id", "")),
                    role=obj.get("role", ""),
                    text=text,
                    native_path=path,
                    turn_sha256=sha256_text(text),
                )
```

- [ ] **Step 2:** `tests/fixtures/codex/rollout.jsonl`
```
{"type":"session_meta","id":"sess-7"}
{"type":"message","role":"user","id":"m1","content":[{"type":"input_text","text":"add validation"}]}
{"type":"message","role":"assistant","id":"m2","content":[{"type":"output_text","text":"Constraint: inputs must be non-empty. TODO: add tests."}]}
{"type":"reasoning","id":"r1"}
```

- [ ] **Step 3:** Append to `tests/test_adapters.py`
```python
from provmem.adapters.codex import CodexAdapter


def test_codex_parses_only_message_events():
    path = str(FIXTURES / "codex" / "rollout.jsonl")
    turns = list(CodexAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["m1", "m2"]
    assert "Constraint" in turns[1].text
    assert turns[0].agent == "codex"
```

- [ ] **Step 4:** Run `pytest tests/test_adapters.py -q` → Expected: `3 passed`.
- [ ] **Step 5:** Commit `git add -A && git commit -m "feat: Codex transcript adapter"`

---

### Task 5: Antigravity adapter (priority 3 — assumed format)

**Files:** Create `src/provmem/adapters/antigravity.py`, `tests/fixtures/antigravity/sample.json`; append tests.

> **Discovery step (do first):** Antigravity's real on-disk format is unconfirmed. Run `find ~ -ipath '*antigravity*' -name '*.json' 2>/dev/null | head` and inspect a real file. If its shape differs from the assumed `{"sessionId", "messages":[{"id","role","text"}]}`, update the fixture and `iter_turns` body to match — keep the `iter_turns -> Turn` interface and `agent="antigravity"` unchanged.

- [ ] **Step 1:** `src/provmem/adapters/antigravity.py`
```python
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
```

- [ ] **Step 2:** `tests/fixtures/antigravity/sample.json`
```json
{"sessionId":"ag-3","messages":[{"id":"x1","role":"user","text":"refactor module"},{"id":"x2","role":"assistant","text":"Decision: split parser into its own file."}]}
```

- [ ] **Step 3:** Append to `tests/test_adapters.py`
```python
from provmem.adapters.antigravity import AntigravityAdapter


def test_antigravity_parses_messages():
    path = str(FIXTURES / "antigravity" / "sample.json")
    turns = list(AntigravityAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["x1", "x2"]
    assert turns[1].agent == "antigravity"
    assert "Decision" in turns[1].text
```

- [ ] **Step 4:** Run `pytest tests/test_adapters.py -q` → Expected: `4 passed`.
- [ ] **Step 5:** Commit `git add -A && git commit -m "feat: Antigravity adapter (assumed format; verify on real data)"`

---

### Task 6: Selector + topic_key

**Files:** Create `src/provmem/select.py`, `tests/test_select.py`.

- [ ] **Step 1:** `src/provmem/select.py`
```python
from __future__ import annotations

import re

from provmem.types import Turn

# Salience heuristic (MVP): keep sentences that look like durable facts.
_SALIENT = re.compile(
    r"\b(decision|constraint|because|prefer|remember|TODO|FIXME)\b|\b[\w./-]+\.\w+:\d+",
    re.IGNORECASE,
)
_SENT = re.compile(r"[^.!?]*[.!?]|[^.!?]+$")


def select_spans(turn: Turn) -> list[tuple[int, int]]:
    """Return (start, end) byte offsets into ``turn.text`` UTF-8 for salient
    sentences. Offsets land on byte boundaries so slicing decodes cleanly."""
    text = turn.text
    spans: list[tuple[int, int]] = []
    for m in _SENT.finditer(text):
        raw = m.group()
        sentence = raw.strip()
        if not sentence or not _SALIENT.search(sentence):
            continue
        start_char = m.start() + (len(raw) - len(raw.lstrip()))
        end_char = start_char + len(sentence)
        start_b = len(text[:start_char].encode())
        end_b = len(text[:end_char].encode())
        spans.append((start_b, end_b))
    return spans


def topic_key(quote: str) -> str:
    """Coarse grouping key so a newer fact can supersede an older one.
    First 4 alphanumeric tokens, lowercased."""
    toks = re.findall(r"[a-z0-9]+", quote.lower())
    return "-".join(toks[:4])
```

- [ ] **Step 2:** `tests/test_select.py`
```python
from provmem.select import select_spans, topic_key
from provmem.types import Turn


def _turn(text):
    return Turn("claude_code", "s1", "t1", "assistant", text, "/x", "sha")


def test_keeps_salient_sentence_only():
    t = _turn("Hello there. Decision: use Ed25519 because it is fast. Bye.")
    quotes = [t.text.encode()[s:e].decode() for s, e in select_spans(t)]
    assert any("Decision: use Ed25519" in q for q in quotes)
    assert not any(q.strip() == "Hello there." for q in quotes)


def test_spans_are_byte_accurate_with_unicode():
    t = _turn("café note. TODO: add tests here.")
    spans = select_spans(t)
    quotes = [t.text.encode()[s:e].decode() for s, e in spans]  # must not raise
    assert any("TODO" in q for q in quotes)


def test_no_salient_returns_empty():
    assert select_spans(_turn("just chatting about nothing.")) == []


def test_topic_key_is_deterministic_first_four_tokens():
    assert topic_key("Decision: use Ed25519 because it is fast.") == "decision-use-ed25519-because"
    assert topic_key("Decision: use Ed25519 because it is slow.") == "decision-use-ed25519-because"
```

- [ ] **Step 3:** Run `pytest tests/test_select.py -q` → Expected: `4 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: heuristic span selector + topic_key"`

---

### Task 7: Store with verified write, hash chain, and staleness

**Files:** Create `src/provmem/store.py`, `tests/test_store.py`.

- [ ] **Step 1:** `src/provmem/store.py`
```python
from __future__ import annotations

import json
import sqlite3
from dataclasses import asdict
from datetime import datetime, timezone

from provmem import crypto
from provmem.select import topic_key
from provmem.types import Fact, FactSource, Turn


class SpanMismatch(Exception):
    """Raised when a requested span does not yield real quote text."""


_SCHEMA = """
CREATE TABLE IF NOT EXISTS facts (
    fact_id TEXT PRIMARY KEY,
    quote TEXT NOT NULL,
    source_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    prev_fact_id TEXT NOT NULL,
    topic_key TEXT NOT NULL,
    superseded_by TEXT NOT NULL DEFAULT '',
    stale INTEGER NOT NULL DEFAULT 0,
    session_id TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    quote, content='facts', content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
    INSERT INTO facts_fts(rowid, quote) VALUES (new.rowid, new.quote);
END;
"""


class Store:
    def __init__(self, db_path: str):
        self.db = sqlite3.connect(db_path)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(_SCHEMA)

    def _last_fact_id(self, session_id: str) -> str:
        row = self.db.execute(
            "SELECT fact_id FROM facts WHERE session_id=? ORDER BY rowid DESC LIMIT 1",
            (session_id,),
        ).fetchone()
        return row["fact_id"] if row else ""

    def _mark_superseded(self, key: str, new_fact_id: str) -> None:
        self.db.execute(
            "UPDATE facts SET stale=1, superseded_by=? "
            "WHERE topic_key=? AND stale=0 AND fact_id<>?",
            (new_fact_id, key, new_fact_id),
        )

    def store_fact(self, turn: Turn, span: tuple[int, int], priv_hex: str,
                   pub_hex: str, tags: list[str] | None = None) -> Fact:
        start, end = span
        quote = turn.text.encode()[start:end].decode(errors="strict")
        if not quote.strip():
            raise SpanMismatch("empty or whitespace quote rejected")

        src = FactSource(turn.agent, turn.native_path, turn.session_id,
                         turn.turn_id, (start, end), turn.turn_sha256)
        src_dict = asdict(src)
        src_dict["span"] = [start, end]

        ch = crypto.content_hash(quote, src_dict)
        fid = crypto.fact_id(ch, pub_hex)
        sig = crypto.sign(priv_hex, ch)
        prev = self._last_fact_id(turn.session_id)
        key = topic_key(quote)
        created = datetime.now(timezone.utc).isoformat()
        tags = tags or []

        self._mark_superseded(key, fid)
        self.db.execute(
            "INSERT OR IGNORE INTO facts VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (fid, quote, json.dumps(src_dict), ch, sig, pub_hex, prev, key,
             "", 0, turn.session_id, json.dumps(tags), created),
        )
        self.db.commit()
        return Fact(fid, quote, src, ch, sig, pub_hex, prev, key, "", False, tags, created)

    def _row_to_fact(self, row) -> Fact:
        sd = json.loads(row["source_json"])
        src = FactSource(sd["agent"], sd["native_path"], sd["session_id"],
                         sd["turn_id"], tuple(sd["span"]), sd["turn_sha256"])
        return Fact(row["fact_id"], row["quote"], src, row["content_hash"],
                    row["signature"], row["pubkey"], row["prev_fact_id"],
                    row["topic_key"], row["superseded_by"], bool(row["stale"]),
                    json.loads(row["tags_json"]), row["created_at"])

    def get(self, fact_id: str) -> Fact | None:
        row = self.db.execute("SELECT * FROM facts WHERE fact_id=?", (fact_id,)).fetchone()
        return self._row_to_fact(row) if row else None

    def search(self, query: str, k: int = 10, include_stale: bool = False) -> list[Fact]:
        clause = "" if include_stale else "AND f.stale=0 "
        rows = self.db.execute(
            "SELECT f.* FROM facts_fts ft JOIN facts f ON f.rowid=ft.rowid "
            f"WHERE facts_fts MATCH ? {clause}ORDER BY rank LIMIT ?",
            (query, k),
        ).fetchall()
        return [self._row_to_fact(r) for r in rows]
```

- [ ] **Step 2:** `tests/test_store.py`
```python
import pytest

from provmem.crypto import new_keypair, verify_sig
from provmem.store import SpanMismatch, Store
from provmem.types import Turn


def _turn(text="Decision: use Ed25519 because fast.", tid="t1", sid="s1"):
    return Turn("claude_code", sid, tid, "assistant", text, "/x.jsonl",
                __import__("provmem.crypto", fromlist=["sha256_text"]).sha256_text(text))


def test_store_and_search(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    span = (0, len(t.text.encode()))
    f = st.store_fact(t, span, priv, pub)
    assert f.quote.startswith("Decision: use Ed25519")
    assert verify_sig(pub, f.content_hash, f.signature)
    hits = st.search("Ed25519")
    assert len(hits) == 1 and hits[0].fact_id == f.fact_id


def test_hash_chain_links_same_session(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    f1 = st.store_fact(t, (0, 8), priv, pub)    # "Decision"
    f2 = st.store_fact(t, (10, 13), priv, pub)  # "use"
    assert f1.prev_fact_id == ""
    assert f2.prev_fact_id == f1.fact_id


def test_rejects_empty_quote(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    with pytest.raises(SpanMismatch):
        st.store_fact(_turn(), (5, 5), priv, pub)


def test_newer_fact_supersedes_older_same_topic(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    old = _turn("Decision: use Ed25519 because it is fast.", tid="a1", sid="s1")
    new = _turn("Decision: use Ed25519 because it is secure.", tid="b1", sid="s2")
    st.store_fact(old, (0, len(old.text.encode())), priv, pub)
    st.store_fact(new, (0, len(new.text.encode())), priv, pub)
    live = st.search("Ed25519")
    assert len(live) == 1 and "secure" in live[0].quote   # stale hidden
    assert len(st.search("Ed25519", include_stale=True)) == 2
```

- [ ] **Step 3:** Run `pytest tests/test_store.py -q` → Expected: `4 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: sqlite store with verified write, FTS5, hash chain, staleness"`

---

### Task 8: Verifier

**Files:** Create `src/provmem/verify.py`, `tests/test_verify.py`.

- [ ] **Step 1:** `src/provmem/verify.py`
```python
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
```

- [ ] **Step 2:** `tests/test_verify.py`
```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.store import Store
from provmem.verify import verify_fact
from tests.conftest import FIXTURES


def _ingest_a1(tmp_path, src_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    turns = list(ClaudeCodeAdapter().iter_turns(src_path))
    a1 = next(t for t in turns if t.turn_id == "a1")
    f = st.store_fact(a1, (0, len(a1.text.encode())), priv, pub)
    return f


def test_verify_ok(tmp_path):
    f = _ingest_a1(tmp_path, str(FIXTURES / "claude_code" / "sample.jsonl"))
    assert verify_fact(f) == "ok"


def test_verify_source_missing(tmp_path):
    f = _ingest_a1(tmp_path, str(FIXTURES / "claude_code" / "sample.jsonl"))
    object.__setattr__(f.source, "native_path", "/no/such.jsonl")
    assert verify_fact(f) == "source-missing"


def test_verify_source_modified_on_same_turn(tmp_path):
    src = tmp_path / "sess.jsonl"
    src.write_text((FIXTURES / "claude_code" / "sample.jsonl").read_text())
    f = _ingest_a1(tmp_path, str(src))
    # edit the SAME turn (a1) the fact came from
    src.write_text(src.read_text().replace("fast and small", "slow and small"))
    assert verify_fact(f) == "source-modified"
```

- [ ] **Step 3:** Run `pytest tests/test_verify.py -q` → Expected: `3 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: byte-compare verifier with typed statuses"`

---

### Task 9: Render

**Files:** Create `src/provmem/render.py`, `tests/test_render.py`.

- [ ] **Step 1:** `src/provmem/render.py`
```python
from __future__ import annotations

from provmem.types import Fact


def render_cards(facts: list[Fact]) -> str:
    """Markdown cards for context injection (the token-lean format the
    research benchmark picked). One line per fact with a clickable citation."""
    lines = []
    for f in facts:
        s = f.source
        lines.append(
            f'- "{f.quote}" — {s.agent}:{s.session_id}#{s.turn_id} [{f.fact_id[:8]}]'
        )
    return "\n".join(lines)
```

- [ ] **Step 2:** `tests/test_render.py`
```python
from provmem.render import render_cards
from provmem.types import Fact, FactSource


def _fact():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    return Fact("abcdef1234", "hello", src, "ch", "sig", "pk", "")


def test_render_card_has_quote_and_citation():
    out = render_cards([_fact()])
    assert '"hello"' in out
    assert "claude_code:s1#t1" in out
    assert "abcdef12" in out


def test_render_empty():
    assert render_cards([]) == ""
```

- [ ] **Step 3:** Run `pytest tests/test_render.py -q` → Expected: `2 passed`.
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: markdown-card render"`

---

### Task 10: Ingest pipeline + CLI

**Files:** Create `src/provmem/ingest.py`, `src/provmem/cli.py`, `tests/test_ingest.py`.

- [ ] **Step 1:** `src/provmem/ingest.py`
```python
from __future__ import annotations

from provmem.select import select_spans
from provmem.store import SpanMismatch, Store


def ingest_file(store: Store, adapter, path: str, priv_hex: str, pub_hex: str) -> int:
    """Run adapter -> selector -> verified store. Returns facts written."""
    count = 0
    for turn in adapter.iter_turns(path):
        for span in select_spans(turn):
            try:
                store.store_fact(turn, span, priv_hex, pub_hex)
                count += 1
            except SpanMismatch:
                continue
    return count
```

- [ ] **Step 2:** `src/provmem/cli.py`
```python
from __future__ import annotations

import argparse
import os

from provmem.adapters.antigravity import AntigravityAdapter
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.adapters.codex import CodexAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.render import render_cards
from provmem.store import Store
from provmem.verify import verify_fact

_ADAPTERS = {
    "claude_code": ClaudeCodeAdapter,
    "codex": CodexAdapter,
    "antigravity": AntigravityAdapter,
}

_KEY_PATH = os.path.expanduser("~/.provmem.key")
_DB_PATH = os.path.expanduser("~/.provmem.db")


def _load_or_make_key() -> tuple[str, str]:
    if os.path.exists(_KEY_PATH):
        priv = open(_KEY_PATH).read().strip()
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        pub = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(priv)).public_key().public_bytes_raw().hex()
        return priv, pub
    priv, pub = new_keypair()
    with open(_KEY_PATH, "w") as fh:
        fh.write(priv)
    os.chmod(_KEY_PATH, 0o600)
    return priv, pub


def main(argv=None) -> int:
    p = argparse.ArgumentParser(prog="provmem")
    sub = p.add_subparsers(dest="cmd", required=True)
    pi = sub.add_parser("ingest", help="ingest a transcript file")
    pi.add_argument("agent", choices=list(_ADAPTERS))
    pi.add_argument("path")
    pr = sub.add_parser("recall", help="search memory")
    pr.add_argument("query")
    pr.add_argument("-k", type=int, default=10)
    args = p.parse_args(argv)

    store = Store(_DB_PATH)
    priv, pub = _load_or_make_key()

    if args.cmd == "ingest":
        n = ingest_file(store, _ADAPTERS[args.agent](), args.path, priv, pub)
        print(f"ingested {n} facts from {args.path}")
        return 0
    if args.cmd == "recall":
        facts = store.search(args.query, args.k)
        print(render_cards(facts) or "(no matches)")
        for f in facts:
            print(f"  verify[{f.fact_id[:8]}] = {verify_fact(f)}")
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3:** `tests/test_ingest.py`
```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.store import Store
from tests.conftest import FIXTURES


def test_ingest_then_search(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    n = ingest_file(st, ClaudeCodeAdapter(),
                    str(FIXTURES / "claude_code" / "sample.jsonl"), priv, pub)
    assert n >= 1
    assert any("Ed25519" in f.quote for f in st.search("Ed25519"))
```

- [ ] **Step 4:** Run `pytest tests/test_ingest.py -q` → Expected: `1 passed`.
- [ ] **Step 5:** Smoke-test the CLI:
Run: `python -m provmem.cli ingest claude_code tests/fixtures/claude_code/sample.jsonl`
Expected: a line like `ingested 1 facts from ...`. (Writes `~/.provmem.db` + key — fine.)
- [ ] **Step 6:** Commit `git add -A && git commit -m "feat: ingest pipeline + provmem CLI"`

---

### Task 11: MCP server (optional dependency)

**Files:** Create `src/provmem/mcp_server.py`, `tests/test_mcp_handlers.py`.

- [ ] **Step 1:** `src/provmem/mcp_server.py`
```python
from __future__ import annotations

import json

from provmem.render import render_cards
from provmem.store import Store
from provmem.verify import verify_fact


def make_handlers(store: Store) -> dict:
    """Plain handler functions (testable without any MCP transport)."""

    def recall(query: str, k: int = 10) -> str:
        return render_cards(store.search(query, k))

    def verify(fact_id: str) -> str:
        f = store.get(fact_id)
        return "not-found" if f is None else verify_fact(f)

    def cite(fact_id: str) -> str:
        f = store.get(fact_id)
        return "{}" if f is None else json.dumps(f.to_dict()["source"])

    return {"recall": recall, "verify": verify, "cite": cite}


def build_server(db_path: str):
    from fastmcp import FastMCP

    store = Store(db_path)
    h = make_handlers(store)
    mcp = FastMCP("prov-memory")
    mcp.tool(name="recall")(h["recall"])
    mcp.tool(name="verify")(h["verify"])
    mcp.tool(name="cite")(h["cite"])
    return mcp


if __name__ == "__main__":
    import os

    build_server(os.environ.get("PROVMEM_DB", os.path.expanduser("~/.provmem.db"))).run()
```

- [ ] **Step 2:** `tests/test_mcp_handlers.py`
```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.mcp_server import make_handlers
from provmem.store import Store
from tests.conftest import FIXTURES


def test_handlers_recall_verify_cite(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    ingest_file(st, ClaudeCodeAdapter(),
                str(FIXTURES / "claude_code" / "sample.jsonl"), priv, pub)
    h = make_handlers(st)
    assert "Ed25519" in h["recall"]("Ed25519", 5)
    fid = st.search("Ed25519")[0].fact_id
    assert h["verify"](fid) == "ok"
    assert "claude_code" in h["cite"](fid)
    assert h["recall"]("zzzznotpresent", 5) == ""
    assert h["verify"]("deadbeef") == "not-found"
```

- [ ] **Step 3:** Run `pytest tests/test_mcp_handlers.py -q` → Expected: `1 passed`. (Handlers don't import fastmcp; `build_server` does, lazily.)
- [ ] **Step 4:** Commit `git add -A && git commit -m "feat: MCP recall/verify/cite handlers + server"`

---

### Task 12: End-to-end demo — the headline test

**Files:** Create `tests/test_e2e_demo.py`.

> This proves the rev2 thesis precisely: tampering the SAME turn a fact came from is detected, while tampering a DIFFERENT turn does NOT raise a false positive (the bug the per-turn hash fixed).

- [ ] **Step 1:** `tests/test_e2e_demo.py`
```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.store import Store
from provmem.verify import verify_fact
from tests.conftest import FIXTURES


def test_tamper_same_turn_detected_other_turn_ignored(tmp_path):
    priv, pub = new_keypair()
    src = tmp_path / "session.jsonl"
    src.write_text((FIXTURES / "claude_code" / "sample.jsonl").read_text())
    st = Store(str(tmp_path / "m.db"))

    assert ingest_file(st, ClaudeCodeAdapter(), str(src), priv, pub) >= 1
    facts = st.search("Ed25519")
    assert facts and all(verify_fact(f) == "ok" for f in facts)

    # tamper a DIFFERENT turn (u2 "sounds good") -> facts stay ok (no false positive)
    src.write_text(src.read_text().replace("sounds good", "sounds great"))
    assert all(verify_fact(f) == "ok" for f in facts)

    # tamper the SAME turn (a1) the facts came from -> detected
    src.write_text(src.read_text().replace("fast and small", "slow and small"))
    assert any(verify_fact(f) == "source-modified" for f in facts)
```

- [ ] **Step 2:** Run `pytest tests/test_e2e_demo.py -q` → Expected: `1 passed`.
- [ ] **Step 3:** Commit `git add -A && git commit -m "test: end-to-end tamper-detection demo"`

---

### Task 13: Eval harness (the measurable claim)

**Files:** Create `eval/fixtures/labeled.jsonl`, `eval/recall_eval.py`, `tests/test_eval.py`.

**Purpose:** Measure selector quality: of sentences labeled salient, how many does the selector capture (recall), and how many non-salient leak through (false-positive rate). This is the seed of the LongMemEval/LoCoMo comparison if the project later goes for a paper.

- [ ] **Step 1:** `eval/fixtures/labeled.jsonl` — each line: a sentence + whether it is salient.
```
{"text":"Decision: adopt sqlite-vec for local vectors.","salient":true}
{"text":"The weather was nice today.","salient":false}
{"text":"Constraint: must run fully offline.","salient":true}
{"text":"I had coffee this morning.","salient":false}
{"text":"TODO: write the README.","salient":true}
{"text":"We chatted about the weekend.","salient":false}
{"text":"Prefer markdown cards because they are token-lean.","salient":true}
{"text":"See parser.py:42 for the bug.","salient":true}
```

- [ ] **Step 2:** `eval/recall_eval.py`
```python
"""Selector quality eval. Run: python eval/recall_eval.py

Reports recall (salient captured) and false-positive rate (non-salient kept)
for the heuristic selector against a labeled fixture."""
from __future__ import annotations

import json
import os

from provmem.select import select_spans
from provmem.types import Turn

_FIX = os.path.join(os.path.dirname(__file__), "fixtures", "labeled.jsonl")


def _selects_any(text: str) -> bool:
    t = Turn("eval", "s", "t", "assistant", text, "/x", "sha")
    return len(select_spans(t)) > 0


def evaluate(path: str = _FIX) -> dict:
    rows = [json.loads(l) for l in open(path) if l.strip()]
    salient = [r for r in rows if r["salient"]]
    nonsalient = [r for r in rows if not r["salient"]]
    tp = sum(_selects_any(r["text"]) for r in salient)
    fp = sum(_selects_any(r["text"]) for r in nonsalient)
    return {
        "recall": tp / len(salient) if salient else 0.0,
        "false_positive_rate": fp / len(nonsalient) if nonsalient else 0.0,
        "n_salient": len(salient),
        "n_nonsalient": len(nonsalient),
    }


if __name__ == "__main__":
    m = evaluate()
    print(f"recall={m['recall']:.2f}  fp_rate={m['false_positive_rate']:.2f}  "
          f"(salient={m['n_salient']}, nonsalient={m['n_nonsalient']})")
```

- [ ] **Step 3:** `tests/test_eval.py`
```python
from eval.recall_eval import evaluate


def test_selector_meets_quality_bar():
    m = evaluate()
    assert m["recall"] >= 0.8          # catch most salient facts
    assert m["false_positive_rate"] <= 0.25  # leak few non-facts
```

> If `from eval.recall_eval import evaluate` fails to import, add an empty `eval/__init__.py`. Keep `eval/` out of the package (it is a dev tool), so do not list it under `[tool.setuptools.packages.find]`.

- [ ] **Step 4:** Run `python eval/recall_eval.py` then `pytest tests/test_eval.py -q`
Expected: a metrics line (recall ~1.00, fp_rate ~0.00 for this fixture) and `1 passed`.
- [ ] **Step 5:** Commit `git add -A && git commit -m "feat: selector eval harness"`

---

### Task 14 (stretch, post-MVP): local vector recall via sqlite-vec

> Do this only after Tasks 0–13 are green. Keep all tests offline.

- [ ] Create `src/provmem/embed.py` with an `Embedder` Protocol, a `FakeEmbedder` (deterministic hash-based vector, for tests), and an `OnnxEmbedder` that loads a local `nomic-embed-text` ONNX model and raises a clear error if the model file is absent (NEVER call a network API).
- [ ] Add `embedding BLOB` to the `facts` table and a `vec0` virtual table; add `Store.vsearch(query_vec, k)` using sqlite-vec cosine. Guard the `import sqlite_vec` with try/except → document FTS5 fallback in `search`.
- [ ] `tests/test_embed.py` using `FakeEmbedder`: assert nearest-neighbour ordering on toy vectors. No model download, no network.
- [ ] Run full suite, then commit `feat: optional local vector recall (sqlite-vec) with FTS5 fallback`.

---

### Task 15: README + integration docs + tag

**Files:** Create `README.md`. Then verify + tag.

- [ ] **Step 1:** `README.md` covering:
  - What it is (grounded, self-checking cross-agent memory — honest framing, NOT "tamper-proof security").
  - Quickstart: `pip install -e .`; `provmem ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl`; `provmem recall "Ed25519"`.
  - How verification works (re-open source turn, byte-compare; statuses).
  - Staleness (newer fact on same topic hides older; `--include-stale` via API).
  - MCP integration: how to register `python -m provmem.mcp_server` as an MCP server in Claude Code / Codex (`recall`, `verify`, `cite` tools), with a sample config block.
  - Research artifacts: link `research/AUDIT.md` and `research/format-benchmark/RESULTS.md`.
  - License: Apache-2.0.

- [ ] **Step 2:** Full suite green:
Run: `pytest -q`
Expected: **`26 passed`** (3+4+2+1+1+4+4+3+2+1+1+1 from Tasks 1–13). If the count differs, a task was skipped — go back.

- [ ] **Step 3:** Commit + tag
```bash
git add -A && git commit -m "docs: README and integration guide"
git tag v0.1.0
```

---

## Final self-review (verify before declaring done)

- **Spec coverage:** §3 fact schema → Tasks 1,2,7. byte-compare verify + statuses → Task 8. no-hallucination (reject quote absent from turn) → Task 7 `SpanMismatch` + Task 12. per-turn hash fix → Tasks 1,3,8,12. staleness/supersession → Tasks 6,7. signing (cross-trust-ready) → Tasks 2,7,8. adapters CC→Codex→Antigravity → Tasks 3,4,5. heuristic selector → Task 6. sqlite FTS5 → Task 7; sqlite-vec local → Task 14 stretch. markdown cards → Task 9. MCP recall/verify/cite → Task 11. eval/measurable claim → Task 13. CLI usability → Task 10. ✓
- **No placeholders:** every code step is complete final code. Task 5 discovery + Task 14 stretch are explicitly flagged. ✓
- **Type/name consistency:** `turn_sha256` used in Turn/FactSource/adapters/verify; `topic_key` defined in select, used in store; `store_fact(turn, span, priv, pub)`, `verify_fact(fact)`, `make_handlers(store)` keys `recall/verify/cite`, `ingest_file(store, adapter, path, priv, pub)` — all consistent across tasks. ✓
- **Byte-span subtlety:** spans are UTF-8 byte offsets into `Turn.text`; selector emits boundary-aligned offsets (slice on char boundary then measure encoded prefix). Covered by `test_spans_are_byte_accurate_with_unicode`. ✓
- **Known intentional behavior:** tampering a turn OTHER than a fact's source does NOT flag that fact (Task 12) — this is correct, it is the whole point of the per-turn-hash fix. ✓
