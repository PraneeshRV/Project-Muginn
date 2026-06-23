import struct

import pytest

from provmem.crypto import new_keypair
from provmem.embed import FakeEmbedder
from provmem.store import Store
from provmem.types import Turn
import provmem.crypto as _c


def _turn(text, tid="t1", sid="s1"):
    return Turn("claude_code", sid, tid, "assistant", text, "/x.jsonl",
                _c.sha256_text(text))


def test_fake_embedder_is_deterministic():
    e = FakeEmbedder()
    assert e.embed("hello") == e.embed("hello")


def test_fake_embedder_unit_norm():
    e = FakeEmbedder()
    v = e.embed("test text")
    norm = sum(x*x for x in v)**0.5
    assert abs(norm - 1.0) < 1e-5


def test_fake_embedder_different_texts_differ():
    e = FakeEmbedder()
    assert e.embed("apple") != e.embed("banana")


def test_vsearch_nearest_neighbour(tmp_path):
    priv, pub = new_keypair()
    e = FakeEmbedder()
    st = Store(str(tmp_path / "m.db"), embedder=e)
    t1 = _turn("Decision: use Ed25519 because fast.", tid="t1", sid="s1")
    t2 = _turn("Constraint: must be offline.", tid="t2", sid="s1")
    f1 = st.store_fact(t1, (0, len(t1.text.encode())), priv, pub)
    f2 = st.store_fact(t2, (0, len(t2.text.encode())), priv, pub)
    # query embedding closest to t1
    qvec = e.embed("Decision: use Ed25519 because fast.")
    results = st.vsearch(qvec, k=2)
    assert results[0].fact_id == f1.fact_id   # nearest neighbour first
