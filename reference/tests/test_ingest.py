from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.store import Store
from tests.conftest import FIXTURES


def test_ingest_then_search(tmp_path):
    priv, pub = new_keypair()
    st = Store(str(tmp_path / "m.db"))
    n = ingest_file(st, ClaudeCodeAdapter(),
                    str(FIXTURES / "claude_code" / "sample.jsonl"), priv, pub)
    assert n >= 1
    assert any("Ed25519" in f.quote for f in st.search("Ed25519"))
