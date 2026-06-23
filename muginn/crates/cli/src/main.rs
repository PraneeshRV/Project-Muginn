use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use muginn_adapters::claude_code;
use muginn_crypto::new_keypair;
use muginn_render::render_cards;
use muginn_select::select_spans;
use muginn_store::Store;
use muginn_compile::{enforce_for_topic, NullCompiler, Compiler};
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
    let turns = match agent {
        "claude_code" => claude_code::iter_turns(path),
        other => {
            eprintln!("unknown agent: {other}");
            return 0;
        }
    };
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (priv_hex, pub_hex) = load_or_create_keypair().context("key")?;
    let store = Store::open(&db_path());

    match cli.cmd {
        Cmd::Ingest { agent, path } => {
            let n = ingest_file(&store, &agent, &path, &priv_hex, &pub_hex);
            println!("ingested {n} atoms from {path}");
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
    }
    Ok(())
}
