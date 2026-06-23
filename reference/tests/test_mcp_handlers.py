from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.mcp_server import make_handlers
from provmem.store import Store
from tests.conftest import FIXTURES


def test_handlers_recall_verify_cite(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    ingest_file(st, ClaudeCodeAdapter(),
                str(FIXTURES / "claude_code" / "sample.jsonl"), priv, pub)
    h = make_handlers(st)
    assert "Ed25519" in h["recall"]("Ed25519", 5)
    fid = st.search("Ed25519")[0].fact_id
    assert h["verify"](fid) == "ok"
    assert "claude_code" in h["cite"](fid)
    assert h["recall"]("zzzznotpresent", 5) == ""
    assert h["verify"]("deadbeef") == "not-found"
