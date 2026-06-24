# Muginn

**Tamper-evident provenance for AI agent memory.**

Most agent-memory tools store *paraphrased* facts an LLM distilled from a conversation —
you have to trust the distillation was faithful. Muginn stores the opposite: every fact
is a **verbatim quote** from a native agent transcript, cryptographically bound (Ed25519)
to the exact byte-span it came from. Any reader can re-open the source and confirm the
quote still exists at the recorded position. Verification is a byte comparison, not a
semantic judgment.

That makes two failure modes detectable instead of silent:

- **Tampering** — if a stored fact, or the source it cites, is edited after ingest,
  verification fails.
- **Fabricated citations** — compiled prose can only cite facts that verify `ok`; a
  hallucinated or unverifiable citation is quarantined, never silently included.

OWASP added **Memory Poisoning (ASI06)** to its 2026 Top 10 for agentic apps, and the
named defenses include *provenance tracking* and *trust-aware retrieval*. Muginn is a
small, local-first take on exactly that layer — meant to sit alongside an existing memory
store, not replace it.

Agents supported: Claude Code, Codex CLI, Cursor, ChatGPT. License: Apache-2.0.

---

## What it is — and what it isn't

Muginn proves a quote **existed verbatim in the transcript at ingest, and has not been
edited since.** Be precise about what that does and doesn't buy:

| Defends against | Does **not** defend against |
|---|---|
| Post-hoc tampering of a stored fact or its source | Whether the quote is *true* — it proves origin, not correctness |
| Hallucinated / fabricated citations in compiled output | Upstream injection — poison already in the transcript ingests like any other quote |
| Silent drift between memory and its source | Retrieval quality — recall today is keyword/FTS5; semantic recall is on the roadmap |

If you need *semantic* recall, use Mem0 / Zep / MemPalace. Muginn's job is the
**verifiable-provenance layer over** whatever you store and retrieve.

See it in ~10 seconds — [`bash scripts/demo.sh`](DEMO.md): ingest a fact, edit its source,
watch verification flip to `source-modified`; then watch a fabricated citation fail.

---

## Structure

```
muginn/        Rust workspace (CLI, library crates, eval harness)
plugin/        Obsidian community plugin (TypeScript)
```

The provenance primitive lives in a standalone, dependency-light crate —
[`bytecite`](muginn/crates/bytecite) (signed byte-verifiable citations, no file I/O) — so
it can be reused outside Muginn.

See [`muginn/README.md`](muginn/README.md) for the full quickstart, architecture, and
MCP config.

---

## Install

Prebuilt binaries (no toolchain needed) — macOS, Linux, Windows:

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/PraneeshRV/Project-Muginn/releases/latest/download/muginn-cli-installer.sh | sh
# Windows (PowerShell)
irm https://github.com/PraneeshRV/Project-Muginn/releases/latest/download/muginn-cli-installer.ps1 | iex
```

Or from source:

```bash
cd muginn
cargo install --path crates/cli
```

Then:

```bash
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
A local Ollama compile backend is scaffolded (`MUGINN_COMPILE_URL`) but not yet wired in;
the default compile path is fully offline.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
