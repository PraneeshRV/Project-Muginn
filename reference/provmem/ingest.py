from __future__ import annotations

from provmem.select import select_spans
from provmem.store import SpanMismatch, Store


def ingest_file(store: Store, adapter, path: str, priv_hex: str, pub_hex: str) -> int:
    """Run adapter -> selector -> verified store. Returns facts written."""
    count = 0
    for turn in adapter.iter_turns(path):
        for span in select_spans(turn):
            try:
                store.store_fact(turn, span, priv_hex, pub_hex)
                count += 1
            except SpanMismatch:
                continue
    return count
