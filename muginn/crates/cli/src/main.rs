mod config;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use muginn_compile::{enforce_for_topic, Compiler, NullCompiler};
use muginn_crypto::new_keypair;
use muginn_render::render_cards;
use muginn_select::select_spans;
use muginn_store::Store;
use muginn_vault::{resolve_project, write_atom_note, write_page_note, write_stale_note};
use muginn_verify::verify_atom;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[derive(Parser)]
#[command(name = "muginn", about = "Verifiable memory for AI agents")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Ingest {
        agent: String,
        path: String,
    },
    /// Ingest all configured agent roots from muginn.toml.
    IngestAll {
        #[arg(long, default_value = "muginn.toml")]
        config: String,
    },
    /// Run the MCP server over stdio.
    Serve,
    Recall {
        query: String,
        #[arg(short, long, default_value = "5")]
        k: usize,
    },
    Sync {
        #[arg(long, default_value = ".")]
        root: String,
    },
    Compile {
        topic: String,
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Run eval harness: provenance coverage, poison rejection, staleness, selector recall.
    Eval {
        /// Path to labeled selector fixture (JSONL: {"text":"...","salient":bool})
        #[arg(long)]
        selector_fixture: Option<String>,
        /// Number of fabricated atoms to inject for poison rejection test
        #[arg(long, default_value = "10")]
        poison_n: usize,
    },
}

fn key_path() -> std::path::PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".muginn.key")
}

fn db_path() -> String {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".muginn.db")
        .to_string_lossy()
        .into_owned()
}

fn load_or_create_keypair() -> Result<(String, String)> {
    let kp = key_path();
    if kp.exists() {
        let contents = fs::read_to_string(&kp)?;
        let parts: Vec<&str> = contents.trim().split('\n').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    let (priv_hex, pub_hex) = new_keypair();
    fs::write(&kp, format!("{}\n{}", priv_hex, pub_hex))?;
    fs::set_permissions(&kp, fs::Permissions::from_mode(0o600))?;
    Ok((priv_hex, pub_hex))
}

fn ingest_file(store: &Store, agent: &str, path: &str, priv_hex: &str, pub_hex: &str) -> usize {
    let turns = muginn_adapters::iter_turns(agent, path);
    if turns.is_empty() && !std::path::Path::new(path).exists() {
        eprintln!("unknown agent or missing file: {agent} {path}");
        return 0;
    }
    let mut count = 0;
    for turn in &turns {
        for span in select_spans(turn) {
            if store.store_atom(turn, span, priv_hex, pub_hex, vec![]).is_ok() {
                count += 1;
            }
        }
    }
    count
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (priv_hex, pub_hex) = load_or_create_keypair().context("key")?;

    // Serve runs the MCP server, which owns its own Store.
    if let Cmd::Serve = cli.cmd {
        let server = muginn_server::MuginnServer::new(&db_path(), priv_hex, pub_hex);
        return muginn_server::serve_stdio(server).await;
    }

    let store = Store::open(&db_path());

    match cli.cmd {
        Cmd::Serve => unreachable!(),
        Cmd::Ingest { agent, path } => {
            let n = ingest_file(&store, &agent, &path, &priv_hex, &pub_hex);
            println!("ingested {n} atoms from {path}");
        }
        Cmd::IngestAll { config } => {
            let cfg = Config::load(std::path::Path::new(&config))
                .with_context(|| format!("load config {config}"))?;
            let transcripts = cfg.discover_transcripts();
            let mut total = 0usize;
            let mut files = 0usize;
            for (agent, path) in &transcripts {
                let n = ingest_file(&store, agent, path, &priv_hex, &pub_hex);
                total += n;
                files += 1;
            }
            println!("ingested {total} atoms from {files} transcripts across {} agents", cfg.agents.len());
        }
        Cmd::Sync { root } => {
            let root_path = std::path::Path::new(&root);
            let all_atoms = store.get_all(true);
            // Separate live and stale
            let live: Vec<_> = all_atoms.iter().filter(|a| !a.stale).collect();
            let stale: Vec<_> = all_atoms.iter().filter(|a| a.stale).collect();
            let mut written = 0usize;
            for atom in &live {
                let (ws, proj) = resolve_project(&atom.citation);
                write_atom_note(root_path, &ws, &proj, atom);
                written += 1;
            }
            for atom in &stale {
                let (ws, proj) = resolve_project(&atom.citation);
                // Find the superseding atom
                let new_atom = all_atoms
                    .iter()
                    .find(|a| a.atom_id == atom.superseded_by)
                    .cloned();
                if let Some(new) = new_atom {
                    write_stale_note(root_path, &ws, &proj, atom, &new);
                } else {
                    // No superseder found — write as stale without diff
                    let dummy = muginn_core::Atom {
                        atom_id: atom.superseded_by.clone(),
                        quote: String::new(),
                        citation: atom.citation.clone(),
                        content_hash: String::new(),
                        signature: String::new(),
                        pubkey: String::new(),
                        prev_atom_id: String::new(),
                        topic_key: String::new(),
                        superseded_by: String::new(),
                        stale: false,
                        tags: vec![],
                        created_at: atom.created_at.clone(),
                    };
                    write_stale_note(root_path, &ws, &proj, atom, &dummy);
                }
                written += 1;
            }
            println!("synced {written} atoms to {root}");
        }
        Cmd::Compile { topic, root } => {
            let atoms = store.search(&topic, 50, false);
            if atoms.is_empty() {
                println!("no atoms for topic: {topic}");
                return Ok(());
            }
            let draft = NullCompiler.compile(&topic, &atoms);
            let page = enforce_for_topic(&draft, &store, &topic);
            println!(
                "compiled {} sentences, coverage {:.0}%, {} quarantined",
                page.verdicts.len(),
                page.coverage * 100.0,
                page.quarantined().len()
            );
            // Write page note using first atom's project resolution
            let (ws, proj) = resolve_project(&atoms[0].citation);
            let root_path = std::path::Path::new(&root);
            let path = write_page_note(root_path, &ws, &proj, &topic, &page, "null");
            println!("page written: {}", path.display());
        }
        Cmd::Recall { query, k } => {
            let atoms = store.search(&query, k, false);
            if atoms.is_empty() {
                println!("no results for: {query}");
                return Ok(());
            }
            println!("{}", render_cards(&atoms));
            for atom in &atoms {
                let status = verify_atom(atom);
                let id8 = &atom.atom_id[..8.min(atom.atom_id.len())];
                println!("  verify[{id8}] = {status}");
            }
        }
        Cmd::Eval { selector_fixture, poison_n } => {
            println!("muginn eval\n");

            // ── Selector recall / FP rate ────────────────────────────────────
            let fixture_path = selector_fixture.unwrap_or_else(|| {
                // Default: bundled fixture, relative to the workspace root (run from muginn/)
                "crates/eval/fixtures/labeled.jsonl".to_string()
            });
            if std::path::Path::new(&fixture_path).exists() {
                let rows: Vec<serde_json::Value> = std::fs::read_to_string(&fixture_path)
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                let sel = muginn_eval::eval_selector(&rows);
                println!("selector recall/fp  (fixture: {fixture_path})");
                println!("  recall            {:.2}", sel.recall);
                println!("  false_positive    {:.2}", sel.false_positive_rate);
                println!("  salient           {}", sel.n_salient);
                println!("  non-salient       {}", sel.n_nonsalient);
            } else {
                println!("selector fixture not found at {fixture_path} — skipping selector eval");
                println!("  hint: run from project root or pass --selector-fixture <path>");
            }
            println!();

            // ── Provenance coverage + format benchmark ────────────────────────
            let all_atoms = store.get_all(false);
            if all_atoms.is_empty() {
                println!("provenance coverage  (no atoms in store — run 'muginn ingest' first)");
                println!("  coverage          n/a");
            } else {
                let prov = muginn_eval::eval_provenance_coverage(&all_atoms, &store);
                println!("provenance coverage  ({} atoms)", all_atoms.len());
                println!("  coverage          {:.2}  (target ≥0.95)", prov.coverage);
                println!("  verified          {}", prov.n_verified);
                println!("  quarantined       {}", prov.n_quarantined);
                println!();

                let fmt = muginn_eval::eval_format_overhead(&all_atoms);
                println!("format overhead  (markdown cards vs JSON)");
                println!("  md chars          {}", fmt.md_chars);
                println!("  json chars        {}", fmt.json_chars);
                println!("  json/md ratio     {:.2}x", fmt.json_overhead_ratio);
            }
            println!();

            // ── Poison rejection ─────────────────────────────────────────────
            let poison = muginn_eval::eval_poison_rejection(&all_atoms, poison_n, &store);
            println!("poison rejection  ({} fabricated atoms injected)", poison_n);
            println!("  quarantined       {}/{}", poison.n_quarantined, poison.n_injected);
            println!("  rejection rate    {:.2}  (target 1.00)", poison.rejection_rate);
            println!();

            // ── Staleness ────────────────────────────────────────────────────
            let all_with_stale = store.get_all(true);
            let n_stale = all_with_stale.iter().filter(|a| a.stale).count();
            let n_live = all_with_stale.iter().filter(|a| !a.stale).count();
            let labeled: Vec<(muginn_core::Atom, bool)> = all_with_stale
                .into_iter()
                .map(|a| {
                    let expected_stale = !a.superseded_by.is_empty();
                    (a, expected_stale)
                })
                .collect();
            let stal = muginn_eval::eval_staleness(&labeled);
            println!("staleness  (live={n_live}, stale={n_stale})");
            println!("  precision         {:.2}", stal.precision);
            println!("  recall            {:.2}", stal.recall);
            println!();

            // ── LongMemEval / LoCoMo parity (offline subset) ─────────────────
            let fixture_pairs = [
                ("crates/eval/fixtures/longmemeval.jsonl", "LongMemEval-offline"),
                ("crates/eval/fixtures/locomo.jsonl", "LoCoMo-offline"),
            ];
            for (path, name) in fixture_pairs {
                if std::path::Path::new(path).exists() {
                    let content = std::fs::read_to_string(path).unwrap_or_default();
                    let rows = muginn_eval::parity::parse_fixture(&content);
                    let m = muginn_eval::parity::run_parity(&rows, name, 5);
                    println!("{name}  ({} questions)", m.n_questions);
                    println!("  hit@1             {:.2}", m.hit_at_1);
                    println!("  hit@3             {:.2}", m.hit_at_3);
                    println!("  hit@5             {:.2}", m.hit_at_k);
                } else {
                    println!("{name}  fixture not found at {path}");
                }
                println!();
            }
        }
    }
    Ok(())
}
