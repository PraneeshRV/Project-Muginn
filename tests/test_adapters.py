from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.crypto import sha256_text
from tests.conftest import FIXTURES


def test_claude_parses_turns():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    turns = list(ClaudeCodeAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["u1", "a1", "u2"]
    assert turns[1].role == "assistant"
    assert "Ed25519" in turns[1].text
    assert turns[1].agent == "claude_code"
    assert turns[1].turn_sha256 == sha256_text(turns[1].text)


def test_claude_is_deterministic():
    path = str(FIXTURES / "claude_code" / "sample.jsonl")
    a = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    b = [t.text for t in ClaudeCodeAdapter().iter_turns(path)]
    assert a == b


from provmem.adapters.codex import CodexAdapter


def test_codex_parses_only_message_events():
    path = str(FIXTURES / "codex" / "rollout.jsonl")
    turns = list(CodexAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["m1", "m2"]
    assert "Constraint" in turns[1].text
    assert turns[0].agent == "codex"


from provmem.adapters.antigravity import AntigravityAdapter


def test_antigravity_parses_messages():
    path = str(FIXTURES / "antigravity" / "sample.json")
    turns = list(AntigravityAdapter().iter_turns(path))
    assert [t.turn_id for t in turns] == ["x1", "x2"]
    assert turns[1].agent == "antigravity"
    assert "Decision" in turns[1].text
