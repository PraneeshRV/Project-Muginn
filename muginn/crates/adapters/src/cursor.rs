// Cursor transcript adapter.
// Format: JSONL with {role, content, id?} per line (OpenAI-style chat messages).
use muginn_core::Turn;
use bytecite::sha256_hex;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const AGENT: &str = "cursor";

pub fn iter_turns(path: &str) -> Vec<Turn> {
    let session_id = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    let mut idx = 0usize;
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let role = obj.get("role").and_then(|r| r.as_str()).unwrap_or("").to_string();
        if role.is_empty() { continue; }
        let text = match obj.get("content") {
            Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
            Some(v) if v.is_array() => v
                .as_array()
                .unwrap()
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(""),
            _ => continue,
        };
        if text.is_empty() { continue; }
        let turn_id = obj
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("turn-{idx}"));
        idx += 1;
        out.push(Turn {
            agent: AGENT.into(),
            session_id: session_id.clone(),
            turn_id,
            role,
            text: text.clone(),
            native_path: path.to_string(),
            turn_sha256: sha256_hex(&text),
        });
    }
    out
}
