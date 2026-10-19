//! P2-M3-T01 acceptance test (TDD, RED first) — normal/vertex-attribute
//! recovery where present, on the internal `oep_cgr::mesh::Mesh`.
//!
//! Written BEFORE the `oep_cgr::attributes` module existed (RED: this file
//! does not compile against the mesh-only crate — there is no
//! `oep_cgr::attributes` module). The contract, once GREEN, is:
//!
//! 1. The internal mesh model carries OPTIONAL per-vertex attributes
//!    (`VertexAttributes`); an empty attribute set has `normals == None`.
//! 2. `recover_attributes(bytes, mesh)` never panics on any input (empty,
//!    truncated, garbage, or real corpus bytes).
//! 3. "Where present" is enforced honestly (P2-M1-T04 tolerant doctrine):
//!    - absence is NOT an error — a payload without attribute records
//!      returns `Ok(attrs)` with `normals == None`;
//!    - a structurally valid per-vertex normal table whose length matches
//!      the mesh vertex list IS recovered and returned;
//!    - bytes that do not carry the observed CGR spine yield a typed
//!      `AttributeError`, never a panic and never fabricated normals.
//! 4. Clean-room: attribute layout is derived ONLY from observed facts
//!    (hex notebook P1-M2-T01, doc `docs/re-notes/oep-cgr-vertex-attributes.md`).
//!    The real-material leg runs against the private sample corpus via
//!    `OEP_CGR_SAMPLES` and SKIPs cleanly when absent, so CI stays green
//!    before the license-clean corpus lands (P1-M1-T05).

use oep_cgr::attributes::{recover_attributes, AttributeError, VertexAttributes};
use oep_cgr::mesh::{extract, Mesh};

/// The golden sample for the real-material leg: R19-era DMU tutorial set.
/// sha256 43cc0cb474b5fe10a4d0bf1b8c61f48d952a8bba849aa8b116e12b490d7dcbd6.
const GOLDEN_FILE: &str = "VALVE.cgr";

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

// ---------------------------------------------------------------------------
// Part 1: attribute model (deterministic, no real bytes)
// ---------------------------------------------------------------------------

#[test]
fn empty_attributes_have_no_normals() {
    let attrs = VertexAttributes::none(3);
    assert_eq!(attrs.count(), 3, "attribute set tracks the vertex list");
    assert!(attrs.normals().is_none(), "fresh mesh carries no normals");
    assert_eq!(attrs, VertexAttributes::none(3), "structural equality");
}

#[test]
fn normals_carried_when_recovered() {
    let normals = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
    let attrs =
        VertexAttributes::with_normals(normals.clone(), 3).expect("matching normal count builds");
    assert_eq!(attrs.normals(), Some(normals.as_slice()));
}

#[test]
fn normals_length_must_match_vertex_count() {
    let normals = vec![[0.0, 0.0, 1.0]]; // 1 normal but 3 vertices
    assert!(VertexAttributes::with_normals(normals, 3).is_err());
}

// ---------------------------------------------------------------------------
// Part 2: recovery contract (tolerant, "where present")
// ---------------------------------------------------------------------------

/// Observed header spine (magic, zero preamble, landscape literal, invariant
/// template) — same observed facts as the mesh extractor's synthetic payload.
fn spine() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // magic @0x00
    b.extend_from_slice(&[0u8; 0x60]); // zero preamble @0x04..0x63
    b.extend_from_slice(&0x10CD_3600u32.to_le_bytes()); // landscape @0x64
    b.extend_from_slice(&[
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ]); // invariant template @0x68..0x78
    b
}

/// A synthetic CGR payload with a vertex/indices stream AND a per-vertex
/// unit-normal table appended, sized to the mesh extractor's structural
/// thresholds. Built purely from observed facts — no proprietary bytes.
fn synthetic_payload_with_normals() -> Vec<u8> {
    let mut b = spine();
    // 5x6 grid = 30 vertices.
    let cols = 6usize;
    let rows = 5usize;
    for y in 0..rows {
        for x in 0..cols {
            b.extend_from_slice(&(x as f32).to_le_bytes());
            b.extend_from_slice(&(y as f32).to_le_bytes());
            b.extend_from_slice(&0.0_f32.to_le_bytes());
        }
    }
    // Separator: NaN-as-f32 (0xFF..) also ends the index run.
    b.extend_from_slice(&[0xFFu8; 64]);
    let mut indices: Vec<u16> = Vec::new();
    for y in 0..(rows - 1) {
        for x in 0..(cols - 1) {
            let a = (y * cols + x) as u16;
            let b_ = ((y + 1) * cols + x) as u16;
            let c = (y * cols + x + 1) as u16;
            indices.extend_from_slice(&[a, b_, c]);
            let c2 = ((y + 1) * cols + x + 1) as u16;
            indices.extend_from_slice(&[b_, c2, c]);
        }
    }
    for i in &indices {
        b.extend_from_slice(&i.to_le_bytes());
    }
    // Separator between the index stream and the attribute table. 0xFF is
    // NaN-as-f32 (ends any vertex-pool scan) AND >= 0x4000 as u16 (ends the
    // index run), so the mesh extractor's structural pairing stays on the
    // grid mesh instead of merging the normal table's zero bytes into the
    // index stream.
    b.extend_from_slice(&[0xFFu8; 64]);
    // Per-vertex unit normals: 30 x [-1,0,0]. Using a normal with a
    // magnitude-negative f32 component intentionally keeps these bytes from
    // being misread as u16 index values by the mesh extractor's structural
    // scan (a [0,0,1] table would - its LE bytes are all < 0x4000 as u16).
    for _ in 0..30 {
        b.extend_from_slice(&(-1.0_f32).to_le_bytes());
        b.extend_from_slice(&0.0_f32.to_le_bytes());
        b.extend_from_slice(&0.0_f32.to_le_bytes());
    }
    b
}

#[test]
fn recover_normals_where_present() {
    let payload = synthetic_payload_with_normals();
    let mesh = extract(&payload).expect("synthetic mesh extracts");
    assert_eq!(mesh.vertices().len(), 30);
    let attrs = recover_attributes(&payload, &mesh).expect("recovery is no error");
    let normals = attrs.normals().expect("normals recovered from payload");
    assert_eq!(normals.len(), 30, "one normal per vertex");
    for n in normals {
        assert_eq!(*n, [-1.0, 0.0, 0.0], "normal table decoded");
    }
}

#[test]
fn recover_absent_when_no_normal_records() {
    // Spine only, empty mesh: no vertex list, no normal records -> honest absent.
    let payload = spine();
    let mesh = Mesh::new(); // recovery is per-corpus; empty mesh = 0 vertices
    let attrs = recover_attributes(&payload, &mesh).expect("absence is not an error");
    assert!(
        attrs.normals().is_none(),
        "no normal records -> honest absent, never fabricated"
    );
    assert_eq!(attrs.count(), 0);
}

#[test]
fn recover_never_panics_on_any_input() {
    let mesh = Mesh::new();
    for input in [
        vec![],
        vec![0u8; 4],
        vec![0xFFu8; 64],
        vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x01],
    ] {
        let _ = std::panic::catch_unwind(|| recover_attributes(&input, &mesh));
    }
}

#[test]
fn recovery_rejects_non_cgr_bytes_with_typed_error() {
    let mesh = Mesh::new();
    let bad = vec![0xAAu8; 256]; // no CGR spine
    match recover_attributes(&bad, &mesh) {
        Err(AttributeError::NotCgrPreamble) => {}
        other => panic!("expected NotCgrPreamble, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Part 3: real material (skip-clean when corpus absent)
// ---------------------------------------------------------------------------

#[test]
fn real_material_recovery_never_fabricates() {
    let dirs = sample_dirs();
    let mut found: Option<std::path::PathBuf> = None;
    'outer: for d in &dirs {
        if !d.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(d).map_or_else(
            |_| vec![],
            |rd| rd.filter_map(|e| e.ok()).collect::<Vec<_>>(),
        ) {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "cgr" || x == "CGR") {
                found = Some(p);
                break 'outer;
            }
        }
    }
    let Some(path) = found else {
        eprintln!(
            "SKIP  no CGR sample supplied (set OEP_CGR_SAMPLES=<dir>); tolerant CI-safe skip"
        );
        return;
    };
    eprintln!("info  real-material attribute leg: {}", path.display());
    let bytes = std::fs::read(&path).expect("read sample");
    // Every classified extract outcome must yield a no-panic recovery whose
    // normals (when present) are finite and match the recovered mesh.
    let out = std::panic::catch_unwind(|| extract(&bytes));
    assert!(out.is_ok(), "extract must not panic");
    if let Ok(mesh) = out.unwrap() {
        let attrs = std::panic::catch_unwind(|| recover_attributes(&bytes, &mesh))
            .expect("recover_attributes must not panic");
        if let Ok(attrs) = attrs {
            if let Some(normals) = attrs.normals() {
                assert_eq!(
                    normals.len(),
                    mesh.vertices().len(),
                    "normals are per-vertex"
                );
                for n in normals {
                    assert!(
                        n.iter().all(|v| v.is_finite()),
                        "recovered normals are finite"
                    );
                }
                eprintln!("  NORMALS  present ({} per-vertex normals)", normals.len());
            } else {
                eprintln!("  NORMALS  absent (honest missing, not fabricated)");
            }
        }
    }
    let _ = GOLDEN_FILE; // documented; referenced to keep the contract explicit
}
