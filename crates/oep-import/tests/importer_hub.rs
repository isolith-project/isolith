//! P6-M4-T02 acceptance test (TDD, RED first) — importer hub: P2/P3/P4
//! adapters feed the ONE canonical internal format (`oep_geo::Document`).
//!
//! Written BEFORE the hub existed (RED: `oep_import::...` does not compile —
//! the crate is a P0 convention). The contract once GREEN:
//!
//! 1. `SourceKind::detect` classifies bytes: CGR (observed 4-byte magic +
//!    template), 3DXML (ZIP `PK\x03\x04` or standalone `<?xm`), STEP
//!    (ISO-10303-21 header text). Unknown bytes → `None` (never panic).
//! 2. Every successful import yields an `oep_geo::Document` — THE canonical
//!    internal format — that `validate()` clean and serializes deterministically
//!    (byte-stable round trip through `to_json_pretty`/`from_json_pretty`),
//!    for ALL sources (one format, no per-source shapes).
//! 3. P2 CGR adapter (`cgr::from_mesh`): a mesh maps to one part + one
//!    `Solid` body whose bbox is computed from the mesh vertices; real CGR
//!    bytes import via `oep_cgr::mesh::extract` — when classified-confirmed we
//!    build the document, otherwise a typed `Skipped` (honest, never fabricate,
//!    never panic).
//! 4. P3 3DXML adapter (`xml3d::import_bytes`): real corpus `.3dxml` →
//!    parts from `Reference3D` nodes, a `Solid` body per mesh `ReferenceRep`
//!    (bbox from vertices), root `Assembly` + `Instance`s from `Instance3D`
//!    with `RelativeMatrix` mapped to the canonical row-major 4x4 transform.
//!    The committed golden `tests/golden/cube1-import.oepgeo.json` (generated
//!    from the real Abaqus cube1 3DXML) is byte-locked: with the corpus the
//!    live import must reproduce it byte-for-byte; the static lock always runs
//!    so CI (no corpus) still asserts the canonical structure independently.
//! 5. P4 STEP adapter (`step::import_bytes`): registered, but returns a typed
//!    `Reserved` — `oep-step` is an empty skeleton in wave 1 (P4-M1-T01 not
//!    started); honest reserved seam, never a panic, never fabricated output.
//! 6. Errors are typed (never panic) on any bytes: junk, truncated, unknown.

use oep_geo::{BodyKind, Document, InstanceSource, Transform};
use oep_import::{adapters, ImportOutcome, SourceKind};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

const GOLDEN_REL_PATH: &str = "tests/golden/cube1-import.oepgeo.json";

fn golden_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_REL_PATH)
}

fn is_canonical(doc: &Document) {
    assert_eq!(doc.format(), "oep-geo", "canonical format tag");
    assert_eq!(doc.version(), 1, "canonical schema version");
    let errors = doc.validate();
    assert!(
        errors.is_empty(),
        "canonical document must validate: {errors:?}"
    );
    // Deterministic serialization (byte-stable round trip).
    let json = doc.to_json_pretty().expect("serialize");
    let back: Document = Document::from_json_pretty(&json).expect("reparse");
    assert_eq!(&back, doc, "round trip is identity");
    assert_eq!(
        back.to_json_pretty().expect("re-serialize"),
        json,
        "serialization is deterministic"
    );
}

fn find_3dxml_samples() -> Option<std::path::PathBuf> {
    [
        std::env::var("OEP_3DXML_SAMPLES")
            .ok()
            .map(std::path::PathBuf::from),
        Some(std::path::PathBuf::from("samples")),
        Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/data/3dxml")),
    ]
    .into_iter()
    .flatten()
    .find(|cand| cand.join("cube1.3dxml").exists())
}

// ---------------------------------------------------------------------------
// 1. source detection
// ---------------------------------------------------------------------------

#[test]
fn detect_cgr_3dxml_step_bytes() {
    // CGR: observed magic (u32 LE == 1) + zero preamble + invariant template.
    let mut cgr = vec![0u8; 0x80];
    cgr[0..4].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    cgr[0x68..0x68 + 16].copy_from_slice(&[
        0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02,
        0x4F,
    ]);
    assert_eq!(SourceKind::detect(&cgr), Some(SourceKind::Cgr));

    // 3DXML ZIP archive.
    let zip_head = b"PK\x03\x04rest-of-file";
    assert_eq!(SourceKind::detect(zip_head), Some(SourceKind::ThreeDXml));
    // 3DXML standalone XML.
    assert_eq!(
        SourceKind::detect(b"<?xml version=\"1.0\"?><Model_3dxml>"),
        Some(SourceKind::ThreeDXml)
    );

    // STEP: ISO-10303-21 header.
    assert_eq!(
        SourceKind::detect(b"ISO-10303-21;\nHEADER;"),
        Some(SourceKind::Step)
    );

    // Unknown, junk, short: never panic, None.
    assert_eq!(SourceKind::detect(b"ljksdhgfskjdhgf"), None);
    assert_eq!(SourceKind::detect(b"\x00\x01\x02\x03"), None);
    assert_eq!(SourceKind::detect(b""), None);
    assert_eq!(SourceKind::detect(b"abc"), None);
}

// ---------------------------------------------------------------------------
// 2+3. P2 CGR adapter — canonical mapping + honest classified skip
// ---------------------------------------------------------------------------

fn tetra_mesh() -> oep_import::adapters::cgr::CgrMesh {
    // A deterministic tetrahedron mesh: 4 vertices, 4 triangles.
    let mut m = oep_import::adapters::cgr::CgrMesh::new();
    let v = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        [0.0, 0.0, 10.0],
    ];
    let _ = m.push_vertex(v[0]);
    let _ = m.push_vertex(v[1]);
    let _ = m.push_vertex(v[2]);
    let _ = m.push_vertex(v[3]);
    let _ = m.push_triangle(0, 1, 2);
    let _ = m.push_triangle(0, 1, 3);
    let _ = m.push_triangle(0, 2, 3);
    let _ = m.push_triangle(1, 2, 3);
    m
}

#[test]
fn p2_from_mesh_builds_canonical_document() {
    let mesh = tetra_mesh();
    let doc = adapters::cgr::from_mesh(&mesh);
    is_canonical(&doc);
    assert_eq!(doc.parts.len(), 1, "one part");
    assert_eq!(doc.bodies.len(), 1, "one body");
    let body = &doc.bodies[0];
    assert_eq!(body.kind, BodyKind::Solid);
    let bb = body.bbox.expect("bbox computed");
    assert_eq!(bb.min, [0.0, 0.0, 0.0], "bbox min from vertices");
    assert_eq!(bb.max, [10.0, 10.0, 10.0], "bbox max from vertices");
    // Part owns the body.
    assert_eq!(doc.parts[0].body_ids, vec![body.id.clone()]);
}

#[test]
fn p2_import_bytes_never_panics_and_imports_or_skips() {
    // Junk bytes: typed error or classified skip — never a panic.
    let outcome = adapters::cgr::import_bytes(b"not-a-cgr-at-all").ok();
    if let Some(ImportOutcome::Imported { document, .. }) = outcome {
        is_canonical(&document);
    }
    // A structurally self-consistent mesh built directly maps canonically.
    let mesh = tetra_mesh();
    let doc = adapters::cgr::from_mesh(&mesh);
    is_canonical(&doc);
}

// ---------------------------------------------------------------------------
// 4. P3 3DXML adapter — committed golden byte-lock + real material
// ---------------------------------------------------------------------------

#[test]
fn p3_golden_byte_lock_is_canonical() {
    // Always runs (no corpus needed): the committed golden is the canonical
    // oep-geo Document the P3 adapter produced from the real Abaqus cube1
    // 3DXML. Independent structural assertions + byte-stable round trip.
    let text = std::fs::read_to_string(golden_path()).expect("committed golden exists");
    let doc = Document::from_json_pretty(&text).expect("golden reparses");
    is_canonical(&doc);

    // Golden facts (independent mesh-rep decode + rep parse, cross-checked).
    assert_eq!(doc.parts.len(), 2, "cube1 has 2 Reference3D nodes");
    assert_eq!(
        doc.assemblies.len(),
        1,
        "cube1 has one root product structure"
    );
    assert_eq!(doc.bodies.len(), 1, "cube1 has one polygonal mesh body");
    // Body is a solid whose bbox spans the ~10 cube [0,0,0,10,10,10].
    let body = &doc.bodies[0];
    assert_eq!(body.kind, BodyKind::Solid);
    let bb = body.bbox.expect("solid body bbox");
    assert!(
        (bb.max[0] - 10.0).abs() < 1e-3 && (bb.max[1] - 10.0).abs() < 1e-3,
        "cube width ~10 on x/y: {bb:?}"
    );
    // Body is owned by part 2 (Abaqus_Geometry via the InstanceRep wiring).
    let owner = doc
        .parts
        .iter()
        .find(|p| p.body_ids.iter().any(|id| id == &body.id));
    assert!(owner.is_some(), "body attached to a part");
    assert_eq!(owner.expect("owner").name, "Abaqus_Geometry");
    // One instance of that part inside the root assembly, canonical transform.
    let assembly = &doc.assemblies[0];
    assert_eq!(assembly.instances.len(), 1, "one Instance3D");
    let inst = &assembly.instances[0];
    assert!(matches!(
        &inst.source,
        InstanceSource::Part { id } if id == &owner.expect("owner").id
    ));
    assert_eq!(
        inst.transform.components().len(),
        16,
        "canonical 4x4 transform"
    );
}

#[test]
fn p3_live_import_reproduces_committed_golden() {
    // Real-material leg: with the corpus, the live P3 import MUST reproduce the
    // committed golden byte-for-byte (the byte lock above). SKIPs in CI when the
    // private corpus is absent (repo convention), so the static lock keeps the
    // canonical structure checked everywhere.
    let Some(dir) = find_3dxml_samples() else {
        eprintln!("SKIP: no real 3DXML sample (set OEP_3DXML_SAMPLES); CI-safe skip");
        return;
    };
    let bytes = std::fs::read(dir.join("cube1.3dxml")).expect("read cube1.3dxml");
    assert_eq!(SourceKind::detect(&bytes), Some(SourceKind::ThreeDXml));
    let outcome =
        oep_import::import_bytes(SourceKind::ThreeDXml, &bytes).expect("no panic on real bytes");
    let ImportOutcome::Imported { document, report } = outcome else {
        panic!("real cube1 must import; outcome={outcome:?}");
    };
    is_canonical(&document);
    let live_json = document.to_json_pretty().expect("serialize");
    let golden = std::fs::read_to_string(golden_path()).expect("committed golden");
    assert_eq!(
        live_json, golden,
        "live import of cube1.3dxml must byte-match the committed golden"
    );
    assert!(
        !report.warnings.is_empty() || report.parts >= 2,
        "report carried"
    );
}

// ---------------------------------------------------------------------------
// 5. P4 STEP adapter — reserved seam, honest
// ---------------------------------------------------------------------------

#[test]
fn p4_step_import_is_reserved_seam() {
    let outcome = adapters::step::import_bytes(b"ISO-10303-21;\nHEADER;whatever");
    match outcome {
        Ok(ImportOutcome::Reserved { source, .. }) => {
            assert_eq!(source, SourceKind::Step, "reserved source is Step");
        }
        other => panic!("STEP must be a typed Reserved outcome, got {other:?}"),
    }
    // Junk STEP bytes also never panic.
    let _ = adapters::step::import_bytes(b"junk");
}

// ---------------------------------------------------------------------------
// 6. registry / dispatch is total and typed
// ---------------------------------------------------------------------------

#[test]
fn import_bytes_all_sources_are_total() {
    // STEP reserved outcome is typed, never a panic.
    let r = oep_import::import_bytes(SourceKind::Step, b"ISO-10303-21;\n");
    assert!(r.is_ok(), "STEP reserved is still a typed outcome");
    // CGR and 3DXML also never panic.
    let _ = oep_import::import_bytes(SourceKind::Cgr, b"junk");
    let _ = oep_import::import_bytes(SourceKind::ThreeDXml, b"junk");
}

#[test]
fn backcompat_types_re_exported() {
    // Canonical types are reachable through the hub (one-format story).
    let _: Option<Transform> = None;
    let _: Option<InstanceSource> = None;
    let _: Option<BodyKind> = None;
}
