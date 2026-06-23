use muginn_adapters::chatgpt::iter_turns as chatgpt_turns;
use muginn_adapters::claude_code::iter_turns as cc_turns;
use muginn_adapters::codex::iter_turns as codex_turns;
use muginn_adapters::cursor::iter_turns as cursor_turns;
use bytecite::sha256_hex;

#[test]
fn claude_code_parse_sample_fixture() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/claude_code/sample.jsonl"
    );
    let turns = cc_turns(path);
    assert_eq!(turns.len(), 3);
    let ids: Vec<&str> = turns.iter().map(|t| t.turn_id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1", "u2"]);
    assert_eq!(turns[1].role, "assistant");
    assert!(turns[1].text.contains("Ed25519"));
    assert_eq!(turns[1].turn_sha256, sha256_hex(&turns[1].text));
}

#[test]
fn codex_skips_non_message_events() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/codex/sample.jsonl"
    );
    let turns = codex_turns(path);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].turn_id, "m1");
    assert_eq!(turns[1].role, "assistant");
    assert!(turns[1].text.contains("JWT"));
    assert_eq!(turns[1].turn_sha256, sha256_hex(&turns[1].text));
}

#[test]
fn cursor_parses_role_content() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cursor/sample.jsonl"
    );
    let turns = cursor_turns(path);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].turn_id, "c1");
    assert_eq!(turns[1].role, "assistant");
    assert!(turns[1].text.contains("repository pattern"));
    assert_eq!(turns[1].turn_sha256, sha256_hex(&turns[1].text));
}

#[test]
fn chatgpt_jsonl_parses_role_content() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/chatgpt/sample.jsonl"
    );
    let turns = chatgpt_turns(path);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].turn_id, "g1");
    assert_eq!(turns[1].role, "assistant");
    assert!(turns[1].text.contains("Result types"));
    assert_eq!(turns[1].turn_sha256, sha256_hex(&turns[1].text));
}
