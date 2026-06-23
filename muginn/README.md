# muginn

Verifiable memory for AI agents. Every fact is a verbatim quote from a native agent
transcript, cryptographically bound to the exact byte-span it came from, so a reader
can re-open the source and confirm no fact was hallucinated or poisoned during distillation.

**Headline guarantees (measured, offline, no cloud calls):**
- 100% poison rejection — fabricated citations always quarantined
- ≥95% provenance coverage — all claims from real, unmodified atoms verify `ok`
- 0 cloud calls by default — local-first, airgap-safe

Agents supported: Claude Code, Codex CLI, Cursor, ChatGPT.

License: Apache-2.0.

---

## Quickstart

```bash
cargo install --path crates/cli

# Ingest a Claude Code session
muginn ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl

# Search memory
muginn recall "Ed25519"

# Compile a page on a topic
muginn compile "authentication decisions" --root ./vault

# Sync to Obsidian vault
muginn sync --root ~/vaults/muginn
```

`recall` prints markdown cards and shows a live `verify` status for each atom.

---

## How verification works

`verify(atom)` re-opens the source transcript, locates the exact turn by `turn_id`, and
byte-compares `turn.text.as_bytes()[start..end]` against the stored `quote`. It also
checks the Ed25519 signature over `content_hash`.

Possible statuses:

| Status | Meaning |
|---|---|
| `ok` | Quote matches source byte-for-byte; signature valid |
| `bad-signature` | Ed25519 check failed |
| `source-missing` | Transcript file no longer exists |
| `turn-missing` | File exists but the turn_id is gone |
| `source-modified` | Turn text changed since ingest |
| `span-mismatch` | Byte range no longer matches quote |

Tamper-detection is per-turn, not per-file. Appending new turns to a live session never
invalidates existing atoms.

---

## Citation enforcement (compile layer)

```
muginn compile "auth decisions" --root ./vault
```

Each compiled sentence must cite at least one atom that verifies `ok`. Sentences with
missing, tampered, or hallucinated citations are quarantined and surfaced in an Obsidian
callout — never silently included.

This is the anti-poisoning moat: an adversary cannot inject a fabricated fact into a
compiled page because the enforce step re-verifies every citation against the original
source bytes.

---

## MCP server

Register muginn as an MCP server so Claude Code, Codex, or any MCP-aware agent can
recall and verify memory directly.

**Claude Code** (`~/.claude/settings.json`):
```json
{
  "mcpServers": {
    "muginn": {
      "command": "muginn",
      "args": ["serve"]
    }
  }
}
```

**Codex CLI** (`~/.codex/config.toml`):
```toml
[[mcp_servers]]
name = "muginn"
command = ["muginn", "serve"]
```

Available MCP tools: `recall`, `verify`, `cite`, `ingest`, `compile`.

---

## Multi-agent ingest

Configure `muginn.toml` to ingest from multiple agent transcript roots:

```toml
[agents.claude_code]
roots = ["~/.claude/projects"]

[agents.codex]
roots = ["~/.codex/history"]

[vault]
root = "~/vaults/muginn"
```

```bash
muginn ingest-all --config muginn.toml
```

---

## Staleness

Atoms carry a `topic_key` (first four alphanumeric tokens of the quote, lowercased).
When a newer atom with the same `topic_key` is stored, the older atom is marked
`stale=true` and hidden from `recall` by default.

`muginn sync` renders stale atoms to `_stale/` in the Obsidian vault with a unified diff
against the superseding atom — non-destructive, time-travel friendly.

---

## Influences

- [cass](https://github.com/Dicklesworthstone/coding_agent_session_search) — multi-agent connector formats (22 agents); referenced for Claude Code, Codex, Cursor, ChatGPT transcript shapes
- [AIngram (bozbuilds)](https://github.com/bozbuilds/AIngram) — Ed25519 content-addressed Merkle-DAG + temporal KG; confirmed the attestation design space
- [basic-memory](https://github.com/basicmachines-co/basic-memory) — Obsidian-native vault pattern
- [ai-memory](https://github.com/cpacker/ai-memory) — local-first memory semantics
- Andrej Karpathy — "every LLM output should be grounded and citable"
