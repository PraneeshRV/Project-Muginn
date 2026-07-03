# ⚠️ Archived — 2026-07

**Muginn is no longer under active development.** The code works and the repo stays up as a
reference; I'm not shipping new features.

## Why
I'm consolidating everything I do around **AI red teaming** — finding how autonomous AI systems can
be manipulated. Muginn (tamper-evident provenance for AI-agent memory) is solid engineering, but
it sits on the *build a tool* side rather than the *break a system* side, and the provenance space
is now driven by well-resourced standards — [Agent Trace](https://agent-trace.dev/) (Cursor,
Cloudflare, Vercel, Google Jules) and Git AI. A solo project competing there is the wrong fight for
me right now.

## What it was
Tamper-evident provenance for AI-agent memory: every stored fact is a verbatim quote from a native
agent transcript, cryptographically bound (Ed25519) to the exact byte-span it came from.
Verification is a byte comparison, not a semantic judgment — it catches post-hoc tampering and
fabricated citations (the OWASP ASI06 memory-poisoning surface). Reached **v0.8.1**: 11 lockstep
`muginn-*` crates, cargo-dist 4-platform builds, 54 tests, 0 clippy warnings. The provenance
primitive was extracted into a standalone crate, `bytecite` (Citation + sha256 + Ed25519 +
`verify_quote()`, zero deps).

## What I took from it
- Rust at real depth — workspaces, crate extraction, cargo-dist release engineering.
- Ed25519 signing, sha256 hash chains, deterministic length-prefixed encoding.
- The hard lesson (v0.8.0→v0.8.1): **test the documented user workflow against the shipped binary,
  not just `cargo test`** — the flagship byte-verify feature was unreachable from its own CLI
  discovery path while every unit test passed.

## If you want this capability
Pair a semantic memory store (Mem0 / Zep / MemPalace) with an attribution standard (Agent Trace or
Git AI). For the crypto-sealing idea specifically, `bytecite` is the reusable piece.
