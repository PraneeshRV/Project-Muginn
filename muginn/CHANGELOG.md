# Changelog

## 0.8.1 - 2026-06-24

Patch release.

### Fixes

- **`verify` / `cite` now accept the short atom id that `recall` prints.** `recall`
  surfaces an 8-character id prefix (e.g. `verify[d027121b]`), but `verify` and `cite`
  did an exact 64-char lookup, so the printed id always returned `not-found` — over both
  the CLI and the MCP server. `Store::get` now resolves an unambiguous id prefix to the
  full id (git short-hash style): exact matches still win, ambiguous or wildcard-bearing
  prefixes resolve to nothing, never the wrong atom.
- **`verify` and `cite` exit non-zero on failure.** Both previously exited `0` even on
  `not-found` / `source-modified`, so scripts couldn't gate on the result. A non-`ok`
  verify status and a not-found cite now exit `1` (the status text is still printed).
- **Clippy clean** — fixed `flatten()`-on-`Result` (could loop on a persistently erroring
  reader; now `map_while(Result::ok)`) across the four adapters, plus minor lints in
  `core`, `select`, `verify`, and `vault`. `cargo clippy --workspace --all-targets` now
  reports 0 warnings.

54 tests passing, 0 clippy warnings.

## 0.8.0 - 2026-06-24

First public release.

**Tamper-evident provenance for AI agent memory.** Every stored fact is a verbatim quote
from a native agent transcript, cryptographically bound (Ed25519) to the exact byte-span it
came from — re-open the source and byte-verify it later.

### What's in this release

- **Verifiable core** — ingest transcripts (Claude Code, Codex CLI, Cursor, ChatGPT) →
  salient verbatim atoms → SQLite/FTS5 store with Ed25519-signed, byte-addressable citations.
- **CLI** — `ingest`, `ingest-all`, `recall`, `verify <id>`, `cite <id>`, `compile`, `sync`,
  `serve` (MCP stdio), `eval`.
- **MCP server** — recall / verify / cite / ingest / compile over stdio.
- **Obsidian plugin** — live verify badges, source-jump, coverage indicators, recompile.
- **`bytecite`** — standalone, dependency-light crate for signed byte-verifiable citations,
  reusable outside Muginn.
- **Eval harness** — selector recall/FP, provenance coverage, poison rejection, staleness
  precision/recall, format overhead, offline LongMemEval/LoCoMo subset.

### Honest scope

Muginn proves a quote existed verbatim at ingest and wasn't edited since. It does **not**
prove the quote is true, and does not stop upstream injection (poison already in the
transcript ingests like any other quote). Retrieval is keyword/FTS5 today; semantic recall
is on the roadmap. Pair it with a semantic store (Mem0/Zep/MemPalace) rather than replacing
one.

### Install

Prebuilt binaries (macOS/Linux/Windows) are attached below, or build from source:

```bash
cd muginn && cargo install --path crates/cli
```

### Fixes

- Closed a shell-injection hole in the Obsidian plugin (now uses `execFileSync` argv).
- Cross-platform build — the Windows target compiles (`#[cfg(unix)]`-gated key permissions).
- 52 tests passing, 0 warnings.

License: Apache-2.0.
