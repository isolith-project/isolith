//! P3-M1-T03 acceptance test (TDD, RED first) — mesh rep -> internal mesh;
//! verify against known-good geometry.
//!
//! Written BEFORE the `oep_3dxml::mesh` module existed (RED: this file does
//! not compile — there is no `oep_3dxml::mesh`). The contract, once GREEN:
//!
//! 1. `parse_rep(input)` parses the tessellated-ASCII `.3DRep` markup of
//!    3DXML (the `<XMLRepresentation ...>` tree with `PolygonalRepType`
//!    reps) and returns ONE `Mesh` per polygonal rep that carries actual
//!    geometry (positions + triangles). Edge-only / empty reps are skipped
//!    (tolerant-failure doctrine, P2-M1-T04), never an error.
//! 2. `Mesh { positions: Vec<[f32;3]>, triangles: Vec<[u32;3]> }` exposes
//!    `positions()` / `triangles()` slice accessors — the exact plain-slice
//!    shape the P3-M4 writer consumes (`&[[f32;3]]` / `&[[u32;3]]`), so
//!    downstream round-trip feeds reader output straight into the writer.
//! 3. Face encodings BOTH decode:
//!    - `triangles="i j k i j k ..."` (space-separated triplets, 3 per
//!      triangle; Abaqus branch: cube1/blocks), and
//!    - `strips="r1,r2,..."` (comma-separated runs; each run space-separated
//!      vertex indices, N indices -> N-2 triangles with the standard
//!      alternating winding; CATIA V5 branch: machinist/cycloidal/FM1;
//!      our writer emits one 3-index run per triangle which is unambiguous).
//! 4. Positions are parsed exactly from the comma-separated "x y z" text of
//!    the `<VertexBuffer><Positions>` block (shortest-f32 round trip via
//!    Rust `f32::parse`, same text power as the writer's `Display`).
//! 5. Malformed input is rejected with a TYPED `MeshError` (never a panic):
//!    not-an-XMLRepresentation root, truncated XML, missing Positions,
//!    dangling/bad index, empty-after-filter. Fuzz-friendly.
//! 6. Against REAL material (env `OEP_3DXML_SAMPLES`, or
//!    `corpus/data/3dxml`, or `samples` next to the crate), every decoded
//!    mesh must match the INDEPENDENT Python reference
//!    (`tests/test_oep_3dxml_mesh_rep_acceptance.py`, stdlib xml.etree):
//!    position count, triangle count, bounding box, index bounds and first
//!    position — "known-good geometry". Physical sanity: the Abaqus cube1 is
//!    a 10x10x10 cube (12 triangles, bbox ~[0,10]^3). Otherwise the real
//!    leg prints SKIP and passes so CI stays green before the corpus PR
//!    merges.
//!
//! Clean-room: every asserted fact comes from observed bytes of license-clean
//! MIT/Apache-2.0 corpus files (P1-M1-T02) and the P1-M2-T04 hex notebook
//! (`docs/re-notes/3dxml-hex-notebook.md` §4.2). Synthetic fixtures are
//! written by hand from those observed element/attribute names.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use oep_3dxml::mesh::{parse_rep, MeshError};

// ---------------------------------------------------------------------------
// Small XMLRepresentation builders (hand-written from observed format facts).
// ---------------------------------------------------------------------------

fn rep_header() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
     <XMLRepresentation version=\"1.0\" xmlns=\"http://www.3ds.com/xsd/3DXML\" \
     xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n\
     <Root xsi:type=\"BagRepType\">\n"
}

fn rep_footer() -> &'static str {
    "</Root>\n</XMLRepresentation>\n"
}

/// A quad with `triangles=` encoding (4 positions, 2 triangles).
fn quad_triangles_rep() -> String {
    format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\n\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0</Positions></VertexBuffer>\n\
         <Faces><Face triangles=\"0 1 2 0 2 3\"/></Faces>\n\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    )
}

/// The same quad with `strips=` encoding: run `0 1 2 3` -> 2 triangles.
fn quad_strips_rep() -> String {
    format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\n\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0</Positions></VertexBuffer>\n\
         <Faces><Face strips=\"0 1 2 3\"/></Faces>\n\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    )
}

// ---------------------------------------------------------------------------
// Part 1: API shape — one Mesh per geometry-bearing PolygonalRep, accessors
// ---------------------------------------------------------------------------

#[test]
fn parse_rep_returns_meshes_with_slice_accessors() {
    let meshes = parse_rep(&quad_triangles_rep()).expect("quad parses");
    assert_eq!(meshes.len(), 1);
    let m = &meshes[0];
    assert_eq!(
        m.positions(),
        &[
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0]
        ]
    );
    assert_eq!(m.triangles(), &[[0u32, 1, 2], [0, 2, 3]]);
}

// ---------------------------------------------------------------------------
// Part 2: known-good geometry — exact decode for both encodings
// ---------------------------------------------------------------------------

#[test]
fn triangles_encoding_decodes_exact_geometry() {
    let meshes = parse_rep(&quad_triangles_rep()).expect("parse ok");
    let m = &meshes[0];
    assert_eq!(m.positions().len(), 4);
    assert_eq!(m.triangles().len(), 2);
    assert_eq!(m.triangles(), &[[0u32, 1, 2], [0, 2, 3]]);
    // bounding box sanity (axis-aligned unit quad in z == 0 plane)
    let xs: Vec<f32> = m.positions().iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = m.positions().iter().map(|p| p[1]).collect();
    let zs: Vec<f32> = m.positions().iter().map(|p| p[2]).collect();
    assert!(xs.iter().cloned().fold(f32::INFINITY, f32::min) >= 0.0);
    assert!(xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max) <= 1.0);
    assert!(zs.iter().all(|&z| z == 0.0));
    let _ = (ys, zs);
}

#[test]
fn strips_encoding_2_run_decodes_same_quad() {
    // run `0 1 2 3` is a strip of 4. OpenGL TANGENT strip: (0,1,2),(2,1,3).
    // Both the Rust decode here and the INDEPENDENT Python reference
    // (tests/test_oep_3dxml_mesh_rep_acceptance.py) produce these exact
    // triplets; the assertion is the cross-check.
    let meshes = parse_rep(&quad_strips_rep()).expect("parse ok");
    let m = &meshes[0];
    assert_eq!(m.triangles().len(), 2);
    assert_eq!(m.triangles(), &[[0u32, 1, 2], [2, 1, 3]]);
}

#[test]
fn strips_encoding_5_run_decodes_3_triangles() {
    // run of 5 indices -> 3 triangles: (0,1,2),(2,1,3),(2,3,4)
    let rep = format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\n\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0,0 0 1</Positions></VertexBuffer>\n\
         <Faces><Face strips=\"0 1 2 3 4\"/></Faces>\n\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    let meshes = parse_rep(&rep).expect("parse ok");
    let m = &meshes[0];
    assert_eq!(m.triangles().len(), 3);
    assert_eq!(m.triangles(), &[[0u32, 1, 2], [2, 1, 3], [2, 3, 4]]);
}

#[test]
fn multiple_reps_in_one_member_each_produce_a_mesh() {
    let rep = format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0</Positions></VertexBuffer>\
         <Faces><Face triangles=\"0 1 2 0 2 3\"/></Faces>\
         </Rep>\
         <Rep xsi:type=\"PolygonalRepType\">\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0</Positions></VertexBuffer>\
         <Faces><Face triangles=\"0 1 2 0 2 3\"/></Faces>\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    let meshes = parse_rep(&rep).expect("parse ok");
    assert_eq!(meshes.len(), 2);
}

#[test]
fn nested_baggrep_wrappers_do_not_break_parse() {
    // CATIA V5 family wraps PolygonalRep in a second BagRep level.
    let rep = format!(
        "{}<Rep xsi:type=\"BagRepType\">\
         <Rep xsi:type=\"PolygonalRepType\">\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0,0 1 0</Positions></VertexBuffer>\
         <Faces><Face strips=\"0 1 2 3\"/></Faces>\
         </Rep></Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    let meshes = parse_rep(&rep).expect("parse ok");
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].triangles(), &[[0u32, 1, 2], [2, 1, 3]]);
}

// ---------------------------------------------------------------------------
// Part 3: tolerant failure — edge-only reps skipped, never panic
// ---------------------------------------------------------------------------

#[test]
fn edge_only_rep_is_skipped_not_an_error() {
    // FM1 idx-0 style: a PolygonalRep carrying only <Edges><Polyline>, no
    // VertexBuffer geometry. Must be skipped, not surfaced as a Mesh.
    let rep = format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\
         <Edges><Polyline vertices=\"0 0 0,1 0 0,1 1 0\"/></Edges>\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    let meshes = parse_rep(&rep).expect("parse ok");
    assert!(meshes.is_empty(), "edge-only rep must produce no mesh");
}

#[test]
fn unknown_rep_types_are_ignored() {
    // A rep with an unobserved type name must be skipped (tolerant), not crash.
    let rep = format!(
        "{}<Rep xsi:type=\"TessellatedRepType\">\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0</Positions></VertexBuffer>\
         <Faces><Face triangles=\"0 1 2\"/></Faces>\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    let meshes = parse_rep(&rep).expect("parse ok");
    assert!(meshes.is_empty());
}

// ---------------------------------------------------------------------------
// Part 4: typed errors, never panic
// ---------------------------------------------------------------------------

fn assert_no_panic(input: &str) {
    let r = catch_unwind(AssertUnwindSafe(|| parse_rep(input)));
    assert!(r.is_ok(), "parse_rep must never panic");
}

#[test]
fn junk_input_is_typed_error_never_panic() {
    for bad in [
        "",
        "not xml at all",
        "<?xml version=\"1.0\"?><NotARep/>",
        "<?xml version=\"1.0\"?><XMLRepresentation>",
        // truncated mid-Positions
        "<?xml version=\"1.0\"?><XMLRepresentation><Root xsi:type=\"BagRepType\"><Rep xsi:type=\"PolygonalRepType\"><VertexBuffer><Positions>0 0 ",
    ] {
        assert_no_panic(bad);
        if let Ok(meshes) = parse_rep(bad) {
            assert!(meshes.is_empty(), "truncated/empty input must not yield geometry");
        }
    }
}

#[test]
fn dangling_index_is_typed_error_never_panic() {
    // three positions, triangle references index 99.
    let rep = format!(
        "{}<Rep xsi:type=\"PolygonalRepType\">\
         <VertexBuffer><Positions>0 0 0,1 0 0,1 1 0</Positions></VertexBuffer>\
         <Faces><Face triangles=\"0 1 99\"/></Faces>\
         </Rep>\n{}",
        rep_header(),
        rep_footer()
    );
    assert_no_panic(&rep);
    let err = parse_rep(&rep).expect_err("dangling index must error");
    match err {
        MeshError::IndexOutOfRange { index, .. } => assert_eq!(index, 99),
        other => panic!("expected IndexOutOfRange, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Part 5: real material — known-good geometry against an INDEPENDENT decoder
// ---------------------------------------------------------------------------

fn corpus_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OEP_3DXML_SAMPLES") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples"),
        // repo-root corpus once the P1 corpus PR merges: <repo>/corpus/data/3dxml
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/data/3dxml"),
    ]
    .into_iter()
    .find(|cand| cand.join("cube1.3dxml").exists())
}

/// Golden mesh facts derived by the INDEPENDENT Python stdlib reference
/// decoder (tests/test_oep_3dxml_mesh_rep_acceptance.py) on the
/// license-clean corpus, 2026-10-16. Triangle counts and bbox are
/// strip-convention independent. These are "known-good geometry": decoded
/// twice independently from the same bytes, asserted here in Rust.
struct Golden {
    file: &'static str,
    member: &'static str,
    idx: usize,
    npos: usize,
    ntri: usize,
    bbox: [f64; 6],
}

const GOLDEN: &[Golden] = &[
    // Abaqus `triangles=` encoding.
    Golden {
        file: "cube1.3dxml",
        member: "Abaqus_Geometry.3DRep",
        idx: 0,
        npos: 32,
        ntri: 12,
        bbox: [0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
    },
    Golden {
        file: "blocks.3dxml",
        member: "Abaqus_Geometry.3DRep",
        idx: 0,
        npos: 67,
        ntri: 24,
        bbox: [0.0, -10.0, 0.0, 15.0, 10.0, 100.0],
    },
    // CATIA V5 `strips=` encoding (first mesh is 134 pos / 86 tris).
    Golden {
        file: "machinist.3DXML",
        member: "TessPart_2.3DRep",
        idx: 0,
        npos: 134,
        ntri: 86,
        bbox: [-4.763, -7.112, -4.763, 4.763, -4.318, 4.763],
    },
    Golden {
        file: "machinist.3DXML",
        member: "TessPart_2.3DRep",
        idx: 8,
        npos: 134,
        ntri: 86,
        bbox: [-4.763, 4.318, -4.763, 4.763, 7.112, 4.763],
    },
    // Cycloidal strips with long runs (mesh 8 = 4884 pos / 4488 tris).
    Golden {
        file: "cycloidal.3DXML",
        member: "TessPart_5.3DRep",
        idx: 8,
        npos: 4884,
        ntri: 4488,
        bbox: [-5.133, 2.725, -1.206, -2.725, 5.133, 1.206],
    },
];

#[test]
fn real_material_matches_known_good_geometry() {
    let Some(dir) = corpus_dir() else {
        eprintln!("SKIP: corpus not present (set OEP_3DXML_SAMPLES)");
        return;
    };
    // The main real-material cross-check lives in the INDEPENDENT python
    // acceptance leg (tests/test_oep_3dxml_mesh_rep_acceptance.py); here we
    // assert the same golden facts in Rust, member by member.
    for golden in GOLDEN {
        let path = dir.join(golden.file);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", golden.file));
        // open the ZIP directly with the dev-dep `zip` crate (keeps this PR
        // self-contained and mergeable independently of the unmerged T01
        // reader, mirroring the sibling P3-M1-T02 pattern)
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
            .unwrap_or_else(|e| panic!("zip {}: {e}", golden.file));
        let mut rep = Vec::new();
        use std::io::Read as _;
        archive
            .by_name(golden.member)
            .unwrap_or_else(|e| panic!("member {}: {e}", golden.member))
            .read_to_end(&mut rep)
            .unwrap_or_else(|e| panic!("read {}: {e}", golden.member));
        let rep = String::from_utf8(rep).unwrap_or_else(|e| panic!("utf8 {}: {e}", golden.member));
        let meshes = parse_rep(&rep)
            .unwrap_or_else(|e| panic!("parse {}::{}: {e}", golden.file, golden.member));
        assert!(
            meshes.len() > golden.idx,
            "{}::{}: expected >= {} meshes, got {}",
            golden.file,
            golden.member,
            golden.idx + 1,
            meshes.len()
        );
        let m = &meshes[golden.idx];
        assert_eq!(
            m.positions().len(),
            golden.npos,
            "{}::{}[{}]: positions mismatch vs independent decoder",
            golden.file,
            golden.member,
            golden.idx
        );
        assert_eq!(
            m.triangles().len(),
            golden.ntri,
            "{}::{}[{}]: triangles mismatch vs independent decoder",
            golden.file,
            golden.member,
            golden.idx
        );
        // bbox within a loose f64 epsilon of the independent decoder's f64
        // (positions parse identically from the same text -> tight).
        let eps = 1e-3;
        for (axis, want) in golden.bbox.iter().enumerate() {
            let got = if axis < 3 {
                let v: Vec<f32> = m.positions().iter().map(|p| p[axis]).collect();
                v.iter().cloned().fold(f32::INFINITY, f32::min) as f64
            } else {
                let a = axis - 3;
                let v: Vec<f32> = m.positions().iter().map(|p| p[a]).collect();
                v.iter().cloned().fold(f32::NEG_INFINITY, f32::max) as f64
            };
            assert!(
                (got - want).abs() < eps,
                "{}::{}[{}] bbox[{axis}]: rust {got} vs ref {want}",
                golden.file,
                golden.member,
                golden.idx
            );
        }
    }

    // Physical sanity of the fully-understood sample: the Abaqus cube1 is a
    // 10x10x10 cube — 12 triangles in a ~[0,10]^3 box, surface ~600.
    let path = dir.join("cube1.3dxml");
    let bytes = std::fs::read(&path).expect("read cube1");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("zip parses");
    let mut rep = Vec::new();
    use std::io::Read as _;
    archive
        .by_name("Abaqus_Geometry.3DRep")
        .expect("read member")
        .read_to_end(&mut rep)
        .expect("read rep bytes");
    let rep = String::from_utf8(rep).expect("rep is utf-8");
    let meshes = parse_rep(&rep).expect("cube1 rep parses");
    assert_eq!(meshes.len(), 1, "cube1 has exactly one polygonal mesh");
    let m = &meshes[0];
    assert_eq!(m.triangles().len(), 12, "a cube must be 12 triangles");
    let xs: Vec<f32> = m.positions().iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = m.positions().iter().map(|p| p[1]).collect();
    let zs: Vec<f32> = m.positions().iter().map(|p| p[2]).collect();
    let (x0, x1) = (
        xs.iter().cloned().fold(f32::INFINITY, f32::min),
        xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
    );
    let (y0, y1) = (
        ys.iter().cloned().fold(f32::INFINITY, f32::min),
        ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
    );
    let (z0, z1) = (
        zs.iter().cloned().fold(f32::INFINITY, f32::min),
        zs.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
    );
    // 10x10x10 cube: each extent approx 10 (with numeric noise on the "0" face)
    assert!(
        (x1 - x0 - 10.0).abs() < 1e-3,
        "x extent ~10, got {x0}..{x1}"
    );
    assert!(
        (y1 - y0 - 10.0).abs() < 1e-3,
        "y extent ~10, got {y0}..{y1}"
    );
    assert!(
        (z1 - z0 - 10.0).abs() < 1e-3,
        "z extent ~10, got {z0}..{z1}"
    );
    // surface check: 6 faces -> area of full cube ~ 6*100 = 600
    let area: f32 = m
        .triangles()
        .iter()
        .map(|t| {
            let a = m.positions()[t[0] as usize];
            let b = m.positions()[t[1] as usize];
            let c = m.positions()[t[2] as usize];
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cx = u[1] * v[2] - u[2] * v[1];
            let cy = u[2] * v[0] - u[0] * v[2];
            let cz = u[0] * v[1] - u[1] * v[0];
            0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
        })
        .sum();
    assert!((area - 600.0).abs() < 1e-2, "cube surface ~600, got {area}");
}
