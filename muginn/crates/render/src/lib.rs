use muginn_core::Atom;

pub fn render_cards(atoms: &[Atom]) -> String {
    if atoms.is_empty() {
        return String::new();
    }
    atoms
        .iter()
        .map(|a| {
            let id8 = &a.atom_id[..8.min(a.atom_id.len())];
            format!(
                "- \"{}\" — {}:{}#{} [{}]",
                a.quote, a.citation.agent, a.citation.session_id, a.citation.turn_id, id8
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::{Atom, Citation};

    fn make_atom(quote: &str, atom_id: &str) -> Atom {
        Atom {
            atom_id: atom_id.to_string(),
            quote: quote.to_string(),
            citation: Citation {
                agent: "claude_code".into(),
                native_path: "/x".into(),
                session_id: "sess1".into(),
                turn_id: "t1".into(),
                span: (0, quote.len()),
                turn_sha256: "sha".into(),
            },
            content_hash: "ch".into(),
            signature: "sig".into(),
            pubkey: "pk".into(),
            prev_atom_id: String::new(),
            topic_key: "test".into(),
            superseded_by: String::new(),
            stale: false,
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn renders_card_with_quote_citation_id8() {
        let atom = make_atom("Decision: use Ed25519", "abcdef1234567890");
        let out = render_cards(&[atom]);
        assert!(out.contains("Decision: use Ed25519"));
        assert!(out.contains("claude_code:sess1#t1"));
        assert!(out.contains("abcdef12"));
    }

    #[test]
    fn empty_slice_returns_empty_string() {
        assert_eq!(render_cards(&[]), "");
    }
}
