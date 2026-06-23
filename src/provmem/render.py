from __future__ import annotations

from provmem.types import Fact


def render_cards(facts: list[Fact]) -> str:
    """Markdown cards for context injection (the token-lean format the
    research benchmark picked). One line per fact with a clickable citation."""
    lines = []
    for f in facts:
        s = f.source
        lines.append(
            f'- "{f.quote}" — {s.agent}:{s.session_id}#{s.turn_id} [{f.fact_id[:8]}]'
        )
    return "\n".join(lines)
