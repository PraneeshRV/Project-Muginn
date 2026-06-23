//! `muginn.toml` config: agent transcript roots, vault root, compile endpoint, overrides.
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    /// Vault output root. (Reserved for `ingest-all --sync`.)
    #[serde(default)]
    #[allow(dead_code)]
    pub vault_root: Option<String>,
    /// Local compile endpoint (Ollama/llama.cpp). (Reserved for compile wiring.)
    #[serde(default)]
    #[allow(dead_code)]
    pub compile_url: Option<String>,
    /// Per-agent transcript root directories.
    #[serde(default)]
    pub agents: Vec<AgentRoot>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentRoot {
    pub name: String,
    pub root: String,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        let raw = std::fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&raw)?;
        Ok(cfg)
    }

    /// Walk each configured agent root, returning (agent, transcript_path) pairs.
    /// Recursively collects `*.jsonl` files under each root.
    pub fn discover_transcripts(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for agent in &self.agents {
            collect_jsonl(&expand_tilde(&agent.root), &agent.name, &mut out);
        }
        out
    }
}

/// Expand a leading `~` / `~/` to the user's home directory. Other paths pass through.
fn expand_tilde(p: &str) -> std::path::PathBuf {
    if p == "~" {
        return dirs_next::home_dir().unwrap_or_else(|| std::path::PathBuf::from("~"));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs_next::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(p)
}

fn collect_jsonl(dir: &Path, agent: &str, out: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, agent, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push((agent.to_string(), path.to_string_lossy().into_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("muginn.toml");
        let mut f = std::fs::File::create(&cfg_path).unwrap();
        write!(
            f,
            r#"
vault_root = "/home/u/vault"
compile_url = "http://localhost:11434/api/generate"

[[agents]]
name = "claude_code"
root = "{}/cc"

[[agents]]
name = "codex"
root = "{}/cx"
"#,
            dir.path().display(),
            dir.path().display()
        )
        .unwrap();

        let cfg = Config::load(&cfg_path).unwrap();
        assert_eq!(cfg.vault_root.as_deref(), Some("/home/u/vault"));
        assert_eq!(cfg.agents.len(), 2);
        assert_eq!(cfg.agents[0].name, "claude_code");
        assert_eq!(cfg.agents[1].name, "codex");
    }

    #[test]
    fn discovers_transcripts_from_two_agents() {
        let dir = tempfile::tempdir().unwrap();
        let cc = dir.path().join("cc");
        let cx = dir.path().join("cx");
        std::fs::create_dir_all(&cc).unwrap();
        std::fs::create_dir_all(&cx).unwrap();
        std::fs::write(cc.join("s1.jsonl"), "{}").unwrap();
        std::fs::write(cx.join("s2.jsonl"), "{}").unwrap();
        std::fs::write(cc.join("ignore.txt"), "x").unwrap();

        let cfg = Config {
            vault_root: None,
            compile_url: None,
            agents: vec![
                AgentRoot { name: "claude_code".into(), root: cc.to_string_lossy().into() },
                AgentRoot { name: "codex".into(), root: cx.to_string_lossy().into() },
            ],
        };
        let found = cfg.discover_transcripts();
        assert_eq!(found.len(), 2);
        let agents: Vec<&str> = found.iter().map(|(a, _)| a.as_str()).collect();
        assert!(agents.contains(&"claude_code"));
        assert!(agents.contains(&"codex"));
    }
}
