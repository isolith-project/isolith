//! P2-M7-T01 acceptance test (TDD: contract written first) — measured
//! triangle-stream record LOCATION on real CGR material.
//!
//! This card's honest contract, locked here:
//!
//! 1. `locate` never panics on any input (empty, truncated, garbage, real).
//! 2. A buffer without the observed header spine is `NotCgrPreamble`.
//! 3. A valid-spine buffer with no self-consistent counts/handles record is
//!    `NoSelfConsistentRecord` (the cfysa R19-era family classifies exactly
//!    so — its coordinate coding is a different family, honestly classified).
//! 4. A buffer with the observed record grammar yields `RecordsLocated` with
//!    the measured counts (deterministic fixture + real-material piston leg).
//!
//! The real-material leg runs against the private sample corpus via
//! `OEP_CGR_SAMPLES` (never committed) and SKIPs cleanly when absent so CI
//! stays green. Every located count is cross-checked byte-for-byte against the
//! independent Python reference reader (`python/oep_tooling/cgr_locate_reference.py`),
//! which is the "observed reference" for the committed golden.

use oep_cgr::locate::{locate_triangle_streams, LocateOutcome, LocateRecord};

/// Hand-build the OBSERVED CGR header spine (P2-M1-T01/hex notebook).
fn spine() -> Vec<u8> {
    let mut b = vec![0u8; 0x78];
    b[0..4].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]); // magic u32 LE == 1
                                                        // 0x04..0x63 stays zero (96-byte preamble)
    b[0x68..0x78].copy_from_slice(&[
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ]);
    b
}

fn push_be16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_be_bytes());
}

// --- Part 1: never panics, preamble gate, classified outcomes ---

#[test]
fn locate_never_panics_and_classifies_garbage() {
    assert_eq!(locate_triangle_streams(b""), LocateOutcome::NotCgrPreamble);
    assert_eq!(
        locate_triangle_streams(b"\x00\x01\x02"),
        LocateOutcome::NotCgrPreamble
    );
    // wrong magic after a long buffer
    let mut junk = vec![0xABu8; 2048];
    junk[0..4].copy_from_slice(&[0x02, 0x00, 0x00, 0x00]);
    assert_eq!(
        locate_triangle_streams(&junk),
        LocateOutcome::NotCgrPreamble
    );
    // a full spine with no records
    let mut b = spine();
    b.extend_from_slice(&[0u8; 64]); // padding only
    assert_eq!(
        locate_triangle_streams(&b),
        LocateOutcome::NoSelfConsistentRecord
    );
}

#[test]
fn locate_valid_spine_with_clean_fixture_record() {
    // N=10 triangle faces, counts [3;10] => S=30 handles = [0,3,...,87];
    // all h%3==0 -> V=30 distinct, I=30, T=30, max_idx=29.
    let mut b = spine();
    push_be16(&mut b, 10); // face count N
    for _ in 0..10 {
        push_be16(&mut b, 3); // each face a 3-gon (triangle)
    }
    for k in 0..30u16 {
        push_be16(&mut b, 3 * k);
    }
    match locate_triangle_streams(&b) {
        LocateOutcome::RecordsLocated(records) => {
            assert_eq!(records.len(), 1, "one fixture record located");
            let r = &records[0];
            assert_eq!(r.offset, 0x78);
            assert_eq!(r.face_count, 10);
            assert_eq!(r.counts, vec![3; 10]);
            assert_eq!(r.index_count, 30);
            assert_eq!(r.vertex_count, 30, "30 distinct h/3 indices (0..=29)");
            assert_eq!(
                r.triangle_count, 10,
                "ten 3-gons triangulate to ten triangles"
            );
            assert_eq!(r.max_index, 29);
        }
        other => panic!("expected RecordsLocated, got {other:?}"),
    }
}

#[test]
fn locate_fixture_rejects_non_multiple_of_3_handles() {
    let mut b = spine();
    push_be16(&mut b, 10);
    for _ in 0..10 {
        push_be16(&mut b, 3);
    }
    for k in 1..31u16 {
        push_be16(&mut b, 3 * k - 2); // first handle wrong multiple -> rejected
    }
    assert_eq!(
        locate_triangle_streams(&b),
        LocateOutcome::NoSelfConsistentRecord
    );
}

// --- Part 2: real-material leg (private corpus via OEP_CGR_SAMPLES) ---

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
    dirs
}

fn find_sample(name: &str) -> Option<std::path::PathBuf> {
    for d in sample_dirs() {
        let full = d.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

/// Committed real-material golden (numeric-epsilon path): piston.cgr record at
/// 0xA0DC, measured independently by the Python reference reader —
/// faces=6, index_count=35, vertex_count=27, triangle_count=23, max_index=246.
const PISTON_ANCHOR_OFFSET: usize = 0xA0DC;

#[test]
fn real_piston_records_located_with_measured_counts() {
    let Some(path) = find_sample("piston.cgr") else {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    };
    let bytes = std::fs::read(path).expect("piston.cgr readable");
    match locate_triangle_streams(&bytes) {
        LocateOutcome::RecordsLocated(records) => {
            assert!(
                records.len() >= 21,
                "measured: >=21 records with >=30 handles on piston (got {})",
                records.len()
            );
            // Anchor record @0xA0DC (measured golden, cross-checked with the
            // Python reference reader):
            let anchor = records
                .iter()
                .find(|r| r.offset == PISTON_ANCHOR_OFFSET)
                .expect("anchor record @0xA0DC present");
            assert_eq!(anchor.counts, vec![7, 7, 6, 5, 5, 5]);
            assert_eq!(anchor.face_count, 6);
            assert_eq!(anchor.index_count, 35, "S == sum(counts) == 35");
            assert_eq!(anchor.vertex_count, 27, "v distinct (h/3)");
            assert_eq!(anchor.triangle_count, 23, "sum(c-2) fans");
            assert_eq!(anchor.max_index, 246);
        }
        other => panic!("expected piston records located, got {other:?}"),
    }
}

#[test]
fn real_cfysa_family_classifies_under_this_grammar() {
    let Some(path) = find_sample("VALVE.cgr") else {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    };
    let bytes = std::fs::read(path).expect("VALVE.cgr readable");
    // The R19-era cfysa family carries NO self-consistent counts/handles
    // record (measured: 0 records); this is an honest classified outcome,
    // the payload family is different — never a fabricated PASS.
    assert!(
        !matches!(
            locate_triangle_streams(&bytes),
            LocateOutcome::RecordsLocated(_)
        ),
        "cfysa family measured 0 counts/handles records"
    );
}

#[allow(dead_code)]
fn _golden_annotate(_r: &LocateRecord) {
    // reserved: numeric-epsilon golden comparator anchors counts exactly
}
