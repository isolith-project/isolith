//! P2-M1-T01 acceptance test (TDD, RED first) — magic/header parse of the
//! golden sample `VALVE.cgr`.
//!
//! Written BEFORE the `oep_cgr::header` parser existed (RED: this file does
//! not compile against the skeleton crate). The contract, once GREEN, is:
//!
//! 1. `parse_header` accepts the observed CGR header shape and returns the
//!    exact observed fields for the golden sample.
//! 2. Malformed headers (bad magic, truncated, non-zero preamble, changed
//!    invariant template) are rejected with a typed error, never a panic.
//! 3. Against REAL material, when a golden `VALVE.cgr` is supplied, the
//!    parsed fields match the observation registry (P1-M1-T01) and the
//!    hex notebook (P1-M2-T01); otherwise the test skips so CI stays green
//!    before the (never-committed, clean-room) sample corpus is present.
//!
//! Clean-room rule honoured here: no proprietary CGR bytes are committed.
//! The deterministic test constructs its input ONLY from observed format
//! facts already published in `docs/re-notes/oep-cgr-header-parse.md`
//! (magic 01 00 00 00, 96-byte zero preamble, per-sample landscape literal,
//! 16-byte invariant template). The real-material leg reads the actual
//! golden binary from a private observation directory (`OEP_CGR_SAMPLES`).

use oep_cgr::header::{parse_header, CgrError, HEADER_LEN, TEMPLATE};

/// Golden sample: CATIA V5 documentation DMU Space Analysis tutorial set,
/// R19-era, cfysa set. sha256 43cc0cb474b5fe10a4d0bf1b8c61f48d952a8bba849aa8b116e12b490d7dcbd6
/// (P1-M1-T01 registry, id `dassault-dmu-space-analysis-valve`). The binary
/// itself is NEVER committed; only the observed header facts are referenced.
const GOLDEN_FILE: &str = "VALVE.cgr";
const GOLDEN_SIZE: usize = 176128;
const GOLDEN_LANDSCAPE: u32 = 0x10CD_3600;
const T00: u32 = 1; // magic as little-endian u32 == 1

/// Locations searched for real samples: `$OEP_CGR_SAMPLES` (colon/comma
/// separated dirs), else `samples`, else `../samples` relative to the crate.
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

/// Build the observed golden-header byte region WITHOUT committing any
/// proprietary payload: magic + 96 zero bytes + landscape + template.
fn golden_header_shape() -> Vec<u8> {
    let mut b = vec![0u8; HEADER_LEN];
    b[0..4].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]); // magic
    b[0x64..0x68].copy_from_slice(&GOLDEN_LANDSCAPE.to_le_bytes());
    b[0x68..0x78].copy_from_slice(&TEMPLATE); // invariant template
    b
}

#[test]
fn parse_accepts_golden_header_shape() {
    let h = parse_header(&golden_header_shape()).expect("observed golden header shape must parse");
    assert_eq!(h.magic, T00, "u32 LE magic must read as 1");
    assert_eq!(
        h.landscape, GOLDEN_LANDSCAPE,
        "landscape matches golden sample observation"
    );
    assert_eq!(
        h.template, TEMPLATE,
        "invariant template matches golden sample observation"
    );
}

#[test]
fn parse_rejects_bad_magic() {
    let mut b = golden_header_shape();
    b[0] = 0x02;
    match parse_header(&b) {
        Err(CgrError::BadMagic { got }) => assert_eq!(got, [0x02, 0x00, 0x00, 0x00]),
        other => panic!("expected BadMagic, got {:?}", other),
    }
}

#[test]
fn parse_rejects_truncated_input() {
    let b = golden_header_shape();
    assert!(b.len() > 4, "sanity: shape longer than magic");
    match parse_header(&b[..4]) {
        Err(CgrError::TooShort { need, got }) => {
            assert_eq!(need, HEADER_LEN);
            assert_eq!(got, 4);
        }
        other => panic!("expected TooShort, got {:?}", other),
    }
}

#[test]
fn parse_rejects_nonzero_preamble() {
    let mut b = golden_header_shape();
    b[0x10] = 0xAB;
    match parse_header(&b) {
        Err(CgrError::NonZeroPreamble { offset }) => assert_eq!(offset, 0x10),
        other => panic!("expected NonZeroPreamble, got {:?}", other),
    }
}

#[test]
fn parse_rejects_template_drift() {
    let mut b = golden_header_shape();
    b[0x6c] ^= 0xFF; // corrupt one template byte
    assert!(matches!(
        parse_header(&b),
        Err(CgrError::TemplateMismatch { .. })
    ));
}

/// The real-material leg. Fails loud on ANY disagreement with the golden
/// observation; prints SKIP and passes when no sample is supplied, so CI
/// without the (clean-room, never-committed) binary stays green.
#[test]
fn golden_sample_real_material() {
    let mut found = None;
    for dir in sample_dirs() {
        let probe = dir.join(GOLDEN_FILE);
        if probe.is_file() {
            found = Some(probe);
            break;
        }
    }
    let Some(path) = found else {
        eprintln!(
            "SKIP  golden sample '{}' not found (set OEP_CGR_SAMPLES=<dir>); tolerant CI-safe skip",
            GOLDEN_FILE
        );
        return;
    };
    eprintln!(
        "info  verifying header parse on real material: {}",
        path.display()
    );

    let bytes = std::fs::read(&path).expect("read golden sample binary");
    assert_eq!(
        bytes.len(),
        GOLDEN_SIZE,
        "golden sample size anchor (see docs/re-notes/oep-cgr-header-parse.md)"
    );
    assert_eq!(
        bytes.len() % 0x1000,
        0,
        "golden sample is 0x1000-aligned (observed)"
    );

    let h = parse_header(&bytes).expect("real golden sample header must parse");
    assert_eq!(h.magic, T00);
    assert_eq!(h.landscape, GOLDEN_LANDSCAPE);
    assert_eq!(h.template, TEMPLATE);
    eprintln!(
        "PASS  real golden sample parsed: magic={} landscape=0x{:08x} template ok ({} bytes)",
        h.magic,
        h.landscape,
        bytes.len()
    );
}
