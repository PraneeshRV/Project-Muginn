# prov-memory MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **NOTE for THIS environment:** spawned subagents have Read/Bash/Write denied here — subagent-driven execution will fail. Use inline (executing-plans) instead.

**Goal:** Local-first cross-agent memory where every fact is a signed verbatim quote with a byte-verifiable citation to its source transcript.

**Architecture:** Deterministic per-agent adapters parse native transcripts → canonical `Turn`s with byte offsets. A heuristic selector picks candidate byte-spans. The store byte-verifies each span against its source, Ed25519-signs it, hash-chains it per source, and persists to SQLite+FTS5. Recall renders markdown cards; `verify` re-derives the source span and byte-compares. Safety lives in the verifier: a fact whose quote is not physically present in a real signed transcript is rejected.

**Tech Stack:** Python 3.11+, `cryptography` (Ed25519), stdlib `sqlite3` + FTS5, `pytest`, FastMCP (MCP server). sqlite-vec local embeddings = optional stretch (Task 12); MVP recall is FTS5.

**Spec:** `docs/superpowers/specs/2026-06-21-prov-memory-design.md`

**Build order rationale:** types → crypto (pure) → adapters (CC→Codex→Antigravity priority) → selector → store → verifier → render → MCP → e2e demo. Each task leaves the tree green.

**Canonical identifiers (used by every task — do not rename):**
- `Turn(agent, session_id, turn_id, role, text, native_path, native_sha256)` — `text` is the canonical decoded UTF-8 of one transcript turn.
- A **span** is a `(start, end)` pair of **byte offsets into `Turn.text` encoded UTF-8**. `quote = turn.text.encode()[start:end].decode()`.
- `FactSource(agent, native_path, native_sha256, session_id, turn_id, span)`.
- `Fact(fact_id, quote, source, content_hash, signature, pubkey, prev_fact_id, tags, created_at)`.
- `content_hash = sha256(canonical_json({"quote":quote, "source":source_dict}))` (hex).
- `fact_id = sha256(content_hash + pubkey_hex)` (hex).
- `signature = Ed25519_sign(content_hash.encode())` (hex).
- `prev_fact_id` = `fact_id` of the previous stored fact for the same `session_id`, else `""` (genesis).

---

### Task 0: Project scaffold

**Files:**
- Create: `pyproject.toml`
- Create: `src/provmem/__init__.py`
- Create: `tests/__init__.py`
- Create: `tests/conftest.py`

- [ ] **Step 1: Write `pyproject.toml`**

```toml
[project]
name = "provmem"
version = "0.0.1"
description = "Local-first cross-agent memory with byte-verifiable span provenance"
requires-python = ">=3.11"
dependencies = ["cryptography>=42", "fastmcp>=2"]

[project.optional-dependencies]
dev = ["pytest>=8"]

[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[tool.setuptools.packages.find]
where = ["src"]

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
```

- [ ] **Step 2: Create empty packages**

`src/provmem/__init__.py` and `tests/__init__.py` empty. `tests/conftest.py`:

```python
import pathlib
FIXTURES = pathlib.Path(__file__).parent / "fixtures"
```

- [ ] **Step 3: Install dev deps and verify pytest runs**

Run: `pip install -e ".[dev]" --break-system-packages`
Then: `pytest -q`
Expected: `no tests ran` (exit 5) — confirms discovery works.

- [ ] **Step 4: Commit**

```bash
git add pyproject.toml src tests
git commit -m "chore: project scaffold"
```

---

### Task 1: Canonical data types

**Files:**
- Create: `src/provmem/types.py`
- Test: `tests/test_types.py`

- [ ] **Step 1: Write the failing test**

```python
from provmem.types import Turn, FactSource, Fact

def test_turn_holds_canonical_text():
    t = Turn(agent="claude_code", session_id="s1", turn_id="t1",
             role="assistant", text="hello world",
             native_path="/x.jsonl", native_sha256="abc")
    assert t.text.encode()[0:5].decode() == "hello"

def test_factsource_span_is_byte_pair():
    src = FactSource(agent="claude_code", native_path="/x.jsonl",
                     native_sha256="abc", session_id="s1",
                     turn_id="t1", span=(0, 5))
    assert src.span == (0, 5)

def test_fact_to_dict_roundtrip():
    src = FactSource("claude_code", "/x.jsonl", "abc", "s1", "t1", (0, 5))
    f = Fact(fact_id="f", quote="hello", source=src, content_hash="ch",
             signature="sig", pubkey="pk", prev_fact_id="", tags=["x"],
             created_at="2026-01-01T00:00:00Z")
    d = f.to_dict()
    assert d["source"]["span"] == [0, 5]
    assert d["quote"] == "hello"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_types.py -v`
Expected: FAIL — `ModuleNotFoundError: provmem.types`.

- [ ] **Step 3: Write minimal implementation**

```python
from __future__ import annotations
from dataclasses import dataclass, field, asdict

@dataclass(frozen=True)
class Turn:
    agent: str
    session_id: str
    turn_id: str
    role: str
    text: str
    native_path: str
    native_sha256: str

@dataclass(frozen=True)
class FactSource:
    agent: str
    native_path: str
    native_sha256: str
    session_id: str
    turn_id: str
    span: tuple[int, int]

@dataclass(frozen=True)
class Fact:
    fact_id: str
    quote: str
    source: FactSource
    content_hash: str
    signature: str
    pubkey: str
    prev_fact_id: str
    tags: list[str] = field(default_factory=list)
    created_at: str = ""

    def to_dict(self) -> dict:
        d = asdict(self)
        d["source"]["span"] = list(self.source.span)
        return d
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_types.py -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/types.py tests/test_types.py
git commit -m "feat: canonical Turn/FactSource/Fact types"
```

---

### Task 2: Crypto — hashing, signing, fact identity

**Files:**
- Create: `src/provmem/crypto.py`
- Test: `tests/test_crypto.py`

- [ ] **Step 1: Write the failing test**

```python
from provmem.crypto import (
    new_keypair, sign, verify_sig, content_hash, fact_id, canonical_json,
)

def test_canonical_json_is_stable():
    a = canonical_json({"b": 1, "a": 2})
    b = canonical_json({"a": 2, "b": 1})
    assert a == b  # key order independent

def test_sign_then_verify_roundtrip():
    priv, pub = new_keypair()
    ch = content_hash("hello", {"span": [0, 5]})
    sig = sign(priv, ch)
    assert verify_sig(pub, ch, sig) is True

def test_verify_rejects_tampered_hash():
    priv, pub = new_keypair()
    sig = sign(priv, content_hash("hello", {"span": [0, 5]}))
    assert verify_sig(pub, content_hash("HELLO", {"span": [0, 5]}), sig) is False

def test_fact_id_changes_with_pubkey():
    ch = content_hash("hello", {"span": [0, 5]})
    assert fact_id(ch, "pkA") != fact_id(ch, "pkB")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_crypto.py -v`
Expected: FAIL — `ModuleNotFoundError: provmem.crypto`.

- [ ] **Step 3: Write minimal implementation**

```python
from __future__ import annotations
import hashlib, json
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey, Ed25519PublicKey,
)
from cryptography.exceptions import InvalidSignature

def canonical_json(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))

def new_keypair() -> tuple[str, str]:
    priv = Ed25519PrivateKey.generate()
    priv_hex = priv.private_bytes_raw().hex()
    pub_hex = priv.public_key().public_bytes_raw().hex()
    return priv_hex, pub_hex

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
    payload = canonical_json({"quote": quote, "source": source_dict})
    return hashlib.sha256(payload.encode()).hexdigest()

def fact_id(content_hash_hex: str, pubkey_hex: str) -> str:
    return hashlib.sha256((content_hash_hex + pubkey_hex).encode()).hexdigest()

def file_sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_crypto.py -v`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/crypto.py tests/test_crypto.py
git commit -m "feat: Ed25519 signing, content hashing, fact identity"
```

---

### Task 3: Claude Code adapter (priority 1)

**Files:**
- Create: `src/provmem/adapters/__init__.py`
- Create: `src/provmem/adapters/base.py`
- Create: `src/provmem/adapters/claude_code.py`
- Create: `tests/fixtures/claude_code/sample.jsonl`
- Test: `tests/test_adapter_claude.py`

**Background:** Claude Code stores one session as a `.jsonl` file under `~/.claude/projects/<slug>/<uuid>.jsonl`. Each line is a JSON object with at least `uuid`, `type` (`user`/`assistant`), and `message.content` (string, or a list of content blocks with `{"type":"text","text":...}`). The adapter must be **deterministic**: same file bytes → same `Turn`s, so verification can re-derive `Turn.text`.

- [ ] **Step 1: Create the fixture** `tests/fixtures/claude_code/sample.jsonl`

```
{"uuid":"u1","type":"user","message":{"content":"fix the auth bug"}}
{"uuid":"a1","type":"assistant","message":{"content":[{"type":"text","text":"Decision: use Ed25519 because it is fast and small."}]}}
{"uuid":"u2","type":"user","message":{"content":"sounds good"}}
```

- [ ] **Step 2: Write the failing test**

```python
from provmem.adapters.claude_code import ClaudeCodeAdapter
from tests.conftest import FIXTURES

def test_parses_turns_with_ids_and_text():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    turns = list(ClaudeCodeAdapter().iter_turns(path))
    assert len(turns) == 3
    assert turns[0].turn_id == "u1"
    assert turns[1].role == "assistant"
    assert "Ed25519" in turns[1].text
    assert turns[0].agent == "claude_code"
    assert turns[0].session_id  # non-empty

def test_is_deterministic():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    a = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    b = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    assert a == b
```

- [ ] **Step 3: Run test to verify it fails**

Run: `pytest tests/test_adapter_claude.py -v`
Expected: FAIL — module missing.

- [ ] **Step 4: Write the base interface** `src/provmem/adapters/base.py`

```python
from __future__ import annotations
from typing import Iterator, Protocol
from provmem.types import Turn

class Adapter(Protocol):
    agent: str
    def iter_turns(self, path: str) -> Iterator[Turn]: ...
```

`src/provmem/adapters/__init__.py` empty.

- [ ] **Step 5: Write the Claude Code adapter** `src/provmem/adapters/claude_code.py`

```python
from __future__ import annotations
import json, os
from typing import Iterator
from provmem.types import Turn
from provmem.crypto import file_sha256

def _flatten(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = [b.get("text", "") for b in content
                 if isinstance(b, dict) and b.get("type") == "text"]
        return "".join(parts)
    return ""

class ClaudeCodeAdapter:
    agent = "claude_code"

    def iter_turns(self, path: str) -> Iterator[Turn]:
        session_id = os.path.splitext(os.path.basename(path))[0]
        sha = file_sha256(path)
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
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
                    native_sha256=sha,
                )
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pytest tests/test_adapter_claude.py -v`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add src/provmem/adapters tests/test_adapter_claude.py tests/fixtures/claude_code
git commit -m "feat: Claude Code transcript adapter"
```

---

### Task 4: Codex adapter (priority 2)

**Files:**
- Create: `src/provmem/adapters/codex.py`
- Create: `tests/fixtures/codex/rollout.jsonl`
- Test: `tests/test_adapter_codex.py`

**Background:** Codex CLI stores sessions as rollout `.jsonl` under `~/.codex/sessions/`. Lines are events; message events look like `{"type":"message","role":"assistant","id":"m1","content":[{"type":"output_text","text":"..."}]}` (user messages use `input_text`). Non-message events are skipped. Adapter must be deterministic.

- [ ] **Step 1: Create the fixture** `tests/fixtures/codex/rollout.jsonl`

```
{"type":"session_meta","id":"sess-7"}
{"type":"message","role":"user","id":"m1","content":[{"type":"input_text","text":"add validation"}]}
{"type":"message","role":"assistant","id":"m2","content":[{"type":"output_text","text":"Constraint: inputs must be non-empty. TODO: add tests."}]}
{"type":"reasoning","id":"r1"}
```

- [ ] **Step 2: Write the failing test**

```python
from provmem.adapters.codex import CodexAdapter
from tests.conftest import FIXTURES

def test_parses_only_message_events():
    path = str(FIXTURES / "codex" / "rollout.jsonl")
    turns = list(CodexAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["m1", "m2"]
    assert turns[1].role == "assistant"
    assert "Constraint" in turns[1].text
    assert turns[0].agent == "codex"
```

- [ ] **Step 3: Run test to verify it fails**

Run: `pytest tests/test_adapter_codex.py -v`
Expected: FAIL — module missing.

- [ ] **Step 4: Write the Codex adapter** `src/provmem/adapters/codex.py`

```python
from __future__ import annotations
import json, os
from typing import Iterator
from provmem.types import Turn
from provmem.crypto import file_sha256

def _flatten(content) -> str:
    if isinstance(content, list):
        return "".join(b.get("text", "") for b in content
                       if isinstance(b, dict) and b.get("type") in ("input_text", "output_text"))
    if isinstance(content, str):
        return content
    return ""

class CodexAdapter:
    agent = "codex"

    def iter_turns(self, path: str) -> Iterator[Turn]:
        session_id = os.path.splitext(os.path.basename(path))[0]
        sha = file_sha256(path)
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
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
                    native_sha256=sha,
                )
```

- [ ] **Step 5: Run test to verify it passes**

Run: `pytest tests/test_adapter_codex.py -v`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/provmem/adapters/codex.py tests/test_adapter_codex.py tests/fixtures/codex
git commit -m "feat: Codex transcript adapter"
```

---

### Task 5: Antigravity adapter (priority 3 — stretch)

**Files:**
- Create: `src/provmem/adapters/antigravity.py`
- Create: `tests/fixtures/antigravity/sample.json`
- Test: `tests/test_adapter_antigravity.py`

**Background:** Antigravity's on-disk chat format is not yet confirmed. **Step 1 is a discovery step.** If the real format differs, update the fixture + `_load` to match, keeping the `iter_turns -> Turn` interface identical. The plan below assumes a JSON file containing `{"sessionId":..., "messages":[{"id":..,"role":..,"text":..}]}`.

- [ ] **Step 1: Discover the real format (manual)**

Run: `ls ~/.antigravity 2>/dev/null; ls ~/.config/antigravity 2>/dev/null; find ~ -ipath '*antigravity*' -name '*.json' 2>/dev/null | head`
Inspect one file. If structure differs from the assumed shape below, adjust the fixture and `_load()` accordingly. Note findings in a comment at the top of `antigravity.py`.

- [ ] **Step 2: Create the fixture** `tests/fixtures/antigravity/sample.json`

```json
{"sessionId":"ag-3","messages":[{"id":"x1","role":"user","text":"refactor module"},{"id":"x2","role":"assistant","text":"Decision: split parser into its own file."}]}
```

- [ ] **Step 3: Write the failing test**

```python
from provmem.adapters.antigravity import AntigravityAdapter
from tests.conftest import FIXTURES

def test_parses_messages():
    path = str(FIXTURES / "antigravity" / "sample.json")
    turns = list(AntigravityAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["x1", "x2"]
    assert turns[1].agent == "antigravity"
    assert "Decision" in turns[1].text
```

- [ ] **Step 4: Run test to verify it fails**

Run: `pytest tests/test_adapter_antigravity.py -v`
Expected: FAIL — module missing.

- [ ] **Step 5: Write the adapter** `src/provmem/adapters/antigravity.py`

```python
from __future__ import annotations
import json
from typing import Iterator
from provmem.types import Turn
from provmem.crypto import file_sha256

# NOTE: on-disk format assumed {"sessionId":..,"messages":[{"id","role","text"}]}.
# Confirm against a real Antigravity chat file and adjust _load if needed.
class AntigravityAdapter:
    agent = "antigravity"

    def iter_turns(self, path: str) -> Iterator[Turn]:
        sha = file_sha256(path)
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
                native_sha256=sha,
            )
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pytest tests/test_adapter_antigravity.py -v`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/provmem/adapters/antigravity.py tests/test_adapter_antigravity.py tests/fixtures/antigravity
git commit -m "feat: Antigravity transcript adapter (assumed format, verify on real data)"
```

---

### Task 6: Heuristic selector

**Files:**
- Create: `src/provmem/select.py`
- Test: `tests/test_select.py`

**Behavior:** Given a `Turn`, return a list of `(start, end)` byte spans (into `turn.text` UTF-8) for salient sentences. MVP heuristic: split text into sentences; keep a sentence if it matches a salience pattern (`decision`, `constraint`, `because`, `TODO`, `FIXME`, `prefer`, `remember`, or a `file:line` like `foo.py:42`). Spans are byte offsets so they survive multibyte text.

- [ ] **Step 1: Write the failing test**

```python
from provmem.types import Turn
from provmem.select import select_spans

def _turn(text):
    return Turn("claude_code", "s1", "t1", "assistant", text, "/x", "sha")

def test_keeps_salient_sentence_only():
    t = _turn("Hello there. Decision: use Ed25519 because it is fast. Bye.")
    spans = select_spans(t)
    quotes = [t.text.encode()[s:e].decode() for s, e in spans]
    assert any("Decision: use Ed25519" in q for q in quotes)
    assert not any(q.strip() == "Hello there." for q in quotes)

def test_spans_are_byte_accurate_with_unicode():
    t = _turn("café note. TODO: add tests here.")
    spans = select_spans(t)
    for s, e in spans:
        # must decode cleanly on byte boundaries
        assert t.text.encode()[s:e].decode()
    quotes = [t.text.encode()[s:e].decode() for s, e in spans]
    assert any("TODO" in q for q in quotes)

def test_no_salient_returns_empty():
    assert select_spans(_turn("just chatting about nothing.")) == []
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_select.py -v`
Expected: FAIL — module missing.

- [ ] **Step 3: Write the implementation** `src/provmem/select.py`

```python
from __future__ import annotations
import re
from provmem.types import Turn

_SALIENT = re.compile(
    r"\b(decision|constraint|because|prefer|remember|TODO|FIXME)\b|\b[\w./-]+\.\w+:\d+",
    re.IGNORECASE,
)
# sentence boundary: . ! ? followed by space or end
_SENT = re.compile(r"[^.!?]*[.!?]|[^.!?]+$")

def select_spans(turn: Turn) -> list[tuple[int, int]]:
    text = turn.text
    spans: list[tuple[int, int]] = []
    for m in _SENT.finditer(text):
        sentence = m.group().strip()
        if not sentence or not _SALIENT.search(sentence):
            continue
        # locate the trimmed sentence within the original to get char offsets
        start_char = m.start() + (len(m.group()) - len(m.group().lstrip()))
        end_char = start_char + len(sentence)
        # convert char offsets to byte offsets
        start_b = len(text[:start_char].encode())
        end_b = len(text[:end_char].encode())
        spans.append((start_b, end_b))
    return spans
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_select.py -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/select.py tests/test_select.py
git commit -m "feat: heuristic salience span selector"
```

---

### Task 7: SQLite store — schema, verified write, hash chain

**Files:**
- Create: `src/provmem/store.py`
- Test: `tests/test_store.py`

**Behavior:** `Store(db_path)` opens SQLite, creates the `facts` table + FTS5 index. `store_fact(turn, span, priv_hex, pub_hex)` byte-verifies the span against `turn.text`, computes hash/id/signature, sets `prev_fact_id` to the last fact for that `session_id`, inserts, and returns the `Fact`. It **rejects** (raises `SpanMismatch`) if the quote is empty. `search(query)` returns facts via FTS5.

- [ ] **Step 1: Write the failing test**

```python
import pytest
from provmem.types import Turn
from provmem.store import Store, SpanMismatch
from provmem.crypto import new_keypair, verify_sig

def _turn():
    return Turn("claude_code", "s1", "t1", "assistant",
                "Decision: use Ed25519 because fast.", "/x.jsonl", "sha")

def test_store_and_search(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    span = (0, len("Decision: use Ed25519 because fast.".encode()))
    f = st.store_fact(t, span, priv, pub)
    assert f.quote.startswith("Decision: use Ed25519")
    assert verify_sig(pub, f.content_hash, f.signature)
    hits = st.search("Ed25519")
    assert len(hits) == 1 and hits[0].fact_id == f.fact_id

def test_hash_chain_links_same_session(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    f1 = st.store_fact(t, (0, 8), priv, pub)        # "Decision"
    f2 = st.store_fact(t, (10, 13), priv, pub)       # "use"
    assert f1.prev_fact_id == ""
    assert f2.prev_fact_id == f1.fact_id

def test_rejects_empty_quote(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    with pytest.raises(SpanMismatch):
        st.store_fact(_turn(), (5, 5), priv, pub)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_store.py -v`
Expected: FAIL — module missing.

- [ ] **Step 3: Write the implementation** `src/provmem/store.py`

```python
from __future__ import annotations
import sqlite3, json
from datetime import datetime, timezone
from dataclasses import asdict
from provmem.types import Turn, FactSource, Fact
from provmem import crypto

class SpanMismatch(Exception):
    pass

_SCHEMA = """
CREATE TABLE IF NOT EXISTS facts (
    fact_id TEXT PRIMARY KEY,
    quote TEXT NOT NULL,
    source_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    prev_fact_id TEXT NOT NULL,
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
        cur = self.db.execute(
            "SELECT fact_id FROM facts WHERE session_id=? ORDER BY rowid DESC LIMIT 1",
            (session_id,),
        )
        row = cur.fetchone()
        return row["fact_id"] if row else ""

    def store_fact(self, turn: Turn, span: tuple[int, int],
                   priv_hex: str, pub_hex: str, tags: list[str] | None = None) -> Fact:
        start, end = span
        quote = turn.text.encode()[start:end].decode()
        if not quote.strip():
            raise SpanMismatch("empty or whitespace quote rejected")
        src = FactSource(turn.agent, turn.native_path, turn.native_sha256,
                         turn.session_id, turn.turn_id, (start, end))
        src_dict = asdict(src)
        src_dict["span"] = [start, end]
        ch = crypto.content_hash(quote, src_dict)
        fid = crypto.fact_id(ch, pub_hex)
        sig = crypto.sign(priv_hex, ch)
        prev = self._last_fact_id(turn.session_id)
        created = datetime.now(timezone.utc).isoformat()
        tags = tags or []
        self.db.execute(
            "INSERT INTO facts VALUES (?,?,?,?,?,?,?,?,?,?)",
            (fid, quote, json.dumps(src_dict), ch, sig, pub_hex, prev,
             turn.session_id, json.dumps(tags), created),
        )
        self.db.commit()
        return Fact(fid, quote, src, ch, sig, pub_hex, prev, tags, created)

    def _row_to_fact(self, row) -> Fact:
        sd = json.loads(row["source_json"])
        src = FactSource(sd["agent"], sd["native_path"], sd["native_sha256"],
                         sd["session_id"], sd["turn_id"], tuple(sd["span"]))
        return Fact(row["fact_id"], row["quote"], src, row["content_hash"],
                    row["signature"], row["pubkey"], row["prev_fact_id"],
                    json.loads(row["tags_json"]), row["created_at"])

    def get(self, fact_id: str) -> Fact | None:
        row = self.db.execute("SELECT * FROM facts WHERE fact_id=?", (fact_id,)).fetchone()
        return self._row_to_fact(row) if row else None

    def search(self, query: str, k: int = 10) -> list[Fact]:
        rows = self.db.execute(
            "SELECT f.* FROM facts_fts ft JOIN facts f ON f.rowid=ft.rowid "
            "WHERE facts_fts MATCH ? ORDER BY rank LIMIT ?",
            (query, k),
        ).fetchall()
        return [self._row_to_fact(r) for r in rows]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_store.py -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/store.py tests/test_store.py
git commit -m "feat: sqlite store with verified write, signing, FTS5, hash chain"
```

---

### Task 8: Verifier — re-derive source, byte-compare, typed statuses

**Files:**
- Create: `src/provmem/verify.py`
- Test: `tests/test_verify.py`

**Behavior:** `verify_fact(fact, adapter_for)` returns a status string: `ok`, `source-missing`, `source-modified`, `span-mismatch`, or `bad-signature`. It re-opens the native source, re-derives the matching `Turn` via the agent's adapter, slices the byte span, and byte-compares to `fact.quote`; checks `native_sha256` and signature. `adapter_for(agent)` maps an agent name to an adapter instance.

- [ ] **Step 1: Write the failing test**

```python
from provmem.store import Store
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.verify import verify_fact, default_adapter_for
from tests.conftest import FIXTURES

def _ingest(tmp_path, src_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    turns = list(ClaudeCodeAdapter().iter_turns(src_path))
    t = turns[1]  # assistant turn with "Ed25519"
    text_b = t.text.encode()
    span = (0, len(text_b))
    f = st.store_fact(t, span, priv, pub)
    return st, f

def test_verify_ok(tmp_path):
    src = str(FIXTURES / "claude_code" / "sample.jsonl")
    _, f = _ingest(tmp_path, src)
    assert verify_fact(f, default_adapter_for) == "ok"

def test_verify_source_missing(tmp_path):
    src = str(FIXTURES / "claude_code" / "sample.jsonl")
    _, f = _ingest(tmp_path, src)
    # repoint to a non-existent file
    object.__setattr__(f.source, "native_path", "/no/such.jsonl")
    assert verify_fact(f, default_adapter_for) == "source-missing"

def test_verify_source_modified(tmp_path):
    # copy fixture, ingest, then tamper one byte
    src = tmp_path / "tampered.jsonl"
    src.write_bytes((FIXTURES / "claude_code" / "sample.jsonl").read_bytes())
    _, f = _ingest(tmp_path, str(src))
    data = bytearray(src.read_bytes())
    data[-2] ^= 0x20  # flip a byte
    src.write_bytes(data)
    assert verify_fact(f, default_adapter_for) == "source-modified"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_verify.py -v`
Expected: FAIL — module missing.

- [ ] **Step 3: Write the implementation** `src/provmem/verify.py`

```python
from __future__ import annotations
import os
from typing import Callable
from provmem.types import Fact
from provmem.crypto import verify_sig, file_sha256
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.adapters.codex import CodexAdapter
from provmem.adapters.antigravity import AntigravityAdapter

_ADAPTERS = {
    "claude_code": ClaudeCodeAdapter(),
    "codex": CodexAdapter(),
    "antigravity": AntigravityAdapter(),
}

def default_adapter_for(agent: str):
    return _ADAPTERS[agent]

def verify_fact(fact: Fact, adapter_for: Callable[[str], object]) -> str:
    src = fact.source
    if not verify_sig(fact.pubkey, fact.content_hash, fact.signature):
        return "bad-signature"
    if not os.path.exists(src.native_path):
        return "source-missing"
    if file_sha256(src.native_path) != src.native_sha256:
        return "source-modified"
    adapter = adapter_for(src.agent)
    turn = next((t for t in adapter.iter_turns(src.native_path)
                 if t.turn_id == src.turn_id), None)
    if turn is None:
        return "span-mismatch"
    start, end = src.span
    actual = turn.text.encode()[start:end].decode(errors="replace")
    return "ok" if actual == fact.quote else "span-mismatch"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_verify.py -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/verify.py tests/test_verify.py
git commit -m "feat: byte-compare verifier with typed statuses"
```

---

### Task 9: Markdown-card render

**Files:**
- Create: `src/provmem/render.py`
- Test: `tests/test_render.py`

**Behavior:** `render_cards(facts)` returns the benchmark-winning markdown-card form. Each card: `- "<quote>" — <agent>:<session>#<turn> [<fact_id[:8]>]`. This is what gets injected into agent context.

- [ ] **Step 1: Write the failing test**

```python
from provmem.types import FactSource, Fact
from provmem.render import render_cards

def _fact():
    src = FactSource("claude_code", "/x.jsonl", "sha", "s1", "t1", (0, 5))
    return Fact("abcdef1234", "hello", src, "ch", "sig", "pk", "", [], "")

def test_render_card_has_quote_and_citation():
    out = render_cards([_fact()])
    assert '"hello"' in out
    assert "claude_code:s1#t1" in out
    assert "abcdef12" in out  # short id

def test_render_empty():
    assert render_cards([]) == ""
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_render.py -v`
Expected: FAIL — module missing.

- [ ] **Step 3: Write the implementation** `src/provmem/render.py`

```python
from __future__ import annotations
from provmem.types import Fact

def render_cards(facts: list[Fact]) -> str:
    lines = []
    for f in facts:
        s = f.source
        lines.append(
            f'- "{f.quote}" — {s.agent}:{s.session_id}#{s.turn_id} [{f.fact_id[:8]}]'
        )
    return "\n".join(lines)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_render.py -v`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/provmem/render.py tests/test_render.py
git commit -m "feat: markdown-card render for context injection"
```

---

### Task 10: Ingest pipeline + MCP server

**Files:**
- Create: `src/provmem/ingest.py`
- Create: `src/provmem/mcp_server.py`
- Test: `tests/test_ingest.py`

**Behavior:**
- `ingest_file(store, adapter, path, priv, pub)` runs adapter → selector → store, returning the count of facts written.
- MCP handlers `recall(query, k)`, `verify(fact_id)`, `cite(fact_id)` are plain functions taking a `Store`; the FastMCP wiring is thin. We test the handler functions directly (no transport).

- [ ] **Step 1: Write the failing test**

```python
from provmem.store import Store
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.mcp_server import make_handlers
from tests.conftest import FIXTURES

def test_ingest_then_recall_verify_cite(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    src = str(FIXTURES / "claude_code" / "sample.jsonl")
    n = ingest_file(st, ClaudeCodeAdapter(), src, priv, pub)
    assert n >= 1
    h = make_handlers(st)
    cards = h["recall"]("Ed25519", 5)
    assert "Ed25519" in cards
    # grab a fact_id from search to verify+cite
    fid = st.search("Ed25519")[0].fact_id
    assert h["verify"](fid) == "ok"
    assert "claude_code" in h["cite"](fid)

def test_recall_no_match_returns_empty(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    ingest_file(st, ClaudeCodeAdapter(), str(FIXTURES / "claude_code" / "sample.jsonl"), priv, pub)
    assert make_handlers(st)["recall"]("zzzznotpresent", 5) == ""
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_ingest.py -v`
Expected: FAIL — modules missing.

- [ ] **Step 3: Write the ingest pipeline** `src/provmem/ingest.py`

```python
from __future__ import annotations
from provmem.store import Store, SpanMismatch
from provmem.select import select_spans

def ingest_file(store: Store, adapter, path: str, priv_hex: str, pub_hex: str) -> int:
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

- [ ] **Step 4: Write the MCP handlers + server** `src/provmem/mcp_server.py`

```python
from __future__ import annotations
import json
from provmem.store import Store
from provmem.render import render_cards
from provmem.verify import verify_fact, default_adapter_for

def make_handlers(store: Store) -> dict:
    def recall(query: str, k: int = 10) -> str:
        return render_cards(store.search(query, k))

    def verify(fact_id: str) -> str:
        f = store.get(fact_id)
        return "not-found" if f is None else verify_fact(f, default_adapter_for)

    def cite(fact_id: str) -> str:
        f = store.get(fact_id)
        if f is None:
            return "{}"
        return json.dumps(f.to_dict()["source"])
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
    build_server(os.environ.get("PROVMEM_DB", "provmem.db")).run()
```

- [ ] **Step 5: Run test to verify it passes**

Run: `pytest tests/test_ingest.py -v`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add src/provmem/ingest.py src/provmem/mcp_server.py tests/test_ingest.py
git commit -m "feat: ingest pipeline + MCP recall/verify/cite handlers"
```

---

### Task 11: End-to-end tamper demo (the headline test)

**Files:**
- Test: `tests/test_e2e_demo.py`

**Behavior:** The spec's MVP success criterion as one executable test: ingest a real transcript → recall returns cards → every fact verifies `ok` → tamper one byte of the source → the affected fact(s) now verify `source-modified`.

- [ ] **Step 1: Write the test**

```python
from provmem.store import Store
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.verify import verify_fact, default_adapter_for
from tests.conftest import FIXTURES

def test_tamper_one_byte_is_detected(tmp_path):
    priv, pub = new_keypair()
    src = tmp_path / "session.jsonl"
    src.write_bytes((FIXTURES / "claude_code" / "sample.jsonl").read_bytes())
    st = Store(str(tmp_path / "m.db"))

    n = ingest_file(st, ClaudeCodeAdapter(), str(src), priv, pub)
    assert n >= 1
    facts = st.search("Ed25519")
    assert facts and all(verify_fact(f, default_adapter_for) == "ok" for f in facts)

    # tamper: flip a byte inside the file
    data = bytearray(src.read_bytes())
    data[-3] ^= 0x20
    src.write_bytes(data)

    statuses = {verify_fact(f, default_adapter_for) for f in facts}
    assert "source-modified" in statuses
```

- [ ] **Step 2: Run test to verify it passes**

Run: `pytest tests/test_e2e_demo.py -v`
Expected: PASS. (No new implementation — this exercises the assembled system.)

- [ ] **Step 3: Run the whole suite**

Run: `pytest -q`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add tests/test_e2e_demo.py
git commit -m "test: end-to-end tamper-detection demo"
```

---

### Task 12 (stretch): local vector recall via sqlite-vec

**Files:**
- Modify: `src/provmem/store.py` (add optional embedding column + `vsearch`)
- Create: `src/provmem/embed.py`
- Test: `tests/test_embed.py`

**Behavior:** Add a local embedder behind an interface with a deterministic fake for tests; real impl uses a local ONNX `nomic-embed` model. `Store.vsearch(query)` uses sqlite-vec cosine. If the model/extension is unavailable, callers fall back to `search` (FTS5). **Out of MVP critical path — only do this after Tasks 0–11 are green.** Keep tests offline using the fake embedder (no model download, no network).

- [ ] **Step 1: Write `embed.py` with `Embedder` protocol + `FakeEmbedder` (hash-based deterministic vector) + `OnnxEmbedder` stub that raises if model missing.**
- [ ] **Step 2: Write `tests/test_embed.py` using `FakeEmbedder`; assert nearest-neighbor ordering on toy vectors.**
- [ ] **Step 3: Add `embedding BLOB` column + `vec0` virtual table to store; wire `vsearch`; guard with try/except ImportError → document FTS5 fallback.**
- [ ] **Step 4: Run `pytest tests/test_embed.py -v`; then full suite.**
- [ ] **Step 5: Commit** `feat: optional local vector recall (sqlite-vec) with FTS5 fallback`

---

## Self-Review

**Spec coverage:**
- §3 fact schema + content_hash/fact_id/signature → Tasks 1, 2, 7. ✓
- §3 byte-compare verify + typed statuses → Task 8. ✓
- §3 no-poisoning (reject quote absent from source) → Task 7 (`SpanMismatch`) + Task 8 (`source-modified`/`span-mismatch`) + Task 11 (e2e). ✓
- §3 hash chain per source → Task 7 (`prev_fact_id`). ✓
- §3 untrusted-selector corollary → structurally enforced: selector (Task 6) returns spans only; store (Task 7) verifies before write. ✓
- §4.1 adapters CC→Codex→Antigravity priority → Tasks 3, 4, 5. ✓
- §4.2 heuristic selector → Task 6. ✓
- §4.3 sqlite FTS5 + sign → Task 7; sqlite-vec local embed → Task 12 (stretch, with FTS5 fallback per §7). ✓
- §4.4 markdown-card render → Task 9. ✓
- §4.5 MCP recall/verify/cite → Task 10. ✓
- §7 error handling: adapter skips empty/malformed (Tasks 3–5 skip empty text; `json.loads` per line); store rejects bad span (Task 7); verify distinct statuses (Task 8); embeddings degrade to FTS5 (Task 12). ✓
- §8 testing: per-adapter fixtures, round-trip invariant (Task 11), tamper test (Task 11), format benchmark already exists. ✓
- §9 MVP demo criterion → Task 11. ✓

**Placeholder scan:** No TBD/TODO-as-instruction. Task 5 Step 1 is an intentional discovery step for an unconfirmed format (flagged), with a concrete fallback implementation given. Task 12 is explicitly stretch with concrete sub-steps.

**Type consistency:** `Turn`/`FactSource`/`Fact` field names consistent across Tasks 1, 7, 8, 9, 10. `select_spans(turn) -> list[(start,end)]` consumed identically in Tasks 7/10. `store_fact(turn, span, priv, pub)` signature consistent Tasks 7/8/10. `verify_fact(fact, adapter_for)` and `default_adapter_for` consistent Tasks 8/10/11. `make_handlers(store)` dict keys `recall/verify/cite` consistent Tasks 10/11. ✓

**Note on byte-span subtlety (Task 6 ↔ 7 ↔ 8):** spans are UTF-8 byte offsets into `Turn.text`. `quote = turn.text.encode()[start:end].decode()`. The selector emits offsets on byte boundaries (it slices on sentence char boundaries then re-encodes prefix length), so decode is clean — covered by `test_spans_are_byte_accurate_with_unicode`.
