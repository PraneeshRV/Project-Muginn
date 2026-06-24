#!/usr/bin/env bash
#
# Muginn tamper-evidence demo — reproducible in ~10 seconds.
#
# Shows the two failure modes Muginn makes loud instead of silent:
#   1. the source behind a stored memory is edited after the fact  -> source-modified
#   2. a citation points at a fact that was never stored           -> not-found
#
# Self-contained: runs under a throwaway HOME, so it never touches your real
# ~/.muginn.db or signing key. Usage:  bash scripts/demo.sh
set -euo pipefail

# ── locate the muginn binary (PATH, then common build dirs) ──────────────────
MUGINN="$(command -v muginn || true)"
if [ -z "$MUGINN" ]; then
  for c in muginn/target/release/muginn muginn/target/debug/muginn \
           target/release/muginn target/debug/muginn; do
    if [ -x "$c" ]; then MUGINN="$(cd "$(dirname "$c")" && pwd)/$(basename "$c")"; break; fi
  done
fi
[ -n "$MUGINN" ] || { echo "muginn not found — run 'cargo build' in muginn/ or install it"; exit 1; }

WORK="$(mktemp -d)"; export HOME="$WORK"          # throwaway key + db
TRANSCRIPT="$WORK/session.jsonl"
trap 'rm -rf "$WORK"' EXIT
step() { printf '\n\033[1m%s\033[0m\n' "$*"; }

# ── a one-line "agent transcript" with a salient decision ────────────────────
cat > "$TRANSCRIPT" <<'EOF'
{"uuid":"a1","type":"assistant","message":{"content":[{"type":"text","text":"Decision: use Ed25519 because it is fast and the keys are small."}]}}
EOF

step "1) Ingest — Muginn stores the sentence VERBATIM, Ed25519-signed and byte-addressed."
"$MUGINN" ingest claude_code "$TRANSCRIPT"

step "2) Recall — every atom carries a live verify status."
OUT="$("$MUGINN" recall "Ed25519")"; echo "$OUT"
ID="$(printf '%s' "$OUT" | grep -oE 'verify\[[0-9a-f]+\]' | head -1 | sed -E 's/.*\[([0-9a-f]+)\].*/\1/')"

step "3) Verify by id — the quote still matches its source byte-for-byte:"
printf '   muginn verify %s -> ' "$ID"; "$MUGINN" verify "$ID"

step "4) TAMPER the source — someone quietly edits the memory's origin (fast -> slow):"
cat > "$TRANSCRIPT" <<'EOF'
{"uuid":"a1","type":"assistant","message":{"content":[{"type":"text","text":"Decision: use Ed25519 because it is slow and the keys are small."}]}}
EOF
printf '   muginn verify %s -> ' "$ID"; "$MUGINN" verify "$ID" || true
echo "   ^ caught: the stored quote no longer matches its source. A silent edit cannot hide."

step "5) FABRICATED citation — a hallucinated reference to a fact never stored:"
printf '   muginn verify deadbeef -> '; "$MUGINN" verify deadbeef || true
echo "   ^ you cannot pass off a citation to a fact that does not exist."

step "Done. A memory store that keeps PARAPHRASED facts can't tell you any of this happened."
echo "Honest scope: this catches post-hoc tampering + fabricated citations — not poison that"
echo "was already in the transcript at ingest time. See the threat-model table in the README."
