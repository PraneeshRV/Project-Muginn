# Tamper-evidence demo

A 10-second, reproducible demonstration of what Muginn catches that a paraphrased-memory
store can't. Run it:

```bash
cargo build --manifest-path muginn/Cargo.toml   # or install the binary
bash scripts/demo.sh
```

The script runs under a throwaway `HOME`, so it never touches your real `~/.muginn.db`.

## What it shows

```
1) Ingest — Muginn stores the sentence VERBATIM, Ed25519-signed and byte-addressed.
   ingested 1 atoms

2) Recall — every atom carries a live verify status.
   - "Decision: use Ed25519 because it is fast and the keys are small." — claude_code:session#a1 [032d2afa]
     verify[032d2afa] = ok

3) Verify by id — the quote still matches its source byte-for-byte:
   muginn verify 032d2afa -> ok

4) TAMPER the source — someone quietly edits the memory's origin (fast -> slow):
   muginn verify 032d2afa -> source-modified
   ^ caught: the stored quote no longer matches its source.

5) FABRICATED citation — a hallucinated reference to a fact never stored:
   muginn verify deadbeef -> not-found
   ^ you cannot pass off a citation to a fact that does not exist.
```

## Why it matters

Most agent-memory tools store facts an LLM *paraphrased* from a conversation. Once stored,
there is no way to tell whether the fact was faithful, whether its source was later edited,
or whether a citation points at anything real. Muginn keeps the verbatim quote bound to the
exact byte span it came from, so:

- **Step 4** — editing the source (or the stored memory) breaks the byte match. Tampering
  is detected, not silent.
- **Step 5** — a citation can only resolve to a fact that was actually stored and verifies
  `ok`. Fabricated/hallucinated citations fail.

## Honest scope

This catches **post-hoc tampering** and **fabricated citations**. It does **not** prove the
quote is *true*, and it does **not** stop *upstream* injection — if poisoned text was
already in the transcript at ingest time, it is ingested like any other quote. See the
threat-model table in the [README](README.md).
