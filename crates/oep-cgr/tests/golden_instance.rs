//! P2-M4-T02 acceptance test (TDD, RED first) — transform/instance hierarchy
//! (assembly CGR inside part) with a matrix stack.
//!
//! Written BEFORE `oep_cgr::instance` and `oep_cgr::assembly` existed (RED:
//! this file does not compile against the skeleton crate). The contract once
//! GREEN is:
//!
//! 1. `instance` — a format-independent, fully deterministic model:
//!    * `Transform` (row-major 4x4 f32): `from_rows` (rejecting non-finite
//!      input via `TransformError::NonFinite`), `mul` (matrix composition
//!      `self * rhs`), `apply_point` (homogeneous transform of a 3D point),
//!      `is_identity`, and deterministic `from_translation` /
//!      `from_rotation_z` builders.
//!    * `MatrixStack` — the push/pop/multiply-right device that accumulates
//!      parent→child transforms during a depth-first walk of the instance
//!      tree (OpenGL-style semantics for scene graphs). Starts at identity,
//!      `push` saves a frame, `mult_right` composes onto the current frame,
//!      `pop` restores, popping the last frame is an error, `depth` and
//!      `current` report stack state.
//!    * `Instance` — id, optional name, local `Transform`, children, and an
//!      opaque `mesh` reference (u32 index — deliberately decoupled from the
//!      mesh pool so this crate builds on `main` and merges with the
//!      not-yet-landed `mesh` module later).
//!    * `SceneGraph` — roots plus `world_instances()`: a DFS that uses the
//!      stack to compose every instance's local transform with its parents'
//!      and yields `(path, instance, world transform)` in pre-order. This is
//!      the "assembly CGR inside part" scenario: a part CGR whose payload
//!      carries N placed instances forming a hierarchy.
//!
//!    The model is exercised by synthetic fixtures; no real bytes are needed
//!    for these assertions.
//!
//! 2. `assembly` — `scan_assembly_structure(bytes)` reports a *classified*
//!    outcome:
//!    * `NotCgrPreamble` for bytes lacking the observed CGR header spine;
//!    * `Assembly { instances, transforms }` when a self-consistent
//!      transform/instance stream is found (>=2 rigid transforms forming a
//!      thin stream);
//!    * `NoAssembly(reason)` — a typed skip naming the measured evidence
//!      (P2-M1-T04 tolerant doctrine); the scanner never panics and never
//!      fabricates geometry.
//!
//!    The scanner is strict: on the verified old-era single-part corpus
//!    (11 samples, cfysa + piston) the observed facts show ZERO rigid or
//!    near-identity transform records (probe table in
//!    `docs/re-notes/oep-cgr-instance-transforms.md`), so real material
//!    today classifies as `NoAssembly` with the measured reason.
//!
//! 3. Real material: with `OEP_CGR_SAMPLES` set, every `.cgr` yields a
//!    classified outcome (no panics); `VALVE.cgr` is among them. Without it,
//!    the real-material leg prints a tolerant SKIP and passes, so CI stays
//!    green before the (never-committed, clean-room) corpus is present.

use oep_cgr::assembly::{scan_assembly_structure, AssemblyOutcome};
use oep_cgr::instance::{
    Instance, MatrixStack, MatrixStackError, SceneGraph, Transform, TransformError,
};

// ---------------------------------------------------------------------------
// Part 1: Transform — deterministic matrix math
// ---------------------------------------------------------------------------

#[test]
fn identity_is_identity() {
    let i = Transform::identity();
    assert!(i.is_identity());
}

#[test]
fn from_rows_roundtrips() {
    let rows = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let t = Transform::from_rows(rows).expect("finite rows accepted");
    assert_eq!(t.rows(), &rows);
    assert!(!t.is_identity());
}

#[test]
fn from_rows_rejects_non_finite() {
    let bad = [
        [1.0, 2.0, 3.0, f32::NAN],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    assert_eq!(Transform::from_rows(bad), Err(TransformError::NonFinite));
    let inf = bad.map(|r| r.map(|x| if x == 3.0 { f32::INFINITY } else { x }));
    assert_eq!(Transform::from_rows(inf), Err(TransformError::NonFinite));
}

#[test]
fn mul_composes_matrices() {
    // T(1,0,0) * T(2,0,0) == T(3,0,0)
    let a = Transform::from_translation(1.0, 0.0, 0.0);
    let b = Transform::from_translation(2.0, 0.0, 0.0);
    let c = a.mul(&b);
    assert_eq!(c.apply_point([0.0, 0.0, 0.0]), [3.0, 0.0, 0.0]);
    // identity is the multiplicative identity on both sides
    let i = Transform::identity();
    assert_eq!(a.mul(&i).apply_point([5.0, 1.0, 2.0]), [6.0, 1.0, 2.0]);
    assert_eq!(i.mul(&a).apply_point([5.0, 1.0, 2.0]), [6.0, 1.0, 2.0]);
}

#[test]
fn apply_point_homogeneous() {
    let t = Transform::from_translation(10.0, 20.0, 30.0);
    let p = t.apply_point([1.0, 1.0, 1.0]);
    for (got, want) in p.iter().zip([11.0, 21.0, 31.0]) {
        assert!((got - want).abs() < 1e-4, "expected {want}, got {got}");
    }
}

#[test]
fn rotation_z_maps_axes() {
    // rotate 90deg about Z: +X -> +Y
    let r = Transform::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let p = r.apply_point([1.0, 0.0, 0.0]);
    assert!(p[0].abs() < 1e-4, "cos 90 ~ 0, got {}", p[0]);
    assert!((p[1] - 1.0).abs() < 1e-4, "sin 90 ~ 1, got {}", p[1]);
    assert!(p[2].abs() < 1e-4);
    assert!(!r.is_identity());
}

#[test]
fn scale_is_not_rigid() {
    // A scaling matrix must not be reported as identity.
    let s = [
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 2.0, 0.0, 0.0],
        [0.0, 0.0, 2.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let t = Transform::from_rows(s).unwrap();
    assert!(!t.is_identity());
}

// ---------------------------------------------------------------------------
// Part 2: MatrixStack — push/pop/multiply-right scene graph device
// ---------------------------------------------------------------------------

#[test]
fn stack_starts_at_identity() {
    let s = MatrixStack::new();
    assert_eq!(s.depth(), 1);
    assert!(s.current().is_identity());
}

#[test]
fn mult_right_composes_onto_current() {
    let mut s = MatrixStack::new();
    s.mult_right(&Transform::from_translation(1.0, 0.0, 0.0));
    s.mult_right(&Transform::from_translation(2.0, 0.0, 0.0));
    assert_eq!(s.current().apply_point([0.0, 0.0, 0.0]), [3.0, 0.0, 0.0]);
}

#[test]
fn push_and_pop_roundtrip() {
    let mut s = MatrixStack::new();
    s.mult_right(&Transform::from_translation(5.0, 0.0, 0.0));
    s.push();
    s.mult_right(&Transform::from_translation(7.0, 0.0, 0.0));
    assert_eq!(s.current().apply_point([0.0, 0.0, 0.0]), [12.0, 0.0, 0.0]);
    assert_eq!(s.depth(), 2);
    s.pop().expect("pop within bounds");
    assert_eq!(s.depth(), 1);
    assert_eq!(s.current().apply_point([0.0, 0.0, 0.0]), [5.0, 0.0, 0.0]);
}

#[test]
fn pop_below_floor_is_error() {
    let mut s = MatrixStack::new();
    assert_eq!(s.depth(), 1);
    assert_eq!(s.pop(), Err(MatrixStackError::Empty));
    // stack still usable
    s.mult_right(&Transform::from_translation(1.0, 0.0, 0.0));
    assert_eq!(s.current().apply_point([0.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------------
// Part 3: SceneGraph — the assembly-CGR-inside-a-part walk
// ---------------------------------------------------------------------------

/// Build a 3-level hierarchy:
///   root (identity)
///     └─ child A  T(10,0,0)
///          └─ grandchild B  Rz(90deg)
/// with B also carrying a mesh reference (id 7) and A a mesh ref (id 1).
fn sample_scene() -> SceneGraph {
    let mut b = Instance::new(3, Some("B".to_string()));
    b.set_local(Transform::from_rotation_z(std::f32::consts::FRAC_PI_2));
    b.set_mesh(Some(7));

    let mut a = Instance::new(2, Some("A".to_string()));
    a.set_local(Transform::from_translation(10.0, 0.0, 0.0));
    a.set_mesh(Some(1));
    a.add_child(b);

    let mut root = Instance::new(1, Some("root".to_string()));
    root.add_child(a);

    let mut scene = SceneGraph::new();
    scene.add_root(root);
    scene
}

#[test]
fn world_instances_accumulates_parent_transforms() {
    let scene = sample_scene();
    let worlds = scene.world_instances();
    assert_eq!(worlds.len(), 3, "every instance visited exactly once");

    // root world == identity
    let root = worlds
        .iter()
        .find(|w| w.instance.id() == 1)
        .expect("root present");
    assert!(root.world.is_identity());
    assert_eq!(root.path, vec![]);

    // A world translation == T(10,0,0)
    let a = worlds
        .iter()
        .find(|w| w.instance.id() == 2)
        .expect("A present");
    assert_eq!(a.world.apply_point([0.0, 0.0, 0.0]), [10.0, 0.0, 0.0]);
    assert_eq!(a.path, vec![0]);

    // B world = A.w * B.local = translate(10,0,0) * rotateZ(90)
    let b = worlds
        .iter()
        .find(|w| w.instance.id() == 3)
        .expect("B present");
    assert_eq!(b.path, vec![0, 0]);
    // origin maps to (10,0,0)
    let o = b.world.apply_point([0.0, 0.0, 0.0]);
    assert!((o[0] - 10.0).abs() < 1e-4);
    assert!(o[1].abs() < 1e-4);
    // +X (1,0,0) under Rz90 becomes +Y, then translated by A's world
    let p = b.world.apply_point([1.0, 0.0, 0.0]);
    assert!(
        (p[0] - 10.0).abs() < 1e-4,
        "cos90*1 -> ~0 +10, got {}",
        p[0]
    );
    assert!((p[1] - 1.0).abs() < 1e-4, "sin90*1 -> ~1, got {}", p[1]);
    assert!(p[2].abs() < 1e-4);
}

#[test]
fn world_instances_preorder_visits_each_once() {
    let scene = sample_scene();
    let worlds = scene.world_instances();
    let ids: Vec<u32> = worlds.iter().map(|w| w.instance.id()).collect();
    // pre-order from a DFS of root -> A -> B
    assert_eq!(ids, vec![1, 2, 3]);
    // path lengths grow monotonically along the walk
    let mut prev = 0usize;
    for w in &worlds {
        assert!(w.path.len() >= prev);
        prev = w.path.len();
    }
}

#[test]
fn mesh_reference_carries_through() {
    let scene = sample_scene();
    let worlds = scene.world_instances();
    let a = worlds.iter().find(|w| w.instance.id() == 2).unwrap();
    assert_eq!(a.instance.mesh(), Some(1));
    let b = worlds.iter().find(|w| w.instance.id() == 3).unwrap();
    assert_eq!(b.instance.mesh(), Some(7));
    let root = worlds.iter().find(|w| w.instance.id() == 1).unwrap();
    assert_eq!(root.instance.mesh(), None);
}

// ---------------------------------------------------------------------------
// Part 4: assembly scanner — adversarial, never panics
// ---------------------------------------------------------------------------

#[test]
fn scanner_never_panics_on_junk() {
    let inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x01],                                       // just magic
        (0..16).map(|i| i as u8).collect(),               // tiny garbage
        vec![0xff; 4096],                                 // all 0xff
        vec![0x00; 4096],                                 // all 0x00
        (0..4096).map(|i| (i * 7 % 251) as u8).collect(), // pseudo-random
    ];
    for b in inputs {
        let out = scan_assembly_structure(&b);
        match out {
            AssemblyOutcome::NotCgrPreamble
            | AssemblyOutcome::NoAssembly { .. }
            | AssemblyOutcome::Assembly { .. } => {}
        }
    }
}

#[test]
fn scanner_rejects_non_cgr_magic() {
    // Valid-sized but wrong magic -> NotCgrPreamble
    let mut b = vec![0u8; 0x1000];
    b[0] = 0x55; // modern-valve magic is NOT the observed R19 spine magic
    assert!(matches!(
        scan_assembly_structure(&b),
        AssemblyOutcome::NotCgrPreamble
    ));
}

#[test]
fn scanner_classifies_synthetic_assembly() {
    // Build CGR preamble (observed spine) then append a thin stream of two
    // rigid 4x4 transforms. The scanner must report an Assembly, never
    // fabricating instance ids beyond what the bytes carry.
    let mut b = vec![0u8; 0x1000];
    b[0..4].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    b[0x68..0x78].copy_from_slice(&[
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ]);
    // two consecutive 4x4 rigid transforms: translation(1,0,0), translation(2,0,0)
    let t1 = Transform::from_translation(1.0, 0.0, 0.0);
    let t2 = Transform::from_translation(2.0, 0.0, 0.0);
    let off = 0x80;
    for (i, t) in [&t1, &t2].iter().enumerate() {
        let rows = t.rows();
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let o = off + i * 64 + r * 16 + c * 4;
                b[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
    }
    match scan_assembly_structure(&b) {
        AssemblyOutcome::Assembly { instances, .. } => {
            assert!(
                instances.len() >= 2,
                "stream of >=2 transforms -> instances"
            );
        }
        other => panic!("expected Assembly, got {other:?}"),
    }
    // no new-preamble dependency: truncated AFTER the stream still classifies
    let short = &b[..off + 64 + 8];
    let _ = scan_assembly_structure(short);
}

// ---------------------------------------------------------------------------
// Part 5: real material (tolerant skip when corpus absent)
// ---------------------------------------------------------------------------

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

fn collect_samples() -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    for dir in sample_dirs() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("cgr") {
                    if let Ok(bytes) = std::fs::read(&p) {
                        found.push((p.file_name().unwrap().to_string_lossy().into_owned(), bytes));
                    }
                }
            }
        }
    }
    found
}

#[test]
fn real_material_classified_outcome() {
    let samples = collect_samples();
    if samples.is_empty() {
        eprintln!("SKIP: no real CGR corpus at OEP_CGR_SAMPLES — deterministic legs only");
        return; // tolerant skip; CI green without private corpus
    }
    let names: Vec<&str> = samples.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"VALVE.cgr"),
        "golden sample VALVE.cgr must be in the corpus: {names:?}"
    );
    for (name, bytes) in &samples {
        let out = scan_assembly_structure(bytes);
        match out {
            // All three are *classified* outcomes. NotCgrPreamble is a
            // legitimate answer: a differently-magic modern-era CGR (0x55
            // family, see RE note) is not a member of the observed R19 spine
            // family this scanner recognizes yet.
            AssemblyOutcome::Assembly { .. }
            | AssemblyOutcome::NoAssembly { .. }
            | AssemblyOutcome::NotCgrPreamble => {}
        }
        eprintln!("CLASSIFIED {name}: {out:?} ({} bytes)", bytes.len());
    }
    eprintln!(
        "summary: {} samples, all classified, zero panics",
        samples.len()
    );
}
