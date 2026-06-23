pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod chatgpt;

use muginn_core::Turn;

/// Dispatch to the correct adapter by agent name.
pub fn iter_turns(agent: &str, path: &str) -> Vec<Turn> {
    match agent {
        "claude_code" => claude_code::iter_turns(path),
        "codex"       => codex::iter_turns(path),
        "cursor"      => cursor::iter_turns(path),
        "chatgpt"     => chatgpt::iter_turns(path),
        _             => vec![],
    }
}
