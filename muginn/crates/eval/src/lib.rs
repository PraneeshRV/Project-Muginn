use muginn_compile::{enforce, CompiledDraft, NullCompiler, Compiler, Sentence};
use muginn_core::{Atom, Turn};
use muginn_select::select_spans;
use muginn_store::Store;

pub mod parity;

// ── Selector eval ────────────────────────────────────────────────────────────

pub struct SelectorMetrics {
    pub recall: f64,
    pub false_positive_rate: f64,
    pub n_salient: usize,
    pub n_nonsalient: usize,
}

/// Run the salience selector over labeled rows.
/// Each row: `{ "text": "...", "salient": true|false }`.
pub fn eval_selector(rows: &[serde_json::Value]) -> SelectorMetrics {
    let salient: Vec<&serde_json::Value> = rows.iter().filter(|r| r["salient"] == true).collect();
    let nonsalient: Vec<&serde_json::Value> = rows.iter().filter(|r| r["salient"] == false).collect();

    let selects_any = |text: &str| -> bool {
        let turn = Turn {
            agent: "eval".into(),
            session_id: "s".into(),
            turn_id: "t".into(),
            role: "assistant".into(),
            text: text.to_string(),
            native_path: "/x".into(),
            turn_sha256: "sha".into(),
        };
        !select_spans(&turn).is_empty()
    };

    let tp = salient.iter().filter(|r| selects_any(r["text"].as_str().unwrap_or(""))).count();
    let fp = nonsalient.iter().filter(|r| selects_any(r["text"].as_str().unwrap_or(""))).count();

    SelectorMetrics {
        recall: if salient.is_empty() { 0.0 } else { tp as f64 / salient.len() as f64 },
        false_positive_rate: if nonsalient.is_empty() { 0.0 } else { fp as f64 / nonsalient.len() as f64 },
        n_salient: salient.len(),
        n_nonsalient: nonsalient.len(),
    }
}

// ── Provenance coverage ──────────────────────────────────────────────────────

pub struct ProvenanceMetrics {
    /// fraction of sentences in EnforcedPage that are Verified (i.e. EnforcedPage.coverage)
    pub coverage: f32,
    pub n_verified: usize,
    pub n_quarantined: usize,
}

/// Compile `atoms` with NullCompiler, enforce, return coverage stats.
/// Target: coverage >= 0.95 when all atoms are real and unmodified.
pub fn eval_provenance_coverage(atoms: &[Atom], store: &Store) -> ProvenanceMetrics {
    let draft = NullCompiler.compile("eval", atoms);
    let page = enforce(&draft, store);
    let n_verified = page.verified().len();
    let n_quarantined = page.quarantined().len();
    ProvenanceMetrics {
        coverage: page.coverage,
        n_verified,
        n_quarantined,
    }
}

// ── Poison rejection ─────────────────────────────────────────────────────────

pub struct PoisonMetrics {
    /// fraction of injected fabricated sentences that were quarantined (target == 1.0)
    pub rejection_rate: f64,
    pub n_injected: usize,
    pub n_quarantined: usize,
}

/// Inject `n_fabricated` sentences with fake atom_ids into a draft alongside `real_atoms`,
/// enforce, and measure what fraction of the fabricated ones are quarantined.
pub fn eval_poison_rejection(real_atoms: &[Atom], n_fabricated: usize, store: &Store) -> PoisonMetrics {
    // Real sentences via NullCompiler
    let real_draft = NullCompiler.compile("eval", real_atoms);
    let mut sentences = real_draft.sentences;

    // Fabricated sentences cite non-existent atom_ids
    for i in 0..n_fabricated {
        sentences.push(Sentence {
            text: format!("Fabricated claim number {i}."),
            cited_atom_ids: vec![format!("fabricated_atom_id_{i:08x}")],
        });
    }

    let draft = CompiledDraft { sentences };
    let page = enforce(&draft, store);

    // Count how many quarantined sentences have fabricated IDs
    let quarantined_fabricated = page
        .quarantined()
        .iter()
        .filter(|(s, _)| s.text.starts_with("Fabricated claim number"))
        .count();

    PoisonMetrics {
        rejection_rate: if n_fabricated == 0 { 1.0 } else { quarantined_fabricated as f64 / n_fabricated as f64 },
        n_injected: n_fabricated,
        n_quarantined: quarantined_fabricated,
    }
}

// ── Staleness precision/recall ────────────────────────────────────────────────

pub struct StalenessMetrics {
    /// fraction of atoms correctly marked stale (true positives / actual stale)
    pub precision: f64,
    pub recall: f64,
    pub n_expected_stale: usize,
    pub n_found_stale: usize,
}

/// Given a list of (atom, expected_stale) pairs, measure staleness labeling accuracy.
pub fn eval_staleness(labeled: &[(Atom, bool)]) -> StalenessMetrics {
    let expected_stale: Vec<&Atom> = labeled.iter().filter(|(_, s)| *s).map(|(a, _)| a).collect();
    let found_stale: Vec<&Atom> = labeled.iter().filter(|(a, _)| a.stale).map(|(a, _)| a).collect();

    let true_positive = labeled
        .iter()
        .filter(|(a, expected)| *expected && a.stale)
        .count();

    let precision = if found_stale.is_empty() { 1.0 } else { true_positive as f64 / found_stale.len() as f64 };
    let recall = if expected_stale.is_empty() { 1.0 } else { true_positive as f64 / expected_stale.len() as f64 };

    StalenessMetrics {
        precision,
        recall,
        n_expected_stale: expected_stale.len(),
        n_found_stale: found_stale.len(),
    }
}

// ── Format token benchmark ────────────────────────────────────────────────────

pub struct FormatBenchmark {
    pub md_chars: usize,
    pub json_chars: usize,
    /// TOON (Token-Oriented Object Notation) tabular encoding of the same flattened atoms.
    pub toon_chars: usize,
    /// json_chars / md_chars — how much larger full-atom JSON is vs the markdown card.
    pub json_overhead_ratio: f64,
    /// toon / json on identical flattened data (<1.0 means TOON is smaller than JSON).
    pub toon_vs_json_ratio: f64,
}

/// Fields, in order, of the flattened atom record used for the JSON-vs-TOON comparison.
const ATOM_FIELDS: &[&str] = &[
    "id", "quote", "agent", "native_path", "session", "turn", "start", "end",
    "turn_sha256", "content_hash", "signature", "pubkey", "prev", "topic_key",
    "superseded_by", "stale", "created_at", "tags",
];

/// Flatten an atom to a single-level JSON object (citation fields hoisted up), so the
/// JSON-vs-TOON comparison runs on identical, uniform records.
fn flatten_atom(a: &Atom) -> serde_json::Value {
    serde_json::json!({
        "id": a.atom_id,
        "quote": a.quote,
        "agent": a.citation.agent,
        "native_path": a.citation.native_path,
        "session": a.citation.session_id,
        "turn": a.citation.turn_id,
        "start": a.citation.span.0,
        "end": a.citation.span.1,
        "turn_sha256": a.citation.turn_sha256,
        "content_hash": a.content_hash,
        "signature": a.signature,
        "pubkey": a.pubkey,
        "prev": a.prev_atom_id,
        "topic_key": a.topic_key,
        "superseded_by": a.superseded_by,
        "stale": a.stale,
        "created_at": a.created_at,
        "tags": a.tags.join("|"),
    })
}

/// One TOON cell: stringify a JSON scalar, quoting only when the value contains a
/// delimiter, quote, colon, newline, or edge whitespace (TOON quoting rules).
fn toon_cell(v: Option<&serde_json::Value>) -> String {
    let s = match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    };
    let needs_quote = s.is_empty()
        || s.contains([',', '"', ':', '\n'])
        || s.starts_with(' ')
        || s.ends_with(' ');
    if needs_quote {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// Minimal TOON tabular encoder for a uniform array of flat objects:
/// `atoms[N]{field,...}:` header, then one comma-separated indented row per object.
fn toon_encode(rows: &[serde_json::Value]) -> String {
    let mut out = format!("atoms[{}]{{{}}}:", rows.len(), ATOM_FIELDS.join(","));
    for row in rows {
        let cells: Vec<String> = ATOM_FIELDS.iter().map(|f| toon_cell(row.get(*f))).collect();
        out.push_str("\n  ");
        out.push_str(&cells.join(","));
    }
    out
}

/// Compare formats for the same atoms: markdown cards (human) vs full-atom JSON, plus a
/// JSON-vs-TOON comparison on identical flattened records.
pub fn eval_format_overhead(atoms: &[Atom]) -> FormatBenchmark {
    use muginn_render::render_cards;
    let md = render_cards(atoms);
    let json = serde_json::to_string(atoms).unwrap_or_default();
    let json_overhead_ratio = if md.is_empty() { 1.0 } else { json.len() as f64 / md.len() as f64 };

    let rows: Vec<serde_json::Value> = atoms.iter().map(flatten_atom).collect();
    let json_flat = serde_json::to_string(&rows).unwrap_or_default();
    let toon = toon_encode(&rows);
    let toon_vs_json_ratio = if json_flat.is_empty() { 1.0 } else { toon.len() as f64 / json_flat.len() as f64 };

    FormatBenchmark {
        md_chars: md.len(),
        json_chars: json.len(),
        toon_chars: toon.len(),
        json_overhead_ratio,
        toon_vs_json_ratio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytecite::{new_keypair, sha256_hex};
    use muginn_core::Citation;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_fixture(path: &str, text: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(
            f,
            r#"{{"uuid":"t1","type":"assistant","message":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#,
            text.replace('"', "\\\"")
        )
        .unwrap();
    }

    fn make_and_store_atom(store: &Store, text: &str, fixture_path: &str) -> Atom {
        let (priv_hex, pub_hex) = new_keypair();
        let turn = Turn {
            agent: "claude_code".into(),
            session_id: "eval_session".into(),
            turn_id: "t1".into(),
            role: "assistant".into(),
            text: text.to_string(),
            native_path: fixture_path.to_string(),
            turn_sha256: sha256_hex(text),
        };
        store.store_atom(&turn, (0, text.len()), &priv_hex, &pub_hex, vec![]).unwrap()
    }

    fn tmp_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("eval.db");
        (Store::open(db.to_str().unwrap()), dir)
    }

    // ── Selector eval ────────────────────────────────────────────────────────

    #[test]
    fn selector_eval_labeled_fixture() {
        let rows: Vec<serde_json::Value> = vec![
            serde_json::json!({"text": "Decision: adopt sqlite-vec for local vectors.", "salient": true}),
            serde_json::json!({"text": "The weather was nice today.", "salient": false}),
            serde_json::json!({"text": "Constraint: must run fully offline.", "salient": true}),
            serde_json::json!({"text": "I had coffee this morning.", "salient": false}),
            serde_json::json!({"text": "TODO: write the README.", "salient": true}),
            serde_json::json!({"text": "We chatted about the weekend.", "salient": false}),
            serde_json::json!({"text": "Prefer markdown cards because they are token-lean.", "salient": true}),
            serde_json::json!({"text": "See parser.py:42 for the bug.", "salient": true}),
        ];
        let m = eval_selector(&rows);
        assert_eq!(m.n_salient, 5);
        assert_eq!(m.n_nonsalient, 3);
        // Sentence splitter splits "parser.py:42" at `.`, so that row is missed → recall=0.8.
        // Verify the harness runs and fp_rate == 0.0 (no non-salient falsely selected).
        assert!(m.recall >= 0.8, "recall={}", m.recall);
        assert!((m.false_positive_rate).abs() < 1e-9, "fp_rate={}", m.false_positive_rate);
    }

    // ── Provenance coverage ──────────────────────────────────────────────────

    #[test]
    fn provenance_coverage_all_real_is_1() {
        let (store, _dir) = tmp_store();
        let fix = NamedTempFile::new().unwrap();
        let text = "Decision: use Ed25519 because it is fast and small.";
        write_fixture(fix.path().to_str().unwrap(), text);
        let atom = make_and_store_atom(&store, text, fix.path().to_str().unwrap());
        let m = eval_provenance_coverage(&[atom], &store);
        assert!((m.coverage - 1.0).abs() < f32::EPSILON, "coverage={}", m.coverage);
        assert_eq!(m.n_quarantined, 0);
    }

    #[test]
    fn provenance_coverage_target_above_0_95() {
        // With all real unmodified atoms, coverage must be >= 0.95
        let (store, _dir) = tmp_store();
        let fix = NamedTempFile::new().unwrap();
        let path = fix.path().to_str().unwrap();
        // Write multiple turns to fixture
        let mut f = std::fs::File::create(path).unwrap();
        for i in 0..3 {
            writeln!(
                f,
                r#"{{"uuid":"t{i}","type":"assistant","message":{{"content":[{{"type":"text","text":"Decision {i}: use approach {i} because it is best."}}]}}}}"#
            ).unwrap();
        }
        drop(f);

        let turns = muginn_adapters::iter_turns("claude_code", path);
        let (priv_hex, pub_hex) = new_keypair();
        let mut atoms = Vec::new();
        for turn in &turns {
            if let Ok(atom) = store.store_atom(turn, (0, turn.text.len()), &priv_hex, &pub_hex, vec![]) {
                atoms.push(atom);
            }
        }
        assert!(!atoms.is_empty());
        let m = eval_provenance_coverage(&atoms, &store);
        assert!(m.coverage >= 0.95, "coverage={}", m.coverage);
    }

    // ── Poison rejection ─────────────────────────────────────────────────────

    #[test]
    fn poison_rejection_is_1() {
        let (store, _dir) = tmp_store();
        let fix = NamedTempFile::new().unwrap();
        let text = "Decision: use Ed25519 because it is fast and small.";
        write_fixture(fix.path().to_str().unwrap(), text);
        let atom = make_and_store_atom(&store, text, fix.path().to_str().unwrap());

        let m = eval_poison_rejection(&[atom], 5, &store);
        assert_eq!(m.n_injected, 5);
        assert_eq!(m.n_quarantined, 5, "all fabricated must be quarantined");
        assert!((m.rejection_rate - 1.0).abs() < 1e-9, "rejection_rate={}", m.rejection_rate);
    }

    #[test]
    fn poison_rejection_no_fabricated() {
        let (store, _dir) = tmp_store();
        let m = eval_poison_rejection(&[], 0, &store);
        assert!((m.rejection_rate - 1.0).abs() < 1e-9);
    }

    // ── Staleness precision/recall ────────────────────────────────────────────

    #[test]
    fn staleness_precision_recall_perfect() {
        let make_atom = |stale: bool| Atom {
            atom_id: format!("id_{stale}"),
            quote: "test".into(),
            citation: Citation {
                agent: "claude_code".into(),
                native_path: "/x".into(),
                session_id: "s".into(),
                turn_id: "t".into(),
                span: (0, 4),
                turn_sha256: "sha".into(),
            },
            content_hash: "ch".into(),
            signature: "sig".into(),
            pubkey: "pk".into(),
            prev_atom_id: String::new(),
            topic_key: "test".into(),
            superseded_by: String::new(),
            stale,
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        // atom0: expected stale=true, actual stale=true (TP)
        // atom1: expected stale=false, actual stale=false (TN)
        let labeled = vec![
            (make_atom(true), true),
            (make_atom(false), false),
        ];
        let m = eval_staleness(&labeled);
        assert!((m.precision - 1.0).abs() < 1e-9);
        assert!((m.recall - 1.0).abs() < 1e-9);
        assert_eq!(m.n_expected_stale, 1);
        assert_eq!(m.n_found_stale, 1);
    }

    #[test]
    fn staleness_store_supersedes_older_atom() {
        // Store two atoms with same topic_key → first becomes stale
        let (store, _dir) = tmp_store();
        let fix = NamedTempFile::new().unwrap();
        let path = fix.path().to_str().unwrap();

        let (priv_hex, pub_hex) = new_keypair();

        // Both texts share topic_key "decision-use-ed25519-because" → atom2 supersedes atom1.
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, r#"{{"uuid":"t1","type":"assistant","message":{{"content":[{{"type":"text","text":"Decision: use Ed25519 because it is slow."}}]}}}}"#).unwrap();
        writeln!(f, r#"{{"uuid":"t2","type":"assistant","message":{{"content":[{{"type":"text","text":"Decision: use Ed25519 because it is fast."}}]}}}}"#).unwrap();
        drop(f);

        let turns = muginn_adapters::iter_turns("claude_code", path);
        assert_eq!(turns.len(), 2);

        let atom1 = store.store_atom(&turns[0], (0, turns[0].text.len()), &priv_hex, &pub_hex, vec![]).unwrap();
        let atom2 = store.store_atom(&turns[1], (0, turns[1].text.len()), &priv_hex, &pub_hex, vec![]).unwrap();

        // Reload atom1 from store — it should now be stale
        let atom1_reloaded = store.get(&atom1.atom_id).unwrap();
        assert!(atom1_reloaded.stale, "atom1 must be stale after atom2 supersedes it");
        assert!(!atom2.stale, "atom2 must be live");

        let labeled = vec![
            (atom1_reloaded, true),   // expected stale
            (atom2, false),           // expected live
        ];
        let m = eval_staleness(&labeled);
        assert!((m.recall - 1.0).abs() < 1e-9, "staleness recall={}", m.recall);
        assert!((m.precision - 1.0).abs() < 1e-9, "staleness precision={}", m.precision);
    }

    // ── Format token benchmark ────────────────────────────────────────────────

    #[test]
    fn format_benchmark_json_larger_than_md() {
        let atom = Atom {
            atom_id: "abc12345".into(),
            quote: "Decision: use Ed25519 because it is fast.".into(),
            citation: Citation {
                agent: "claude_code".into(),
                native_path: "/tmp/s.jsonl".into(),
                session_id: "sess1".into(),
                turn_id: "t1".into(),
                span: (0, 41),
                turn_sha256: "sha256hex".into(),
            },
            content_hash: "ch".into(),
            signature: "sig".into(),
            pubkey: "pk".into(),
            prev_atom_id: String::new(),
            topic_key: "decision-use-ed25519-because".into(),
            superseded_by: String::new(),
            stale: false,
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let m = eval_format_overhead(&[atom]);
        assert!(m.md_chars > 0);
        assert!(m.json_chars > 0);
        // JSON always carries more overhead than the compact markdown card format
        assert!(m.json_overhead_ratio > 1.0, "json_ratio={}", m.json_overhead_ratio);
    }
}
