//! bytecite — signed, byte-verifiable citations.
//!
//! Bind a quote to an exact byte span in its source, sign it (Ed25519), and later prove
//! the quote still exists verbatim at that span. The library is **pure**: it does no file
//! or transcript I/O — the caller supplies the source bytes. Verification is a byte
//! comparison plus a signature check, never a semantic judgment.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A citation: where a quote came from, and the exact byte span within the source unit
/// (e.g. a transcript turn). `turn_sha256` lets a caller detect that the source unit
/// changed since the citation was made.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Citation {
    pub agent: String,
    pub native_path: String,
    pub session_id: String,
    pub turn_id: String,
    pub span: (usize, usize),
    pub turn_sha256: String,
}

pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

/// Deterministic JSON: serde_json::Value objects are BTreeMap-backed so keys serialize sorted.
pub fn canonical_json(v: &serde_json::Value) -> String {
    serde_json::to_string(v).expect("canonical_json")
}

pub fn new_keypair() -> (String, String) {
    use rand_core::{OsRng, RngCore};
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let sk = SigningKey::from_bytes(&secret);
    let pk = sk.verifying_key();
    (hex::encode(sk.to_bytes()), hex::encode(pk.to_bytes()))
}

/// Sign `message` with a hex Ed25519 secret key. Returns an empty string on a malformed
/// key (rather than panicking); an empty signature simply fails `verify_sig` downstream.
pub fn sign(priv_hex: &str, message: &str) -> String {
    let bytes: [u8; 32] = match hex::decode(priv_hex).ok().and_then(|b| b.try_into().ok()) {
        Some(b) => b,
        None => return String::new(),
    };
    let sk = SigningKey::from_bytes(&bytes);
    hex::encode(sk.sign(message.as_bytes()).to_bytes())
}

pub fn verify_sig(pub_hex: &str, message: &str, sig_hex: &str) -> bool {
    let pk_bytes: [u8; 32] = match hex::decode(pub_hex).ok().and_then(|b| b.try_into().ok()) {
        Some(b) => b,
        None => return false,
    };
    let sig_bytes: [u8; 64] = match hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok()) {
        Some(b) => b,
        None => return false,
    };
    let pk = match VerifyingKey::from_bytes(&pk_bytes) {
        Ok(p) => p,
        Err(_) => return false,
    };
    pk.verify(message.as_bytes(), &Signature::from_bytes(&sig_bytes)).is_ok()
}

pub fn content_hash(quote: &str, citation: &serde_json::Value) -> String {
    let payload = serde_json::json!({ "citation": citation, "quote": quote });
    sha256_hex(&canonical_json(&payload))
}

pub fn atom_id(content_hash_hex: &str, pubkey_hex: &str) -> String {
    sha256_hex(&format!("{content_hash_hex}{pubkey_hex}"))
}

/// Status of a pure quote verification (signature check + byte comparison).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteStatus {
    Ok,
    BadSignature,
    SpanMismatch,
}

/// Verify a quote against raw source bytes. Checks the Ed25519 signature over `signed_msg`
/// (typically the content hash) with `pubkey_hex`, then byte-compares `source[span]`
/// against `expected`. **Pure** — the caller obtains `source` (file read, transcript
/// parsing, …); this never touches the filesystem.
pub fn verify_quote(
    source: &[u8],
    span: (usize, usize),
    expected: &str,
    signed_msg: &str,
    sig_hex: &str,
    pubkey_hex: &str,
) -> QuoteStatus {
    if !verify_sig(pubkey_hex, signed_msg, sig_hex) {
        return QuoteStatus::BadSignature;
    }
    let (start, end) = span;
    if start > end || end > source.len() {
        return QuoteStatus::SpanMismatch;
    }
    let slice = String::from_utf8_lossy(&source[start..end]);
    if slice == expected {
        QuoteStatus::Ok
    } else {
        QuoteStatus::SpanMismatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let (p, k) = new_keypair();
        let ch = content_hash("hello", &serde_json::json!({"span": [0, 5]}));
        assert!(verify_sig(&k, &ch, &sign(&p, &ch)));
    }

    #[test]
    fn verify_rejects_tampered() {
        let (p, k) = new_keypair();
        let sig = sign(&p, &content_hash("hello", &serde_json::json!({"span": [0, 5]})));
        assert!(!verify_sig(&k, &content_hash("HELLO", &serde_json::json!({"span": [0, 5]})), &sig));
    }

    #[test]
    fn atom_id_changes_with_pubkey() {
        let ch = content_hash("hello", &serde_json::json!({"span": [0, 5]}));
        assert_ne!(atom_id(&ch, "pkA"), atom_id(&ch, "pkB"));
    }

    #[test]
    fn sign_bad_key_is_empty_not_panic() {
        assert_eq!(sign("not-hex", "msg"), "");
        assert!(!verify_sig("pk", "msg", ""));
    }

    #[test]
    fn verify_quote_ok() {
        let (p, k) = new_keypair();
        let src = b"Decision: use Ed25519 because it is fast.";
        let quote = "Decision: use Ed25519";
        let span = (0usize, quote.len());
        let ch = content_hash(quote, &serde_json::json!({"span": [span.0, span.1]}));
        let sig = sign(&p, &ch);
        assert_eq!(verify_quote(src, span, quote, &ch, &sig, &k), QuoteStatus::Ok);
    }

    #[test]
    fn verify_quote_bad_signature() {
        let (_p, k) = new_keypair();
        let src = b"hello world";
        assert_eq!(
            verify_quote(src, (0, 5), "hello", "deadbeef", "00", &k),
            QuoteStatus::BadSignature
        );
    }

    #[test]
    fn verify_quote_span_mismatch_and_out_of_range() {
        let (p, k) = new_keypair();
        let src = b"hello world";
        let ch = content_hash("hello", &serde_json::json!({}));
        let sig = sign(&p, &ch);
        // wrong expected text
        assert_eq!(verify_quote(src, (0, 5), "HELLO", &ch, &sig, &k), QuoteStatus::SpanMismatch);
        // start > end and end past len must not panic
        assert_eq!(verify_quote(src, (5, 2), "x", &ch, &sig, &k), QuoteStatus::SpanMismatch);
        assert_eq!(verify_quote(src, (0, 999), "x", &ch, &sig, &k), QuoteStatus::SpanMismatch);
    }

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
}
