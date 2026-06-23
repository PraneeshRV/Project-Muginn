from provmem.select import select_spans, topic_key
from provmem.types import Turn


def _turn(text):
    return Turn("claude_code", "s1", "t1", "assistant", text, "/x", "sha")


def test_keeps_salient_sentence_only():
    t = _turn("Hello there. Decision: use Ed25519 because it is fast. Bye.")
    quotes = [t.text.encode()[s:e].decode() for s, e in select_spans(t)]
    assert any("Decision: use Ed25519" in q for q in quotes)
    assert not any(q.strip() == "Hello there." for q in quotes)


def test_spans_are_byte_accurate_with_unicode():
    t = _turn("café note. TODO: add tests here.")
    spans = select_spans(t)
    quotes = [t.text.encode()[s:e].decode() for s, e in spans]  # must not raise
    assert any("TODO" in q for q in quotes)


def test_no_salient_returns_empty():
    assert select_spans(_turn("just chatting about nothing.")) == []


def test_topic_key_is_deterministic_first_four_tokens():
    assert topic_key("Decision: use Ed25519 because it is fast.") == "decision-use-ed25519-because"
    assert topic_key("Decision: use Ed25519 because it is slow.") == "decision-use-ed25519-because"
