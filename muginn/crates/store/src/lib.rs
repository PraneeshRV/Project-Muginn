use chrono::Utc;
use muginn_core::{Atom, Citation, Turn};
use bytecite::{atom_id, content_hash, sign};
use muginn_select::topic_key;
use rusqlite::{params, Connection};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("span mismatch or invalid UTF-8")]
pub struct SpanMismatch;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(db_path: &str) -> Self {
        let conn = Connection::open(db_path).expect("open db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS atoms (
                atom_id TEXT PRIMARY KEY,
                quote TEXT NOT NULL,
                citation_json TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                signature TEXT NOT NULL,
                pubkey TEXT NOT NULL,
                prev_atom_id TEXT NOT NULL,
                topic_key TEXT NOT NULL,
                superseded_by TEXT NOT NULL DEFAULT '',
                stale INTEGER NOT NULL DEFAULT 0,
                session_id TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS atoms_fts
                USING fts5(quote, content='atoms', content_rowid='rowid');
            CREATE TRIGGER IF NOT EXISTS atoms_ai AFTER INSERT ON atoms BEGIN
                INSERT INTO atoms_fts(rowid, quote) VALUES (new.rowid, new.quote);
            END;",
        )
        .expect("schema");
        Store { conn }
    }

    fn last_atom_id(&self, session_id: &str) -> String {
        self.conn
            .query_row(
                "SELECT atom_id FROM atoms WHERE session_id = ? ORDER BY created_at DESC LIMIT 1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_default()
    }

    fn mark_superseded(&self, key: &str, new_id: &str) {
        self.conn
            .execute(
                "UPDATE atoms SET stale = 1, superseded_by = ?
                 WHERE topic_key = ? AND stale = 0 AND atom_id != ?",
                params![new_id, key, new_id],
            )
            .ok();
    }

    pub fn store_atom(
        &self,
        turn: &Turn,
        span: (usize, usize),
        priv_hex: &str,
        pub_hex: &str,
        tags: Vec<String>,
    ) -> Result<Atom, SpanMismatch> {
        let (start, end) = span;
        let bytes = turn.text.as_bytes();
        if start >= end || end > bytes.len() {
            return Err(SpanMismatch);
        }
        let quote = std::str::from_utf8(&bytes[start..end])
            .map_err(|_| SpanMismatch)?
            .to_string();
        if quote.trim().is_empty() {
            return Err(SpanMismatch);
        }

        let citation = Citation {
            agent: turn.agent.clone(),
            native_path: turn.native_path.clone(),
            session_id: turn.session_id.clone(),
            turn_id: turn.turn_id.clone(),
            span: (start, end),
            turn_sha256: turn.turn_sha256.clone(),
        };

        let citation_val = serde_json::to_value(&citation).unwrap();
        let ch = content_hash(&quote, &citation_val);
        let id = atom_id(&ch, pub_hex);
        let sig = sign(priv_hex, &ch);
        let prev = self.last_atom_id(&turn.session_id);
        let key = topic_key(&quote);
        let created_at = Utc::now().to_rfc3339();

        self.mark_superseded(&key, &id);

        self.conn
            .execute(
                "INSERT OR IGNORE INTO atoms
                 (atom_id, quote, citation_json, content_hash, signature, pubkey,
                  prev_atom_id, topic_key, superseded_by, stale, session_id, tags_json, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'',0,?9,?10,?11)",
                params![
                    id,
                    quote,
                    serde_json::to_string(&citation_val).unwrap(),
                    ch,
                    sig,
                    pub_hex,
                    prev,
                    key,
                    turn.session_id,
                    serde_json::to_string(&tags).unwrap(),
                    created_at,
                ],
            )
            .expect("insert");

        Ok(Atom {
            atom_id: id,
            quote,
            citation,
            content_hash: ch,
            signature: sig,
            pubkey: pub_hex.to_string(),
            prev_atom_id: prev,
            topic_key: key,
            superseded_by: String::new(),
            stale: false,
            tags,
            created_at,
        })
    }

    /// Resolve a full atom id, or an unambiguous id *prefix* (e.g. the short id
    /// `recall` prints), to the full atom id. Returns `None` if nothing matches or
    /// the prefix is ambiguous. Exact matches always win and are never treated as
    /// prefixes. A prefix containing SQL `LIKE` wildcards (`%`/`_`) is rejected, so
    /// it can only ever resolve to one stored id, never a wider set.
    fn resolve_id(&self, atom_id: &str) -> Option<String> {
        // Exact match first — a full id is unambiguous even if it prefixes others.
        if let Ok(id) = self.conn.query_row(
            "SELECT atom_id FROM atoms WHERE atom_id = ?",
            params![atom_id],
            |row| row.get::<_, String>(0),
        ) {
            return Some(id);
        }
        // Don't let a wildcard-bearing prefix fan out across the table.
        if atom_id.is_empty() || atom_id.contains(['%', '_']) {
            return None;
        }
        // Unique-prefix fallback. LIMIT 2 so we can tell unique from ambiguous.
        let mut stmt = self
            .conn
            .prepare("SELECT atom_id FROM atoms WHERE atom_id LIKE ?1 || '%' LIMIT 2")
            .ok()?;
        let ids: Vec<String> = stmt
            .query_map(params![atom_id], |row| row.get::<_, String>(0))
            .ok()?
            .flatten()
            .collect();
        match ids.as_slice() {
            [only] => Some(only.clone()),
            _ => None, // 0 = not found, >1 = ambiguous
        }
    }

    pub fn get(&self, atom_id: &str) -> Option<Atom> {
        let atom_id = self.resolve_id(atom_id)?;
        self.conn
            .query_row(
                "SELECT atom_id, quote, citation_json, content_hash, signature, pubkey,
                        prev_atom_id, topic_key, superseded_by, stale, tags_json, created_at
                 FROM atoms WHERE atom_id = ?",
                params![atom_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i32>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                    ))
                },
            )
            .ok()
            .map(
                |(id, quote, cit_json, ch, sig, pk, prev, tk, sup, stale, tags_json, created_at)| {
                    let citation: Citation = serde_json::from_str(&cit_json).unwrap();
                    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                    Atom {
                        atom_id: id,
                        quote,
                        citation,
                        content_hash: ch,
                        signature: sig,
                        pubkey: pk,
                        prev_atom_id: prev,
                        topic_key: tk,
                        superseded_by: sup,
                        stale: stale != 0,
                        tags,
                        created_at,
                    }
                },
            )
    }

    pub fn get_all(&self, include_stale: bool) -> Vec<Atom> {
        let stale_clause = if include_stale { "" } else { "WHERE stale = 0" };
        let sql = format!("SELECT atom_id FROM atoms {stale_clause} ORDER BY created_at ASC");
        let ids: Vec<String> = match self.conn.prepare(&sql) {
            Ok(mut stmt) => stmt
                .query_map([], |row| row.get(0))
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default(),
            Err(_) => return vec![],
        };
        ids.iter().filter_map(|id| self.get(id)).collect()
    }

    pub fn search(&self, query: &str, k: usize, include_stale: bool) -> Vec<Atom> {
        let stale_clause = if include_stale { "" } else { "AND a.stale = 0" };
        let sql = format!(
            "SELECT a.atom_id FROM atoms a
             JOIN atoms_fts f ON a.rowid = f.rowid
             WHERE atoms_fts MATCH ?1 {stale_clause}
             ORDER BY f.rank LIMIT ?2"
        );
        let ids: Vec<String> = match self.conn.prepare(&sql) {
            Ok(mut stmt) => stmt
                .query_map(params![query, k as i64], |row| row.get(0))
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default(),
            Err(_) => return vec![],
        };
        ids.iter().filter_map(|id| self.get(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytecite::{new_keypair, verify_sig};

    fn make_turn(session_id: &str, turn_id: &str, text: &str) -> Turn {
        use bytecite::sha256_hex;
        Turn {
            agent: "claude_code".into(),
            session_id: session_id.into(),
            turn_id: turn_id.into(),
            role: "assistant".into(),
            text: text.to_string(),
            native_path: "/tmp/test.jsonl".into(),
            turn_sha256: sha256_hex(text),
        }
    }

    #[test]
    fn store_search_roundtrip_signature_verifies() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        let turn = make_turn("s1", "t1", "Decision: use Ed25519 because it is fast and small.");
        let atom = store
            .store_atom(&turn, (0, turn.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        assert!(verify_sig(&atom.pubkey, &atom.content_hash, &atom.signature));
        let results = store.search("Ed25519", 5, false);
        assert!(!results.is_empty());
        assert_eq!(results[0].atom_id, atom.atom_id);
    }

    #[test]
    fn hash_chain_links_same_session() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        let t1 = make_turn("s1", "t1", "Decision: use postgres because it is reliable.");
        let t2 = make_turn("s1", "t2", "Constraint: prefer small binaries because of deploy size.");
        let a1 = store
            .store_atom(&t1, (0, t1.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        let a2 = store
            .store_atom(&t2, (0, t2.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        assert_eq!(a2.prev_atom_id, a1.atom_id);
    }

    #[test]
    fn rejects_empty_quote() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        let turn = make_turn("s1", "t1", "hello");
        assert!(store.store_atom(&turn, (0, 0), &priv_hex, &pub_hex, vec![]).is_err());
    }

    #[test]
    fn newer_supersedes_older_same_topic() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        // Both start with same 4 tokens → same topic_key "prefer-secure-connections-because"
        let t1 = make_turn("s1", "t1", "prefer secure connections because TLS is required.");
        let t2 = make_turn("s1", "t2", "prefer secure connections because encryption is mandatory.");
        let a1 = store
            .store_atom(&t1, (0, t1.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        let _a2 = store
            .store_atom(&t2, (0, t2.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        let live = store.search("secure", 10, false);
        let stale_all = store.search("secure", 10, true);
        assert_eq!(live.len(), 1, "only 1 live atom");
        assert_eq!(stale_all.len(), 2, "2 total including stale");
        let a1_refreshed = store.get(&a1.atom_id).unwrap();
        assert!(a1_refreshed.stale);
    }

    #[test]
    fn get_resolves_short_id_prefix() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        let turn = make_turn("s1", "t1", "Decision: use Ed25519 because it is fast and small.");
        let atom = store
            .store_atom(&turn, (0, turn.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        // The 8-char prefix `recall` prints must resolve to the full atom.
        let short = &atom.atom_id[..8];
        assert_eq!(store.get(short).unwrap().atom_id, atom.atom_id);
        // Full id still works, and a non-matching id is still None.
        assert_eq!(store.get(&atom.atom_id).unwrap().atom_id, atom.atom_id);
        assert!(store.get("ffffffff").is_none());
        // Wildcard prefixes must not fan out to a match.
        assert!(store.get("%").is_none());
        assert!(store.get("_").is_none());
    }

    #[test]
    fn get_ambiguous_prefix_returns_none() {
        let db = tempfile::NamedTempFile::new().unwrap();
        let store = Store::open(db.path().to_str().unwrap());
        let (priv_hex, pub_hex) = new_keypair();
        let a = store
            .store_atom(&make_turn("s1", "t1", "alpha one because reasons here."), (0, 30), &priv_hex, &pub_hex, vec![])
            .unwrap();
        let b = store
            .store_atom(&make_turn("s2", "t2", "beta two because other reasons."), (0, 31), &priv_hex, &pub_hex, vec![])
            .unwrap();
        // Longest common prefix of the two ids is ambiguous → None.
        let common: String = a
            .atom_id
            .chars()
            .zip(b.atom_id.chars())
            .take_while(|(x, y)| x == y)
            .map(|(x, _)| x)
            .collect();
        assert!(store.get(&common).is_none(), "shared prefix must be ambiguous");
        // But each full id still resolves to itself.
        assert_eq!(store.get(&a.atom_id).unwrap().atom_id, a.atom_id);
        assert_eq!(store.get(&b.atom_id).unwrap().atom_id, b.atom_id);
    }
}
