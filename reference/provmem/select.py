from __future__ import annotations

import re

from provmem.types import Turn

# Salience heuristic (MVP): keep sentences that look like durable facts.
_SALIENT = re.compile(
    r"\b(decision|constraint|because|prefer|remember|TODO|FIXME)\b|\b[\w./-]+\.\w+:\d+",
    re.IGNORECASE,
)
_SENT = re.compile(r"[^.!?]*[.!?]|[^.!?]+$")


def select_spans(turn: Turn) -> list[tuple[int, int]]:
    """Return (start, end) byte offsets into ``turn.text`` UTF-8 for salient
    sentences. Offsets land on byte boundaries so slicing decodes cleanly."""
    text = turn.text
    spans: list[tuple[int, int]] = []
    for m in _SENT.finditer(text):
        raw = m.group()
        sentence = raw.strip()
        if not sentence or not _SALIENT.search(sentence):
            continue
        start_char = m.start() + (len(raw) - len(raw.lstrip()))
        end_char = start_char + len(sentence)
        start_b = len(text[:start_char].encode())
        end_b = len(text[:end_char].encode())
        spans.append((start_b, end_b))
    return spans


def topic_key(quote: str) -> str:
    """Coarse grouping key so a newer fact can supersede an older one.
    First 4 alphanumeric tokens, lowercased."""
    toks = re.findall(r"[a-z0-9]+", quote.lower())
    return "-".join(toks[:4])
