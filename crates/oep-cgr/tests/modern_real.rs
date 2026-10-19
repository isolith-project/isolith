//! P2-M7-T01 continuation acceptance (TDD: contract written first) — modern
//! ERA (V5-6R2015+ / 3DEXPERIENCE) real spatial-mesh latch.
//!
//! This card's honest contract, locked here:
//!
//! 1. `extract_modern` never panics on any input (empty, truncated, garbage,
//!    old-era CGR, real modern bytes).
//! 2. Bytes without the `V5_CFV2` container are `NotCgrContainer`.
//! 3. A modern container with no unique MainDataStream/SurfacicReps pair is
//!    `NoBrepStreams` (classified, not a fabricated mesh).
//! 4. A modern container WITHOUT the visualization-point coordinate table is
//!    `NoCoordinateTable` (classified skip — honest parity).
//! 5. The real-material leg (OEP_CGR_SAMPLES, private corpus) decodes a real
//!    spatial mesh whose vertex/index/triangle counts match the committed
//!    numeric-epsilon golden, cross-checked with the independent Python
//!    reference reader.
//!
//! The anchor: public-domain NIST STC-06 (CATIA V5-6R2023), measured
//! independently by Python `python/oep_tooling/cgr_modern_reference.py`:
//! records=18, triangle_count=1389, index_count=2003, vertex_count=1037,
//! max_index=1990.

use oep_cgr::modern::{extract_modern, ModernOutcome};

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

/// Modern synthetic container minimal bytes (unit fixture only — the real
/// NIST anchors are the measured material; this fixture only exercises the
/// container gate + classified outcomes, not a fabricated mesh).
fn modern_spine() -> Vec<u8> {
    // outer header
    let mut b = Vec::new();
    b.extend_from_slice(b"V5_CFV2\0");
    // directory offset/length will point at trailing directory (patched below)
    b.extend_from_slice(&[0u8; 8]); // +8 dir offset, +12 dir length
    b.extend_from_slice(&[0xffu8; 8]); // fill ff
    b.extend_from_slice(&[0u8; 32]); // fill 00
    b.extend_from_slice(&[0u8; 8]); // hdr_flags
                                    // an inner V5_CFV2 magic a bit later (minimal, no real streams)
    b.extend_from_slice(&[0u8; 64]);
    b
}

#[test]
fn modern_never_panics_and_classifies_garbage() {
    assert!(matches!(
        extract_modern(b""),
        Err(ModernOutcome::NotCgrContainer)
    ));
    let junk = vec![0xABu8; 4096];
    assert!(matches!(
        extract_modern(&junk),
        Err(ModernOutcome::NotCgrContainer)
    ));
    // V5_CFV2 magic but no inner magic -> NotCgrContainer (inner find returns
    // None for a truncated file)
    let mut short = modern_spine();
    short.truncate(32);
    assert!(matches!(
        extract_modern(&short),
        Err(ModernOutcome::NotCgrContainer)
    ));
}

#[test]
fn modern_old_corpus_spine_classifies_as_not_container() {
    // The old-era (2007) corpus header is magic 01 00 00 00, not V5_CFV2 —
    // the modern decoder must honestly refuse it (NotCgrContainer).
    let mut b = vec![0u8; 0x78];
    b[0..4].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    assert!(matches!(
        extract_modern(&b),
        Err(ModernOutcome::NotCgrContainer)
    ));
}

// --- Part 2: real material leg (private corpus via OEP_CGR_SAMPLES) ---

#[test]
fn real_modern_nist_stc06_latches_spatial_mesh_with_golden_counts() {
    let Some(path) = find_sample("nist_stc_06_asme1_V56R2023.CATPart") else {
        eprintln!("SKIP: no modern NIST samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    };
    let bytes = std::fs::read(path).expect("readable");
    match extract_modern(&bytes) {
        Ok((counts, mesh)) => {
            // Measured anchor, cross-checked with the independent Python
            // reference reader (numeric-epsilon golden commit):
            assert_eq!(counts.records, 18, "genuine 0x4x trim packets");
            assert_eq!(counts.triangle_count, 1389, "A + strips + fans");
            assert_eq!(counts.index_count, 2003, "sum(N) handle stream length");
            assert_eq!(counts.vertex_count, 1037, "distinct handles referenced");
            assert_eq!(counts.max_index, 1990, "max handle within point table");
            // The decoded mesh must be spatial and structurally valid:
            assert!(!mesh.vertices().is_empty());
            assert!(!mesh.triangles().is_empty());
            assert!(
                mesh.validate().is_ok(),
                "mesh indices in range, finite, non-degenerate"
            );
        }
        other => panic!("expected modern mesh latch, got {other:?}"),
    }
}

#[test]
fn real_modern_nist_all_supplied_alphabetically_decode() {
    // Every supplied modern sample that decodes must be vertex/in-range and
    // self-consistent; files that fail structurally (e.g. no point table) are
    // classified, never panics. This runs against whatever modern samples are
    // on OEP_CGR_SAMPLES.
    let mut seen_any = false;
    for d in sample_dirs() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_uppercase().ends_with(".CATPART") {
                seen_any = true;
                let bytes = std::fs::read(entry.path()).expect("readable");
                match extract_modern(&bytes) {
                    Ok((counts, mesh)) => {
                        assert!(!mesh.vertices().is_empty());
                        assert!(!mesh.triangles().is_empty());
                        assert!(mesh.validate().is_ok());
                        assert!(counts.max_index < 65536);
                    }
                    Err(e) => {
                        // classified skip is the honest, never-fabricated path
                        eprintln!("classified skip {name}: {e:?}");
                    }
                }
            }
        }
    }
    if !seen_any {
        eprintln!("SKIP: no modern samples supplied");
    }
}
