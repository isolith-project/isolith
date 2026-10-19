//! Standalone runner for the round-1 fuzz campaign (P2-M1-T05) on the
//! pinned STABLE toolchain.
//!
//! Usage:
//!   cargo run -p oep-cgr --example fuzz_campaign [-- --iters N --huge ...]
//!
//! With `OEP_CGR_SAMPLES=<dirs>` set, the real-material leg also runs every
//! real `.cgr` it can reach through the header parser and reports
//! `files_scanned` / `panics`. A panic count above 0 is a fail signal.

use oep_cgr::fuzz_round1::{self, CampaignConfig};
use std::path::PathBuf;

fn main() {
    println!("== oep-cgr fuzz round 1 (header parser) ==");
    let cfg = CampaignConfig::default();
    let mut report = fuzz_round1::run_campaign(&cfg);
    println!(
        "deterministic campaign: {} inputs, {} panics, outcomes={:?}",
        report.inputs, report.panics, report.outcomes
    );

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        dirs.extend(env.split([':', ',', ';']).map(PathBuf::from));
    }
    if !dirs.is_empty() {
        report = fuzz_round1::run_campaign_on_samples(&cfg, &dirs);
        println!(
            "real-material leg: {} file(s) scanned, {} total inputs, {} panics",
            report.files_scanned, report.inputs, report.panics
        );
    }
    if report.files_scanned > 0 {
        println!(
            "PASS  zero panics on real material ({})",
            report.files_scanned
        );
    } else {
        println!("note  no real samples (set OEP_CGR_SAMPLES) — deterministic legs only");
    }
    if report.panics > 0 {
        println!("FAIL  {} panics — contract violated", report.panics);
        std::process::exit(1);
    }
}
