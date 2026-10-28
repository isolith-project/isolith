//! P7-M4-T02 rules demo on a REAL part.
//!
//! Loads a committed oep-geo part (default: the canonical bracket golden at
//! `crates/oep-geo/tests/golden/example-bracket.oepgeo.json`), evaluates the
//! committed knowledge set (`crates/oep-rules/tests/golden/iso-bracket.json`)
//! against it, and prints the deterministic JSON report on stdout.
//!
//! Everything printed is the report serialized by the crate itself; nothing
//! else goes to stdout, so `cargo run --example demo_rules -- <part> <ks>`
//! output is directly machine-consumable (the acceptance test reads it).
//!
//! Usage: cargo run --example demo_rules -- [PART_JSON] [KNOWLEDGE_JSON]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oep_geo::Document;
use oep_rules::{KnowledgeSet, PartContext};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let part_path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| here.join("../oep-geo/tests/golden/example-bracket.oepgeo.json"));
    let ks_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| here.join("tests/golden/iso-bracket.json"));

    match run(&part_path, &ks_path) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("rules demo failed: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(part_path: &Path, ks_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let part_text = fs::read_to_string(part_path)?;
    let doc = Document::from_json_pretty(&part_text)?;
    let ctx = PartContext::from_document(&doc, None)?;
    let knowledge_text = fs::read_to_string(ks_path)?;
    let ks = KnowledgeSet::from_json(&knowledge_text)?;
    let report = ks.evaluate(&ctx)?;
    Ok(report.to_json_pretty()?)
}
