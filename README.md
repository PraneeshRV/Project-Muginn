# Muginn

Verifiable memory for AI agents. Every fact is a verbatim quote from a native agent
transcript, cryptographically bound to the exact byte-span it came from, so a reader can
re-open the source and confirm no fact was hallucinated or poisoned during distillation.

**Headline guarantees (measured offline, zero cloud calls):**
- 100% poison rejection — fabricated citations always quarantined
- ≥95% provenance coverage — claims from real, unmodified atoms verify `ok`
- 0 cloud calls by default — local-first, airgap-safe

Agents supported: Claude Code, Codex CLI, Cursor, ChatGPT. License: Apache-2.0.

---

## Implementation

The project is a Rust workspace under [`muginn/`](muginn/). See
[muginn/README.md](muginn/README.md) for the full quickstart, MCP config, and architecture.

```bash
cd muginn
cargo install --path crates/cli

muginn ingest claude_code ~/.claude/projects/<slug>/<uuid>.jsonl
muginn recall "Ed25519"
muginn eval          # prints provenance / poison / staleness / selector metrics
```

The earlier Python prototype has been retired; the Rust workspace is the canonical
implementation (44 tests green, full parity with the original 32-test prototype).

---

## How it works

`verify(atom)` re-opens the source transcript, locates the exact turn by `turn_id`, and
byte-compares `turn.text.as_bytes()[start..end]` against the stored quote, plus an Ed25519
signature check over `content_hash`. Tamper-detection is per-turn, not per-file — appending
new turns to a live session never invalidates existing atoms.

The compile layer enforces that every compiled sentence cites at least one atom that
verifies `ok`; missing, tampered, or hallucinated citations are quarantined, never silently
included. That is the anti-poisoning moat.

---

## Research artifacts

- [research/AUDIT.md](research/AUDIT.md) — competitor audit (AIngram, cass, others)
- [research/format-benchmark/RESULTS.md](research/format-benchmark/RESULTS.md) — token-efficiency benchmark; markdown cards beat JSON by ~61%
- [docs/superpowers/](docs/superpowers/) — design specs and the master implementation plan

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
