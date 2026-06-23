use muginn_core::Turn;
use muginn_crypto::sha256_hex;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const AGENT: &str = "claude_code";

fn flatten(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect();
    }
    String::new()
}

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
    for line in BufReader::new(f).lines().flatten() {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content = obj
            .get("message")
            .and_then(|m| m.get("content"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let text = flatten(&content);
        if text.is_empty() {
            continue;
        }
        out.push(Turn {
            agent: AGENT.into(),
            session_id: session_id.clone(),
            turn_id: obj
                .get("uuid")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string(),
            role: obj
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
            turn_sha256: sha256_hex(&text),
            text,
            native_path: path.to_string(),
        });
    }
    out
}
