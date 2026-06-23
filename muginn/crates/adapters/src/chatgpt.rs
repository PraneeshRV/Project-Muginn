// ChatGPT transcript adapter.
// Supports two formats:
//   1. ChatGPT export JSON: { "title": "...", "mapping": { id: { message: {id, role, content} } } }
//   2. Simple JSONL: one {role, content, id?} per line
use muginn_core::Turn;
use muginn_crypto::sha256_hex;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const AGENT: &str = "chatgpt";

fn content_to_text(content: &serde_json::Value) -> String {
    // content.parts array (ChatGPT export)
    if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
        return parts
            .iter()
            .filter_map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join("");
    }
    // Plain string
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    // {text: "..."} block
    if let Some(s) = content.get("text").and_then(|t| t.as_str()) {
        return s.to_string();
    }
    String::new()
}

fn session_id(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

pub fn iter_turns(path: &str) -> Vec<Turn> {
    let sid = session_id(path);
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    // Try ChatGPT export JSON first (has "mapping" key)
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&raw) {
        if let Some(mapping) = obj.get("mapping").and_then(|m| m.as_object()) {
            // The mapping is keyed by node id, which iterates in id order — NOT conversation
            // order. Capture each message's `create_time` and sort ascending so the hash
            // chain and staleness ordering follow the real conversation timeline.
            let mut rows: Vec<(f64, Turn)> = Vec::new();
            for (id, node) in mapping {
                let msg = match node.get("message") {
                    Some(m) if !m.is_null() => m,
                    _ => continue,
                };
                let role = msg.get("author")
                    .and_then(|a| a.get("role"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                if role.is_empty() || role == "system" { continue; }
                let content = msg.get("content").unwrap_or(&serde_json::Value::Null);
                let text = content_to_text(content);
                if text.is_empty() { continue; }
                let create_time = msg.get("create_time").and_then(|t| t.as_f64()).unwrap_or(0.0);
                rows.push((create_time, Turn {
                    agent: AGENT.into(),
                    session_id: sid.clone(),
                    turn_id: id.clone(),
                    role,
                    text: text.clone(),
                    native_path: path.to_string(),
                    turn_sha256: sha256_hex(&text),
                }));
            }
            rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            return rows.into_iter().map(|(_, t)| t).collect();
        }
    }

    // Fallback: JSONL
    let mut out = Vec::new();
    let mut idx = 0usize;
    for line in BufReader::new(raw.as_bytes()).lines().flatten() {
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let role = obj.get("role").and_then(|r| r.as_str()).unwrap_or("").to_string();
        if role.is_empty() { continue; }
        let content = obj.get("content").cloned().unwrap_or(serde_json::Value::Null);
        let text = content_to_text(&content);
        if text.is_empty() { continue; }
        let turn_id = obj
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("turn-{idx}"));
        idx += 1;
        out.push(Turn {
            agent: AGENT.into(),
            session_id: sid.clone(),
            turn_id,
            role,
            text: text.clone(),
            native_path: path.to_string(),
            turn_sha256: sha256_hex(&text),
        });
    }
    out
}
