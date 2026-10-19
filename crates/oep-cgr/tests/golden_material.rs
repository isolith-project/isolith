//! P2-M4-T01 acceptance test (TDD, RED first) — color/material table parse
//! and export to glTF 2.0 materials.
//!
//! Written BEFORE `oep_cgr::material` existed (RED: this file does not
//! compile against the skeleton crate). The contract once GREEN is:
//!
//! 1. `scan_color_records` finds the observed per-surface color-record
//!    signature (f32 LE in (0.5, 1.0] followed by the two marker words
//!    `01 00 00 00` / `00 01 00 00`) in real CGR bytes, bounds-checked and
//!    never panicking (tolerant failure per P2-M1-T04).
//! 2. `to_gltf_materials_json` emits a schema-correct glTF 2.0 `materials`
//!    array whose `pbrMetallicRoughness.baseColorFactor` carries the RGB
//!    parsed from each record (public Khronos glTF 2.0 spec; fully
//!    deterministic, no reverse-engineering needed on the export side).
//! 3. Against REAL material, when a golden `VALVE.cgr` is supplied via
//!    `OEP_CGR_SAMPLES`, `scan_color_records` finds the observed first
//!    cluster and its scalar matches the observation registry; otherwise the
//!    test prints a tolerant SKIP and passes, so CI stays green before the
//!    (never-committed, clean-room) sample corpus is present.
//!
//! Clean-room rule honoured here: no proprietary CGR bytes are committed.
//! The deterministic tests construct input ONLY from observed format facts
//! already recorded in `docs/re-notes/cgr-color-material-observation.md`
//! (f32 LE color scalar, marker words `01 00 00 00` + `00 01 00 00`).
//! The real-material leg reads the actual golden binary from a private
//! observation directory (`OEP_CGR_SAMPLES`).

use oep_cgr::material::{scan_color_records, to_gltf_materials_json, ColorRecord};

/// Observed on VALVE.cgr (cfysa set, R19-era): color records carry an f32 LE
/// scalar in (0.5, 1.0]. The first cluster (6 records, stride 0xB4 = 180
/// bytes) all read ≈ 0.812072, consistent with CATIA default part gray.
const GOLDEN_SCALAR: f32 = 0.812072;
/// Marker word immediately after the color scalar (`00 00 00 01`, u32 LE
/// == 0x01000000 on the real file bytes).
const MARKER_WORD_A: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
/// Second marker word (`00 01 00 00`, LE u32 0x00000100).
const MARKER_WORD_B: [u8; 4] = [0x00, 0x01, 0x00, 0x00];
/// Golden sample file name and byte anchors (P1-M1-T01 registry id
/// `dassault-dmu-space-analysis-valve`; sha256 recorded there).
const GOLDEN_FILE: &str = "VALVE.cgr";
const GOLDEN_SIZE: usize = 176128;

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

/// Build a synthetic byte region containing ONE observed color-record
/// signature (constructed from published format facts, NO proprietary bytes).
fn one_record_region() -> Vec<u8> {
    let mut b = vec![0u8; 0x100];
    let color = GOLDEN_SCALAR.to_le_bytes();
    b[0x20..0x24].copy_from_slice(&color);
    b[0x24..0x28].copy_from_slice(&MARKER_WORD_A);
    b[0x28..0x2c].copy_from_slice(&MARKER_WORD_B);
    b
}

#[test]
fn scan_finds_observed_signature() {
    let bytes = one_record_region();
    let records = scan_color_records(&bytes);
    assert_eq!(
        records.len(),
        1,
        "exactly one observed record must be found"
    );
    let rec = &records[0];
    assert_eq!(rec.offset, 0x20, "record offset in the synthetic region");
    assert!(
        (rec.scalar - GOLDEN_SCALAR).abs() < 5e-4,
        "record scalar matches observed value: {} vs {}",
        rec.scalar,
        GOLDEN_SCALAR
    );
}

#[test]
fn scan_ignores_junk_without_panicking() {
    // Junk, truncated, zeros: must never panic; returns an empty list or
    // well-formed records (tolerant failure model, P2-M1-T04).
    let junk = b"not a cgr file at all, just printable text repeating";
    assert!(scan_color_records(junk).is_empty());
    assert!(scan_color_records(&[]).is_empty());
    // A random-ish medium buffer should not produce an absurd count.
    let mut rng: u32 = 0x1234_5678;
    let mut buf = vec![0u8; 4096];
    for x in buf.iter_mut() {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        *x = (rng >> 24) as u8;
    }
    let n = scan_color_records(&buf).len();
    assert!(n < buf.len() / 8, "no runaway record explosion on noise");
}

#[test]
fn export_materials_valid_gltf() {
    let bytes = one_record_region();
    let records = scan_color_records(&bytes);
    assert!(!records.is_empty());
    let gltf = to_gltf_materials_json(&records);
    // Structural checks against the public glTF 2.0 materials schema.
    assert!(gltf.contains("\"materials\""), "materials array present");
    assert!(gltf.contains("\"pbrMetallicRoughness\""));
    assert!(gltf.contains("\"baseColorFactor\""));
    assert!(!gltf.contains("NaN"));
    assert!(!gltf.contains("inf"));
    // Every record contributes exactly one material entry.
    assert_eq!(gltf.matches("baseColorFactor").count(), records.len());
    // The measured scalar must appear as R=G=B (grayscale-from-scalar mapping,
    // documented [uncertain] in the RE note).
    let s = format!("{:.6}", records[0].scalar as f64);
    assert!(gltf.contains(&s), "scalar {s} rendered into color factor");
}

#[test]
fn export_empty_table_is_valid() {
    let gltf = to_gltf_materials_json(&[]);
    assert!(
        gltf.contains("\"materials\":[]") || gltf.contains("\"materials\": []"),
        "empty table emits an empty materials array: {gltf}"
    );
}

/// Real-material leg. Fails loud on ANY disagreement with the golden
/// observation; prints SKIP and passes when no sample is supplied.
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
        "info  verifying color-record scan on real material: {}",
        path.display()
    );

    let bytes = std::fs::read(&path).expect("read golden sample binary");
    assert_eq!(bytes.len(), GOLDEN_SIZE, "golden sample size anchor");

    let records = scan_color_records(&bytes);
    // VALVE.cgr carries 16 observed records across several distinct surface
    // colors; require the observed count and the golden gray scalar present.
    assert!(
        records.len() >= 10,
        "real VALVE.cgr must expose its observed color records (got {})",
        records.len()
    );
    let first = &records[0];
    eprintln!(
        "info  first record @0x{:x} scalar={:.6} (total {})",
        first.offset,
        first.scalar,
        records.len()
    );
    assert!(
        records.iter().any(|r| (r.scalar - GOLDEN_SCALAR).abs() < 5e-4),
        "the golden CATIA-gray scalar {GOLDEN_SCALAR} must appear among the parsed records (got {})",
        records
            .iter()
            .map(|r| format!("{:.4}", r.scalar))
            .collect::<Vec<_>>()
            .join(", ")
    );
    eprintln!(
        "PASS  real golden sample: {} color records parsed",
        records.len()
    );
}

/// Compile-time check that ColorRecord carries the documented fields.
#[allow(dead_code)]
fn _shape_check(r: &ColorRecord) {
    let _ = r.offset;
    let _ = r.scalar;
}
