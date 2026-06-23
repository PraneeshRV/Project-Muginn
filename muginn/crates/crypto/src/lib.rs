use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey, Signature};
use sha2::{Digest, Sha256};

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

pub fn sign(priv_hex: &str, message: &str) -> String {
    let bytes: [u8; 32] = hex::decode(priv_hex).unwrap().try_into().unwrap();
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
}
