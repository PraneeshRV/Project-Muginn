# Competitor Audit — local cross-agent memory (2026-06-21)

Read-only source audit of 5 repos cloned to /tmp. Goal: is "fact→exact source transcript span + attestation + poisoning defense, all local" unoccupied?

## Per-repo

### cass (`Dicklesworthstone/coding_agent_session_search`) — Rust, MIT
- **What:** best-in-class native transcript INGESTION. Connectors for 22 agents incl. `claude_code.rs`, `codex.rs`, `antigravity.rs`, `chatgpt.rs`, `cursor.rs` (`/tmp/cass/src/connectors/`). Indexed search (TUI/CLI).
- **Attestation:** NONE. `packet_audit.rs` / `conversation_packet.rs` are analytics/audit, no crypto signing. grep ed25519/sign = no crypto hits in src.
- **Memory/provenance:** stores raw conversation packets, not distilled facts. No KG, no signed ledger.
- **Verdict:** ingestion king, zero memory/attestation.

### AIngram (`bozbuilds/AIngram`) — Python — CLOSEST COMPETITOR
- **Storage:** SQLite, sqlite-vec (QJL 1-bit), FTS5. Schema `aingram/storage/schema.py`.
- **Attestation:** REAL + strong. `memory_entries` has `content_hash`, `prev_entry_id`, `signature` (schema.py:24). `entry_id = compute_entry_id(content_data, parent_ids, pubkey)` — content-addressed Merkle-DAG; `signature = session.sign(entry_id)` Ed25519 (store.py:184, trust/signing.py:38). mcp_server.py:199 "walk hash chain from genesis to tip, checking every link and signature" = tamper-evident ledger. `agent_sessions.public_key` per agent.
- **KG:** entities/relationships with `t_valid`/`t_invalid` (temporal) + `source_memory`, cross_references (builds_on/contradicts/supersedes). schema.py:88-112.
- **Ingest:** LIVE capture via hooks — `ClaudeCodeAdapter`, cursor/gemini/cline adapters (capture/daemon.py). Captures during agent runs, NOT indexing of existing historical transcript corpus.
- **Local?** HYBRID. Local embed (processing/embedder.py, extraction/local.py) BUT optional cloud LLM extraction (extraction/sonnet.py → Anthropic). Not airgap by default.
- **GAP:** (1) fact extraction via LLM (sonnet) paraphrases → **exact source span citation is lost** at distill time. (2) signs its OWN ledger entry_id; no verifiable pointer "this fact ⇐ CC session X, turn 47, chars [1200,1350]" you can re-open & check. (3) live-hook capture, not corpus ingestion of existing logs. (4) cloud extraction breaks strict-local.

### AIngram (`StevenJohnson998/AIngram`) — different project — JS, restrictive license
- Agorai/community **knowledge marketplace**: accounts, votes, flags, sanctions, subscriptions, `injection_scores`/`injection_log` (poisoning tracker, migration 056). Multi-agent debate curation. Cloud/Supabase, Docker. NOT local-first, NOT transcript-grounded. Different problem (social curation), not relevant competitor.

### Memory-Vault (`fusae/Memory-Vault`) — TS
- SQLite `memories` + `memory_versions` (versioning, not provenance), sqlite-vec + Ollama (local embed), optional AES-256-GCM E2EE + Supabase sync. No signing, no source-span provenance, no transcript ingest. Solid local memory, no attestation.

### MeMesh (`PCIRCLE-AI/memesh-llm-memory`) — TS
- FTS5 + sqlite-vec, multi-factor scoring (relevance/recency/frequency/confidence). No crypto attestation, no source-span provenance, no native transcript ingest.

## Verdict

| | ingest native transcripts | signed/tamper-evident | **fact→exact source SPAN, verifiable** | poisoning defense | strict-local default |
|---|---|---|---|---|---|
| cass | ✓ (22 agents) | ✗ | ✗ | ✗ | ✓ |
| AIngram_boz | ~ (live hook) | ✓ (Ed25519 DAG) | ✗ (LLM extract severs span) | ~ | ✗ (cloud extract) |
| Memory-Vault | ✗ | ✗ | ✗ | ✗ | ✓ |
| MeMesh | ✗ | ✗ | ✗ | ✗ | ✓ |
| AIngram_steven | ✗ | ~ | ✗ | ✓ (social) | ✗ |

**Naive novelty "signed local memory" = DEAD** — AIngram_boz already ships Ed25519 content-addressed hash-chained ledger + temporal KG + local embed.

**Surviving (genuinely unoccupied) novelty:** **verifiable span-level provenance that survives LLM extraction** — every distilled fact carries a cryptographic, re-checkable citation to the exact {agent, session, turn, char-span} in the original native transcript, so a reader can re-open the source and confirm the fact wasn't hallucinated or poisoned during distillation. Nobody binds *distilled memory* back to *exact source span verifiably*: cass ingests but doesn't distill/attest; AIngram distills + signs its own ledger but loses the source-span link at extraction. Combine: cass-grade multi-agent corpus ingestion + AIngram-grade signed ledger + NEW extraction-surviving span-citation attestation + strict-local-by-default. Second axis (token-efficient format) has zero competitor benchmark.

**Closest competitor:** AIngram_boz. What it lacks: extraction-surviving exact-span citations, corpus (not just live-hook) ingestion, strict-local default, format token benchmark.
