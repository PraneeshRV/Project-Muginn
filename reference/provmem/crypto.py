from __future__ import annotations

import hashlib
import json

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)


def canonical_json(obj) -> str:
    """Deterministic JSON: sorted keys, no whitespace. Stable for hashing."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def new_keypair() -> tuple[str, str]:
    priv = Ed25519PrivateKey.generate()
    return priv.private_bytes_raw().hex(), priv.public_key().public_bytes_raw().hex()


def sign(priv_hex: str, message: str) -> str:
    priv = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(priv_hex))
    return priv.sign(message.encode()).hex()


def verify_sig(pub_hex: str, message: str, signature_hex: str) -> bool:
    try:
        pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(pub_hex))
        pub.verify(bytes.fromhex(signature_hex), message.encode())
        return True
    except (InvalidSignature, ValueError):
        return False


def content_hash(quote: str, source_dict: dict) -> str:
    """Bind the quote to its full source citation (including turn_sha256)."""
    payload = canonical_json({"quote": quote, "source": source_dict})
    return hashlib.sha256(payload.encode()).hexdigest()


def fact_id(content_hash_hex: str, pubkey_hex: str) -> str:
    return hashlib.sha256((content_hash_hex + pubkey_hex).encode()).hexdigest()
