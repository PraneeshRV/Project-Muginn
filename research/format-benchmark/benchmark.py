"""Memory-format token/space benchmark.

Encodes the SAME canonical memory facts into 5 formats and measures tokens
(tiktoken cl100k_base proxy) + bytes. Answers: which format stores a memory
fact in the fewest tokens for context injection?

Caveat: cl100k_base (OpenAI) != Anthropic tokenizer. Numbers are RELATIVE,
for comparing formats, not absolute Claude token counts.
"""
from __future__ import annotations
import json, os

try:
    import tiktoken
    _ENC = tiktoken.get_encoding("cl100k_base")
    def ntok(s: str) -> int: return len(_ENC.encode(s))
    TOK_METHOD = "tiktoken/cl100k_base"
except Exception:
    def ntok(s: str) -> int: return max(1, round(len(s) / 4))
    TOK_METHOD = "fallback chars/4"

# 30 canonical facts a coding agent would store. Mix of categories; some carry
# a 'why' (rationale) which lossy formats (KG triples) cannot fully preserve.
FACTS = [
    {"id": "f01", "type": "pref", "subject": "user", "rel": "prefers", "object": "no Co-Authored-By lines in git commits"},
    {"id": "f02", "type": "pref", "subject": "user", "rel": "prefers", "object": "caveman-terse responses to save tokens"},
    {"id": "f03", "type": "pref", "subject": "user", "rel": "uses", "object": "Arch Linux with zsh shell"},
    {"id": "f04", "type": "pref", "subject": "user", "rel": "role", "object": "AI-security researcher targeting top-tier paper + job portfolio"},
    {"id": "f05", "type": "constraint", "subject": "project Securin", "rel": "constraint", "object": "2-month timeline, team of 4, no confidential-compute hardware", "why": "scopes attestation to key-signed, defers zkML to toy-scale"},
    {"id": "f06", "type": "constraint", "subject": "prov-memory", "rel": "constraint", "object": "fully local, no cloud embedding API", "why": "airgap requirement; cloud embeddings would exfiltrate chat text"},
    {"id": "f07", "type": "constraint", "subject": "memory tier", "rel": "must", "object": "inject distilled cards not verbatim transcripts", "why": "verbatim burns context tokens; fetch on demand instead"},
    {"id": "f08", "type": "decision", "subject": "team", "rel": "chose", "object": "Model-Attested Authority (PACT-MA) as paper novelty", "why": "agentic security is open hiring lane; reuses existing proof moat"},
    {"id": "f09", "type": "decision", "subject": "team", "rel": "chose", "object": "markdown frontmatter cards as primary memory format", "why": "token-lean, human+AI readable, git-diffable"},
    {"id": "f10", "type": "decision", "subject": "team", "rel": "rejected", "object": "rebuilding local sqlite-vec memory layer", "why": "commodity; AIngram/Memory-Vault already do it"},
    {"id": "f11", "type": "decision", "subject": "team", "rel": "anchored novelty on", "object": "verifiable span-level provenance surviving LLM extraction", "why": "only unoccupied gap after competitor audit"},
    {"id": "f12", "type": "code", "subject": "schema.py", "rel": "defines", "object": "memory_entries table at line 24"},
    {"id": "f13", "type": "code", "subject": "store.py", "rel": "signs entries at line 184 via", "object": "session.sign(entry_id) Ed25519"},
    {"id": "f14", "type": "code", "subject": "cass connectors dir", "rel": "contains", "object": "claude_code.rs codex.rs antigravity.rs adapters"},
    {"id": "f15", "type": "code", "subject": "trust/signing.py", "rel": "implements", "object": "sign_entry returning 128-char hex Ed25519 signature at line 38"},
    {"id": "f16", "type": "rel", "subject": "AIngram_boz", "rel": "is closest competitor to", "object": "prov-memory project"},
    {"id": "f17", "type": "rel", "subject": "cass", "rel": "ingests transcripts of", "object": "22 coding agents"},
    {"id": "f18", "type": "rel", "subject": "Ed25519 signature", "rel": "binds", "object": "content-addressed entry_id to agent pubkey"},
    {"id": "f19", "type": "rel", "subject": "LLM extraction", "rel": "severs", "object": "exact source span provenance", "why": "paraphrasing loses char offsets into original transcript"},
    {"id": "f20", "type": "rel", "subject": "memory poisoning", "rel": "is", "object": "OWASP top agentic risk 2026"},
    {"id": "f21", "type": "fact", "subject": "PAM protocol", "rel": "authored by", "object": "Microsoft, arxiv 2605.11032"},
    {"id": "f22", "type": "fact", "subject": "OKF", "rel": "published by", "object": "Google, markdown+YAML frontmatter standard"},
    {"id": "f23", "type": "fact", "subject": "Mem0", "rel": "has", "object": "~48K GitHub stars, leads community adoption"},
    {"id": "f24", "type": "fact", "subject": "Zep/Graphiti", "rel": "scores", "object": "63.8% on LongMemEval vs Mem0 49.0%"},
    {"id": "f25", "type": "fact", "subject": "TokenMizer", "rel": "achieves", "object": "47.3% token reduction via graph session memory"},
    {"id": "f26", "type": "decision", "subject": "storage tier", "rel": "uses", "object": "sqlite + FTS5 + sqlite-vec", "why": "single file, indexed, never fully loaded into context"},
    {"id": "f27", "type": "constraint", "subject": "embeddings", "rel": "must run via", "object": "ollama or onnx local model", "why": "no api_key, no external POST = real airgap test"},
    {"id": "f28", "type": "pref", "subject": "user", "rel": "wants", "object": "secure usable-by-everyone open-source project"},
    {"id": "f29", "type": "rel", "subject": "prov-memory", "rel": "distinct from but reuses idea of", "object": "Securin PACT-MA paper"},
    {"id": "f30", "type": "decision", "subject": "subagents", "rel": "unusable because", "object": "this environment denies Read/Bash/Write to spawned agents", "why": "all audit+benchmark work must run in main thread"},
]


def enc_json_pretty(facts): return json.dumps(facts, indent=2)
def enc_json_min(facts): return json.dumps(facts, separators=(",", ":"))

def enc_yaml(facts):
    out = []
    for f in facts:
        out.append(f"- id: {f['id']}")
        for k, v in f.items():
            if k == "id": continue
            out.append(f"  {k}: {v}")
    return "\n".join(out)

def enc_md_cards(facts):
    out = []
    for f in facts:
        line = f"- **{f['subject']}** {f['rel']} {f['object']}"
        if f.get("why"): line += f" — why: {f['why']}"
        out.append(line)
    return "\n".join(out)

def enc_kg_triples(facts):
    # densest, lossy: drops 'why' rationale and type/id metadata
    return "\n".join(f"{f['subject']} | {f['rel']} | {f['object']}" for f in facts)


FORMATS = {
    "json_pretty": enc_json_pretty,
    "json_min": enc_json_min,
    "yaml": enc_yaml,
    "md_cards": enc_md_cards,
    "kg_triples": enc_kg_triples,
}

N = len(FACTS)


def run():
    rows = []
    for name, fn in FORMATS.items():
        s = fn(FACTS)
        rows.append((name, ntok(s), len(s.encode("utf-8")), s))
    base = next(t for n, t, *_ in rows if n == "json_pretty")
    rows.sort(key=lambda r: r[1])

    samples_dir = os.path.join(os.path.dirname(__file__), "samples")
    os.makedirs(samples_dir, exist_ok=True)
    for name, _, _, s in rows:
        with open(os.path.join(samples_dir, f"{name}.txt"), "w") as fh:
            fh.write(s)

    lines = []
    lines.append("# Memory-Format Token/Space Benchmark — RESULTS\n")
    lines.append(f"- Facts: **{N}** canonical memory facts (prefs, constraints, decisions+why, code-locations, relations, facts).")
    lines.append(f"- Tokenizer: **{TOK_METHOD}**. Caveat: cl100k_base ≠ Anthropic tokenizer — numbers are RELATIVE (format-vs-format), not absolute Claude counts.")
    lines.append(f"- Lossy note: `kg_triples` DROPS the `why` rationale and id/type metadata — densest but lossy.\n")
    lines.append("## Ranked (fewest tokens first)\n")
    lines.append("| format | tokens | tokens/fact | bytes | % token savings vs json_pretty |")
    lines.append("|---|---|---|---|---|")
    for name, tok, byts, _ in rows:
        sav = round(100 * (base - tok) / base)
        lines.append(f"| {name} | {tok} | {tok/N:.1f} | {byts} | {sav}% |")
    winner = rows[0][0]
    lines.append(f"\n**Densest:** `{winner}`. **Best lossless (keeps rationale):** `md_cards`.\n")
    lines.append("## Readability samples (first 3 facts)\n")
    for name, fn in FORMATS.items():
        s = fn(FACTS[:3])
        lines.append(f"### {name}\n```\n{s}\n```\n")
    report = "\n".join(lines)
    with open(os.path.join(os.path.dirname(__file__), "RESULTS.md"), "w") as fh:
        fh.write(report)

    print(f"tokenizer: {TOK_METHOD}")
    print(f"{'format':12} {'tokens':>7} {'tok/fact':>9} {'bytes':>7} {'sav%':>6}")
    for name, tok, byts, _ in rows:
        sav = round(100 * (base - tok) / base)
        print(f"{name:12} {tok:7d} {tok/N:9.1f} {byts:7d} {sav:5d}%")


if __name__ == "__main__":
    run()
