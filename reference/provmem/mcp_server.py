from __future__ import annotations

import json

from provmem.render import render_cards
from provmem.store import Store
from provmem.verify import verify_fact


def make_handlers(store: Store) -> dict:
    """Plain handler functions (testable without any MCP transport)."""

    def recall(query: str, k: int = 10) -> str:
        return render_cards(store.search(query, k))

    def verify(fact_id: str) -> str:
        f = store.get(fact_id)
        return "not-found" if f is None else verify_fact(f)

    def cite(fact_id: str) -> str:
        f = store.get(fact_id)
        return "{}" if f is None else json.dumps(f.to_dict()["source"])

    return {"recall": recall, "verify": verify, "cite": cite}


def build_server(db_path: str):
    from fastmcp import FastMCP

    store = Store(db_path)
    h = make_handlers(store)
    mcp = FastMCP("prov-memory")
    mcp.tool(name="recall")(h["recall"])
    mcp.tool(name="verify")(h["verify"])
    mcp.tool(name="cite")(h["cite"])
    return mcp


if __name__ == "__main__":
    import os

    build_server(os.environ.get("PROVMEM_DB", os.path.expanduser("~/.provmem.db"))).run()
