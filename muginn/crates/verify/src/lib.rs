//! Muginn's transcript-aware verifier. It resolves an atom's source file/turn (the I/O that
//! is specific to Muginn), then delegates the pure signature + byte-span check to `bytecite`.

use muginn_core::Atom;
use bytecite::{verify_quote, verify_sig, QuoteStatus};

pub fn verify_atom(atom: &Atom) -> &'static str {
    // Signature first, so a forged atom is rejected before any file is opened.
    if !verify_sig(&atom.pubkey, &atom.content_hash, &atom.signature) {
        return "bad-signature";
    }

    let path = &atom.citation.native_path;
    if !std::path::Path::new(path).exists() {
        return "source-missing";
    }

    let turns = muginn_adapters::iter_turns(&atom.citation.agent, path);
    if turns.is_empty() {
        return "source-missing";
    }
    let turn = match turns.iter().find(|t| t.turn_id == atom.citation.turn_id) {
        Some(t) => t,
        None => return "turn-missing",
    };

    if turn.turn_sha256 != atom.citation.turn_sha256 {
        return "source-modified";
    }

    // Delegate the signature + byte-span comparison to the pure bytecite primitive.
    match verify_quote(
        turn.text.as_bytes(),
        atom.citation.span,
        &atom.quote,
        &atom.content_hash,
        &atom.signature,
        &atom.pubkey,
    ) {
        QuoteStatus::Ok => "ok",
        QuoteStatus::BadSignature => "bad-signature",
        QuoteStatus::SpanMismatch => "span-mismatch",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::{Atom, Citation};
    use bytecite::{atom_id, content_hash, new_keypair, sha256_hex, sign};
    use std::io::Write;

    fn make_atom(
        text: &str,
        path: &str,
        turn_id: &str,
        span: (usize, usize),
        priv_hex: &str,
        pub_hex: &str,
    ) -> Atom {
        let quote = text[span.0..span.1].to_string();
        let turn_sha = sha256_hex(text);
        let citation = Citation {
            agent: "claude_code".into(),
            native_path: path.to_string(),
            session_id: "s1".into(),
            turn_id: turn_id.to_string(),
            span,
            turn_sha256: turn_sha,
        };
        let cit_val = serde_json::to_value(&citation).unwrap();
        let ch = content_hash(&quote, &cit_val);
        let id = atom_id(&ch, pub_hex);
        let sig = sign(priv_hex, &ch);
        Atom {
            atom_id: id,
            quote,
            citation,
            content_hash: ch,
            signature: sig,
            pubkey: pub_hex.to_string(),
            prev_atom_id: String::new(),
            topic_key: "test".into(),
            superseded_by: String::new(),
            stale: false,
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn write_fixture(path: &str, text: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(
            f,
            r#"{{"uuid":"a1","type":"assistant","message":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#,
            text
        )
        .unwrap();
    }

    #[test]
    fn verify_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.jsonl");
        let path_str = path.to_str().unwrap();
        let text = "Decision: use Ed25519 because it is fast and small.";
        write_fixture(path_str, text);
        let (priv_hex, pub_hex) = new_keypair();
        let atom = make_atom(text, path_str, "a1", (0, text.len()), &priv_hex, &pub_hex);
        assert_eq!(verify_atom(&atom), "ok");
    }

    #[test]
    fn verify_source_missing() {
        let (priv_hex, pub_hex) = new_keypair();
        let text = "Decision: use Ed25519 because it is fast.";
        let atom = make_atom(text, "/no/such/file.jsonl", "a1", (0, text.len()), &priv_hex, &pub_hex);
        assert_eq!(verify_atom(&atom), "source-missing");
    }

    #[test]
    fn verify_source_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.jsonl");
        let path_str = path.to_str().unwrap();
        let original = "Decision: use Ed25519 because it is fast and small.";
        write_fixture(path_str, original);
        let (priv_hex, pub_hex) = new_keypair();
        let atom = make_atom(original, path_str, "a1", (0, original.len()), &priv_hex, &pub_hex);
        // Tamper the file
        write_fixture(path_str, "Decision: use Ed25519 because it is slow and small.");
        assert_eq!(verify_atom(&atom), "source-modified");
    }
}
