from provmem.render import render_cards
from provmem.types import Fact, FactSource


def _fact():
    src = FactSource("claude_code", "/x.jsonl", "s1", "t1", (0, 5), "sha")
    return Fact("abcdef1234", "hello", src, "ch", "sig", "pk", "")


def test_render_card_has_quote_and_citation():
    out = render_cards([_fact()])
    assert '"hello"' in out
    assert "claude_code:s1#t1" in out
    assert "abcdef12" in out


def test_render_empty():
    assert render_cards([]) == ""
