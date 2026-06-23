from provmem.crypto import (
    canonical_json,
    content_hash,
    fact_id,
    new_keypair,
    sign,
    verify_sig,
)


def test_canonical_json_is_stable():
    assert canonical_json({"b": 1, "a": 2}) == canonical_json({"a": 2, "b": 1})


def test_sign_then_verify_roundtrip():
    priv, pub = new_keypair()
    ch = content_hash("hello", {"span": [0, 5]})
    assert verify_sig(pub, ch, sign(priv, ch)) is True


def test_verify_rejects_tampered_hash():
    priv, pub = new_keypair()
    sig = sign(priv, content_hash("hello", {"span": [0, 5]}))
    assert verify_sig(pub, content_hash("HELLO", {"span": [0, 5]}), sig) is False


def test_fact_id_changes_with_pubkey():
    ch = content_hash("hello", {"span": [0, 5]})
    assert fact_id(ch, "pkA") != fact_id(ch, "pkB")
