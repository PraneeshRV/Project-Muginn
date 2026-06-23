// Codex transcript adapter.
// Format: JSONL where each line is a message event.
// Relevant lines have `type == "message"` with `role` and `content` (string or array).
use muginn_core::Turn;
use bytecite::sha256_hex;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const AGENT: &str = "codex";

fn extract_text(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("");
    }
    // Also handle {input_text} / {output_text} top-level keys (older Codex format)
    if let Some(s) = content.get("input_text").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = content.get("output_text").and_then(|v| v.as_str()) {
        return s.to_string();
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
        if line.is_empty() { continue; }
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Only process "message" type events
        if obj.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let role = obj.get("role").and_then(|r| r.as_str()).unwrap_or("").to_string();
        let content = obj.get("content").cloned().unwrap_or(serde_json::Value::Null);
        let text = extract_text(&content);
        if text.is_empty() { continue; }
        let turn_id = obj.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
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
