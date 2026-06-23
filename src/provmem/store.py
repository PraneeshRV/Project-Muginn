from __future__ import annotations

import json
import sqlite3
from dataclasses import asdict
from datetime import datetime, timezone

from provmem import crypto
from provmem.select import topic_key
from provmem.types import Fact, FactSource, Turn


class SpanMismatch(Exception):
    """Raised when a requested span does not yield real quote text."""


_SCHEMA = """
CREATE TABLE IF NOT EXISTS facts (
    fact_id TEXT PRIMARY KEY,
    quote TEXT NOT NULL,
    source_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    prev_fact_id TEXT NOT NULL,
    topic_key TEXT NOT NULL,
    superseded_by TEXT NOT NULL DEFAULT '',
    stale INTEGER NOT NULL DEFAULT 0,
    session_id TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    quote, content='facts', content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
    INSERT INTO facts_fts(rowid, quote) VALUES (new.rowid, new.quote);
END;
"""


class Store:
    def __init__(self, db_path: str):
        self.db = sqlite3.connect(db_path)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(_SCHEMA)

    def _last_fact_id(self, session_id: str) -> str:
        row = self.db.execute(
            "SELECT fact_id FROM facts WHERE session_id=? ORDER BY rowid DESC LIMIT 1",
            (session_id,),
        ).fetchone()
        return row["fact_id"] if row else ""

    def _mark_superseded(self, key: str, new_fact_id: str) -> None:
        self.db.execute(
            "UPDATE facts SET stale=1, superseded_by=? "
            "WHERE topic_key=? AND stale=0 AND fact_id<>?",
            (new_fact_id, key, new_fact_id),
        )

    def store_fact(self, turn: Turn, span: tuple[int, int], priv_hex: str,
                   pub_hex: str, tags: list[str] | None = None) -> Fact:
        start, end = span
        quote = turn.text.encode()[start:end].decode(errors="strict")
        if not quote.strip():
            raise SpanMismatch("empty or whitespace quote rejected")

        src = FactSource(turn.agent, turn.native_path, turn.session_id,
                         turn.turn_id, (start, end), turn.turn_sha256)
        src_dict = asdict(src)
        src_dict["span"] = [start, end]

        ch = crypto.content_hash(quote, src_dict)
        fid = crypto.fact_id(ch, pub_hex)
        sig = crypto.sign(priv_hex, ch)
        prev = self._last_fact_id(turn.session_id)
        key = topic_key(quote)
        created = datetime.now(timezone.utc).isoformat()
        tags = tags or []

        self._mark_superseded(key, fid)
        self.db.execute(
            "INSERT OR IGNORE INTO facts VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (fid, quote, json.dumps(src_dict), ch, sig, pub_hex, prev, key,
             "", 0, turn.session_id, json.dumps(tags), created),
        )
        self.db.commit()
        return Fact(fid, quote, src, ch, sig, pub_hex, prev, key, "", False, tags, created)

    def _row_to_fact(self, row) -> Fact:
        sd = json.loads(row["source_json"])
        src = FactSource(sd["agent"], sd["native_path"], sd["session_id"],
                         sd["turn_id"], tuple(sd["span"]), sd["turn_sha256"])
        return Fact(row["fact_id"], row["quote"], src, row["content_hash"],
                    row["signature"], row["pubkey"], row["prev_fact_id"],
                    row["topic_key"], row["superseded_by"], bool(row["stale"]),
                    json.loads(row["tags_json"]), row["created_at"])

    def get(self, fact_id: str) -> Fact | None:
        row = self.db.execute("SELECT * FROM facts WHERE fact_id=?", (fact_id,)).fetchone()
        return self._row_to_fact(row) if row else None

    def search(self, query: str, k: int = 10, include_stale: bool = False) -> list[Fact]:
        clause = "" if include_stale else "AND f.stale=0 "
        rows = self.db.execute(
            "SELECT f.* FROM facts_fts ft JOIN facts f ON f.rowid=ft.rowid "
            f"WHERE facts_fts MATCH ? {clause}ORDER BY rank LIMIT ?",
            (query, k),
        ).fetchall()
        return [self._row_to_fact(r) for r in rows]
