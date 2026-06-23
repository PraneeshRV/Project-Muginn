# prov-memory

Local-first, cross-agent memory where every fact is a verbatim quote from a
coding-agent transcript with a byte-verifiable citation back to its exact source
turn, plus staleness detection so superseded facts stop surfacing.

**Honest framing:** For a single-user local tool, Ed25519 signing is not a
security guarantee against yourself — it is there so memory can later cross
trust boundaries. The real day-one value is **grounded citations** (no
hallucinated facts; every quote physically exists in a real transcript turn)
plus **staleness detection** (newer fact on same topic hides the old one).

Agents supported: Claude Code, Codex CLI, Antigravity.

License: Apache-2.0.

---

## Quickstart

```bash
pip install -e .

# ingest a Claude Code session
provmem ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl

# search memory
provmem recall "Ed25519"
```

`recall` prints matching facts as markdown cards and shows a live `verify`
status for each one.

---

## How verification works

`verify(fact)` re-opens the source transcript, locates the exact turn by
`turn_id`, and byte-compares `turn.text.encode()[start:end]` against the stored
`quote`. It also checks the Ed25519 signature over `content_hash`.

Possible statuses:

| Status | Meaning |
|---|---|
| `ok` | Quote matches source byte-for-byte; signature valid |
| `bad-signature` | Ed25519 check failed (key mismatch or tampered hash) |
| `source-missing` | Transcript file no longer exists at recorded path |
| `turn-missing` | File exists but the turn_id is gone |
| `source-modified` | Turn text changed since ingest (sha256 mismatch) |
| `span-mismatch` | File and turn present but byte range no longer matches quote |

Tamper-detection is scoped to the specific turn a fact came from, not the whole
file. Appending new turns to a live session never invalidates existing facts —
this is the key design choice (`turn_sha256` hashes per-turn text, not the full
file).

---

## Staleness

Facts carry a `topic_key` (first four alphanumeric tokens of the quote,
lowercased). When a newer fact with the same `topic_key` is stored, the older
fact is marked `stale=True` and hidden from `recall` by default.

To surface stale facts via the Python API:

```python
store.search("Ed25519", include_stale=True)
```

The CLI `recall` command always hides stale facts (same default as the API).

---

## MCP integration

Register `python -m provmem.mcp_server` as an MCP server so Claude Code, Codex,
or any MCP-aware agent can call it.

### Claude Code — `.claude/settings.json`

```json
{
  "mcpServers": {
    "prov-memory": {
      "command": "python",
      "args": ["-m", "provmem.mcp_server"],
      "env": {"PROVMEM_DB": "~/.provmem.db"}
    }
  }
}
```

### Codex — `~/.codex/config.json`

```json
{
  "mcpServers": {
    "prov-memory": {
      "command": "python",
      "args": ["-m", "provmem.mcp_server"],
      "env": {"PROVMEM_DB": "~/.provmem.db"}
    }
  }
}
```

### Available tools

| Tool | Signature | Returns |
|---|---|---|
| `recall` | `recall(query, k=10)` | Markdown cards for top-k matching facts |
| `verify` | `verify(fact_id)` | Status string (see table above) |
| `cite` | `cite(fact_id)` | JSON source citation `{agent, session_id, turn_id, span, ...}` |

---

## Research artifacts

- [research/AUDIT.md](research/AUDIT.md) — competitor audit (AIngram, cass, others)
- [research/format-benchmark/RESULTS.md](research/format-benchmark/RESULTS.md) — token-efficiency benchmark; markdown cards beat JSON by ~61%, KG triples densest but lossy

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
