use muginn_core::Turn;
use regex::Regex;

pub fn select_spans(turn: &Turn) -> Vec<(usize, usize)> {
    let salience = Regex::new(
        r"(?i)\b(decision|constraint|because|prefer|remember|TODO|FIXME)\b|[\w./-]+\.\w+:\d+"
    ).unwrap();

    let sentences = split_sentences(&turn.text);
    let mut spans = Vec::new();

    for sentence in sentences {
        if salience.is_match(sentence) {
            if let Some(pos) = find_sentence_offset(&turn.text, sentence) {
                let start = pos;
                let end = pos + sentence.len();
                spans.push((start, end));
            }
        }
    }
    spans
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    for &(byte_pos, ch) in &chars {
        if ch == '.' || ch == '!' || ch == '?' {
            let end = byte_pos + ch.len_utf8();
            let s = text[start..end].trim();
            if !s.is_empty() {
                sentences.push(&text[start..end]);
            }
            start = end;
        }
    }
    if start < text.len() {
        let s = text[start..].trim();
        if !s.is_empty() {
            sentences.push(&text[start..]);
        }
    }
    sentences
}

fn find_sentence_offset(text: &str, sentence: &str) -> Option<usize> {
    let text_ptr = text.as_ptr() as usize;
    let sent_ptr = sentence.as_ptr() as usize;
    if sent_ptr >= text_ptr && sent_ptr + sentence.len() <= text_ptr + text.len() {
        Some(sent_ptr - text_ptr)
    } else {
        text.find(sentence)
    }
}

pub fn topic_key(quote: &str) -> String {
    let tokens: Vec<String> = quote
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(4)
        .map(|s| s.to_lowercase())
        .collect();
    tokens.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::Turn;

    fn make_turn(text: &str) -> Turn {
        Turn {
            agent: "claude_code".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            role: "user".into(),
            text: text.to_string(),
            native_path: "/x".into(),
            turn_sha256: "sha".into(),
        }
    }

    #[test]
    fn keeps_salient_only() {
        let turn = make_turn("Fix the bug. Decision: use Ed25519 because it is fast.");
        let spans = select_spans(&turn);
        assert!(!spans.is_empty());
        for (s, e) in &spans {
            let slice = &turn.text.as_bytes()[*s..*e];
            let decoded = std::str::from_utf8(slice).unwrap();
            assert!(
                decoded.to_lowercase().contains("decision")
                    || decoded.to_lowercase().contains("because"),
            );
        }
    }

    #[test]
    fn byte_accurate_unicode() {
        let turn = make_turn("café note. TODO: add tests here.");
        let spans = select_spans(&turn);
        assert!(!spans.is_empty());
        for (s, e) in &spans {
            let slice = &turn.text.as_bytes()[*s..*e];
            let decoded = std::str::from_utf8(slice).expect("span must be valid UTF-8");
            assert!(decoded.to_lowercase().contains("todo"));
        }
    }

    #[test]
    fn empty_when_none_salient() {
        let turn = make_turn("Hello world. This is fine.");
        let spans = select_spans(&turn);
        assert!(spans.is_empty());
    }

    #[test]
    fn topic_key_correct() {
        assert_eq!(
            topic_key("Decision: use Ed25519 because it is fast."),
            "decision-use-ed25519-because"
        );
    }
}
