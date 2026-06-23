from provmem.types import Fact, FactSource, Turn


def test_turn_holds_canonical_text():
    t = Turn("claude_code", "s1", "t1", "assistant", "hello world", "/x.jsonl", "sha")
    assert t.text.encode()[0:5].decode() == "hello"


def test_factsource_span_is_byte_pair():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    assert src.span == (0, 5)


def test_fact_to_dict_roundtrip():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    f = Fact("f", "hello", src, "ch", "sig", "pk", "", tags=["x"], created_at="2026")
    d = f.to_dict()
    assert d["source"]["span"] == [0, 5]
    assert d["quote"] == "hello"
    assert d["stale"] is False
