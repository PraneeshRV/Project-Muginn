# Verifiable Compiled Memory — Design & Strategy

**Date:** 2026-06-23 · **Status:** planning (pivot from prov-memory MVP) · **Effort:** ultracode
**Decisions locked:** (1) scope = two-layer *verifiable compiled memory*; (2) build surface = MCP server + CLI, the Obsidian vault is the output (plugin comes later); (3) foundation = **Rust rewrite** (the Python `provmem` MVP becomes the executable reference spec).

> **Name (locked): Muginn.** Raven of memory (variant of Norse Huginn/Muninn) — the deliberate spelling sidesteps the crowded Muninn/Munin/Huginn namespace; crates.io `muginn` is free and GitHub has no competing project (only personal Huginn forks). Crate + binary = `muginn`. Repo: `github.com/PraneeshRV/muginn` (the `Muginn` org username is taken). Tagline: *"Muginn — verifiable memory for AI agents. Every fact carries a mark to its source."*

---

## 1. One-liner

**The first AI memory that is both LLM-compiled (readable) and byte-verifiable to source (un-poisonable), rendered as a self-healing Obsidian knowledge graph across every coding agent you use.**

---

## 2. Problem

LLM coding agents lose all context when a session ends, and every tool that fixes this splits into two camps — each with a fatal flaw:

- **LLM-compiled memory** (ai-memory, basic-memory, mem0, cognee): readable, synthesized into pages/graphs. **But it can hallucinate or be poisoned.** A false "fact" compiled once persists and resurfaces sessions later. There is no way to verify a compiled memory against its source.
- **Extractive / verbatim memory** (MemPalace, the prov-memory MVP): trustworthy because it stores exact source text. **But it is not readable as synthesized knowledge** — no narrative, no "what did we decide and why," just a pile of quotes.

### The named unsolved problems of 2026 (from the field, see §13 Sources)

1. **Memory poisoning** — adversary implants false memory that persists across sessions; now a top-3 agentic-security threat. *Nothing readable defends against it*, because the readable tools are the poisonable compiled ones.
2. **Contradiction reconciliation** — appending new memory without reconciling old → accumulating noise.
3. **Staleness in high-relevance facts** — "use Postgres" stays confidently retrieved after the team moved to SQLite. Decay handles low-relevance; staleness in *high-value* facts is the hard open case.
4. **Context rot** — keep everything → quality degrades; prune → lose needed info.

**Thesis:** the gap is a memory that is *simultaneously* compiled-readable and source-verifiable, with reconciliation and staleness built into the data model, surfaced in a tool people already love to look at (Obsidian).

---

## 3. Prior art & influences (credit where due)

We stand on these; the README will credit them explicitly (community goodwill is part of the star strategy).

| Project | Stars | What we take | What it lacks (our wedge) |
|---|---|---|---|
| [ai-memory](https://github.com/akitaonrails/ai-memory) (akitaonrails) | new | compile-not-retrieve; Rust single-binary; markdown-on-disk; cross-agent handoff | grounding/provenance; Obsidian is incidental; compiled = poisonable |
| [basic-memory](https://github.com/basicmachines-co/basic-memory) | 3.3k | markdown-as-source-of-truth; bidirectional Obsidian sync; MCP | LLM-written facts can hallucinate; **no provenance** |
| [cognee](https://github.com/topoteretes/cognee) | 19.4k | pipeline composition; triplet/graph reasoning | Docker-heavy; cloud LLM; no grounding |
| [A-MEM](https://arxiv.org/abs/2502.12110) | paper | Zettelkasten atomic notes; link evolution; bidirectional memory updates | research only; no vault; no grounding |
| [Karpathy LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) | gist | the **compile-not-retrieve** pattern | concept, not a system |
| [cass](https://github.com/Dicklesworthstone/coding_agent_session_search) | — | 22-agent native transcript ingestion (Rust connectors) | no distillation, no attestation |
| AIngram (bozbuilds) | — | Ed25519 content-addressed hash-chain; temporal KG | LLM extraction **severs the source-span link**; cloud extract |
| mem0 | — | extract+update facts; token-efficient retrieval; benchmark rigor (LoCoMo 92.5, LongMemEval 94.4) | cloud; **no verifiable citations** |

**Net:** nobody combines cass-grade ingestion + AIngram-grade attestation + Karpathy-grade compilation + basic-memory-grade Obsidian-native UX — with a **citation that survives compilation down to source bytes.** That last primitive is the moat.

---

## 4. The novel primitive — two-layer verifiable compiled memory

```
 native transcripts (CC / Codex / Cursor / ChatGPT …)
        │  adapters → canonical Turns (per-turn hashed)
        ▼
 ┌─────────────────────────────────────────────────────────┐
 │ LAYER 0 — GROUNDED ATOMS  (extractive, never paraphrased)│
 │ atom = verbatim span + citation{agent,session,turn,bytes}│
 │ byte-verifiable: quote == turn.text.encode()[start:end]  │
 │ Ed25519-signed; per-turn sha256; reject-on-mismatch      │
 └─────────────────────────────────────────────────────────┘
        │  compile (LLM, compile-not-retrieve)
        ▼
 ┌─────────────────────────────────────────────────────────┐
 │ LAYER 1 — COMPILED PAGES  (readable, synthesized)        │
 │ every sentence carries [[atom-id]] citations             │
 │ a page claim is VALID only if its cited atoms verify     │
 │ uncited / unverifiable claims are flagged in-vault       │
 └─────────────────────────────────────────────────────────┘
        │  render
        ▼
 OBSIDIAN VAULT  (atoms + pages = notes; wikilinks; graph; Dataview)
   stale notes greyed + `superseded by [[…]]` + diff (self-healing)
```

**The key inversion vs every competitor:** the compiler is *untrusted*. It may only assemble prose around atoms it cites; the verifier — not the compiler — decides what is true. A hallucinated sentence cites an atom whose bytes don't match the transcript → the citation fails → the claim is quarantined. This is how you get a *readable* memory that is *also* un-poisonable. (Safety lives in the verifier, not the selector/compiler — carried over from the prov-memory thesis.)

### 4.1 Anti-poisoning guarantee

- *No hallucination:* a Layer-1 claim with no verifiable Layer-0 citation never ships to context; it renders as `> [!warning] unverified`.
- *No poisoning:* you cannot store a Layer-0 atom whose quote is not physically present in a real transcript turn (write-time byte check).
- *Tamper-evident:* per-turn `sha256` + Ed25519 detect an edited transcript; verify re-opens the source and byte-compares.

### 4.2 Self-healing / reconciliation

- Each atom derives a `topic_key` (coarse grouping). A newer atom on the same topic marks older ones `stale` — but **never deletes**: the stale note stays, greyed, with `superseded_by: [[new]]` and a rendered diff. Full audit trail.
- Compiled pages re-compile incrementally: when an atom goes stale, pages citing it are marked dirty and re-compiled, and the change is shown as a vault diff (git-versioned).

---

## 5. Architecture (Rust, single binary)

`muginn` is one Rust binary exposing **MCP (stdio + HTTP)** and a **CLI**, writing an **Obsidian vault** as the human-facing source of truth plus a SQLite index for retrieval.

```
muginn/
  crates/
    core/        # types: Turn, Atom, Citation, Page, Supersession
    crypto/      # ed25519-dalek, sha256, content-addressing
    adapters/    # claude_code, codex, cursor, chatgpt … (cass-style)
    select/      # extractive salience → candidate spans
    verify/      # re-open source, byte-compare, status enum
    store/       # rusqlite + FTS5 + sqlite-vec; markdown is source of truth
    compile/     # compile-not-retrieve; citation enforcement
    vault/       # Obsidian renderer: frontmatter, wikilinks, Dataview, diffs
    server/      # axum MCP/HTTP; rmcp (official Rust MCP SDK)
    cli/         # clap
  data/          # <vault>/  + db/ + raw/  (git-versioned vault)
```

**Data directory (matches ai-memory's grep-able/Obsidian-able layout):**
```
<root>/
  vault/<workspace>/<project>/
    atoms/   <atom-id>.md      # frontmatter: citation, sha, sig, topic_key, stale
    pages/   <topic>.md        # compiled; body sentences carry [[atom-id]]
    _stale/  …                 # greyed, superseded, kept for audit
  raw/       <session>.jsonl   # immutable copy of ingested transcript
  db/        index.sqlite      # FTS5 + vec; rebuildable from vault+raw
```

### 5.1 Core types (carried from the verified Python spec)

- `Turn{agent, session_id, turn_id, role, text, native_path, turn_sha256}` — `turn_sha256 = sha256(text)` per-turn (appending later turns never invalidates earlier atoms).
- `Atom{atom_id, quote, citation, content_hash, signature, pubkey, prev_atom_id, topic_key, superseded_by, stale, created_at}`.
- `Citation{agent, native_path, session_id, turn_id, span:[start,end], turn_sha256}`; `quote == turn.text.encode()[start:end].decode()`.
- `content_hash = sha256(canonical_json({quote, citation}))`; `atom_id = sha256(content_hash + pubkey)`; `signature = Ed25519(content_hash)`.
- `Page{page_id, topic, body_md, cited_atom_ids[], compiler_model, compiled_at, dirty}`.
- verify statuses: `ok | bad-signature | source-missing | turn-missing | source-modified | span-mismatch`.

### 5.2 MCP tools

- `recall(query, k)` → compiled page excerpt + grounded atom cards (markdown), each with a `verify` status.
- `verify(atom_id | page_id)` → re-derives source, byte-compares; for a page, verifies all cited atoms and reports coverage %.
- `cite(atom_id)` → exact `{agent, session, turn, span}` for click-through.
- `ingest(agent, path)` → run adapter → select → verified store → mark supersessions → enqueue compile.
- `compile(topic)` → (re)compile a page from current non-stale atoms with citation enforcement.

### 5.3 Rust crate choices

`tokio` · `axum` + `rmcp` (official Rust MCP SDK) · `rusqlite` (bundled SQLite, FTS5) · `sqlite-vec` (local vectors; FTS5 fallback) · `ed25519-dalek` · `sha2` · `serde`/`serde_json` · `clap` · `git2` (vault versioning) · local embeddings via `ort` (ONNX, nomic-embed) — **no network calls by default.**

---

## 6. Eval — the measurable, novel claim

Benchmark scores are saturating (mem0 ~92–94 on LoCoMo/LongMemEval), and "proving longitudinal value beyond benchmark scores" is the field's stated open challenge. So we compete on a **new axis nobody reports**:

- **Provenance coverage** — % of compiled-page claims with a verifying citation (target: >95%).
- **Poison-rejection rate** — inject N fabricated facts into the compile input; measure % quarantined (target: 100% by construction).
- **Staleness precision/recall** — on a labeled supersession set, do we grey the right notes?
- Plus standard **LongMemEval / LoCoMo** for parity, and the existing **selector recall / fp-rate** harness.
- Plus the **format token benchmark** (md-cards 61% leaner than JSON) → why the vault is token-efficient as injected context.

A headline figure: *"readable memory with 100% poison rejection and >95% claim provenance, fully local."* No competitor can print that.

---

## 7. Roadmap (phased; each phase ships something demoable)

**Phase 0 — Port the verified core (Rust).** core + crypto + claude_code adapter + select + verify + store. Re-prove the prov-memory invariants in Rust (port the 32 tests). *Demo: `muginn ingest` + `verify` byte-roundtrip.*

**Phase 1 — Obsidian vault renderer.** Atoms → notes w/ frontmatter + wikilinks; supersession → greyed `_stale/` + diff; graph view works. *Demo: point at `~/.claude`, open the vault in Obsidian, see the graph.*

**Phase 2 — Compile layer + citation enforcement.** compile-not-retrieve pages; quarantine uncited claims; incremental re-compile on staleness. *Demo: a compiled "Decisions" page where every line links to a source span; inject a fake fact → it's quarantined on camera.*

**Phase 3 — MCP server + multi-agent.** rmcp stdio/HTTP; `recall/verify/cite/ingest/compile`; add codex + cursor + chatgpt adapters. *Demo: Claude Code and Codex sharing one verifiable vault.*

**Phase 4 — Eval + polish.** provenance-coverage + poison-rejection harness; LongMemEval/LoCoMo parity; single-binary releases (cargo-dist: macOS/Linux/Windows, AUR, Homebrew).

**Phase 5 (post-launch) — Obsidian community plugin.** in-vault UX: live verify badges, click-to-source, re-compile button. Community-store install = the star funnel.

---

## 8. Go-to-market (the actual 10k-star track — tech is necessary, not sufficient)

- **One viral artifact:** a 20-second GIF — run one command, Obsidian graph blooms from your real agent history, click a node → jump to the exact source line, inject a poisoned fact → watch it get quarantined. This single asset does more than any feature.
- **Zero-friction install:** single binary (`brew install muginn` / AUR / `curl | sh`). No Docker (cognee's friction is our wedge).
- **Launch surfaces, timed together:** HN "Show HN", r/ObsidianMD, r/LocalLLaMA, r/ClaudeAI, X/Bluesky thread, the Obsidian Discord. Lead with the *security* angle (poison-proof memory) — that's novel and shareable, not "another memory tool."
- **Credit the influences loudly** (ai-memory, basic-memory, A-MEM, Karpathy) — the PKM/agent community rewards good citizenship; those maintainers amplify.
- **A claim only we can make:** "100% poison rejection, >95% provenance, 0 cloud calls by default." Put it in the first line of the README.
- **Honest framing:** never market signing as "tamper-proof against yourself" (it isn't for a local single-user tool — it's cross-trust-ready). Over-claiming gets corrected publicly (MemPalace got burned). Under-promise the crypto, over-deliver the grounding.

---

## 9. Risks & honest tradeoffs

- **Rust rewrite cost:** throws away a working 32-test Python MVP. Mitigation: the Python repo is the *executable spec* — port test-for-test, don't redesign.
- **Compile-layer LLM dependency:** Layer 1 needs an LLM. Mitigation: local model by default; Layer 0 (the verifiable part) is fully offline and useful alone; compilation is optional/incremental.
- **Obsidian-native vs lock-in:** plain markdown means no lock-in, but "graph view bloom" needs sensible note granularity — too many atoms = hairball. Mitigation: pages are the default view; atoms are folded under them.
- **Crowded space:** many memory tools. Mitigation: the *verifiable + readable + local + Obsidian* intersection is empty; lead with the security wedge, not "memory."
- **Adapter rot:** native transcript formats drift. Mitigation: per-turn hashing tolerates appends; adapters are small and fixture-tested (cass proves 22 is maintainable).

---

## 10. Resolved decisions

1. **Name — Muginn.** Crate/binary `muginn`, repo `github.com/PraneeshRV/muginn`.
2. **Compile model — BYO, local-default, never bundled.** Bundling a model would kill the single-binary distribution (100MB+). Layer 0 (grounding + verify) needs no LLM, so *"0 cloud calls by default"* stays literally true. Layer 1 compile is pluggable: auto-detect a local Ollama/llama.cpp endpoint; cloud key opt-in; if neither present, skip compile and ship the verifiable atom vault alone. Small binary, honest claim, cloud strictly optional.
3. **Vault granularity — atom-per-note; pages are the default view; graph scoped by folder/tag.** Atom-per-note is required for per-claim click-through, the graph-bloom demo, and backlinks (the moat depends on it). Hairball avoided by defaulting the UX to pages and folder-scoping the Obsidian graph; `atoms/` folds away until you want the dense view.
4. **Project identity — adapter-derived cwd/slug, with a `muginn.toml` override.** We do *not* try to solve the general 2026 identity-resolution problem. Coding agents already scope by working directory (e.g. `~/.claude/projects/<cwd-slug>/`); derive `project_id` from that and `workspace` from git root/remote. Deterministic, matching the project's verifiable ethos; config file resolves ambiguous cases.
5. **Adapters — own thin ones; cass as format reference (credited); start with 4.** cass connectors emit *its* search packets, not our byte-offset, per-turn-hashed `Turn`, so we'd re-adapt regardless; adapters are ~40 lines each and the byte discipline is core to the moat, so we own them. MVP covers Claude Code + Codex + Cursor + ChatGPT; the long tail is deferred. No external-churn coupling, leaner binary.

---

## 11. What we deliberately are NOT building (YAGNI)

Live-hook capture (corpus-ingest first), social/marketplace curation, multi-user sync, a GUI beyond Obsidian, cloud anything by default, abstractive facts without citations.

---

## 12. Why this can win (summary)

It is the only memory that is **readable AND verifiable AND local AND Obsidian-native**, it directly answers the *named* 2026 unsolved problems (poisoning, reconciliation, staleness), it has a screenshot-perfect demo, it installs in one command, and it can print a security claim no competitor can match. The moat is the citation that survives compilation to source bytes — and that is exactly the thing the rest of the field threw away.

---

## 13. Sources

- ai-memory — https://github.com/akitaonrails/ai-memory
- basic-memory — https://github.com/basicmachines-co/basic-memory
- cognee — https://github.com/topoteretes/cognee
- A-MEM (arXiv 2502.12110) — https://arxiv.org/abs/2502.12110
- Karpathy LLM Wiki — https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f
- Agent Memory Race of 2026 (OSS Insight) — https://ossinsight.io/blog/agent-memory-race-2026
- State of AI Agent Memory 2026 (mem0) — https://mem0.ai/blog/state-of-ai-agent-memory-2026
- AI Memory Benchmarks 2026 (mem0) — https://mem0.ai/blog/ai-memory-benchmarks-in-2026
- Persistent memory poisoning (C. Schneider) — https://christian-schneider.net/blog/persistent-memory-poisoning-in-ai-agents/
- Top Agentic AI Security Threats Late 2026 (Stellar Cyber) — https://stellarcyber.ai/learn/agentic-ai-securiry-threats/
- Vault Operator (Obsidian) — https://github.com/pssah4/vault-operator
- Local-first MCP for Obsidian (Nooscope writeup) — https://www.rodneydyer.com/your-vault-your-vectors-building-a-local-first-mcp-server-for-obsidian/
