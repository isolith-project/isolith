//! P2-M6-T01 standalone round-2 fuzz campaign runner.
//!
//! Usage (from the repo root):
//!
//! ```text
//! cargo run --release -p oep-cgr --example fuzz_campaign [SEED] [ITERS]
//! ```
//!
//! Exits 0 when the campaign is clean (zero panics across structure-depth,
//! huge-file and truncated-tail inputs with the pinned limits), non-zero when
//! any untrusted input made the walker panic. Report includes the
//! outcome-kind histogram, per-class input counts, and the peak node count
//! (memory-boundedness evidence).

use oep_cgr::fuzz::{run_campaign, CampaignConfig};
use oep_cgr::structure::{MAX_BLOCKS, MAX_BLOCK_LEN, MAX_DEPTH};

const DEFAULT_SEED: u64 = 0x4B_45_45_4C_F0_0D_52_02;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = args
        .get(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SEED);
    let iters = args
        .get(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(100_000);

    let cfg = CampaignConfig {
        seed,
        iters,
        huge_tail_bytes: 64 * 1024 * 1024,
        truncation_step: 7,
        depth_sweep_max: 5_000,
    };

    println!("=== oep-cgr fuzz campaign round 2 (P2-M6-T01) ===");
    println!(
        "seed={:#x}  iters={}  huge_tail={} MiB  depth_sweep=0..={}",
        seed,
        iters,
        cfg.huge_tail_bytes / (1024 * 1024),
        cfg.depth_sweep_max
    );
    println!(
        "limits: MAX_DEPTH={} MAX_BLOCKS={} MAX_BLOCK_LEN={} MiB",
        MAX_DEPTH,
        MAX_BLOCKS,
        MAX_BLOCK_LEN / (1024 * 1024)
    );

    let rep = run_campaign(&cfg);

    println!();
    println!("inputs     : {}", rep.inputs);
    println!("panics     : {}", rep.panics);
    println!(
        "first panic: {}",
        rep.first_panic.as_deref().unwrap_or("(none)")
    );
    println!("max nodes  : {} (<= {})", rep.max_nodes, MAX_BLOCKS);
    println!(
        "flags      : depth={} length={} truncated={}",
        rep.saw_depth_limit, rep.saw_length_limit, rep.saw_truncated
    );
    println!();
    println!("outcome histogram:");
    for (k, v) in &rep.outcomes {
        println!("  {k:<14} {v}");
    }
    println!("input classes:");
    for (k, v) in &rep.class_counts {
        println!("  {k:<14} {v}");
    }

    if rep.panics == 0 {
        println!();
        println!("CAMPAIGN: PASS — zero panics across all round-2 input classes.");
    } else {
        eprintln!();
        eprintln!("CAMPAIGN: FAIL — the walker panicked; see first_panic above.");
        std::process::exit(1);
    }
}
