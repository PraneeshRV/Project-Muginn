# Muginn

Verifiable memory for AI agents. Every stored fact is a verbatim quote from a native
agent transcript, cryptographically bound to the exact byte-span it was extracted from.
Any reader can re-open the source transcript and confirm the quote exists at the recorded
position — no fact can survive distillation if the source byte-range no longer matches.

Agents supported: Claude Code, Codex CLI, Cursor, ChatGPT. License: Apache-2.0.

---

## Quickstart

```bash
cargo install --path crates/cli

# Ingest a Claude Code session
muginn ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl

# Search memory
muginn recall "Ed25519"

# Verify or cite a single atom by id (used by the Obsidian plugin)
muginn verify <atom_id>   # prints: ok | bad-signature | source-missing | … | not-found
muginn cite   <atom_id>   # prints the citation JSON {agent, session, turn, span}

# Run the eval harness against your live store
muginn eval
```

`recall` prints markdown cards with a live `verify` status per atom.

---

## How it works

### Atoms

An *atom* is a salience-selected sentence from a transcript turn. On ingest:

1. Each turn is split into sentences. Salient sentences (containing `decision`,
   `constraint`, `because`, `prefer`, `remember`, `TODO`, `FIXME`, or a `file:line`
   reference) are kept; the rest are discarded.
2. For each kept sentence, muginn records the exact byte range `[start, end)` within the
   turn text, SHA-256s the entire turn (`turn_sha256`), builds a `content_hash` over the
   citation + quote, signs it with Ed25519, and stores the atom in SQLite with FTS5 indexing.

### Verification

`verify(atom)` re-opens the source transcript, locates the turn by `turn_id`, and
byte-compares `turn.text.as_bytes()[start..end]` against the stored quote.

| Status | Meaning |
|---|---|
| `ok` | Quote matches source byte-for-byte; Ed25519 signature valid |
| `bad-signature` | Signature check failed |
| `source-missing` | Transcript file no longer exists at recorded path |
| `turn-missing` | File exists but `turn_id` is absent |
| `source-modified` | Turn text changed since ingest (`turn_sha256` mismatch) |
| `span-mismatch` | File and turn present but byte range no longer matches quote |

Tamper detection is per-turn, not per-file: appending new turns to a live session never
invalidates existing atoms.

### Citation enforcement

`muginn compile <topic>` retrieves atoms, generates prose (currently via `NullCompiler`,
which emits one sentence per atom — each citing itself), and enforces that every sentence
cites at least one atom whose `verify` status is `ok`. Sentences with no citation, a
missing atom, or a non-`ok` verify result are quarantined and surfaced in an Obsidian
warning callout — never silently included in the compiled page.

A `LocalCompiler` that POSTs to a local Ollama/llama.cpp endpoint (`MUGINN_COMPILE_URL`)
exists in the `compile` crate but is not yet wired into the CLI or MCP server — the
default `compile` path always uses `NullCompiler`.

### Staleness

Atoms carry a `topic_key` (first four alphanumeric tokens, lowercased, hyphen-joined).
When a newer atom with the same `topic_key` is stored, the older one is marked
`stale = true` and hidden from `recall` by default. `muginn sync` renders stale atoms
to `_stale/` in the Obsidian vault with a unified diff — non-destructive.

---

## MCP server

```bash
muginn serve   # stdio transport
```

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

MCP tools: `recall`, `verify`, `cite`, `ingest`, `compile`.

---

## Multi-agent ingest

```toml
# muginn.toml
vault_root = "~/vaults/muginn"   # reserved; `sync` currently takes --root

[[agents]]
name = "claude_code"
root = "~/.claude/projects"

[[agents]]
name = "codex"
root = "~/.codex/sessions"
```

`root` paths may start with `~` (expanded to your home directory). `ingest-all`
recursively collects every `*.jsonl` under each agent root.

```bash
muginn ingest-all --config muginn.toml
muginn sync --root ~/vaults/muginn
```

---

## Eval harness

```bash
muginn eval [--selector-fixture <path>] [--poison-n <n>]
```

Reports:
- **Selector recall / FP rate** — fraction of salient sentences captured vs non-salient
  falsely kept, measured against a labeled fixture.
- **Provenance coverage** — fraction of compiled sentences whose cited atoms verify `ok`
  on the live store.
- **Poison rejection** — fraction of injected fabricated atom IDs that are quarantined.
  Fabricated IDs cannot exist in the store, so rejection is 100% by construction.
- **Staleness precision / recall** — accuracy of stale labeling against expected supersession.
- **Format overhead** — character count ratio of JSON serialization vs markdown card output.
- **LongMemEval / LoCoMo offline subset** — hit@1/3/5 on bundled 5-question fixtures using
  FTS5 keyword retrieval.

Run `muginn eval` after ingesting real transcripts for numbers that reflect your data.

---

## Workspace layout

```
muginn/
  crates/
    core/      types: Turn, Citation, Atom
    crypto/    sha256, Ed25519 sign/verify, content_hash, atom_id
    adapters/  claude_code, codex, cursor, chatgpt transcript parsers
    select/    salience selector + topic_key
    store/     SQLite + FTS5, hash chain, staleness
    verify/    byte-compare verifier
    render/    markdown card renderer
    vault/     Obsidian vault writer (atom notes, stale notes, page notes)
    compile/   compile trait, NullCompiler, citation enforcement
    server/    rmcp MCP server (stdio)
    eval/      eval harness + LongMemEval/LoCoMo parity fixtures
    cli/       clap binary: muginn
plugin/        Obsidian community plugin (TypeScript)
```

---

## Influences

- [cass](https://github.com/Dicklesworthstone/coding_agent_session_search) — transcript
  connector formats for Claude Code, Codex, Cursor, ChatGPT
- [AIngram (bozbuilds)](https://github.com/bozbuilds/AIngram) — Ed25519 content-addressed
  hash-chained ledger design
- [basic-memory](https://github.com/basicmachines-co/basic-memory) — Obsidian-native vault
  pattern
