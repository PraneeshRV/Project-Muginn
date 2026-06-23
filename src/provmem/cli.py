from __future__ import annotations

import argparse
import os

from provmem.adapters.antigravity import AntigravityAdapter
from provmem.adapters.claude_code import ClaudeCodeAdapter
from provmem.adapters.codex import CodexAdapter
from provmem.crypto import new_keypair
from provmem.ingest import ingest_file
from provmem.render import render_cards
from provmem.store import Store
from provmem.verify import verify_fact

_ADAPTERS = {
    "claude_code": ClaudeCodeAdapter,
    "codex": CodexAdapter,
    "antigravity": AntigravityAdapter,
}

_KEY_PATH = os.path.expanduser("~/.provmem.key")
_DB_PATH = os.path.expanduser("~/.provmem.db")


def _load_or_make_key() -> tuple[str, str]:
    if os.path.exists(_KEY_PATH):
        priv = open(_KEY_PATH).read().strip()
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        pub = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(priv)).public_key().public_bytes_raw().hex()
        return priv, pub
    priv, pub = new_keypair()
    with open(_KEY_PATH, "w") as fh:
        fh.write(priv)
    os.chmod(_KEY_PATH, 0o600)
    return priv, pub


def main(argv=None) -> int:
    p = argparse.ArgumentParser(prog="provmem")
    sub = p.add_subparsers(dest="cmd", required=True)
    pi = sub.add_parser("ingest", help="ingest a transcript file")
    pi.add_argument("agent", choices=list(_ADAPTERS))
    pi.add_argument("path")
    pr = sub.add_parser("recall", help="search memory")
    pr.add_argument("query")
    pr.add_argument("-k", type=int, default=10)
    args = p.parse_args(argv)

    store = Store(_DB_PATH)
    priv, pub = _load_or_make_key()

    if args.cmd == "ingest":
        n = ingest_file(store, _ADAPTERS[args.agent](), args.path, priv, pub)
        print(f"ingested {n} facts from {args.path}")
        return 0
    if args.cmd == "recall":
        facts = store.search(args.query, args.k)
        print(render_cards(facts) or "(no matches)")
        for f in facts:
            print(f"  verify[{f.fact_id[:8]}] = {verify_fact(f)}")
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
