use muginn_core::{Atom, Citation};
use std::path::{Path, PathBuf};

// ── Task 1.1: atom → Obsidian note ──────────────────────────────────────────

/// Write a live atom note to `<root>/vault/<workspace>/<project>/atoms/<id8>.md`.
/// Returns the path written.
pub fn write_atom_note(root: &Path, workspace: &str, project: &str, atom: &Atom) -> PathBuf {
    let dir = root.join("vault").join(workspace).join(project).join("atoms");
    std::fs::create_dir_all(&dir).expect("create atoms dir");
    let id8 = &atom.atom_id[..8.min(atom.atom_id.len())];
    let path = dir.join(format!("{id8}.md"));
    std::fs::write(&path, atom_note_body(atom, false, None)).expect("write atom note");
    path
}

fn atom_note_body(atom: &Atom, stale: bool, superseded_by_id8: Option<&str>) -> String {
    let (start, end) = atom.citation.span;
    let sup_line = superseded_by_id8
        .map(|id| format!("superseded_by: \"[[{id}]]\"\n"))
        .unwrap_or_default();
    format!(
        "---\natom_id: {}\nagent: {}\nsession: {}\nturn: {}\nspan: [{}, {}]\nturn_sha256: {}\ntopic_key: {}\nstale: {}\n{}created_at: {}\n---\n> \"{}\"\n\nSource: `{}:{}#{}` bytes [{},{}]\n",
        atom.atom_id,
        atom.citation.agent,
        atom.citation.session_id,
        atom.citation.turn_id,
        start, end,
        atom.citation.turn_sha256,
        atom.topic_key,
        stale,
        sup_line,
        atom.created_at,
        atom.quote,
        atom.citation.agent,
        atom.citation.session_id,
        atom.citation.turn_id,
        start, end,
    )
}

// ── Task 1.2: project/workspace resolution ──────────────────────────────────

/// Derive (workspace, project) from a Citation's native_path.
///
/// Rules (in order):
/// 1. Check for a `muginn.toml` in the path ancestry — use `[projects]` overrides if present.
/// 2. Claude Code paths `~/.claude/projects/<slug>/…` → project = slug.
/// 3. Git root basename → workspace; parent dir basename → project.
/// 4. Fallback: workspace = "default", project = stem of the path's parent dir.
pub fn resolve_project(citation: &Citation) -> (String, String) {
    let path = Path::new(&citation.native_path);

    // Check muginn.toml override anywhere in ancestry
    if let Some(override_val) = find_muginn_toml_override(path) {
        return override_val;
    }

    // Claude Code pattern: ~/.claude/projects/<slug>/
    if let Some(slug) = extract_claude_code_slug(path) {
        let workspace = find_git_root(path)
            .and_then(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "default".to_string());
        return (workspace, slug);
    }

    // Git root → workspace; immediate parent basename → project
    let workspace = find_git_root(path)
        .and_then(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "default".to_string());
    let project = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "default".to_string());
    (workspace, project)
}

fn extract_claude_code_slug(path: &Path) -> Option<String> {
    let mut components = path.components().peekable();
    let mut found_projects = false;
    while let Some(c) = components.next() {
        let s = c.as_os_str().to_string_lossy();
        if s == "projects" && found_projects == false {
            found_projects = true;
            // next component is the slug
            if let Some(slug_comp) = components.next() {
                return Some(slug_comp.as_os_str().to_string_lossy().into_owned());
            }
        }
        if s == ".claude" {
            found_projects = false; // reset, look for "projects" after ".claude"
            // Actually set a flag to look for projects after .claude
            // re-check next
            if let Some(next) = components.next() {
                if next.as_os_str() == "projects" {
                    if let Some(slug_comp) = components.next() {
                        return Some(slug_comp.as_os_str().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_file() { path.parent()? } else { path };
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn find_muginn_toml_override(path: &Path) -> Option<(String, String)> {
    let mut dir = if path.is_file() { path.parent()? } else { path };
    loop {
        let toml_path = dir.join("muginn.toml");
        if toml_path.exists() {
            // Minimal parse: look for workspace = "..." and project = "..."
            if let Ok(contents) = std::fs::read_to_string(&toml_path) {
                let ws = extract_toml_string(&contents, "workspace");
                let proj = extract_toml_string(&contents, "project");
                if ws.is_some() || proj.is_some() {
                    return Some((
                        ws.unwrap_or_else(|| "default".to_string()),
                        proj.unwrap_or_else(|| "default".to_string()),
                    ));
                }
            }
        }
        dir = dir.parent()?;
    }
}

fn extract_toml_string(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            if let Some(eq_pos) = trimmed.find('=') {
                let val = trimmed[eq_pos + 1..].trim().trim_matches('"');
                return Some(val.to_string());
            }
        }
    }
    None
}

// ── Task 1.3: supersession — non-destructive, greyed, diff ──────────────────

/// Write a stale atom note to `<root>/vault/<workspace>/<project>/_stale/<id8>.md`.
/// Includes `stale: true`, a `superseded_by` wikilink, and a diff block.
pub fn write_stale_note(
    root: &Path,
    workspace: &str,
    project: &str,
    old: &Atom,
    new: &Atom,
) -> PathBuf {
    let dir = root
        .join("vault")
        .join(workspace)
        .join(project)
        .join("_stale");
    std::fs::create_dir_all(&dir).expect("create _stale dir");
    let old_id8 = &old.atom_id[..8.min(old.atom_id.len())];
    let new_id8 = &new.atom_id[..8.min(new.atom_id.len())];
    let path = dir.join(format!("{old_id8}.md"));
    let mut body = atom_note_body(old, true, Some(new_id8));
    body.push_str(&render_diff(&old.quote, &new.quote));
    std::fs::write(&path, body).expect("write stale note");
    path
}

fn render_diff(old: &str, new: &str) -> String {
    format!(
        "\n```diff\n- {}\n+ {}\n```\n",
        old.replace('\n', "\n- "),
        new.replace('\n', "\n+ "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::{Atom, Citation};

    fn make_atom(id: &str, quote: &str, session_id: &str, turn_id: &str, topic: &str) -> Atom {
        Atom {
            atom_id: id.to_string(),
            quote: quote.to_string(),
            citation: Citation {
                agent: "claude_code".into(),
                native_path: "/home/user/.claude/projects/my-proj/session.jsonl".into(),
                session_id: session_id.into(),
                turn_id: turn_id.into(),
                span: (0, quote.len()),
                turn_sha256: "sha256abc".into(),
            },
            content_hash: "ch".into(),
            signature: "sig".into(),
            pubkey: "pk".into(),
            prev_atom_id: String::new(),
            topic_key: topic.into(),
            superseded_by: String::new(),
            stale: false,
            tags: vec![],
            created_at: "2026-06-23T00:00:00Z".into(),
        }
    }

    #[test]
    fn atom_note_frontmatter_fields_match() {
        let dir = tempfile::tempdir().unwrap();
        let atom = make_atom("abcdef1234567890", "Decision: use Ed25519.", "s1", "t1", "decision-use-ed25519");
        let path = write_atom_note(dir.path(), "myws", "myproj", &atom);

        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "abcdef12.md");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("atom_id: abcdef1234567890"));
        assert!(contents.contains("agent: claude_code"));
        assert!(contents.contains("session: s1"));
        assert!(contents.contains("turn: t1"));
        assert!(contents.contains("stale: false"));
        assert!(contents.contains("Decision: use Ed25519."));
    }

    #[test]
    fn resolve_project_claude_code_path() {
        let cit = Citation {
            agent: "claude_code".into(),
            native_path: "/home/user/.claude/projects/my-cool-project/session.jsonl".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            span: (0, 5),
            turn_sha256: "sha".into(),
        };
        let (_ws, proj) = resolve_project(&cit);
        assert_eq!(proj, "my-cool-project");
    }

    #[test]
    fn stale_note_has_wikilink_and_diff() {
        let dir = tempfile::tempdir().unwrap();
        let old = make_atom("aaaa1111bbbb2222", "Decision: use sqlite.", "s1", "t1", "decision-use-sqlite");
        let new = make_atom("cccc3333dddd4444", "Decision: use postgres.", "s1", "t2", "decision-use-postgres");

        let path = write_stale_note(dir.path(), "ws", "proj", &old, &new);

        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("stale: true"));
        assert!(contents.contains("[[cccc3333]]"));
        assert!(contents.contains("```diff"));
        assert!(contents.contains("- Decision: use sqlite."));
        assert!(contents.contains("+ Decision: use postgres."));
    }
}
