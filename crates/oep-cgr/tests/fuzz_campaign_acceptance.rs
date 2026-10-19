//! P2-M6-T01 acceptance test (TDD, RED first) — Fuzz campaign round 2:
//! structure depth, huge files, truncated tails.
//!
//! Written BEFORE `oep_cgr::structure` and `oep_cgr::fuzz` existed (RED: the
//! crate skeleton does not export these modules, so this file does not
//! compile). The contract, once GREEN, is:
//!
//! 1. `structure::walk_probe` is a tolerant, bounds-checked, depth-limited
//!    record walker: for ANY input it returns a classified `Outcome` (Ok,
//!    Truncated, LengthLimitExceeded, DepthLimitExceeded,
//!    BlockBudgetExceeded) and never panics.
//! 2. Structure depth: a chain of nested records past `MAX_DEPTH` yields
//!    `DepthLimitExceeded`; a very deep synthetic chain (200_000 levels) is
//!    walked iteratively (explicit stack) without a stack overflow.
//! 3. Huge files: a declared record length of `u32::MAX` yields
//!    `LengthLimitExceeded` (allocation-safe); a canonical CGR header plus a
//!    multi-megabyte tail parses to `Ok` with a bounded node count
//!    (slice-based walk, no per-block allocation). NOTE: all "huge" inputs
//!    here are synthesized from the PUBLIC observed header facts in
//!    `docs/re-notes/oep-cgr-header-parse.md`; no proprietary bytes are
//!    created or committed.
//! 4. Truncated tails: every truncation point of the canonical spine and of
//!    a nested-record chain yields a non-panicking classified outcome;
//!    truncating a payload mid-record yields `Truncated`, truncating exactly
//!    on a record boundary yields `Ok`.
//! 5. The campaign (`fuzz::run_campaign`) is deterministic for a fixed seed,
//!    reports zero panics, and exercised every round-2 class (saw flags).
//! 6. Real material (when `OEP_CGR_SAMPLES` supplies the never-committed
//!    corpus): every real `.cgr` is classified with zero panics (PASS) or
//!    the leg skips so CI without the private bins stays green.
//!
//! Clean-room rule honoured here (board.program doctrine): real CGR binaries
//! are NEVER committed. Every synthesized input below is derived from the
//! observed, published format facts (magic `01 00 00 00`, 0x60 zero
//! preamble, landscape u32, 16-byte invariant TEMPLATE) and deterministic
//! synthetic payload patterns.

use oep_cgr::fuzz::{self, CampaignConfig, CampaignReport};
use oep_cgr::structure::{walk_probe, Limits, Outcome, MAX_BLOCKS, MAX_DEPTH};

/// Fixed seed for a reproducible campaign.
const SEED: u64 = 0x4B_45_45_4C_F0_0D_52_02; // "KEELF0DR2"

// ---------------------------------------------------------------------------
// 1. Walker safety invariant: any input is classified, never a panic.
// ---------------------------------------------------------------------------

#[test]
fn walker_never_panics_on_arbitrary_bytes() {
    let limits = Limits::default();
    // Fuzz-ish deterministic sweep across sizes 0..=4096 with rolling content.
    for n in 0..=4096usize {
        let bytes: Vec<u8> = (0..n).map(|i| ((i * 31) ^ n) as u8).collect();
        let _ = walk_probe(&bytes, &limits); // must return, not panic
    }
    // A few tiny hand-built cases.
    for bytes in [
        Vec::new(),
        vec![0u8; 1],
        vec![0xFF; 7],
        vec![0x01, 0x00, 0x00, 0x00],
        vec![0xFF; 8],
    ] {
        let _ = walk_probe(&bytes, &limits);
    }
}

// ---------------------------------------------------------------------------
// 2. Structure depth.
// ---------------------------------------------------------------------------

#[test]
fn depth_at_cap_parses_and_past_cap_is_rejected() {
    let limits = Limits::default();
    let at_cap = fuzz::nested_records(MAX_DEPTH);
    let rep = walk_probe(&at_cap, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "depth == MAX_DEPTH must parse");
    assert_eq!(rep.max_depth_reached, MAX_DEPTH);

    let over_cap = fuzz::nested_records(MAX_DEPTH + 1);
    let rep = walk_probe(&over_cap, &limits);
    match rep.outcome {
        Outcome::DepthLimitExceeded { depth } => {
            assert_eq!(depth, MAX_DEPTH + 1, "limit trips exactly at cap+1")
        }
        other => panic!("expected DepthLimitExceeded, got {:?}", other),
    }
}

#[test]
fn extreme_depth_is_iterative_no_stack_overflow() {
    // 200_000 nested records would smash a recursive-walker call stack.
    // The campaign walker uses an explicit stack, so this must complete and
    // report a DepthLimitExceeded (217/256) — definitely NOT a SIGSEGV.
    let limits = Limits::default();
    let deep = fuzz::nested_records(200_000);
    let rep = walk_probe(&deep, &limits);
    match rep.outcome {
        Outcome::DepthLimitExceeded { depth } => {
            assert_eq!(depth, MAX_DEPTH + 1)
        }
        other => panic!("expected DepthLimitExceeded, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 3. Huge files.
// ---------------------------------------------------------------------------

const MIB: usize = 1024 * 1024;

#[test]
fn declared_u32max_length_is_rejected_without_allocating() {
    let limits = Limits::default();
    // One record: tag=1, len=u32::MAX — any implementation that allocates the
    // declared payload would try to grab ~4 GiB. It must bail as
    // LengthLimitExceeded at the offending record.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let rep = walk_probe(&bytes, &limits);
    match rep.outcome {
        Outcome::LengthLimitExceeded { offset, .. } => {
            assert_eq!(
                offset, 8,
                "flag the record header immediately after tag/len"
            )
        }
        other => panic!("expected LengthLimitExceeded, got {:?}", other),
    }
}

#[test]
fn observed_landscape_word_is_refused_as_length_limit() {
    // FUZZ FINDING (deterministic, real-adjacent): under the round-2 record
    // framing, the OBSERVED landscape u32 at header offset 0x64 reads, on the
    // natural 8-byte record alignment, as the `len` field of the record whose
    // tag sits at 0x60. The observed value 0x10CD3600 (281,884,160) is a
    // 281 MB block length, far above MAX_BLOCK_LEN (64 MiB) — so the walker
    // REFUSES it as LengthLimitExceeded at offset 0x68 instead of allocating
    // ~281 MB. This is exactly the memory-bomb defence round 2 must prove.
    let limits = Limits::default();
    let rep = walk_probe(&fuzz::canonical_header(), &limits);
    match rep.outcome {
        Outcome::LengthLimitExceeded { offset, declared } => {
            assert_eq!(offset, 0x68, "refusal happens at the payload start");
            assert_eq!(
                declared, 0x10CD_3600,
                "the refused length is the observed landscape literal"
            );
        }
        other => panic!("expected LengthLimitExceeded, got {:?}", other),
    }
}

#[test]
fn huge_zero_container_is_classified_and_node_bounded() {
    let limits = Limits::default();
    // A zeros-only container is a stream of (tag=0, len=0) records, 8 bytes
    // each. 8 MiB -> 1_048_576 records, above MAX_BLOCKS: the walker must
    // stop with BlockBudgetExceeded and may NOT visit more than MAX_BLOCKS —
    // never OOM, never unbounded CPU.
    let bytes = vec![0u8; 8 * MIB];
    let rep = walk_probe(&bytes, &limits);
    assert!(
        matches!(
            rep.outcome,
            Outcome::Ok | Outcome::BlockBudgetExceeded { .. }
        ),
        "a huge zero container must classify, not OOM or panic (got {:?})",
        rep.outcome
    );
    // Node budget is the memory-boundedness proof: even a huge file cannot
    // push the walker past MAX_BLOCKS visited records.
    assert!(rep.nodes <= MAX_BLOCKS, "node budget honoured");

    // A modest container (64 KiB zeroes -> 8_192 records) stays comfortably
    // under the budget and must parse cleanly.
    let ok = vec![0u8; 64 * 1024];
    let rep = walk_probe(&ok, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "sub-budget huge container parses");
    assert_eq!(rep.nodes, 8_192);
}

// ---------------------------------------------------------------------------
// 4. Truncated tails.
// ---------------------------------------------------------------------------

#[test]
fn every_truncation_classifies_never_panics() {
    let limits = Limits::default();
    let bases: [Vec<u8>; 2] = [fuzz::canonical_header(), fuzz::nested_records(3)];
    for base in bases {
        let full = base.len();
        for k in 0..=full {
            // Truncate at every prefix length, including 0 and the full file.
            let _ = walk_probe(&base[..k], &limits); // must never panic
        }
        // And every single byte deleted (sliding shrink) sanity.
        for del in 0..full {
            let mut b = base.clone();
            b.remove(del);
            let _ = walk_probe(&b, &limits);
        }
    }
}

#[test]
fn truncation_mid_record_is_truncated_on_boundary_is_ok() {
    let limits = Limits::default();
    // nested_records(3): R0[tag,len,payload R1[tag,len,payload R2[tag,len]]]
    // sizes: R2=8, R1=16, R0=24 (total 24).
    let chain = fuzz::nested_records(3);
    assert_eq!(chain.len(), 24, "sanity: 3-level chain is 24 bytes");

    // Cut inside R1's record header (after R0's own 8-byte header): the walk
    // enters R0, then fails reading R1's header -> Truncated.
    let rep = walk_probe(&chain[..10], &limits);
    assert!(
        matches!(rep.outcome, Outcome::Truncated { .. }),
        "mid-record header cut must classify as Truncated (got {:?})",
        rep.outcome
    );

    // Cut exactly at the R0 record boundary -> Ok.
    let rep = walk_probe(&chain[..24], &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "boundary cut must parse");

    // Cut at 22: R0's payload (8..24) no longer fits in the 22-byte input,
    // so the walk classifies Truncated (the payload is a truncated tail).
    let rep = walk_probe(&chain[..22], &limits);
    assert!(
        matches!(rep.outcome, Outcome::Truncated { .. }),
        "overrunning payload must classify as Truncated (got {:?})",
        rep.outcome
    );
}

// ---------------------------------------------------------------------------
// 5. Campaign: determinism, zero panics, all classes exercised.
// ---------------------------------------------------------------------------

#[test]
fn campaign_is_deterministic_zero_panics_and_covers_all_classes() {
    let cfg = CampaignConfig {
        seed: SEED,
        iters: 5_000,
        huge_tail_bytes: 64 * 1024,
        truncation_step: 11,
        depth_sweep_max: 300,
    };
    let a = fuzz::run_campaign(&cfg);
    let b = fuzz::run_campaign(&cfg);

    assert_eq!(
        impl_signature(&a),
        impl_signature(&b),
        "same seed must give an identical campaign report"
    );
    assert_eq!(a.panics, 0, "zero panics across the campaign");
    assert_eq!(a.inputs, b.inputs);
    assert!(
        a.inputs > 1_000,
        "campaign must process a real corpus of inputs"
    );
    assert!(a.saw_depth_limit, "structure-depth class must be exercised");
    assert!(
        a.saw_length_limit,
        "huge-file (length-bomb) class must be exercised"
    );
    assert!(a.saw_truncated, "truncated-tail class must be exercised");
    assert!(
        a.max_nodes <= MAX_BLOCKS,
        "walker node budget never exceeded"
    );

    // No panic-capable leg may be empty.
    for class in ["depth", "huge", "truncated", "mutation", "canonical"] {
        assert!(
            a.class_counts.get(class).map_or(0, |&n| n) > 0,
            "class '{class}' produced inputs"
        );
    }
}

/// Serialize the observables of a report into one comparable string.
fn impl_signature(r: &CampaignReport) -> String {
    format!(
        "inputs={} panics={} outcomes={:?} classes={:?} flags={}{}{} max_nodes={}",
        r.inputs,
        r.panics,
        r.outcomes,
        r.class_counts,
        r.saw_depth_limit,
        r.saw_length_limit,
        r.saw_truncated,
        r.max_nodes
    )
}

// ---------------------------------------------------------------------------
// 6. Real material.
// ---------------------------------------------------------------------------

#[test]
fn real_material_campaign_skips_without_samples_passes_with_them() {
    let cfg = CampaignConfig {
        seed: SEED,
        iters: 2_000,
        huge_tail_bytes: 128 * 1024,
        truncation_step: 13,
        depth_sweep_max: 128,
    };
    let dirs = fuzz::sample_dirs();
    let rep = fuzz::run_campaign_on_samples(&cfg, &dirs);
    if rep.files_scanned == 0 {
        eprintln!(
            "SKIP  no real CGR samples found (set OEP_CGR_SAMPLES=<dirs>); tolerant CI-safe skip"
        );
        return;
    }
    eprintln!(
        "info  real-material campaign: {} real file(s) scanned, {} total inputs walked",
        rep.files_scanned, rep.inputs
    );
    assert_eq!(rep.panics, 0, "zero panics on REAL CGR material");
    assert!(
        rep.files_scanned >= 1,
        "at least one real sample must be scanned when samples are available"
    );
    eprintln!(
        "PASS  real-material campaign clean ({} files, {} inputs, {} samples)",
        rep.files_scanned,
        rep.inputs,
        rep.outcomes.values().sum::<u64>()
    );
}
