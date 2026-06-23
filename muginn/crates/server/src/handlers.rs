//! Pure MCP handlers — testable without any transport.
//! Mirrors the Python `make_handlers` separation: logic here, rmcp binding in `lib.rs`.

use muginn_compile::{enforce_for_topic, Compiler, NullCompiler};
use muginn_render::render_cards;
use muginn_select::select_spans;
use muginn_store::Store;
use muginn_verify::verify_atom;

/// `recall(query, k)` → compiled markdown cards + per-atom verify status.
pub fn recall(store: &Store, query: &str, k: usize) -> String {
    let atoms = store.search(query, k, false);
    if atoms.is_empty() {
        return format!("no results for: {query}");
    }
    let mut out = render_cards(&atoms);
    out.push('\n');
    for atom in &atoms {
        let status = verify_atom(atom);
        let id8 = &atom.atom_id[..8.min(atom.atom_id.len())];
        out.push_str(&format!("  verify[{id8}] = {status}\n"));
    }
    out
}

/// `verify(atom_id)` → status string, or `not-found` if the atom is unknown.
pub fn verify(store: &Store, atom_id: &str) -> String {
    match store.get(atom_id) {
        Some(atom) => verify_atom(&atom).to_string(),
        None => "not-found".to_string(),
    }
}

/// `cite(atom_id)` → exact citation JSON for click-through, or an error object.
pub fn cite(store: &Store, atom_id: &str) -> serde_json::Value {
    match store.get(atom_id) {
        Some(atom) => serde_json::to_value(&atom.citation)
            .unwrap_or_else(|_| serde_json::json!({"error": "serialize failed"})),
        None => serde_json::json!({"error": "not-found", "atom_id": atom_id}),
    }
}

/// `ingest(agent, path)` → run adapter → select → verified store; returns count.
pub fn ingest(
    store: &Store,
    agent: &str,
    path: &str,
    priv_hex: &str,
    pub_hex: &str,
) -> usize {
    let turns = muginn_adapters::iter_turns(agent, path);
    let mut count = 0;
    for turn in &turns {
        for span in select_spans(turn) {
            if store.store_atom(turn, span, priv_hex, pub_hex, vec![]).is_ok() {
                count += 1;
            }
        }
    }
    count
}

/// `compile(topic)` → (re)compile a page; returns a summary line with coverage.
pub fn compile(store: &Store, topic: &str) -> String {
    let atoms = store.search(topic, 50, false);
    if atoms.is_empty() {
        return format!("no atoms for topic: {topic}");
    }
    let draft = NullCompiler.compile(topic, &atoms);
    let page = enforce_for_topic(&draft, store, topic);
    format!(
        "compiled {} sentences, coverage {:.0}%, {} quarantined",
        page.verdicts.len(),
        page.coverage * 100.0,
        page.quarantined().len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::Turn;
    use muginn_crypto::{new_keypair, sha256_hex};

    fn tmp_store() -> (Store, tempfile::NamedTempFile) {
        let f = tempfile::NamedTempFile::new().unwrap();
        let s = Store::open(f.path().to_str().unwrap());
        (s, f)
    }

    fn seed(store: &Store) -> String {
        let (priv_hex, pub_hex) = new_keypair();
        let text = "Decision: use Ed25519 because it is fast and small.";
        let turn = Turn {
            agent: "claude_code".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            role: "assistant".into(),
            text: text.to_string(),
            native_path: "/tmp/none.jsonl".into(),
            turn_sha256: sha256_hex(text),
        };
        let atom = store
            .store_atom(&turn, (0, text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        atom.atom_id
    }

    #[test]
    fn recall_returns_cards() {
        let (store, _f) = tmp_store();
        seed(&store);
        let out = recall(&store, "Ed25519", 5);
        assert!(out.contains("Ed25519"));
        assert!(out.contains("verify["));
    }

    #[test]
    fn recall_empty_query_message() {
        let (store, _f) = tmp_store();
        let out = recall(&store, "nonexistent", 5);
        assert!(out.contains("no results"));
    }

    #[test]
    fn verify_returns_status_or_not_found() {
        let (store, _f) = tmp_store();
        let id = seed(&store);
        // source file /tmp/none.jsonl missing → source-missing
        let status = verify(&store, &id);
        assert_eq!(status, "source-missing");
        // unknown id
        assert_eq!(verify(&store, "deadbeef"), "not-found");
    }

    #[test]
    fn cite_returns_citation_or_error() {
        let (store, _f) = tmp_store();
        let id = seed(&store);
        let c = cite(&store, &id);
        assert_eq!(c["agent"], "claude_code");
        assert_eq!(c["turn_id"], "t1");
        let err = cite(&store, "deadbeef");
        assert_eq!(err["error"], "not-found");
    }

    #[test]
    fn compile_reports_coverage() {
        let (store, _f) = tmp_store();
        seed(&store);
        let out = compile(&store, "Ed25519");
        assert!(out.contains("coverage"));
    }
}
