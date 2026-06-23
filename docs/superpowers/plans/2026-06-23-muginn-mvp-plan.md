# Muginn — MASTER Implementation Plan

> **For the executing agent (incl. subagents / lower-effort Opus):** Build top-to-bottom. Phase 0 tasks are a faithful Rust port of the already-verified `provmem` Python MVP (32 passing tests) — type the code, run the test command, confirm expected output, commit. Phases 1–5 are *implement-to-spec*: each task gives the file, public interface, the test contract, and acceptance criteria; write the code to satisfy the test, then commit.
>
> **Golden rules:** (1) TDD — write/keep the test, make it pass. (2) Run the test command; it must pass before you commit. (3) If Phase-0 code fails to compile, fix the typo / reconcile crate-version API drift (`cargo build` is truth) — do not redesign the semantics. (4) No network/cloud calls in Layer 0 (grounding/verify/store) or in any test. (5) No "Co-Authored-By" lines in commits. (6) Match the verified Python semantics in `src/provmem/` — it is the executable reference spec.

**Project:** Muginn — verifiable memory for AI agents. Crate/binary `muginn`. Repo `github.com/PraneeshRV/muginn`.
**Design:** see `docs/superpowers/specs/2026-06-23-verifiable-compiled-memory-design.md`.
**Reference impl (semantics ground truth):** `src/provmem/*.py` + `tests/test_*.py` in this repo.

---

## Canonical contracts (Rust — every task obeys; do not rename)

- `Turn { agent, session_id, turn_id, role, text, native_path, turn_sha256 }` — `turn_sha256 = sha256_hex(text)` per-turn.
- `Citation { agent, native_path, session_id, turn_id, span: (usize, usize), turn_sha256 }` — `quote == &turn.text.as_bytes()[start..end]` decoded UTF-8.
- `Atom { atom_id, quote, citation, content_hash, signature, pubkey, prev_atom_id, topic_key, superseded_by, stale, tags, created_at }`.
- `content_hash = sha256_hex(canonical_json({"citation": citation_map, "quote": quote}))` (hex). Canonical JSON = sorted keys, compact separators.
- `atom_id = sha256_hex(content_hash + pubkey_hex)`.
- `signature = ed25519_sign(priv, content_hash.as_bytes())` (hex).
- `prev_atom_id` = previous stored `atom_id` for the same `session_id`, else `""`.
- `topic_key` = first 4 alphanumeric tokens of quote, lowercased, joined by `-`.
- verify statuses (exact strings): `ok | bad-signature | source-missing | turn-missing | source-modified | span-mismatch`.

> **Naming map from the Python reference:** `Fact → Atom`, `FactSource → Citation`, `fact_id → atom_id`, `prev_fact_id → prev_atom_id`. Field semantics are identical; only names change.

---

## Cargo workspace layout

```
muginn/
  Cargo.toml                 # [workspace]
  crates/
    core/      # types: Turn, Citation, Atom, Page  (no deps beyond serde)
    crypto/    # sha256, canonical_json, ed25519 sign/verify, content_hash, atom_id
    adapters/  # claude_code (P0); codex, cursor, chatgpt (P3)
    select/    # salience spans + topic_key
    store/     # rusqlite + FTS5 + (sqlite-vec P-later); verified write, hash chain, staleness
    verify/    # re-open source, byte-compare, status enum
    render/    # markdown cards (P0); obsidian vault renderer lives in `vault` (P1)
    vault/     # P1: Obsidian renderer (frontmatter, wikilinks, stale/diff)
    compile/   # P2: compile-not-retrieve + citation enforcement
    server/    # P3: axum + rmcp MCP (stdio/HTTP)
    cli/       # clap; bin name `muginn`
  src/provmem/ # (kept) Python reference — do not delete until Phase 4 green
```

Root `Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
edition = "2021"
license = "Apache-2.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
hex = "0.4"
ed25519-dalek = "2"
rand_core = { version = "0.6", features = ["getrandom"] }
rusqlite = { version = "0.31", features = ["bundled"] }   # bundled SQLite w/ FTS5
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "1"
regex = "1"
chrono = "0.4"
```

> **Crypto API note (ed25519-dalek v2):** `SigningKey::generate(&mut OsRng)`, `.to_bytes()/.from_bytes(&[u8;32])`, `.verifying_key()`, `.sign(msg)`, `VerifyingKey::verify(msg, &sig)`. Reconcile `rand_core`/`OsRng` versions with `cargo build`.

---

# PHASE 0 — Port the verified core (full target code)

Acceptance: `cargo test` green across the workspace; `muginn ingest claude_code <file>` writes a SQLite DB; `muginn recall <q>` prints cards + verify status. Mirrors the 32 Python tests.

### Task 0.0 — Workspace scaffold

- [ ] Create root `Cargo.toml` (above) + the 8 Phase-0 crate dirs each with a stub `Cargo.toml` + `src/lib.rs` (empty `pub fn _stub() {}` to start). `cli` is `src/main.rs` with `fn main() {}`, bin name `muginn`.
- [ ] `cargo build` succeeds (empty).
- [ ] Commit: `chore: muginn rust workspace scaffold`

### Task 0.1 — core types  (`crates/core`)

`crates/core/Cargo.toml` deps: `serde`, `serde_json`.

```rust
// crates/core/src/lib.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Turn {
    pub agent: String,
    pub session_id: String,
    pub turn_id: String,
    pub role: String,
    pub text: String,
    pub native_path: String,
    pub turn_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Citation {
    pub agent: String,
    pub native_path: String,
    pub session_id: String,
    pub turn_id: String,
    pub span: (usize, usize),
    pub turn_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Atom {
    pub atom_id: String,
    pub quote: String,
    pub citation: Citation,
    pub content_hash: String,
    pub signature: String,
    pub pubkey: String,
    pub prev_atom_id: String,
    pub topic_key: String,
    pub superseded_by: String,
    pub stale: bool,
    pub tags: Vec<String>,
    pub created_at: String,
}
```

Test (`crates/core/src/lib.rs` `#[cfg(test)]` or `tests/`):
```rust
#[test]
fn citation_span_is_byte_pair() {
    let c = Citation { agent:"claude_code".into(), native_path:"/x".into(),
        session_id:"s1".into(), turn_id:"t1".into(), span:(0,5), turn_sha256:"sha".into() };
    assert_eq!(c.span, (0,5));
}
#[test]
fn turn_slice_decodes() {
    let t = "hello world";
    assert_eq!(std::str::from_utf8(&t.as_bytes()[0..5]).unwrap(), "hello");
}
```
- [ ] `cargo test -p muginn-core` → green. Commit: `feat(core): Turn/Citation/Atom types`

### Task 0.2 — crypto  (`crates/crypto`)

Deps: `sha2`, `hex`, `serde_json`, `ed25519-dalek`, `rand_core`.

```rust
// crates/crypto/src/lib.rs
use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey, Signature};
use sha2::{Digest, Sha256};

pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

/// Deterministic JSON: sorted keys (serde_json::Value object is BTreeMap-backed),
/// compact separators. Stable for hashing.
pub fn canonical_json(v: &serde_json::Value) -> String {
    serde_json::to_string(v).expect("canonical_json")
}

pub fn new_keypair() -> (String, String) {
    use rand_core::OsRng;
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    (hex::encode(sk.to_bytes()), hex::encode(pk.to_bytes()))
}

pub fn sign(priv_hex: &str, message: &str) -> String {
    let bytes: [u8; 32] = hex::decode(priv_hex).unwrap().try_into().unwrap();
    let sk = SigningKey::from_bytes(&bytes);
    hex::encode(sk.sign(message.as_bytes()).to_bytes())
}

pub fn verify_sig(pub_hex: &str, message: &str, sig_hex: &str) -> bool {
    let pk_bytes: [u8; 32] = match hex::decode(pub_hex).ok().and_then(|b| b.try_into().ok()) {
        Some(b) => b, None => return false,
    };
    let sig_bytes: [u8; 64] = match hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok()) {
        Some(b) => b, None => return false,
    };
    let pk = match VerifyingKey::from_bytes(&pk_bytes) { Ok(p) => p, Err(_) => return false };
    pk.verify(message.as_bytes(), &Signature::from_bytes(&sig_bytes)).is_ok()
}

pub fn content_hash(quote: &str, citation: &serde_json::Value) -> String {
    let payload = serde_json::json!({ "citation": citation, "quote": quote });
    sha256_hex(&canonical_json(&payload))
}

pub fn atom_id(content_hash_hex: &str, pubkey_hex: &str) -> String {
    sha256_hex(&format!("{content_hash_hex}{pubkey_hex}"))
}
```

Tests (mirror Python `test_crypto.py`):
```rust
#[test] fn sign_verify_roundtrip() {
    let (p, k) = new_keypair();
    let ch = content_hash("hello", &serde_json::json!({"span":[0,5]}));
    assert!(verify_sig(&k, &ch, &sign(&p, &ch)));
}
#[test] fn verify_rejects_tampered() {
    let (p, k) = new_keypair();
    let sig = sign(&p, &content_hash("hello", &serde_json::json!({"span":[0,5]})));
    assert!(!verify_sig(&k, &content_hash("HELLO", &serde_json::json!({"span":[0,5]})), &sig));
}
#[test] fn atom_id_changes_with_pubkey() {
    let ch = content_hash("hello", &serde_json::json!({"span":[0,5]}));
    assert_ne!(atom_id(&ch, "pkA"), atom_id(&ch, "pkB"));
}
```
- [ ] `cargo test -p muginn-crypto` → green. Commit: `feat(crypto): ed25519 sign, content hash, atom identity`

> **Canonical-JSON caveat:** `serde_json::Value` objects use BTreeMap → keys serialize sorted; default `to_string` is compact (`,`/`:`). This matches Python `json.dumps(sort_keys=True, separators=(",",":"))`. Cross-impl interop with the Python DB is NOT required (clean rewrite), only internal consistency.

### Task 0.3 — claude_code adapter  (`crates/adapters`)

Deps: `muginn-core`, `muginn-crypto`, `serde_json`. Port of `src/provmem/adapters/claude_code.py`.

```rust
// crates/adapters/src/claude_code.rs
use muginn_core::Turn;
use muginn_crypto::sha256_hex;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const AGENT: &str = "claude_code";

fn flatten(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() { return s.to_string(); }
    if let Some(arr) = content.as_array() {
        return arr.iter().filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str())).collect();
    }
    String::new()
}

pub fn iter_turns(path: &str) -> Vec<Turn> {
    let session_id = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let f = match std::fs::File::open(path) { Ok(f) => f, Err(_) => return vec![] };
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().flatten() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let obj: serde_json::Value = match serde_json::from_str(line) { Ok(v) => v, Err(_) => continue };
        let content = obj.get("message").and_then(|m| m.get("content")).cloned().unwrap_or(serde_json::Value::Null);
        let text = flatten(&content);
        if text.is_empty() { continue; }
        out.push(Turn {
            agent: AGENT.into(),
            session_id: session_id.clone(),
            turn_id: obj.get("uuid").and_then(|u| u.as_str()).unwrap_or("").to_string(),
            role: obj.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            turn_sha256: sha256_hex(&text),
            text,
            native_path: path.to_string(),
        });
    }
    out
}
```
Fixture `crates/adapters/tests/fixtures/claude_code/sample.jsonl` (exact 3 lines — copy from `tests/fixtures/claude_code/sample.jsonl`):
```
{"uuid":"u1","type":"user","message":{"content":"fix the auth bug"}}
{"uuid":"a1","type":"assistant","message":{"content":[{"type":"text","text":"Decision: use Ed25519 because it is fast and small."}]}}
{"uuid":"u2","type":"user","message":{"content":"sounds good"}}
```
Test (mirror `test_adapters.py`): parse → ids `["u1","a1","u2"]`, `turns[1].role=="assistant"`, `"Ed25519"` in `turns[1].text`, `turn_sha256 == sha256_hex(text)`.
- [ ] `cargo test -p muginn-adapters` → green. Commit: `feat(adapters): claude_code transcript adapter`

### Task 0.4 — select + topic_key  (`crates/select`)

Deps: `muginn-core`, `regex`. Port of `src/provmem/select.py` (byte-offset spans, salience regex, topic_key). **Critical:** spans are UTF-8 byte offsets; compute by encoding the char-prefix length, exactly as Python does, so `&text.as_bytes()[start..end]` decodes cleanly (Unicode test must pass).

Salience regex: `(?i)\b(decision|constraint|because|prefer|remember|TODO|FIXME)\b|\b[\w./-]+\.\w+:\d+`. Sentence split: `[^.!?]*[.!?]|[^.!?]+$`.
Public: `pub fn select_spans(turn: &Turn) -> Vec<(usize,usize)>` and `pub fn topic_key(quote: &str) -> String`.

Tests (mirror `test_select.py`, 4 cases): keeps salient only; byte-accurate with `"café note. TODO: add tests here."`; empty when none; `topic_key("Decision: use Ed25519 because it is fast.") == "decision-use-ed25519-because"`.
- [ ] `cargo test -p muginn-select` → green. Commit: `feat(select): salience span selector + topic_key`

### Task 0.5 — store  (`crates/store`)

Deps: `muginn-core`, `muginn-crypto`, `muginn-select`, `rusqlite` (bundled), `serde_json`, `chrono`, `thiserror`. Port of `src/provmem/store.py`.

Schema (verbatim semantics; table `atoms`, FTS5 `atoms_fts`, AFTER INSERT trigger):
```sql
CREATE TABLE IF NOT EXISTS atoms (
  atom_id TEXT PRIMARY KEY, quote TEXT NOT NULL, citation_json TEXT NOT NULL,
  content_hash TEXT NOT NULL, signature TEXT NOT NULL, pubkey TEXT NOT NULL,
  prev_atom_id TEXT NOT NULL, topic_key TEXT NOT NULL,
  superseded_by TEXT NOT NULL DEFAULT '', stale INTEGER NOT NULL DEFAULT 0,
  session_id TEXT NOT NULL, tags_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS atoms_fts USING fts5(quote, content='atoms', content_rowid='rowid');
CREATE TRIGGER IF NOT EXISTS atoms_ai AFTER INSERT ON atoms BEGIN
  INSERT INTO atoms_fts(rowid, quote) VALUES (new.rowid, new.quote);
END;
```
Public API:
- `Store::open(db_path: &str) -> Store`
- `store_atom(&self, turn: &Turn, span: (usize,usize), priv_hex: &str, pub_hex: &str, tags: Vec<String>) -> Result<Atom, SpanMismatch>` — slice `turn.text.as_bytes()[start..end]`, UTF-8 decode (error → `SpanMismatch`); reject empty/whitespace; build `Citation`; `content_hash`/`atom_id`/`sign`; `prev = last_atom_id(session)`; `topic_key`; `_mark_superseded(key, id)` (UPDATE stale=1, superseded_by where same topic, stale=0, id<>); `INSERT OR IGNORE` (14? no — 13 cols here, Phase 0 has no embedding); commit.
- `get(&self, atom_id) -> Option<Atom>`
- `search(&self, query, k, include_stale) -> Vec<Atom>` — FTS5 MATCH join; **wrap execute in a Result and return `vec![]` on FTS5 syntax error** (carry over the fix from `272210a`); `AND stale=0` unless include_stale; `ORDER BY rank LIMIT k`.

Tests (mirror `test_store.py`, 4 cases): store+search roundtrip + signature verifies; hash-chain links same session (`f2.prev_atom_id == f1.atom_id`); rejects empty quote (`SpanMismatch`); newer supersedes older same topic (live=1 "secure", include_stale=2).
- [ ] `cargo test -p muginn-store` → green. Commit: `feat(store): sqlite store, FTS5, hash chain, staleness`

### Task 0.6 — verify  (`crates/verify`)

Deps: `muginn-core`, `muginn-crypto`, `muginn-adapters`. Port of `src/provmem/verify.py`.
`pub fn verify_atom(atom: &Atom) -> &'static str`:
1. `verify_sig(pubkey, content_hash, signature)` false → `"bad-signature"`.
2. source path missing → `"source-missing"`.
3. adapter-by-agent → find turn by `turn_id`; none → `"turn-missing"`.
4. `turn.turn_sha256 != citation.turn_sha256` → `"source-modified"`.
5. slice bytes `[start..end]`, decode lossy; `== atom.quote` ? `"ok"` : `"span-mismatch"`.

Adapter dispatch: `match citation.agent { "claude_code" => adapters::claude_code::iter_turns, ... }`.
Tests (mirror `test_verify.py`, 3 cases): ok; source-missing (point path at `/no/such`); source-modified (copy fixture to tmp, edit the a1 turn `"fast and small"→"slow and small"`, expect `"source-modified"`).
- [ ] `cargo test -p muginn-verify` → green. Commit: `feat(verify): byte-compare verifier with typed statuses`

### Task 0.7 — render + ingest + CLI  (`crates/render`, `crates/cli`)

- `render::render_cards(atoms: &[Atom]) -> String` — `- "<quote>" — <agent>:<session>#<turn> [<atom_id[..8]>]` per line. Test: quote+citation+id8 present; empty → `""`. (mirror `test_render.py`)
- `ingest` (put in `store` or a small `ingest` module): `ingest_file(store, agent, path, priv, pub) -> usize` = for each turn, for each `select_spans`, `store_atom` (skip `SpanMismatch`), count. Test: ≥1 and `"Ed25519"` found. (mirror `test_ingest.py`)
- **E2E test** (mirror `test_e2e_demo.py`): ingest tmp copy → all `ok`; tamper a DIFFERENT turn (`"sounds good"→"sounds great"`) → still all `ok` (no false positive); tamper the SAME turn → some `"source-modified"`.
- `cli` (`muginn`): `clap` subcommands `ingest <agent> <path>` and `recall <query> [-k N]`. Key at `~/.muginn.key` (0600), DB at `~/.muginn.db`. `recall` prints cards + `  verify[<id8>] = <status>` per atom.
- [ ] `cargo test --workspace` → all green (≈ the 32 Python tests, names mapped). Smoke: `cargo run -p muginn-cli -- ingest claude_code crates/adapters/tests/fixtures/claude_code/sample.jsonl` → `ingested 1 atoms ...`.
- [ ] Commit: `feat: ingest pipeline + muginn CLI + e2e tamper demo`

**Phase 0 DONE when:** `cargo test --workspace` green and the tamper demo passes. This re-establishes the prov-memory thesis in Rust.

---

# PHASE 1 — Obsidian vault renderer  (`crates/vault`)

Goal: render atoms (and later pages) as a real Obsidian vault; supersession is visible, not destructive. *Demo: point at `~/.claude`, open vault in Obsidian, graph view blooms.*

Vault layout (per `<root>/vault/<workspace>/<project>/`): `atoms/<atom_id8>.md`, `_stale/<atom_id8>.md`, later `pages/<topic>.md`.

### Task 1.1 — atom → note
- File: `crates/vault/src/lib.rs`. Public: `pub fn write_atom_note(root: &Path, workspace: &str, project: &str, atom: &Atom) -> PathBuf`.
- Note body = frontmatter (YAML) + the verbatim quote + a citation line.
  ```
  ---
  atom_id: <full>
  agent: claude_code
  session: <id>
  turn: <id>
  span: [start, end]
  turn_sha256: <hex>
  topic_key: <key>
  stale: false
  created_at: <iso>
  ---
  > "<quote>"

  Source: `<agent>:<session>#<turn>` bytes [start,end]
  ```
- Test contract: parse the written file's frontmatter back → fields equal the atom's; body contains the quote.
- Acceptance: file exists at `vault/<ws>/<proj>/atoms/<id8>.md`.
- Commit: `feat(vault): render atom as obsidian note with frontmatter`

### Task 1.2 — project/workspace resolution
- Public: `pub fn resolve_project(citation: &Citation) -> (String /*workspace*/, String /*project*/)`.
- Rule: derive from the transcript path / agent cwd-slug (e.g. Claude Code `~/.claude/projects/<slug>/…` → slug = project). Workspace = git root basename if resolvable else `"default"`. Allow `muginn.toml` `[projects]` overrides.
- Test contract: a sample Claude Code path maps to its slug; override file changes it.
- Commit: `feat(vault): deterministic project/workspace resolution`

### Task 1.3 — supersession = greyed + diff, non-destructive
- On staleness (atom.stale or superseded_by set), write to `_stale/` instead of `atoms/`, frontmatter `stale: true`, `superseded_by: "[[<new id8>]]"`, and append a rendered unified diff (old quote vs new). Never delete.
- Public: `pub fn write_stale_note(root, ws, proj, old: &Atom, new: &Atom) -> PathBuf`.
- Test contract: stale note has `stale: true`, a `superseded_by` wikilink to the new id8, and a diff block.
- Acceptance: live atom in `atoms/`, stale one in `_stale/`, wikilink resolves by filename.
- Commit: `feat(vault): non-destructive supersession with diff`

### Task 1.4 — vault sync command
- CLI: `muginn sync [--root <vault>]` = re-render all non-stale atoms to notes + all superseded to `_stale/`, idempotently (stable filenames = atom_id8). Optional `git2` commit of the vault for time-travel.
- Test contract: running sync twice produces identical tree (idempotent); a new superseding atom moves the old note to `_stale/`.
- Acceptance: open in Obsidian, graph view shows atoms linked by topic; stale greyed via a CSS snippet shipped in `vault/obsidian-snippet.css`.
- Commit: `feat(cli): muginn sync — render vault, optional git versioning`

**Phase 1 demo:** `muginn ingest … && muginn sync && open vault in Obsidian`.

---

# PHASE 2 — Compile layer + citation enforcement  (`crates/compile`)

Goal: readable compiled pages whose every claim cites a verifiable atom; uncited/unverifiable claims are quarantined. The compiler is **untrusted**. *Demo: a "Decisions" page where each line links to a source span; inject a fake fact → it's quarantined on camera.*

### Task 2.1 — compile interface (model-agnostic, local-default)
- Public trait `Compiler { fn compile(&self, topic: &str, atoms: &[Atom]) -> CompiledDraft; }` where `CompiledDraft { sentences: Vec<Sentence> }`, `Sentence { text: String, cited_atom_ids: Vec<String> }`.
- Impls: `LocalCompiler` (POST to an Ollama/llama.cpp endpoint, env `MUGINN_COMPILE_URL`); `NullCompiler` (no model → returns each atom as its own sentence citing itself, so Phase 2 works with zero LLM). Default: local if reachable, else Null.
- The prompt instructs the model: "Write prose using ONLY these atoms; tag each sentence with the atom-ids it derives from; do not introduce facts not in the atoms."
- Test contract (uses `NullCompiler`, no network): compiling N atoms yields N sentences, each citing exactly its own atom.
- Commit: `feat(compile): model-agnostic compiler trait + Null/Local impls`

### Task 2.2 — citation enforcement (the moat)
- Public: `pub fn enforce(draft: &CompiledDraft, store: &Store) -> EnforcedPage` where each sentence is classified: `Verified` (all cited atoms exist AND `verify_atom == "ok"`), or `Quarantined { reason }` (no citation, missing atom, or non-`ok` verify).
- `EnforcedPage { verified: Vec<Sentence>, quarantined: Vec<(Sentence, String)>, coverage: f32 }` where `coverage = verified / total`.
- Test contract: a draft with one fabricated sentence (cites a non-existent atom_id) → that sentence is `Quarantined`, coverage < 1.0; all-real draft → coverage == 1.0.
- **Poison test:** inject a sentence citing an atom whose stored quote was byte-tampered → quarantined (verify ≠ ok). This is the headline guarantee.
- Commit: `feat(compile): citation enforcement — quarantine unverifiable claims`

### Task 2.3 — page → obsidian note + incremental recompile
- `vault::write_page_note`: render `pages/<topic>.md` — verified sentences with inline `[[<atom id8>]]` links; a `> [!warning] Unverified` callout listing quarantined sentences; frontmatter `coverage`, `compiler_model`, `compiled_at`.
- Incremental: when an atom goes stale, mark pages citing it `dirty`; `muginn compile --dirty` re-compiles only those; vault diff shows the change.
- Test contract: page note contains wikilinks for verified lines and a warning callout for quarantined; staleness flips a citing page to dirty.
- Commit: `feat(compile+vault): compiled pages with inline citations + incremental recompile`

**Phase 2 demo:** compiled Decisions page, every line click-through to source; live poison-injection quarantined.

---

# PHASE 3 — MCP server + multi-agent  (`crates/server`, `crates/adapters`)

Goal: agents read the verifiable vault over MCP; ingest from 4 agents. *Demo: Claude Code + Codex sharing one vault.*

### Task 3.1 — more adapters
- Port `codex` (only `type=="message"` events; `input_text`/`output_text`), add `cursor` and `chatgpt` adapters (discover real formats; fixture-test each; keep `iter_turns -> Vec<Turn>` + byte/per-turn-hash discipline). Credit cass in comments for format reference.
- Test contract: each adapter parses its fixture to expected turn ids; non-message events skipped.
- Commit: `feat(adapters): codex, cursor, chatgpt`

### Task 3.2 — MCP server (rmcp)
- Deps: `tokio`, `axum`, `rmcp` (official Rust MCP SDK). Tools: `recall(query,k)`, `verify(id)`, `cite(id)`, `ingest(agent,path)`, `compile(topic)`.
- Mirror the Python `make_handlers` separation: pure handler fns (testable without transport) + a thin `rmcp` binding. stdio + HTTP transports.
- Test contract: pure handlers — `recall` returns cards, `verify` returns status or `not-found`, `cite` returns citation JSON; no transport needed.
- Commit: `feat(server): rmcp MCP server (recall/verify/cite/ingest/compile)`

### Task 3.3 — config + multi-agent ingest
- `muginn.toml`: agent transcript roots, vault root, compile endpoint, project overrides. `muginn ingest --all` walks configured roots across agents into one vault.
- Test contract: config parse; `--all` over a fixture dir ingests from ≥2 agents into one DB/vault.
- Commit: `feat(cli): muginn.toml config + multi-agent ingest`

**Phase 3 demo:** register `muginn` as MCP server in Claude Code AND Codex; both recall/verify the same vault.

---

# PHASE 4 — Eval + release  (`crates/eval` or `eval/`)

Goal: the measurable, novel claims + shippable binaries.

### Task 4.1 — provenance + poison eval
- `provenance_coverage`: over a labeled compiled corpus, % claims with verifying citation (target >0.95).
- `poison_rejection`: inject N fabricated facts into compile input, % quarantined (target 1.0 by construction).
- `staleness_precision_recall`: labeled supersession set.
- Port the selector `recall/fp_rate` harness + the format token benchmark (md-cards vs json) from `eval/`.
- Test contract: harness runs offline; coverage ≥0.95, poison_rejection == 1.0 on fixtures.
- Commit: `feat(eval): provenance coverage + poison rejection + staleness metrics`

### Task 4.2 — LongMemEval / LoCoMo parity harness (optional, offline subset)
- Adapter to run a small offline subset for parity reporting (full runs need datasets; keep network-free in CI).
- Commit: `feat(eval): LongMemEval/LoCoMo parity harness (offline subset)`

### Task 4.3 — release
- `cargo-dist` config: macOS/Linux/Windows binaries, AUR, Homebrew tap. README with the headline claim ("100% poison rejection, >95% provenance, 0 cloud calls by default"), quickstart, MCP config blocks, the demo GIF, credited influences (ai-memory, basic-memory, A-MEM, Karpathy, cass).
- Commit: `chore(release): cargo-dist packaging + README`; tag `v0.1.0`.
- Retire the Python reference: once `cargo test --workspace` ≥ the 32-test parity, move `src/provmem` + `tests/` to `reference/` (or delete) in a dedicated commit.

---

# PHASE 5 (post-launch) — Obsidian community plugin  (`plugin/`, TypeScript)

Lighter spec; only after core lands. In-vault UX over the same vault + MCP/HTTP:
- Live verify badge per atom note (calls `muginn verify`); click-to-source opens the native transcript at the byte span; "recompile dirty" button; coverage indicator on pages; stale notes styled via the shipped CSS snippet.
- Distribute via Obsidian community store (the star funnel). Tasks: scaffold (esbuild), settings (muginn endpoint), verify-badge view, source-jump command, submit to community plugins.
- Commit cadence: per feature; tag plugin releases independently.

---

## Execution guidance (for the human dispatching agents)

- **Parallelizable now (independent crates, no shared files):** Tasks 0.1 (core), 0.2 (crypto) can go to two subagents at once; 0.3/0.4 depend only on 0.1+0.2; 0.5 depends on 0.1/0.2/0.4; 0.6 on 0.3/0.5; 0.7 last. Give each subagent ONE task, its code block, its test, and "run `cargo test -p <crate>`, commit on green."
- **Lower-effort Opus is fine for Phase 0** — it's a faithful port with code + tests provided; the model's job is transcription + making `cargo test` pass. Hand it the canonical-contracts block + one task at a time.
- **Phases 1–5 need higher effort** (new design): give the task spec, have the agent write the test first (TDD), then implement, then `cargo test -p <crate>`.
- **Always:** worktree isolation per subagent, run a `caveman:cavecrew-reviewer` pass on each phase's diff before merging (it found the 2 real bugs last time), and re-run `cargo test --workspace` after every merge.
- **Reviewer note:** the canonical-JSON sorting, the byte-offset span math (Unicode), the FTS5 syntax-error guard, and the per-turn (not per-file) hashing are the four spots most likely to harbor a port bug — point reviewers at them.

---

## Coverage check (plan ↔ design)

- Two-layer verifiable compiled memory → P0 (atoms/Layer0) + P2 (pages/Layer1 + enforcement). ✓
- Anti-poisoning (quote must exist; quarantine unverifiable) → P0 `SpanMismatch` + P2 enforce + poison test. ✓
- Self-healing/staleness (non-destructive supersession + diff) → P0 staleness + P1 `_stale`/diff + P2 incremental recompile. ✓
- Obsidian-native, atom-per-note, pages default, graph scoped → P1. ✓
- Cross-agent (own thin adapters, start with 4) → P0 (CC) + P3 (codex/cursor/chatgpt). ✓
- BYO local-default compile, never bundled, 0-cloud-by-default → P2 Null/Local compiler. ✓
- Deterministic project identity → P1.2. ✓
- Eval on new axis (provenance/poison/staleness) + parity + format benchmark → P4. ✓
- MCP recall/verify/cite/ingest/compile → P3. ✓
- Single-binary distribution → P4.3 cargo-dist. ✓
- Plugin star-funnel → P5. ✓
```
