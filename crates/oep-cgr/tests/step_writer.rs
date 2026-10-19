//! P2-M5-T02 acceptance test (TDD, RED first) — STEP B-Rep passthrough writer
//! and the `oep-cgr2step` CLI.
//!
//! Written BEFORE `oep_cgr::step` and the `oep-cgr2step` binary existed (RED:
//! this file does not compile — there is no `oep_cgr::step` module and no
//! `oep-cgr2step` bin target). The contract, once GREEN, is:
//!
//! 1. `oep_cgr::step::write_step(mesh, opts, writer)` emits a valid
//!    ISO 10303-21 (PART 21) exchange file whose B-Rep is a FACETED shell:
//!    one `SHELL_BASED_SURFACE_MODEL`, per connected component a
//!    `CLOSED_SHELL` (watertight manifold) or `OPEN_SHELL` (boundary or
//!    non-manifold), one `FACE_SURFACE` per triangle bounded by a
//!    `FACE_OUTER_BOUND` -> `POLY_LOOP` of the triangle's 3
//!    `CARTESIAN_POINT`s, each face's surface a `PLANE`. No NURBS, no
//!    analytic B-Rep claims.
//! 2. The file is HONESTLY MARKED as tessellated, NOT claimed as exact:
//!    the `FILE_DESCRIPTION` string, the product name/description and the
//!    shape-representation identifier all spell out that this is a
//!    tessellated faceted passthrough derived from the source CGR
//!    tessellation and is NOT an exact analytic B-Rep.
//! 3. Output is deterministic: the same mesh + options produce byte-identical
//!    bytes (golden-style).
//! 4. Never panics, typed errors: `EmptyMesh`, `IndexOutOfRange`,
//!    `NonFiniteVertex`, `Io`. Degenerate (zero-area) triangles are skipped
//!    with a counted warning (tolerant export), not fatal — a mesh that is
//!    *entirely* degenerate is `EmptyMesh`.
//! 5. CLI `oep-cgr2step INPUT.cgr OUTPUT.step` (binary tested through
//!    `CARGO_BIN_EXE_`): documented exit codes 0 success, 1 I/O, 2 usage,
//!    3 not-a-CGR (header spine mismatch), 4 classified skip / no usable
//!    tessellation, 5 internal mesh geometry invariant failure. Emits a
//!    human-readable tessellation honesty warning at runtime.
//! 6. Real-material leg: every real sample supplied via `OEP_CGR_SAMPLES`
//!    must reach a *classified* CLI outcome (success + valid STEP, or one of
//!    the documented non-zero codes) — never a crash/signal. SKIPs cleanly
//!    when no samples are supplied so CI stays green before corpus sync.

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::process::Command;

use oep_cgr::mesh::Mesh;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oep-cgr2step"))
}

fn sample_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(PathBuf::from(chunk));
            }
        }
    }
    dirs.push(PathBuf::from("samples"));
    dirs.push(PathBuf::from("../samples"));
    dirs
}

fn all_cgr_samples() -> Vec<PathBuf> {
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

/// A unit tetrahedron: 4 vertices, 4 triangles, watertight closed shell.
fn tetrahedron() -> Mesh {
    let mut m = Mesh::new();
    let v0 = m.push_vertex([0.0, 0.0, 0.0]);
    let v1 = m.push_vertex([1.0, 0.0, 0.0]);
    let v2 = m.push_vertex([0.0, 1.0, 0.0]);
    let v3 = m.push_vertex([0.0, 0.0, 1.0]);
    for t in [
        [v0, v2, v1], // base, wound so normal +z
        [v0, v1, v3],
        [v1, v2, v3],
        [v2, v0, v3],
    ] {
        m.push_triangle(t[0], t[1], t[2]).unwrap();
    }
    m
}

fn write_to_string(mesh: &Mesh, product: &str) -> String {
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: product,
        source_name: "TEST.cgr",
        timestamp: "",
    };
    oep_cgr::step::write_step(mesh, &opts, &mut buf).expect("write_step ok");
    String::from_utf8(buf).expect("STEP output is UTF-8 text")
}

// ---------------------------------------------------------------------------
// Part 1: honest tessellated marking, NOT claimed as exact
// ---------------------------------------------------------------------------

#[test]
fn step_output_is_valid_part21_envelope() {
    let s = write_to_string(&tetrahedron(), "tetra");
    assert!(s.starts_with("ISO-10303-21;\n"), "PART 21 start token");
    assert!(s.contains("\nHEADER;\n"), "HEADER section");
    assert!(s.contains("\nDATA;\n"), "DATA section");
    assert!(
        s.trim_end().ends_with("END-ISO-10303-21;"),
        "PART 21 end token"
    );
}

#[test]
fn step_file_description_declares_tessellated_not_exact() {
    let s = write_to_string(&tetrahedron(), "tetra");
    let lc = s.to_ascii_lowercase();
    assert!(
        lc.contains("tessellated"),
        "FILE_DESCRIPTION / header spells out tessellated: {s}"
    );
    assert!(
        lc.contains("not ") && lc.contains("exact"),
        "FILE_DESCRIPTION says NOT exact (never claims an analytic B-Rep): {s}"
    );
}

#[test]
fn product_and_representation_names_mark_tessellation() {
    let s = write_to_string(&tetrahedron(), "tetra-sample");
    let lc = s.to_ascii_lowercase();
    // The product name/description and the shape-representation identifier
    // must carry the tessellated marker so the intent survives downstream.
    assert!(
        s.contains("tetra-sample"),
        "product name carries the caller's product label"
    );
    assert!(
        lc.matches("tessellat").count() >= 2,
        "tessellation marker appears in >=2 places (description + representation id): {s}"
    );
}

// ---------------------------------------------------------------------------
// Part 2: the B-Rep passthrough is FACETED (planar shell), structure-correct
// ---------------------------------------------------------------------------

#[test]
fn step_represents_mesh_as_shell_of_planar_faces() {
    let s = write_to_string(&tetrahedron(), "tetra");
    for token in [
        "SHELL_BASED_SURFACE_MODEL",
        "CLOSED_SHELL",
        "FACE_SURFACE",
        "FACE_OUTER_BOUND",
        "POLY_LOOP",
        "PLANE",
        "AXIS2_PLACEMENT_3D",
        "CARTESIAN_POINT",
    ] {
        assert!(s.contains(token), "expected entity token {token}");
    }
    // Four triangles => four planar faces.
    assert_eq!(
        s.matches("FACE_SURFACE(").count(),
        4,
        "one face per triangle"
    );
    assert_eq!(s.matches("POLY_LOOP(").count(), 4, "one bound per face");
    // A tetrahedron is a closed shell; there must be a CLOSED_SHELL and no
    // OPEN_SHELL in this output.
    assert_eq!(s.matches("CLOSED_SHELL(").count(), 1);
    assert!(
        !s.contains("OPEN_SHELL"),
        "watertight mesh -> closed shell only"
    );
    // No analytic surface/NURBS claims may appear.
    assert!(!s.contains("ADVANCED_FACE"), "no ADVANCED_FACE");
    assert!(!s.contains("B_SPLINE"), "no NURBS");
    assert!(!s.contains("CONICAL_SURFACE"), "no analytic surfaces");
    assert!(!s.contains("CYLINDRICAL_SURFACE"), "no analytic surfaces");
    assert!(!s.contains("SPHERICAL_SURFACE"), "no analytic surfaces");
}

#[test]
fn step_open_mesh_uses_open_shell() {
    // A single triangle is an open (boundary) surface: OPEN_SHELL.
    let mut m = Mesh::new();
    m.push_vertex([0.0, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]);
    m.push_vertex([0.0, 1.0, 0.0]);
    m.push_triangle(0, 1, 2).unwrap();
    let s = write_to_string(&m, "tri");
    assert!(s.contains("OPEN_SHELL"), "single triangle -> OPEN_SHELL");
    assert!(
        !s.contains("CLOSED_SHELL"),
        "open mesh must not claim CLOSED_SHELL"
    );
    assert_eq!(s.matches("FACE_SURFACE(").count(), 1);
}

#[test]
fn step_disconnected_components_become_separate_shells() {
    // Two disconnected tetrahedra => a SHELL_BASED_SURFACE_MODEL with two
    // CLOSED_SHELL entries (one per connected component).
    let mut m = Mesh::new();
    for base in [0.0f32, 10.0f32] {
        let v0 = m.push_vertex([base + 0.0, 0.0, 0.0]);
        let v1 = m.push_vertex([base + 1.0, 0.0, 0.0]);
        let v2 = m.push_vertex([base + 0.0, 1.0, 0.0]);
        let v3 = m.push_vertex([base + 0.0, 0.0, 1.0]);
        for t in [[v0, v2, v1], [v0, v1, v3], [v1, v2, v3], [v2, v0, v3]] {
            m.push_triangle(t[0], t[1], t[2]).unwrap();
        }
    }
    let s = write_to_string(&m, "two-tets");
    assert_eq!(
        s.matches("CLOSED_SHELL(").count(),
        2,
        "two components -> two shells"
    );
    assert_eq!(s.matches("FACE_SURFACE(").count(), 8);
}

// ---------------------------------------------------------------------------
// Part 3: determinism, numeric hygiene
// ---------------------------------------------------------------------------

#[test]
fn step_output_is_deterministic_byte_for_byte() {
    let a = write_to_string(&tetrahedron(), "tetra");
    let b = write_to_string(&tetrahedron(), "tetra");
    assert_eq!(a, b, "same mesh + opts -> identical bytes");
}

#[test]
fn step_output_has_no_nan_or_infinity() {
    let s = write_to_string(&tetrahedron(), "tetra");
    for bad in ["NaN", "nan", "inf", "Infinity"] {
        assert!(!s.contains(bad), "output must not contain {bad}");
    }
}

#[test]
fn step_output_references_are_real_entity_ids() {
    // Every `#` in the DATA body must be a well-formed `#<digits>` reference.
    // Guards against a stray unresolved `#{var}` format placeholder leaking
    // into the file (which only a full parser, not a substring assert, would
    // otherwise catch).
    let s = write_to_string(&tetrahedron(), "tetra");
    assert!(!s.contains("#{"), "no unresolved '#{{...}}' may leak: {s}");
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' {
            let next = bytes.get(i + 1);
            assert!(
                next.is_some_and(|c| c.is_ascii_digit()),
                "'#' must be followed by a digit at offset {i}: {s}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Part 4: typed errors — never a panic on bad meshes
// ---------------------------------------------------------------------------

#[test]
fn step_empty_mesh_is_typed_error() {
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: "empty",
        source_name: "x.cgr",
        timestamp: "",
    };
    let err = oep_cgr::step::write_step(&Mesh::new(), &opts, &mut buf).unwrap_err();
    assert_eq!(err, oep_cgr::step::StepError::EmptyMesh);
}

#[test]
fn step_entirely_degenerate_mesh_is_empty_geometry() {
    let mut m = Mesh::new();
    m.push_vertex([0.0, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]); // colinear with v0
    m.push_vertex([2.0, 0.0, 0.0]);
    m.push_triangle(0, 1, 2).unwrap();
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: "flat",
        source_name: "x.cgr",
        timestamp: "",
    };
    let err = oep_cgr::step::write_step(&m, &opts, &mut buf).unwrap_err();
    assert_eq!(err, oep_cgr::step::StepError::EmptyMesh);
}

#[test]
fn step_skips_degenerate_triangles_but_keeps_good_ones() {
    let mut m = Mesh::new();
    let v0 = m.push_vertex([0.0, 0.0, 0.0]);
    let v1 = m.push_vertex([1.0, 0.0, 0.0]);
    let v2 = m.push_vertex([0.0, 1.0, 0.0]);
    let d2 = m.push_vertex([2.0, 0.0, 0.0]); // colinear -> degenerate with v0,v1
    m.push_triangle(v0, v1, v2).unwrap(); // good facet
    m.push_triangle(v0, v1, d2).unwrap(); // zero-area facet
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: "mixed",
        source_name: "x.cgr",
        timestamp: "",
    };
    let stats = oep_cgr::step::write_step(&m, &opts, &mut buf).expect("mixed mesh exports");
    assert_eq!(stats.faces, 1, "one usable face");
    assert_eq!(
        stats.degenerate_skipped, 1,
        "one degenerate triangle skipped, not fatal"
    );
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.matches("FACE_SURFACE(").count(), 1);
}

#[test]
fn step_out_of_range_index_is_typed_error() {
    let mut m = Mesh::new();
    m.push_vertex([0.0, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]);
    m.push_vertex([0.0, 1.0, 0.0]);
    m.push_triangle_raw(0, 1, 99); // index 99 does not exist
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: "bad",
        source_name: "x.cgr",
        timestamp: "",
    };
    let err = oep_cgr::step::write_step(&m, &opts, &mut buf).unwrap_err();
    assert!(matches!(
        err,
        oep_cgr::step::StepError::IndexOutOfRange { .. }
    ));
}

#[test]
fn step_nonfinite_vertex_is_typed_error() {
    let mut m = Mesh::new();
    m.push_vertex([f32::NAN, 0.0, 0.0]);
    m.push_vertex([1.0, 0.0, 0.0]);
    m.push_vertex([0.0, 1.0, 0.0]);
    m.push_triangle(0, 1, 2).unwrap();
    let mut buf = Vec::new();
    let opts = oep_cgr::step::StepOpts {
        product_name: "nan",
        source_name: "x.cgr",
        timestamp: "",
    };
    let err = oep_cgr::step::write_step(&m, &opts, &mut buf).unwrap_err();
    let description = format!("{err}");
    assert!(
        description.to_lowercase().contains("non-fin"),
        "NonFiniteVertex typed, got: {description}"
    );
}

#[test]
fn step_writer_never_panics_on_random_meshes() {
    // Crude mini-fuzz: deterministic pseudo-random meshes must never unwind.
    let mut state = 0x9E37_79B9u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32 * 20.0 - 10.0
    };
    for _ in 0..50 {
        let mut m = Mesh::new();
        let vn = 3 + (state as usize % 30);
        for _ in 0..vn {
            m.push_vertex([next(), next(), next()]);
        }
        let tn = state as usize % 60;
        for _ in 0..tn {
            let tri = [
                next() as u32 % vn as u32,
                next() as u32 % vn as u32,
                next() as u32 % vn as u32,
            ];
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let _ = m.push_triangle(tri[0], tri[1], tri[2]);
            }));
        }
        let opts = oep_cgr::step::StepOpts {
            product_name: "fuzz",
            source_name: "fuzz.cgr",
            timestamp: "",
        };
        let mut buf = Vec::new();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            oep_cgr::step::write_step(&m, &opts, &mut buf)
        }));
        assert!(result.is_ok(), "write_step must never panic");
        if let Ok(Ok(stats)) = result {
            assert!(stats.faces > 0 || m.triangles().is_empty() || stats.degenerate_skipped > 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Part 5: CLI behaviour (exit codes, honesty warning, real material leg)
// ---------------------------------------------------------------------------

/// Classified CLI outcome per real sample: success (STEP produced) or one of
/// the documented non-zero codes. Anything else (signal, unknown code) fails.
fn classify_cli(out: &std::process::Output) -> Option<i32> {
    match out.status.code() {
        Some(c) if c == 0 || (1..=5).contains(&c) => Some(c),
        Some(c) => {
            panic!("CLI exited with undocumented code {c}");
        }
        None => panic!("CLI terminated by signal"),
    }
}

#[test]
fn cli_missing_args_is_usage_error() {
    let out = cli().output().expect("run cli");
    classify_cli(&out);
    assert_eq!(out.status.code(), Some(2), "exit 2 = usage error");
    let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(err.contains("usage"), "usage text on stderr: {err}");
}

#[test]
fn cli_extra_args_is_usage_error() {
    let tmp = std::env::temp_dir().join("oep-cgr2step-extra.tmp");
    let out = cli()
        .arg(&tmp)
        .arg(&tmp)
        .arg(&tmp)
        .output()
        .expect("run cli");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn cli_missing_input_file_is_io_error() {
    let missing = std::env::temp_dir().join("oep-cgr2step-definitely-missing.cgr");
    let out_path = std::env::temp_dir().join("oep-cgr2step-out.step");
    let _ = std::fs::remove_file(&out_path);
    let out = cli()
        .arg(&missing)
        .arg(&out_path)
        .output()
        .expect("run cli");
    classify_cli(&out);
    assert_eq!(out.status.code(), Some(1), "exit 1 = I/O error");
}

#[test]
fn cli_garbage_input_is_not_a_cgr() {
    let mut tmp = std::env::temp_dir();
    tmp.push("oep-cgr2step-garbage.cgr");
    std::fs::write(&tmp, b"this is definitely not a CGR file").unwrap();
    let out_path = std::env::temp_dir().join("oep-cgr2step-garbage.step");
    let _ = std::fs::remove_file(&out_path);
    let out = cli().arg(&tmp).arg(&out_path).output().expect("run cli");
    classify_cli(&out);
    assert_eq!(out.status.code(), Some(3), "exit 3 = not a CGR");
    assert!(!out_path.exists(), "no STEP written for a non-CGR input");
}

/// CGR-shaped payload with a valid observed header spine but NO triangle
/// stream: the extractor must produce a classified skip, the CLI exit 4.
fn cgr_payload_without_mesh() -> Vec<u8> {
    use oep_cgr::mesh::{extract, MeshError};
    let mut b = Vec::new();
    b.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    b.extend_from_slice(&[0u8; 0x60]);
    b.extend_from_slice(&0x10CD_3600u32.to_le_bytes());
    let template: [u8; 16] = [
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ];
    b.extend_from_slice(&template);
    b.extend_from_slice(&[0xFFu8; 256]); // NaN/opaque -> no vertex pool, no index run
    assert!(matches!(extract(&b), Err(MeshError::MeshNotFound(_))));
    b
}

#[test]
fn cli_cgr_without_tessellated_mesh_is_classified_skip() {
    let payload = cgr_payload_without_mesh();
    let mut tmp = std::env::temp_dir();
    tmp.push("oep-cgr2step-nomesh.cgr");
    std::fs::write(&tmp, &payload).unwrap();
    let out_path = std::env::temp_dir().join("oep-cgr2step-nomesh.step");
    let _ = std::fs::remove_file(&out_path);
    let out = cli().arg(&tmp).arg(&out_path).output().expect("run cli");
    classify_cli(&out);
    assert_eq!(
        out.status.code(),
        Some(4),
        "exit 4 = classified skip, no geometry"
    );
    assert!(!out_path.exists());
}

/// Build a CGR-shaped payload that `extract` latches into a mesh (the
/// observed header spine followed by a contiguous LE-f32 vertex pool, a
/// separator, then a LE-u16 index stream). Independent of, but matching the
/// same observed facts as, the P2-M2-T01 golden synthetic payload.
fn cgr_payload_with_grid_mesh(cols: usize, rows: usize) -> Vec<u8> {
    use oep_cgr::mesh::extract;
    let mut b = Vec::new();
    b.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    b.extend_from_slice(&[0u8; 0x60]);
    b.extend_from_slice(&0x10CD_3600u32.to_le_bytes());
    let template: [u8; 16] = [
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ];
    b.extend_from_slice(&template);
    for y in 0..rows {
        for x in 0..cols {
            b.extend_from_slice(&(x as f32).to_le_bytes());
            b.extend_from_slice(&(y as f32).to_le_bytes());
            b.extend_from_slice(&0.0_f32.to_le_bytes());
        }
    }
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
    assert!(extract(&b).is_ok(), "test payload must latch a mesh");
    b
}

#[test]
fn cli_converts_mesh_bearing_cgr_to_valid_step() {
    let payload = cgr_payload_with_grid_mesh(6, 5); // 30 verts, 40 tris, open grid
    let mut input = std::env::temp_dir();
    input.push("oep-cgr2step-grid.cgr");
    std::fs::write(&input, &payload).unwrap();
    let out_path = std::env::temp_dir().join("oep-cgr2step-grid.step");
    let _ = std::fs::remove_file(&out_path);

    let out = cli().arg(&input).arg(&out_path).output().expect("run cli");
    classify_cli(&out);
    assert_eq!(out.status.code(), Some(0), "exit 0 on success");
    assert!(out_path.exists(), "STEP file written");
    let s = std::fs::read_to_string(&out_path).expect("read step");
    assert!(s.starts_with("ISO-10303-21;"), "valid PART 21 head");
    assert!(
        s.trim_end().ends_with("END-ISO-10303-21;"),
        "valid PART 21 tail"
    );
    let lc = s.to_ascii_lowercase();
    assert!(lc.contains("tessellat"), "CLI output is marked tessellated");
    assert!(
        lc.contains("not ") && lc.contains("exact"),
        "CLI output says NOT exact"
    );

    // The honesty warning must reach the user's stderr.
    let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        err.contains("tessellat") && err.contains("not"),
        "runtime honesty warning on stderr: {err}"
    );
}

// ---------------------------------------------------------------------------
// Part 6: real material (skip-clean when the corpus is absent)
// ---------------------------------------------------------------------------

#[test]
fn real_material_every_sample_classified_cli_outcome() {
    let samples = all_cgr_samples();
    if samples.is_empty() {
        eprintln!("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)");
        return;
    }
    let tmp = std::env::temp_dir();
    let mut n_success = 0usize;
    let mut n_skip = 0usize;
    let mut n_noncgr = 0usize;
    for p in &samples {
        let out_path = tmp.join(format!(
            "oep-cgr2step-real-{}.step",
            p.file_stem().unwrap().to_string_lossy()
        ));
        let _ = std::fs::remove_file(&out_path);
        let out = cli()
            .arg(p)
            .arg(&out_path)
            .output()
            .expect("run cli on real sample");
        let code = classify_cli(&out);
        match code {
            Some(0) => {
                n_success += 1;
                assert!(out_path.exists(), "success must write a STEP: {p:?}");
                let s = std::fs::read_to_string(&out_path).unwrap();
                assert!(
                    s.starts_with("ISO-10303-21;"),
                    "success STEP is PART 21: {p:?}"
                );
                eprintln!("  STEP  {:?}", p.file_name().unwrap());
            }
            Some(3) => {
                n_noncgr += 1;
                eprintln!("  NOT-CGR {:?}", p.file_name().unwrap());
            }
            Some(4) => {
                n_skip += 1;
                eprintln!(
                    "  SKIP  {:?} (no tessellation found)",
                    p.file_name().unwrap()
                );
            }
            _ => {
                // 1 (I/O) or 5 (internal invariant) on a readable sample: fail loudly.
                panic!("unexpected CLI code {code:?} on readable sample {p:?}");
            }
        }
        assert!(
            !out.status.code().is_none(),
            "CLI must never be killed by a signal on {p:?}"
        );
    }
    eprintln!(
        "  summary: {} step, {} not-cgr, {} classified skip of {} samples",
        n_success,
        n_noncgr,
        n_skip,
        samples.len()
    );
    assert_eq!(
        n_success + n_noncgr + n_skip,
        samples.len(),
        "every real sample reaches a classified outcome"
    );
}
