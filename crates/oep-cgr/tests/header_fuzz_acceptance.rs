//! P2-M1-T05 acceptance test (TDD, RED first) — Fuzz harness (round 1) on
//! the CGR header parser; zero panics on corpus.
//!
//! Written BEFORE `oep_cgr::fuzz_round1` existed (RED: the P2-M1-T01 crate
//! exports only `header`, so this file does not compile until the round-1
//! campaign module lands). The contract, once GREEN, is:
//!
//! 1. `header::parse_header` is total: ANY byte string maps to `Ok` or a
//!    typed `CgrError` (TooShort / BadMagic / NonZeroPreamble /
//!    TemplateMismatch) and never panics.
//! 2. Truncation: every prefix length of the observed 0x78-byte spine
//!    classifies; a head shorter than `HEADER_LEN` is `TooShort`, the full
//!    0x78-byte spine parses `Ok`.
//! 3. The cargo-fuzz contract is expressed on the pinned STABLE toolchain as
//!    a deterministic campaign (`fuzz_round1::run_campaign`): fixed seed ->
//!    byte-identical report, `panics == 0`, every header error class
//!    exercised, a real corpus of inputs processed.
//! 4. Real material (when `OEP_CGR_SAMPLES` supplies the never-committed
//!    corpus): every real `.cgr` header parses with zero panics (PASS), or
//!    the leg skips so CI without the private bins stays green.
//!
//! The nightly `libFuzzer` leg lives in `crates/oep-cgr/fuzz/` (target
//! `fuzz_targets/header_parse.rs`) and is workspace-excluded: the pinned
//! stable toolchain never builds it. This stable test IS the enforced
//! zero-panic proof on the corpus.
//!
//! Clean-room rule (board.program doctrine): real CGR binaries are NEVER
//! committed. Every synthesized input below is derived from the observed,
//! published format facts (magic `01 00 00 00`, 0x60 zero preamble,
//! landscape u32 at 0x64, 16-byte invariant TEMPLATE at 0x68..0x78) and
//! deterministic patterns.

use oep_cgr::fuzz_round1::{self, CampaignConfig, CampaignReport};
use oep_cgr::header::{parse_header, CgrError, HEADER_LEN, MAGIC, TEMPLATE};

/// Fixed seed for a reproducible campaign ("KEELR1_05").
const SEED: u64 = 0x4B_45_45_4C_52_31_5F_05;

// ---------------------------------------------------------------------------
// 1. Totality invariant: any bytes classify, never a panic.
// ---------------------------------------------------------------------------

#[test]
fn header_never_panics_on_arbitrary_bytes() {
    // Rolling-content sweep across every size 0..=4096 covers arbitrary
    // lengths AND arbitrary content: parse_header must return, not panic.
    for n in 0..=4096usize {
        let bytes: Vec<u8> = (0..n).map(|i| ((i * 31) ^ n) as u8).collect();
        let _ = parse_header(&bytes);
    }
    // A few hand-built adversarial cases.
    for bytes in [
        Vec::new(),
        vec![0u8; 1],
        vec![0xFF; 7],
        vec![MAGIC[0], 0, 0, 0],
        vec![0xFF; HEADER_LEN],
        vec![0u8; HEADER_LEN],
    ] {
        let _ = parse_header(&bytes);
    }
}

#[test]
fn observed_spine_parses_ok() {
    let spine = fuzz_round1::canonical_header();
    assert_eq!(spine.len(), HEADER_LEN);
    let h = parse_header(&spine).expect("observed spine must parse Ok");
    assert_eq!(h.magic, 1);
    assert_eq!(h.template, TEMPLATE);
}

// ---------------------------------------------------------------------------
// 2. Truncation / typed-error classes.
// ---------------------------------------------------------------------------

#[test]
fn every_truncation_classifies_never_panics() {
    let spine = fuzz_round1::canonical_header();
    let full = spine.len();
    for k in 0..=full {
        let _ = parse_header(&spine[..k]); // must never panic
    }
    // Sliding single-byte deletion sanity.
    for del in 0..full {
        let mut b = spine.clone();
        b.remove(del);
        let _ = parse_header(&b);
    }
}

#[test]
fn short_head_is_too_short_and_full_spine_is_ok() {
    let spine = fuzz_round1::canonical_header();
    for k in 0..HEADER_LEN {
        assert_eq!(
            parse_header(&spine[..k]),
            Err(CgrError::TooShort {
                need: HEADER_LEN,
                got: k
            }),
            "prefix of length {k} must be TooShort"
        );
    }
    assert!(parse_header(&spine).is_ok(), "full spine parses Ok");
}

#[test]
fn each_corruption_class_maps_to_its_typed_error() {
    let spine = fuzz_round1::canonical_header();

    // Bad first four bytes -> BadMagic.
    let mut bad_magic = spine.clone();
    bad_magic[0] ^= 0xFF;
    assert!(matches!(
        parse_header(&bad_magic),
        Err(CgrError::BadMagic { .. })
    ));

    // Non-zero byte inside the 0x60-byte zero preamble -> NonZeroPreamble.
    let mut bad_preamble = spine.clone();
    bad_preamble[0x40] = 1;
    assert!(matches!(
        parse_header(&bad_preamble),
        Err(CgrError::NonZeroPreamble { offset }) if offset == 0x40
    ));

    // Byte flip inside the invariant 16-byte template -> TemplateMismatch.
    let mut bad_template = spine.clone();
    bad_template[0x68] ^= 1;
    assert!(matches!(
        parse_header(&bad_template),
        Err(CgrError::TemplateMismatch { .. })
    ));
}

// ---------------------------------------------------------------------------
// 3. Campaign: determinism, zero panics, all error classes exercised.
// ---------------------------------------------------------------------------

#[test]
fn campaign_is_deterministic_zero_panics_and_covers_all_classes() {
    let cfg = CampaignConfig {
        seed: SEED,
        iters: 5_000,
        truncation_step: 7,
        arbitrary_max: 4096,
    };
    let a = fuzz_round1::run_campaign(&cfg);
    let b = fuzz_round1::run_campaign(&cfg);

    assert_eq!(
        impl_signature(&a),
        impl_signature(&b),
        "same seed must give an identical campaign report"
    );
    assert_eq!(a.panics, 0, "zero panics across the round-1 campaign");
    assert_eq!(a.inputs, b.inputs);
    assert!(
        a.inputs > 1_000,
        "campaign must process a real corpus of inputs (got {})",
        a.inputs
    );
    // Every header error class must be reachable by the campaign.
    for kind in [
        "ok",
        "too_short",
        "bad_magic",
        "nonzero_preamble",
        "template_mismatch",
    ] {
        assert!(
            a.outcomes.get(kind).copied().unwrap_or(0) > 0,
            "outcome class '{kind}' must be exercised"
        );
    }
    // No panic-capable leg may be empty.
    for class in ["canonical", "truncated", "mutation", "arbitrary"] {
        assert!(
            a.class_counts.get(class).copied().unwrap_or(0) > 0,
            "class '{class}' produced inputs"
        );
    }
}

/// Serialize the observables of a report into one comparable string.
fn impl_signature(r: &CampaignReport) -> String {
    format!(
        "inputs={} panics={} outcomes={:?} classes={:?} files={}",
        r.inputs, r.panics, r.outcomes, r.class_counts, r.files_scanned
    )
}

// ---------------------------------------------------------------------------
// 4. Real material.
// ---------------------------------------------------------------------------

#[test]
fn real_material_campaign_skips_without_samples_passes_with_them() {
    let cfg = CampaignConfig {
        seed: SEED,
        iters: 2_000,
        truncation_step: 11,
        arbitrary_max: 1024,
    };
    let dirs = fuzz_round1::sample_dirs();
    let rep = fuzz_round1::run_campaign_on_samples(&cfg, &dirs);
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
