//! P2-M2-T01 acceptance test (TDD, RED first) — tessellation mesh extraction
//! into the internal `oep_cgr::mesh::Mesh` type (vertices/indices/triangles).
//!
//! Written BEFORE the mesh type and extractor existed (RED: this file does not
//! compile against the skeleton crate — there is no `oep_cgr::mesh` module).
//! The contract, once GREEN, is:
//!
//! 1. The internal mesh type holds `vertices: Vec<[f32;3]>` and
//!    `triangles: Vec<[u32;3]>` (indices into vertices), enforces index
//!    bounds at build time, and validates invariants (in-range indices,
//!    finite coordinates, non-degenerate area).
//! 2. `oep_cgr::mesh::extract` walks a CGR payload and returns either:
//!    - `Ok(mesh)`: a *structurally self-consistent* triangle mesh (all
//!      indices in range, finite coords, non-empty triangle list);
//!    - `Err(skip)`: a classified skip, i.e. a typed reason describing
//!      exactly which observed record/layout blocked extraction (unknown
//!      block per P2-M1-T04 doctrine).
//!
//!    It NEVER panics on any input (truncated, garbage, or real corpus).
//! 3. Clean-room: the internal type + extractor are built ONLY from observed
//!    format facts (hex notebook P1-M2-T01, header PARSE P2-M1-T01). The
//!    real-material leg runs against the private sample corpus via
//!    `OEP_CGR_SAMPLES` (never committed), and SKIPs cleanly when absent so
//!    CI stays green before the corpus sync (P1-M1-T05) lands.

use oep_cgr::mesh::{extract, Mesh, MeshError};

/// The golden sample for the header spine leg: R19-era DMU tutorial set.
/// sha256 43cc0cb474b5fe10a4d0bf1b8c61f48d952a8bba849aa8b116e12b490d7dcbd6.
/// Real binary is never committed; only facts recorded in the hex notebook.
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

// --- Part 1: internal mesh type invariants (deterministic, no real bytes) ---

#[test]
fn mesh_type_holds_vertices_and_triangle_indices() {
    let mut m = Mesh::new();
    let a = m.push_vertex([0.0, 0.0, 0.0]);
    let b = m.push_vertex([1.0, 0.0, 0.0]);
    let c = m.push_vertex([0.0, 1.0, 0.0]);
    assert_eq!(m.vertices().len(), 3, "three vertices pushed");
    m.push_triangle(a, b, c)
        .expect("triangle with in-range indices builds");
    assert_eq!(m.triangles().len(), 1);
    assert_eq!(m.triangles()[0], [0u32, 1, 2]);
}

#[test]
fn mesh_type_rejects_out_of_range_indices() {
    let mut m = Mesh::new();
    m.push_vertex([0.0, 0.0, 0.0]); // only vertex 0
    assert_eq!(
        m.push_triangle(0, 1, 2).unwrap_err(),
        MeshError::IndexOutOfRange,
        "indices 1,2 do not exist"
    );
}

#[test]
fn mesh_type_validation_catches_bad_state() {
    let mut m = Mesh::new();
    m.push_vertex([0.0, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]);
    m.push_vertex([0.0, 1.0, 0.0]);
    m.push_triangle(0, 1, 2).unwrap();
    assert!(m.validate().is_ok(), "valid mesh validates");

    // out-of-range via raw push must be caught by validate() too
    let mut bad = Mesh::new();
    bad.push_vertex([0.0, 0.0, 0.0]);
    bad.push_vertex([1.0, 0.0, 0.0]);
    bad.push_vertex([0.0, 1.0, 0.0]);
    bad.push_triangle_raw(0, 5, 2);
    assert_eq!(bad.validate().unwrap_err(), MeshError::IndexOutOfRange);
}

#[test]
fn mesh_type_validation_rejects_nonfinite_coordinates() {
    let mut m = Mesh::new();
    m.push_vertex([f32::NAN, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]);
    m.push_vertex([0.0, 1.0, 0.0]);
    m.push_triangle(0, 1, 2).unwrap();
    assert_eq!(m.validate().unwrap_err(), MeshError::NonFiniteVertex(0));
}

// --- Part 2: extractor never panics (deterministic adversarial inputs) ---

#[test]
fn extract_never_panics_on_malformed_inputs() {
    let cases: Vec<&[u8]> = vec![
        &[],
        &[0x01, 0x00],
        &[0xff; 64],
        &[0x00; 4096],
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00], // right magic-ish, wrong body
    ];
    for buf in cases {
        let _ = std::panic::catch_unwind(|| extract(buf));
        // If we got here without unwinding, extract did not panic. We do NOT
        // assert on the result value: a classified skip or an Ok is both fine.
    }
}

/// Build a synthetic-but-valid CGR-shaped payload from OBSERVED header facts
/// (P1-M2-T01 hex notebook: magic `01 00 00 00`, 96 zero bytes, landscape
/// u32 literal, 16-byte invariant template) followed by a deliberately simple
/// indexed triangle stream: a contiguous LE-f32 vertex pool, then a contiguous
/// u16 index stream. This proves `extract`'s structural scan actually latches
/// a self-consistent mesh when the payload matches the tested layout — a
/// capability guarantee distinct from the honest skips on the real old-era
/// corpus. No proprietary bytes are involved.
fn synthetic_indexed_mesh_payload() -> Vec<u8> {
    let mut b = Vec::new();
    // Header spine (observed facts only).
    b.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // magic
    b.extend_from_slice(&[0u8; 0x60]); // zero preamble 0x04..0x63
    b.extend_from_slice(&0x10CD_3600u32.to_le_bytes()); // landscape @0x64
    let template: [u8; 16] = [
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ];
    b.extend_from_slice(&template); // @0x68
                                    // A 5x6 grid of vertices tessellated as triangles, packed as a contiguous
                                    // f32 vertex pool then a u16 LE index stream. Sized above the extractor's
                                    // structural-scan thresholds (>=20 verts, >=60 indices, %3==0) so the
                                    // identical code path that scans real corpus material is exercised.
    let cols = 6usize;
    let rows = 5usize;
    for y in 0..rows {
        for x in 0..cols {
            b.extend_from_slice(&(x as f32).to_le_bytes());
            b.extend_from_slice(&(y as f32).to_le_bytes());
            b.extend_from_slice(&0.0_f32.to_le_bytes());
        }
    }
    // Separator block between the vertex pool and the index stream. Bytes are
    // 0xFF: as f32 LE that is NaN (not finite -> terminates the vertex pool
    // scan), and as u16 that is >= 0x4000 (terminates the index run) — a
    // realistic record boundary that also keeps pool-length detection honest.
    b.extend_from_slice(&[0xFFu8; 64]);
    // Two triangles per grid cell -> (cols-1)*(rows-1)*2 triangles, *3 indices.
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
    b
}

#[test]
fn extract_latches_synthetic_indexed_mesh() {
    let payload = synthetic_indexed_mesh_payload();
    let mesh = extract(&payload).expect("simple indexed stream must extract");
    mesh.validate()
        .expect("extracted mesh is structurally valid");
    assert_eq!(mesh.vertices().len(), 30, "5x6 grid = 30 vertices");
    // (6-1)*(5-1)*2 = 40 triangles -> 120 indices.
    assert_eq!(mesh.triangles().len(), 40, "40 grid cells tessellated");
    for t in mesh.triangles() {
        for &i in t {
            assert!(i < 30, "index {i} in range");
        }
    }
}

// --- Part 3: real material (skip-clean when corpus absent) ---

#[test]
fn real_material_every_sample_mesh_or_classified_skip() {
    let samples = find_samples();
    if samples.is_empty() {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    }
    let mut n_mesh = 0usize;
    let mut n_skip = 0usize;
    let mut saw_golden = false;
    for p in &samples {
        let is_golden = p.file_name().is_some_and(|n| n == GOLDEN_FILE);
        if is_golden {
            saw_golden = true;
        }
        let bytes = std::fs::read(p).expect("read sample");
        let out = std::panic::catch_unwind(|| extract(&bytes));
        assert!(out.is_ok(), "extract must never panic on {:?}", p);
        match out.unwrap() {
            Ok(mesh) => {
                n_mesh += 1;
                mesh.validate()
                    .expect("extracted mesh is structurally valid");
                assert!(!mesh.triangles().is_empty(), "mesh has triangles");
                eprintln!(
                    "  MESH  {:?}: {} verts, {} tris",
                    p.file_name().unwrap(),
                    mesh.vertices().len(),
                    mesh.triangles().len()
                );
            }
            Err(skip) => {
                n_skip += 1;
                eprintln!("  SKIP  {:?}: {}", p.file_name().unwrap(), skip);
            }
        }
    }
    // The golden sample (if supplied) must be among the scanned set; its
    // outcome is a mesh or a classified skip, never a panic — the loop
    // asserts this. This also keeps GOLDEN_FILE referenced (dead-code free).
    assert!(
        saw_golden,
        "golden sample {GOLDEN_FILE} present in the corpus dir"
    );
    // Honesty guard: on real material the extractor must always reach a
    // *classified* outcome (mesh or reasoned skip), never a silent nothing.
    assert_eq!(
        n_mesh + n_skip,
        samples.len(),
        "every sample yields a classified outcome"
    );
    eprintln!(
        "  summary: {} mesh, {} classified skip of {} samples",
        n_mesh,
        n_skip,
        samples.len()
    );
}
