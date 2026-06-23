use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use muginn_adapters::claude_code;
use muginn_crypto::new_keypair;
use muginn_render::render_cards;
use muginn_select::select_spans;
use muginn_store::Store;
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
