use serde::{Deserialize, Serialize};

// Citation is the general, reusable provenance type — it lives in the `bytecite` crate.
// Re-exported here so `muginn_core::Citation` keeps resolving across the workspace.
pub use bytecite::Citation;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citation_span_is_byte_pair() {
        let c = Citation {
            agent: "claude_code".into(),
            native_path: "/x".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            span: (0, 5),
            turn_sha256: "sha".into(),
        };
        assert_eq!(c.span, (0, 5));
    }

    #[test]
    fn turn_slice_decodes() {
        let t = "hello world";
        assert_eq!(&t[0..5], "hello");
    }
}
