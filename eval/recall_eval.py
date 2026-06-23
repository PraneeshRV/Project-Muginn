"""Selector quality eval. Run: python eval/recall_eval.py

Reports recall (salient captured) and false-positive rate (non-salient kept)
for the heuristic selector against a labeled fixture."""
from __future__ import annotations

import json
import os

from provmem.select import select_spans
from provmem.types import Turn

_FIX = os.path.join(os.path.dirname(__file__), "fixtures", "labeled.jsonl")


def _selects_any(text: str) -> bool:
    t = Turn("eval", "s", "t", "assistant", text, "/x", "sha")
    return len(select_spans(t)) > 0


def evaluate(path: str = _FIX) -> dict:
    rows = [json.loads(l) for l in open(path) if l.strip()]
    salient = [r for r in rows if r["salient"]]
    nonsalient = [r for r in rows if not r["salient"]]
    tp = sum(_selects_any(r["text"]) for r in salient)
    fp = sum(_selects_any(r["text"]) for r in nonsalient)
    return {
        "recall": tp / len(salient) if salient else 0.0,
        "false_positive_rate": fp / len(nonsalient) if nonsalient else 0.0,
        "n_salient": len(salient),
        "n_nonsalient": len(nonsalient),
    }


if __name__ == "__main__":
    m = evaluate()
    print(f"recall={m['recall']:.2f}  fp_rate={m['false_positive_rate']:.2f}  "
          f"(salient={m['n_salient']}, nonsalient={m['n_nonsalient']})")
