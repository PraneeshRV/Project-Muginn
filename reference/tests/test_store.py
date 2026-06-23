import pytest

from provmem.crypto import new_keypair, verify_sig
from provmem.store import SpanMismatch, Store
from provmem.types import Turn


def _turn(text="Decision: use Ed25519 because fast.", tid="t1", sid="s1"):
    return Turn("claude_code", sid, tid, "assistant", text, "/x.jsonl",
                __import__("provmem.crypto", fromlist=["sha256_text"]).sha256_text(text))


def test_store_and_search(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    span = (0, len(t.text.encode()))
    f = st.store_fact(t, span, priv, pub)
    assert f.quote.startswith("Decision: use Ed25519")
    assert verify_sig(pub, f.content_hash, f.signature)
    hits = st.search("Ed25519")
    assert len(hits) == 1 and hits[0].fact_id == f.fact_id


def test_hash_chain_links_same_session(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    t = _turn()
    f1 = st.store_fact(t, (0, 8), priv, pub)    # "Decision"
    f2 = st.store_fact(t, (10, 13), priv, pub)  # "use"
    assert f1.prev_fact_id == ""
    assert f2.prev_fact_id == f1.fact_id


def test_rejects_empty_quote(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    with pytest.raises(SpanMismatch):
        st.store_fact(_turn(), (5, 5), priv, pub)


def test_newer_fact_supersedes_older_same_topic(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    old = _turn("Decision: use Ed25519 because it is fast.", tid="a1", sid="s1")
    new = _turn("Decision: use Ed25519 because it is secure.", tid="b1", sid="s2")
    st.store_fact(old, (0, len(old.text.encode())), priv, pub)
    st.store_fact(new, (0, len(new.text.encode())), priv, pub)
    live = st.search("Ed25519")
    assert len(live) == 1 and "secure" in live[0].quote   # stale hidden
    assert len(st.search("Ed25519", include_stale=True)) == 2
