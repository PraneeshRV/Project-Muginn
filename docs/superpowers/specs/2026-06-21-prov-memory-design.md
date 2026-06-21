# prov-memory — Design Spec

**Date:** 2026-06-21 · **Status:** approved; **revised 2026-06-21 (rev2)** after honest audit · **Name:** `prov-memory` (placeholder)

> **rev2 re-positioning (post-audit):** Lead with **grounded memory + staleness detection**, not "tamper-proof security" (which is theater for a local single-user tool — signing only earns its keep across trust boundaries). Concrete changes: (1) **per-turn hashing** `turn_sha256 = sha256(turn.text)` replaces per-file sha — the per-file sha falsely invalidated every fact whenever Claude Code *appended* to a live session log; (2) add **staleness/supersession**: facts carry `topic_key` + `created_at`; a newer fact on the same topic marks the older `stale` and recall hides stale by default — this is the real edge over AIngram's ungrounded temporal KG; (3) signing stays in the schema but is framed **cross-trust-ready**, not a local security guarantee; (4) add a **small eval hook** so there is a measurable recall claim. Verifiable source *citations* + staleness are the honest, useful core.

## 1. Summary

Local-first, cross-agent memory where every fact is a **verbatim quote** from an AI-coding-agent transcript that can be re-opened and byte-verified against its exact source turn, with **temporal staleness detection** so superseded facts stop surfacing. Optionally Ed25519-signed for cross-trust transfer. You cannot hallucinate a fact into memory: a quote not physically present in a real transcript turn is rejected at write time.

Standalone open-source project (Apache-2.0). Reuses the *Model-Attested Authority* idea from the separate Securin/PACT-MA research but is independent of it.

## 2. Motivation / novelty

Competitor audit (`research/AUDIT.md`) showed the naive idea ("signed local memory") is already shipped: `bozbuilds/AIngram` has an Ed25519 content-addressed hash-chained ledger + temporal KG + local embeddings; `cass` ingests transcripts from 22 agents. The **unoccupied** gap:

> **Verifiable span-level provenance that survives extraction.** Every memory fact carries a cryptographic, re-checkable citation to the exact `{agent, session, turn, byte-span}` in its source transcript.

- `cass` ingests transcripts but never distills or attests facts.
- `AIngram` distills + signs its own ledger, but **LLM paraphrasing severs the source-span link** and it extracts via cloud Sonnet by default.
- Nobody binds *distilled memory → exact source span, verifiably, fully locally.*

Second contribution: a **token-efficiency benchmark** of memory formats (`research/format-benchmark/`) — markdown cards = best lossless (61% fewer tokens than JSON); KG triples densest but lossy (71%); JSON worst. This decides the render format.

## 3. Core mechanic

A **memory fact** is never paraphrased:

```
Fact {
  fact_id        # = content-address of (quote + source + pubkey)
  quote          # EXACT bytes copied from the transcript
  source { agent, session_id, turn_id, byte_span:[start,end], source_sha256 }
  content_hash   # sha256 of canonical(quote + source)
  signature      # Ed25519 over content_hash
  pubkey         # signer identity (per ingest session/agent)
  prev_fact_id   # hash-chain per source for tamper-evidence
  tags, trust, created_at, supersedes?
}
```

**Verification = byte-compare.** `verify(fact)` re-reads `source.session @ byte_span`, confirms the bytes equal `quote`, confirms `source_sha256` still matches the transcript file, and checks `content_hash` + `signature`. Any mismatch → fact flagged invalid.

**Security properties (the thesis):**
- *No poisoning:* you cannot store a fact whose `quote` is not physically present in a signed source transcript.
- *No hallucination:* extractive-only → the fact text IS the source text.
- *Tamper-evident:* per-source hash chain + `source_sha256` detect edited transcripts.
- *Untrusted selector corollary:* whatever picks which spans to remember (heuristic, local LLM, even a cloud model) cannot poison memory — a returned span that does not byte-match the transcript is dropped. Safety lives in the verifier, not the selector.

## 4. Components (each a small, independently testable unit)

1. **adapters/** — one parser per agent → canonical `Turn` records with stable `turn_id` and byte offsets into the on-disk source. Priority order: **(1) Claude Code** (`~/.claude/projects/**/*.jsonl`), **(2) Codex** (`~/.codex/sessions/`), **(3) Antigravity** (local store — lowest priority). Interface: `iter_turns(path) -> Iterator[Turn]`.
2. **select/** — picks candidate byte spans from turns. MVP = heuristic salience (decisions, constraints, `file:line`, TODO/FIXME, explicit "remember"/user-marked). Returns spans only. Pluggable local-LLM selector is a post-MVP drop-in behind the same interface.
3. **store/** — verify span → hash → Ed25519 sign → persist. SQLite with FTS5 (keyword) + sqlite-vec (vectors via local `nomic-embed` ONNX, no network). Append-only, hash-chained per source.
4. **render/** — emit facts as **markdown cards** for context injection (benchmark winner); verbatim/full context fetched on demand by `source`. Optional KG-triple view for cross-session dedup (post-MVP).
5. **mcp/** — localhost MCP server (FastMCP). Tools: `recall(query, k)`, `verify(fact_id)`, `cite(fact_id)`. This is how Claude Code / Codex / Antigravity read shared memory.

## 5. Data flow

```
native transcript files
   → adapter (parse → Turns w/ byte offsets)
   → selector (candidate spans)
   → verifier+store (byte-verify → hash → Ed25519 sign → sqlite FTS5+vec)
   → render (markdown cards)        ← MCP recall(query)
   → MCP server (localhost)         → CC / Codex / Antigravity
   verify(fact_id): re-open source @ span, byte-compare, check sig
```

## 6. Stack

Python 3.11+ · `cryptography` (Ed25519) · `sqlite-vec` + stdlib `sqlite3` (FTS5) · local `nomic-embed-text` via ONNX (no API key, no external POST = strict-local) · FastMCP for the server · `pytest`. License Apache-2.0.

## 7. Error handling

- Adapter: skip malformed turns, log; never crash the ingest run. Record source file `sha256` at ingest.
- Store: reject (do not write) any fact whose span fails byte-verify; surface count of rejected candidates.
- Verify: distinguish *source-missing*, *source-modified* (sha mismatch), *span-mismatch*, *bad-signature* — each a distinct status, not a generic fail.
- Embeddings: if local model unavailable, degrade to FTS5-only recall with a warning; never silently fall back to a cloud embedder.

## 8. Testing

- Unit: each adapter against a fixture transcript → expected `Turn` offsets.
- Property: for every stored fact, `transcript[byte_span] == quote` (round-trip invariant).
- Security: tamper one byte in a fixture transcript → `verify` returns *source-modified* for affected facts; forge a fact with a quote absent from source → store rejects it.
- Format: `research/format-benchmark` already provides the token table (regression-checkable).

## 9. MVP scope

**In:** Claude Code + Codex adapters (Antigravity if time, lowest priority); heuristic selector; verify+sign+sqlite store; markdown-card render; MCP `recall`/`verify`/`cite`; corpus-index of existing transcript files.

**Out (YAGNI):** live-hook capture, abstractive/LLM-paraphrase facts, local-LLM selector, cloud sync, multi-user/social curation, GUI, KG-triple layer.

**MVP success criterion (the demo):** ingest a real Claude Code + Codex transcript → produce signed markdown memory cards → MCP `recall` returns facts → `verify` re-opens each source and byte-confirms it → tamper one transcript byte and `verify` flags exactly the affected facts. This demo doubles as README gif and paper figure.

## 10. Architecture decision (recorded)

- **Corpus-index existing transcripts first** (chosen) — works on history you already have, no hooks, simplest path to the demo.
- Live-hook capture (AIngram's approach) deferred to a post-MVP adapter behind the same `iter_turns` boundary.
