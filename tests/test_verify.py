from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.store import Store
from provmem.verify import verify_fact
from tests.conftest import FIXTURES


def _ingest_a1(tmp_path, src_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    turns = list(ClaudeCodeAdapter().iter_turns(src_path))
    a1 = next(t for t in turns if t.turn_id == "a1")
    f = st.store_fact(a1, (0, len(a1.text.encode())), priv, pub)
    return f


def test_verify_ok(tmp_path):
    f = _ingest_a1(tmp_path, str(FIXTURES / "claude_code" / "sample.jsonl"))
    assert verify_fact(f) == "ok"


def test_verify_source_missing(tmp_path):
    f = _ingest_a1(tmp_path, str(FIXTURES / "claude_code" / "sample.jsonl"))
    object.__setattr__(f.source, "native_path", "/no/such.jsonl")
    assert verify_fact(f) == "source-missing"


def test_verify_source_modified_on_same_turn(tmp_path):
    src = tmp_path / "sess.jsonl"
    src.write_text((FIXTURES / "claude_code" / "sample.jsonl").read_text())
    f = _ingest_a1(tmp_path, str(src))
    # edit the SAME turn (a1) the fact came from
    src.write_text(src.read_text().replace("fast and small", "slow and small"))
    assert verify_fact(f) == "source-modified"
