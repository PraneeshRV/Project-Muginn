use muginn_core::Atom;
use muginn_store::Store;
use muginn_verify::verify_atom;

// ── Task 2.1: Compiler trait ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Sentence {
    pub text: String,
    pub cited_atom_ids: Vec<String>,
}

pub struct CompiledDraft {
    pub sentences: Vec<Sentence>,
}

pub trait Compiler {
    fn compile(&self, topic: &str, atoms: &[Atom]) -> CompiledDraft;
}

/// NullCompiler: no LLM required. Each atom becomes its own sentence citing itself.
/// Used in tests and when no model endpoint is configured.
pub struct NullCompiler;

impl Compiler for NullCompiler {
    fn compile(&self, _topic: &str, atoms: &[Atom]) -> CompiledDraft {
        CompiledDraft {
            sentences: atoms
                .iter()
                .map(|a| Sentence {
                    text: a.quote.clone(),
                    cited_atom_ids: vec![a.atom_id.clone()],
                })
                .collect(),
        }
    }
}

/// LocalCompiler: POST to a local Ollama/llama.cpp endpoint (MUGINN_COMPILE_URL).
/// Falls back to NullCompiler if the endpoint is unreachable.
pub struct LocalCompiler {
    pub endpoint: String,
}

impl LocalCompiler {
    pub fn new() -> Self {
        LocalCompiler {
            endpoint: std::env::var("MUGINN_COMPILE_URL")
                .unwrap_or_else(|_| "http://localhost:11434/api/generate".to_string()),
        }
    }

    fn try_compile(&self, topic: &str, atoms: &[Atom]) -> anyhow::Result<CompiledDraft> {
        let atom_list: Vec<serde_json::Value> = atoms
            .iter()
            .map(|a| serde_json::json!({"id": a.atom_id, "quote": a.quote}))
            .collect();
        let prompt = format!(
            "Topic: {topic}\n\nAtoms:\n{}\n\nWrite prose ONLY using these atoms. \
             Tag each sentence with the atom-ids it derives from, as JSON at end of line: \
             [\"<id1>\",\"<id2>\"]. Do not introduce facts not in the atoms.",
            serde_json::to_string_pretty(&atom_list)?
        );
        let body = serde_json::json!({"model": "llama3", "prompt": prompt, "stream": false});
        let body_str = serde_json::to_string(&body)?;

        // Parse "http://host:port/path" minimally — no external url crate
        let url = &self.endpoint;
        let without_scheme = url.strip_prefix("http://").unwrap_or(url);
        let (host_port, path) = without_scheme.split_once('/').unwrap_or((without_scheme, "api/generate"));
        let path = format!("/{path}");
        let (host, port_str) = host_port.split_once(':').unwrap_or((host_port, "11434"));
        let port: u16 = port_str.parse().unwrap_or(11434);

        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect_timeout(
            &format!("{host}:{port}").parse()?,
            std::time::Duration::from_secs(5),
        )?;
        write!(
            stream,
            "POST {path} HTTP/1.0\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body_str.len(),
            body_str
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        let body_start = response.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        let resp_json: serde_json::Value = serde_json::from_str(&response[body_start..])?;
        let text = resp_json["response"].as_str().unwrap_or("").to_string();
        Ok(parse_llm_output(&text))
    }
}

impl Default for LocalCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler for LocalCompiler {
    fn compile(&self, topic: &str, atoms: &[Atom]) -> CompiledDraft {
        self.try_compile(topic, atoms)
            .unwrap_or_else(|_| NullCompiler.compile(topic, atoms))
    }
}

/// Parse LLM output: each line may end with a JSON array of atom-ids.
/// Lines without a tag get empty cited_atom_ids (will be quarantined).
fn parse_llm_output(text: &str) -> CompiledDraft {
    let mut sentences = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Try to find trailing JSON array: ["id1","id2"]
        if let Some(bracket) = line.rfind('[') {
            let (prose, tag) = line.split_at(bracket);
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(tag) {
                sentences.push(Sentence {
                    text: prose.trim().to_string(),
                    cited_atom_ids: ids,
                });
                continue;
            }
        }
        sentences.push(Sentence {
            text: line.to_string(),
            cited_atom_ids: vec![],
        });
    }
    CompiledDraft { sentences }
}

// ── Task 2.2: Citation enforcement ──────────────────────────────────────────

#[derive(Debug)]
pub enum SentenceVerdict {
    Verified(Sentence),
    Quarantined { sentence: Sentence, reason: String },
}

pub struct EnforcedPage {
    pub topic: String,
    pub verdicts: Vec<SentenceVerdict>,
    pub coverage: f32,
}

impl EnforcedPage {
    pub fn verified(&self) -> Vec<&Sentence> {
        self.verdicts
            .iter()
            .filter_map(|v| match v {
                SentenceVerdict::Verified(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    pub fn quarantined(&self) -> Vec<(&Sentence, &str)> {
        self.verdicts
            .iter()
            .filter_map(|v| match v {
                SentenceVerdict::Quarantined { sentence, reason } => Some((sentence, reason.as_str())),
                _ => None,
            })
            .collect()
    }
}

/// Enforce citation integrity on a compiled draft.
/// A sentence is Verified iff ALL its cited atom_ids exist in the store AND verify as "ok".
/// Uncited sentences or sentences with any bad/missing atom → Quarantined.
pub fn enforce(draft: &CompiledDraft, store: &Store) -> EnforcedPage {
    enforce_for_topic(draft, store, "")
}

pub fn enforce_for_topic(draft: &CompiledDraft, store: &Store, topic: &str) -> EnforcedPage {
    let total = draft.sentences.len();
    let mut verdicts = Vec::with_capacity(total);
    let mut verified_count = 0usize;

    for sentence in &draft.sentences {
        if sentence.cited_atom_ids.is_empty() {
            verdicts.push(SentenceVerdict::Quarantined {
                sentence: Sentence {
                    text: sentence.text.clone(),
                    cited_atom_ids: vec![],
                },
                reason: "no citation".to_string(),
            });
            continue;
        }

        let mut all_ok = true;
        let mut fail_reason = String::new();

        for atom_id in &sentence.cited_atom_ids {
            match store.get(atom_id) {
                None => {
                    all_ok = false;
                    fail_reason = format!("atom {atom_id} not found");
                    break;
                }
                Some(atom) => {
                    let status = verify_atom(&atom);
                    if status != "ok" {
                        all_ok = false;
                        fail_reason = format!("atom {atom_id} verify={status}");
                        break;
                    }
                }
            }
        }

        if all_ok {
            verified_count += 1;
            verdicts.push(SentenceVerdict::Verified(Sentence {
                text: sentence.text.clone(),
                cited_atom_ids: sentence.cited_atom_ids.clone(),
            }));
        } else {
            verdicts.push(SentenceVerdict::Quarantined {
                sentence: Sentence {
                    text: sentence.text.clone(),
                    cited_atom_ids: sentence.cited_atom_ids.clone(),
                },
                reason: fail_reason,
            });
        }
    }

    let coverage = if total == 0 {
        1.0
    } else {
        verified_count as f32 / total as f32
    };

    EnforcedPage {
        topic: topic.to_string(),
        verdicts,
        coverage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muginn_core::Turn;
    use bytecite::{new_keypair, sha256_hex};

    fn tmp_store() -> (Store, tempfile::NamedTempFile) {
        let f = tempfile::NamedTempFile::new().unwrap();
        let s = Store::open(f.path().to_str().unwrap());
        (s, f)
    }

    fn make_turn_with_fixture(text: &str, path: &str) -> Turn {
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(
            f,
            r#"{{"uuid":"t1","type":"assistant","message":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#,
            text.replace('"', "\\\"")
        )
        .unwrap();
        Turn {
            agent: "claude_code".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            role: "assistant".into(),
            text: text.to_string(),
            native_path: path.to_string(),
            turn_sha256: sha256_hex(text),
        }
    }

    #[test]
    fn null_compiler_n_atoms_n_sentences_each_self_cited() {
        let atoms: Vec<Atom> = (0..3)
            .map(|i| Atom {
                atom_id: format!("id{i}"),
                quote: format!("quote {i}"),
                citation: muginn_core::Citation {
                    agent: "claude_code".into(),
                    native_path: "/x".into(),
                    session_id: "s1".into(),
                    turn_id: format!("t{i}"),
                    span: (0, 7),
                    turn_sha256: "sha".into(),
                },
                content_hash: "ch".into(),
                signature: "sig".into(),
                pubkey: "pk".into(),
                prev_atom_id: String::new(),
                topic_key: "test".into(),
                superseded_by: String::new(),
                stale: false,
                tags: vec![],
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .collect();

        let draft = NullCompiler.compile("test", &atoms);
        assert_eq!(draft.sentences.len(), 3);
        for (i, s) in draft.sentences.iter().enumerate() {
            assert_eq!(s.cited_atom_ids.len(), 1);
            assert_eq!(s.cited_atom_ids[0], format!("id{i}"));
        }
    }

    #[test]
    fn enforce_all_real_atoms_coverage_1() {
        let (store, _f) = tmp_store();
        let (priv_hex, pub_hex) = new_keypair();
        let fixture = tempfile::NamedTempFile::new().unwrap();
        let text = "Decision: use Ed25519 because it is fast and small.";
        let turn = make_turn_with_fixture(text, fixture.path().to_str().unwrap());
        let atom = store
            .store_atom(&turn, (0, turn.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();

        let draft = NullCompiler.compile("test", &[atom]);
        let page = enforce(&draft, &store);
        assert!((page.coverage - 1.0).abs() < f32::EPSILON, "coverage={}", page.coverage);
        assert_eq!(page.quarantined().len(), 0);
    }

    #[test]
    fn enforce_fabricated_atom_id_quarantined() {
        let (store, _f) = tmp_store();
        let draft = CompiledDraft {
            sentences: vec![Sentence {
                text: "A hallucinated claim.".into(),
                cited_atom_ids: vec!["nonexistent_atom_id_abc123".to_string()],
            }],
        };
        let page = enforce(&draft, &store);
        assert_eq!(page.quarantined().len(), 1);
        assert!(page.coverage < 1.0);
    }

    #[test]
    fn poison_test_tampered_atom_quarantined() {
        let (store, _f) = tmp_store();
        let (priv_hex, pub_hex) = new_keypair();
        // Write fixture, store atom
        let fixture = tempfile::NamedTempFile::new().unwrap();
        let text = "Decision: use Ed25519 because it is fast and small.";
        let turn = make_turn_with_fixture(text, fixture.path().to_str().unwrap());
        let atom = store
            .store_atom(&turn, (0, turn.text.len()), &priv_hex, &pub_hex, vec![])
            .unwrap();
        // Tamper the source file — verify_atom will return "source-modified"
        std::fs::write(
            fixture.path(),
            r#"{"uuid":"t1","type":"assistant","message":{"content":[{"type":"text","text":"TAMPERED: use Ed25519 because it is fast and small."}]}}"#,
        )
        .unwrap();
        // Draft cites the atom, but source is now tampered → quarantined
        let draft = CompiledDraft {
            sentences: vec![Sentence {
                text: "Poisoned claim.".into(),
                cited_atom_ids: vec![atom.atom_id.clone()],
            }],
        };
        let page = enforce(&draft, &store);
        assert_eq!(page.quarantined().len(), 1, "tampered atom must be quarantined");
        assert!(page.coverage < 1.0);
    }
}
