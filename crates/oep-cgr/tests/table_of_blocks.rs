//! P2-M1-T03 acceptance test (TDD, RED first) — Central table-of-blocks
//! walker (observed structure) with bounds checks.
//!
//! Written BEFORE `oep_cgr::blocks` existed (RED: the skeleton crate does not
//! export this module, so this file does not compile). The contract, once
//! GREEN, is:
//!
//! 1. `blocks::walk_blocks(data, start, limits)` returns a typed
//!    `WalkReport` for ANY byte string and never panics. Every input is
//!    classified (Ok, Truncated, LengthLimitExceeded, DepthLimitExceeded,
//!    BlockBudgetExceeded, or a header/truncation error).
//! 2. `blocks::anchor_start(data)` validates the OBSERVED CGR header spine
//!    (magic `01 00 00 00`, 96-zero preamble, 16-byte invariant TEMPLATE,
//!    HEADER_LEN 0x78) and returns the measured payload start. Non-CGR or
//!    truncated input is a typed error, never a panic.
//! 3. The walker walks the depth-first record framing of the P1-M2-T01 hex
//!    notebook (§7, `[uncertain]`): `u32 LE tag + u32 LE len + payload`,
//!    records nested inside payloads, NO forward block table. It emits the
//!    ordered table of blocks (offset, tag, len, depth) it entered.
//! 4. Bounds checks are hard: a declared `len` > `MAX_BLOCK_LEN` is refused
//!    (never materialized); nesting past `MAX_DEPTH` is refused; record counts
//!    past `MAX_BLOCKS` are refused; the walk is iterative (explicit stack) so
//!    a 200_000-deep chain cannot smash the call stack; truncated tails
//!    classify, never panic.
//! 5. REAL material leg: when the (never-committed, clean-room) sample corpus
//!    is supplied via `OEP_CGR_SAMPLES`, every real `.cgr` yields a
//!    classified outcome with ZERO panics and the golden sample `VALVE.cgr`
//!    must be among them; otherwise a tolerant SKIP keeps CI green.
//!
//! Clean-room rule honoured here (board.program doctrine): no proprietary CGR
//! bytes are committed. Every deterministic input below is constructed ONLY
//! from observed format facts already published in `docs/re-notes/` (magic,
//! zero preamble, invariant template, landscape literal, record framing
//! hypothesis). Real bytes are only READ at test time from the private
//! observation directory.

use oep_cgr::blocks::{
    anchor_start, walk_blocks, Block, BlocksError, Outcome, MAX_BLOCKS, MAX_DEPTH,
};

// ---------------------------------------------------------------------------
// Observed format facts (published RE notes; no proprietary bytes)
// ---------------------------------------------------------------------------
const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];
const LANDSCAPE: u32 = 0x10CD_3600; // observed R19-era landscape literal

/// Build the canonical observed CGR header spine (magic, 96 zero bytes,
/// landscape, invariant template) from published facts only.
fn canonical_header() -> Vec<u8> {
    let mut b = vec![0u8; 0x78];
    b[0..4].copy_from_slice(&MAGIC);
    b[0x64..0x68].copy_from_slice(&LANDSCAPE.to_le_bytes());
    b[0x68..0x78].copy_from_slice(&TEMPLATE);
    b
}

/// Build a nested record chain `depth` records deep: each record is
/// `u32 LE tag + u32 LE len + payload`, payload being the next inner record.
/// This is the OBSERVED framing hypothesis from the P1-M2-T01 hex notebook §7
/// (`[uncertain]`), re-used here exactly as P2-M6-T01's fuzz campaign does.
fn nested_records(depth: usize) -> Vec<u8> {
    let mut inner = Vec::new();
    for level in 0..depth {
        let tag = u32::try_from(level).unwrap_or(0x0B_AD_B0_0B);
        let len = u32::try_from(inner.len()).unwrap_or(u32::MAX);
        let mut rec = Vec::with_capacity(8 + inner.len());
        rec.extend_from_slice(&tag.to_le_bytes());
        rec.extend_from_slice(&len.to_le_bytes());
        rec.extend_from_slice(&inner);
        inner = rec;
    }
    inner
}

// ---- 1. Total walker: any input classified, never a panic ----------------

#[test]
fn walk_never_panics_on_arbitrary_bytes() {
    let limits = oep_cgr::blocks::Limits::default();
    // Deterministic sweep across sizes 0..=4096 with rolling content.
    for n in 0..=4096usize {
        let bytes: Vec<u8> = (0..n).map(|i| ((i * 31) ^ n) as u8).collect();
        let _ = walk_blocks(&bytes, 0, &limits); // must return, not panic
    }
    for bytes in [
        Vec::new(),
        vec![0u8; 1],
        vec![0xFF; 7],
        vec![0x01, 0x00, 0x00, 0x00],
        vec![0xFF; 8],
        vec![0x00; 4096],
    ] {
        let _ = walk_blocks(&bytes, 0, &limits);
    }
}

// ---- 2. Header anchor: measured spine validation --------------------------

#[test]
fn anchor_accepts_observed_header_spine() {
    let h = canonical_header();
    let start = anchor_start(&h).expect("canonical observed header must anchor");
    assert_eq!(
        start, 0x78,
        "measured payload starts right after the 0x78 spine"
    );
}

#[test]
fn anchor_rejects_non_cgr_magic() {
    let mut h = canonical_header();
    h[0] = 0x02;
    assert_eq!(
        anchor_start(&h).unwrap_err(),
        BlocksError::NotCgrPreamble,
        "wrong magic must be a typed error, not a panic"
    );
}

#[test]
fn anchor_rejects_truncated_spine() {
    for cut in 0..canonical_header().len() {
        let h = canonical_header();
        let _ = anchor_start(&h[..cut]); // never panics
    }
    let err = anchor_start(&canonical_header()[..0x60]).unwrap_err();
    match err {
        BlocksError::TooShort { .. } => {}
        other => panic!("expected TooShort, got {other:?}"),
    }
}

// ---- 3. Block table emission ---------------------------------------------

#[test]
fn emits_ordered_block_table_for_nested_chain() {
    let mut data = canonical_header();
    let chain = nested_records(3); // 24 bytes: R0->R1->R2
    data.extend_from_slice(&chain);
    let limits = oep_cgr::blocks::Limits::default();
    let rep = walk_blocks(&data, 0x78, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "3-level chain must walk Ok");
    // Expected table (offset measured absolute in the file). The test builder
    // wraps each level on the OUTSIDE, so the chain reads tag 2 (outermost) ->
    // tag 0 (innermost):
    //  R0 at 0x78: tag 2, len 16, depth 1
    //  R1 at 0x80: tag 1, len 8,  depth 2
    //  R2 at 0x88: tag 0, len 0,  depth 3
    assert_eq!(
        rep.blocks,
        vec![
            Block {
                offset: 0x78,
                tag: 2,
                len: 16,
                depth: 1
            },
            Block {
                offset: 0x80,
                tag: 1,
                len: 8,
                depth: 2
            },
            Block {
                offset: 0x88,
                tag: 0,
                len: 0,
                depth: 3
            },
        ],
        "block table must record offset/tag/len/depth for every entered record"
    );
    assert_eq!(rep.max_depth_reached, 3);
    assert_eq!(rep.trailing_residue, 0);
}

// ---- 4. Bounds checks -----------------------------------------------------

#[test]
fn declared_u32max_length_is_rejected_without_allocating() {
    let limits = oep_cgr::blocks::Limits::default();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let rep = walk_blocks(&bytes, 0, &limits);
    match rep.outcome {
        Outcome::LengthLimitExceeded { offset, .. } => {
            assert_eq!(offset, 8, "flag the record payload start")
        }
        other => panic!("expected LengthLimitExceeded, got {other:?}"),
    }
}

#[test]
fn observed_landscape_word_is_refused_as_length_limit() {
    // Under the natural 8-byte alignment starting at 0x00, the observed
    // landscape u32 at 0x64 reads as the `len` of the record whose tag sits
    // at 0x60. Its value (281 MB) is far above MAX_BLOCK_LEN (64 MiB), so the
    // walker refuses it instead of allocating ~281 MB (memory-bomb defence).
    let limits = oep_cgr::blocks::Limits::default();
    let rep = walk_blocks(&canonical_header(), 0, &limits);
    match rep.outcome {
        Outcome::LengthLimitExceeded { offset, declared } => {
            assert_eq!(offset, 0x68, "refusal happens at the payload start");
            assert_eq!(
                declared,
                u64::from(LANDSCAPE),
                "the refused length is the landscape literal"
            );
        }
        other => panic!("expected LengthLimitExceeded, got {other:?}"),
    }
}

#[test]
fn depth_at_cap_parses_and_past_cap_is_rejected() {
    let limits = oep_cgr::blocks::Limits::default();
    let at_cap = nested_records(MAX_DEPTH);
    let rep = walk_blocks(&at_cap, 0, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "depth == MAX_DEPTH must parse");
    assert_eq!(rep.max_depth_reached, MAX_DEPTH);

    let over_cap = nested_records(MAX_DEPTH + 1);
    let rep = walk_blocks(&over_cap, 0, &limits);
    match rep.outcome {
        Outcome::DepthLimitExceeded { depth } => {
            assert_eq!(depth, MAX_DEPTH + 1, "limit trips exactly at cap+1")
        }
        other => panic!("expected DepthLimitExceeded, got {other:?}"),
    }
}

#[test]
fn extreme_depth_is_iterative_no_stack_overflow() {
    let limits = oep_cgr::blocks::Limits::default();
    let deep = nested_records(200_000);
    let rep = walk_blocks(&deep, 0, &limits);
    // Must complete and classify DepthLimitExceeded — NOT a SIGSEGV.
    assert_eq!(
        rep.outcome,
        Outcome::DepthLimitExceeded {
            depth: MAX_DEPTH + 1
        }
    );
}

#[test]
fn huge_zero_container_is_classified_and_node_bounded() {
    let limits = oep_cgr::blocks::Limits::default();
    // A zeros-only container is a stream of (tag=0, len=0) records, 8 bytes
    // each. 8 MiB -> 1_048_576 records, above MAX_BLOCKS: stop with
    // BlockBudgetExceeded, never OOM, never unbounded CPU.
    let bytes = vec![0u8; 8 * 1024 * 1024];
    let rep = walk_blocks(&bytes, 0, &limits);
    assert!(
        matches!(
            rep.outcome,
            Outcome::Ok | Outcome::BlockBudgetExceeded { .. }
        ),
        "huge zero container must classify, not OOM or panic (got {:?})",
        rep.outcome
    );
    assert!(
        rep.blocks.len() as u64 <= MAX_BLOCKS,
        "node budget honoured"
    );

    // A modest container (64 KiB zeroes -> 8_192 records) parses cleanly.
    let ok = vec![0u8; 64 * 1024];
    let rep = walk_blocks(&ok, 0, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "sub-budget huge container parses");
    assert_eq!(rep.blocks.len(), 8_192, "exact record count");
}

// ---- 5. Truncated tails --------------------------------------------------

#[test]
fn every_truncation_classifies_never_panics() {
    let limits = oep_cgr::blocks::Limits::default();
    let bases: [Vec<u8>; 2] = [canonical_header(), nested_records(3)];
    for base in bases {
        let full = base.len();
        for k in 0..=full {
            let _ = walk_blocks(&base[..k], 0, &limits); // never panic
        }
        for del in 0..full {
            let mut b = base.clone();
            b.remove(del);
            let _ = walk_blocks(&b, 0, &limits);
        }
    }
}

#[test]
fn truncation_mid_record_is_truncated_on_boundary_is_ok() {
    let limits = oep_cgr::blocks::Limits::default();
    let chain = nested_records(3); // R0[tag,len,R1[tag,len,R2[tag,len]]] = 24 B
    assert_eq!(chain.len(), 24, "sanity: 3-level chain is 24 bytes");

    // Cut inside R1's record header -> Truncated.
    let rep = walk_blocks(&chain[..10], 0, &limits);
    assert!(
        matches!(rep.outcome, Outcome::Truncated { .. }),
        "mid-record header cut must classify as Truncated (got {:?})",
        rep.outcome
    );

    // Cut exactly at the R0 record boundary -> Ok.
    let rep = walk_blocks(&chain[..24], 0, &limits);
    assert_eq!(rep.outcome, Outcome::Ok, "boundary cut must parse");

    // Cut at 22: R0's payload (8..24) no longer fits -> Truncated.
    let rep = walk_blocks(&chain[..22], 0, &limits);
    assert!(
        matches!(rep.outcome, Outcome::Truncated { .. }),
        "overrunning payload must classify as Truncated (got {:?})",
        rep.outcome
    );
}

// ---- 6. Real material (skip-clean when corpus absent) --------------------

fn sample_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(std::path::PathBuf::from(chunk));
            }
        }
    }
    dirs.push(std::path::PathBuf::from("samples"));
    dirs.push(std::path::PathBuf::from("../samples"));
    dirs
}

fn find_samples() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for d in sample_dirs() {
        if !d.is_dir() {
            continue;
        }
        let mut entries: Vec<_> = std::fs::read_dir(&d)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "cgr" || x == "CGR"))
                    .collect()
            })
            .unwrap_or_default();
        out.append(&mut entries);
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn real_material_every_sample_classified_zero_panics() {
    let samples = find_samples();
    if samples.is_empty() {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    }
    let limits = oep_cgr::blocks::Limits::default();
    let mut saw_golden = false;
    // Every sample must anchor (observed spine) and then classify under the
    // walker. Anchor failures are themselves classified outcomes (the legacy
    // piston family may differ at the tail); what matters is: never a panic,
    // always a WalkReport.
    for p in &samples {
        let is_golden = p.file_name().is_some_and(|n| n == "VALVE.cgr");
        if is_golden {
            saw_golden = true;
        }
        let bytes = std::fs::read(p).expect("read sample");
        let out = std::panic::catch_unwind(|| {
            let start = anchor_start(&bytes).unwrap_or(0x78);
            let rep = walk_blocks(&bytes, start, &limits);
            (rep, start)
        });
        assert!(out.is_ok(), "walker must never panic on {:?}", p);
        let (rep, start) = out.unwrap();
        eprintln!(
            "  {:?}: start={:#x} outcome={:?} blocks={} max_depth={} trailing={}",
            p.file_name().unwrap(),
            start,
            rep.outcome,
            rep.blocks.len(),
            rep.max_depth_reached,
            rep.trailing_residue
        );
    }
    assert!(
        saw_golden,
        "golden sample VALVE.cgr present in the corpus dir"
    );
    eprintln!(
        "  verified {} real samples through the central block walker (zero panics)",
        samples.len()
    );
}
