from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.store import Store
from provmem.verify import verify_fact
from tests.conftest import FIXTURES


def test_tamper_same_turn_detected_other_turn_ignored(tmp_path):
    priv, pub = new_keypair()
    src = tmp_path / "session.jsonl"
    src.write_text((FIXTURES / "claude_code" / "sample.jsonl").read_text())
    st = Store(str(tmp_path / "m.db"))

    assert ingest_file(st, ClaudeCodeAdapter(), str(src), priv, pub) >= 1
    facts = st.search("Ed25519")
    assert facts and all(verify_fact(f) == "ok" for f in facts)

    # tamper a DIFFERENT turn (u2 "sounds good") -> facts stay ok (no false positive)
    src.write_text(src.read_text().replace("sounds good", "sounds great"))
    assert all(verify_fact(f) == "ok" for f in facts)

    # tamper the SAME turn (a1) the facts came from -> detected
    src.write_text(src.read_text().replace("fast and small", "slow and small"))
    assert any(verify_fact(f) == "source-modified" for f in facts)
