use muginn_adapters::claude_code::iter_turns;
use muginn_crypto::sha256_hex;

#[test]
fn parse_sample_fixture() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/claude_code/sample.jsonl"
    );
    let turns = iter_turns(path);
    assert_eq!(turns.len(), 3);
    let ids: Vec<&str> = turns.iter().map(|t| t.turn_id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1", "u2"]);
    assert_eq!(turns[1].role, "assistant");
    assert!(turns[1].text.contains("Ed25519"));
    assert_eq!(turns[1].turn_sha256, sha256_hex(&turns[1].text));
}
