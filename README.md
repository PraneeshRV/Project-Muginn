# Muginn

Verifiable memory for AI agents.

Every stored fact is a verbatim quote from a native agent transcript, cryptographically
bound to the exact byte-span it was extracted from. Any reader can re-open the source
file and confirm the quote exists at the recorded position — verification is a byte
comparison, not a semantic judgment.

Agents supported: Claude Code, Codex CLI, Cursor, ChatGPT. License: Apache-2.0.

---

## Structure

```
muginn/        Rust workspace (CLI, library crates, eval harness)
plugin/        Obsidian community plugin (TypeScript)
```

See [`muginn/README.md`](muginn/README.md) for the full quickstart, architecture, and
MCP config.

---

## Install

```bash
cd muginn
cargo install --path crates/cli

muginn ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl
muginn recall "Ed25519"
```

---

## Design properties

**Byte-verifiable citations.** Every atom records `(native_path, turn_id, start, end)`.
`muginn verify` re-opens the file, finds the turn, and byte-compares the stored span
against the live text. If the source was modified after ingest, verification fails.

**Ed25519 signatures.** Each atom is signed over `sha256(canonical_json(citation, quote))`.
Signature verification runs before the byte comparison; a tampered `content_hash` is
caught before the file is even opened.

**Citation enforcement.** Compiled prose must cite atoms that verify `ok`. Uncited or
unverifiable sentences are quarantined — they appear in an Obsidian warning callout and
are excluded from the main text.

**Staleness.** When a newer atom supersedes an older one on the same topic, the older
atom is marked stale and moved to `_stale/` in the vault with a unified diff. Nothing
is deleted.

**Local-first.** No network calls in the core path. The MCP server runs over stdio.
An optional local Ollama endpoint can be configured for the compile layer.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
