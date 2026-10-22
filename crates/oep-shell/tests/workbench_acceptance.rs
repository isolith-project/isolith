//! P7-M2-T01 acceptance (TDD: contract written first) - OEP interchange
//! workbench loads a real latched CGR mesh and a 3DXML through the workbench
//! command surface, and the PDM save path is stubbed against the P8 API
//! contract.
//!
//! Contract locked here:
//!
//! 1. `Workbench::run` never panics on any input (junk bytes, unknown
//!    formats, missing PDM items).
//! 2. `Load(Cgr)` on a modern `V5_CFV2` container returns `Loaded` whose
//!    canonical document carries the committed real-material counts
//!    (records=18, index=2003, vertex=1037, triangle=1389, max_index=1990)
//!    on the import feature - the cross-checked numeric-epsilon golden from
//!    the P2-M7-T01 latch, now flowing through the workbench surface.
//! 3. `Load(ThreeDXml)` on the real cube1 3DXML returns a canonical document
//!    matching the committed importer-hub golden structure (2 parts, 1 solid
//!    body, 1 assembly/instance). The static leg re-parses the committed
//!    golden so it always runs in CI (no private corpus).
//! 4. `Load(Step)` on a real STEP part (committed box-reference.step)
//!    returns a canonical document; the reserved seam is closed.
//! 5. `SaveToPdm` publishes the current document through the PDM API
//!    contract (oep-pdm stub) and `OpenFromPdm` restores it - a real
//!    round trip, never fabricated.
//! 6. The CGR/3DXML/STEP save buttons answer a typed, honest
//!    `InterchangeSavePending` naming the owning writer lane (writers are
//!    P3-M4 / P6-M4 deliverables, not delivered here).

use oep_import::{adapters, Document, ImportOutcome, SourceKind};
use oep_shell::workbench::{Interchange, Workbench, WorkbenchAction};
use oep_shell::WorkbenchOutcome as O;

fn sample_dirs(env_name: &str) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var(env_name) {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(std::path::PathBuf::from(chunk));
            }
        }
    }
    dirs
}

fn find_sample(env_name: &str, name: &str) -> Option<std::path::PathBuf> {
    for d in sample_dirs(env_name) {
        let full = d.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

fn is_canonical(doc: &Document) {
    assert_eq!(doc.format(), "oep-geo");
    // Derive the current schema version from the canonical type itself (the
    // shell consumes oep-geo via oep-import's re-export, so no direct oep_geo
    // dependency path is available here).
    assert_eq!(doc.version(), Document::new("mm").version());
    assert!(doc.validate().is_empty(), "document must validate cleanly");
}

fn tetra_document() -> Document {
    let mut m = adapters::cgr::CgrMesh::new();
    for v in [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        [0.0, 0.0, 10.0],
    ] {
        let _ = m.push_vertex(v);
    }
    for t in [[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]] {
        let _ = m.push_triangle(t[0], t[1], t[2]);
    }
    adapters::cgr::from_mesh(&m)
}

// ---------------------------------------------------------------------------
// 1. Tolerant, non-panicking surface
// ---------------------------------------------------------------------------

#[test]
fn workbench_never_panics_on_junk_and_unknown() {
    let mut wb = Workbench::with_local_vault();
    let junk = vec![0xABu8; 4096];
    for format in [Interchange::Cgr, Interchange::ThreeDXml, Interchange::Step] {
        let out = wb.run(WorkbenchAction::Load {
            format,
            bytes: junk.clone(),
        });
        assert!(
            matches!(out, O::Skipped { .. } | O::Failed { .. }),
            "junk {format:?} must be a typed skip/failure, got {out:?}"
        );
    }
    // Missing PDM item: typed failure, not a panic.
    let out = wb.run(WorkbenchAction::OpenFromPdm {
        item_id: "no-such-item".to_string(),
        revision: "A".to_string(),
    });
    assert!(matches!(out, O::Failed { .. }));
}

// ---------------------------------------------------------------------------
// 2. Real latched modern CGR mesh through the workbench surface
// ---------------------------------------------------------------------------

#[test]
fn workbench_loads_real_latched_modern_cgr_mesh_with_golden_counts() {
    let Some(path) = find_sample("OEP_CGR_SAMPLES", "nist_stc_06_asme1_V56R2023.CATPart") else {
        eprintln!("SKIP: no modern NIST sample supplied (set OEP_CGR_SAMPLES)");
        return;
    };
    let bytes = std::fs::read(&path).expect("readable");
    let mut wb = Workbench::with_local_vault();
    let out = wb.run(WorkbenchAction::Load {
        format: Interchange::Cgr,
        bytes,
    });
    let O::Loaded {
        format,
        document,
        stats,
        note,
    } = out
    else {
        panic!("modern CGR must load through the workbench, got {out:?}");
    };
    assert_eq!(format, "cgr");
    // Structural gate (not a strict parse round trip): serde_json reconstructs
    // the f32-widened bbox min z 1 ULP off (pre-existing oep-geo/serde
    // property, unrelated to this card); the real-material verification is the
    // counts below plus the importer-hub byte lock on the same material.
    assert_eq!(document.format(), "oep-geo");
    assert_eq!(document.version(), Document::new("mm").version());
    assert!(
        document.validate().is_empty(),
        "STC-06 document must validate: {:?}",
        document.validate()
    );
    assert_eq!(stats.parts, 1);
    assert_eq!(stats.bodies, 1);
    // The measured modern counts ride on the import feature (P2-M7-T01 latch).
    let feat = &document.features[0];
    let param = |k: &str| feat.parameters.get(k).and_then(|v| v.as_u64());
    assert_eq!(param("records"), Some(18));
    assert_eq!(param("triangle_count"), Some(1389));
    assert_eq!(param("index_count"), Some(2003));
    assert_eq!(param("vertex_count"), Some(1037));
    assert_eq!(param("max_index"), Some(1990));
    assert!(note.is_some(), "modern load should carry a measured note");
    assert!(
        wb.journal().iter().any(|l| l.starts_with("loaded cgr")),
        "journal records the load: {}",
        wb.journal().join(" | ")
    );
}

#[test]
fn workbench_loads_committed_modern_cgr_golden_structurally() {
    // Always runs: the committed golden is the canonical Document the real
    // STC-06 material produced. Independent structural re-parse.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("oep-import")
        .join("tests/golden/cgr-modern-stc06-import.oepgeo.json");
    let text = std::fs::read_to_string(&dir).expect("committed golden");
    let doc = Document::from_json_pretty(&text).expect("golden reparses");
    is_canonical(&doc);
    assert_eq!(doc.parts.len(), 1);
    assert_eq!(doc.bodies.len(), 1);
    let feat = &doc.features[0];
    let param = |k: &str| feat.parameters.get(k).and_then(|v| v.as_u64());
    assert_eq!(param("records"), Some(18));
    assert_eq!(param("triangle_count"), Some(1389));
    assert_eq!(param("index_count"), Some(2003));
    assert_eq!(param("vertex_count"), Some(1037));
    assert_eq!(param("max_index"), Some(1990));
}

// ---------------------------------------------------------------------------
// 3. 3DXML through the workbench surface
// ---------------------------------------------------------------------------

#[test]
fn workbench_loads_real_cube1_3dxml_from_corpus() {
    let Some(path) = find_sample("OEP_3DXML_SAMPLES", "cube1.3dxml") else {
        eprintln!("SKIP: no cube1.3dxml (set OEP_3DXML_SAMPLES)");
        return;
    };
    let bytes = std::fs::read(&path).expect("readable");
    let mut wb = Workbench::with_local_vault();
    let out = wb.run(WorkbenchAction::Load {
        format: Interchange::ThreeDXml,
        bytes,
    });
    let O::Loaded {
        document, stats, ..
    } = out
    else {
        panic!("cube1 must import, got {out:?}");
    };
    is_canonical(&document);
    assert_eq!(stats.parts, 2, "cube1 has two Reference3D parts");
    assert_eq!(stats.bodies, 1, "cube1 has one mesh body");
    assert_eq!(stats.assemblies, 1, "cube1 has one product structure");
}

#[test]
fn workbench_3dxml_static_committed_golden_is_canonical() {
    // Always runs (no corpus): the committed importer-hub golden.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("oep-import")
        .join("tests/golden/cube1-import.oepgeo.json");
    let text = std::fs::read_to_string(&dir).expect("committed golden");
    let doc = Document::from_json_pretty(&text).expect("golden reparses");
    is_canonical(&doc);
    assert_eq!(doc.parts.len(), 2);
    assert_eq!(doc.bodies.len(), 1);
    assert_eq!(doc.assemblies.len(), 1);
    assert_eq!(doc.assemblies[0].instances.len(), 1);
    let body = &doc.bodies[0];
    let bb = body.bbox.expect("solid body bbox");
    assert!((bb.max[0] - 10.0).abs() < 1e-3, "cube width ~10: {bb:?}");
}

// ---------------------------------------------------------------------------
// 4. STEP through the workbench surface (reserved seam closed)
// ---------------------------------------------------------------------------

fn box_reference_step() -> Vec<u8> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("oep-step")
        .join("tests/golden/box-reference.step");
    std::fs::read(&p).expect("committed STEP fixture")
}

#[test]
fn workbench_loads_real_step_part_from_committed_fixture() {
    let bytes = box_reference_step();
    let mut wb = Workbench::with_local_vault();
    let out = wb.run(WorkbenchAction::Load {
        format: Interchange::Step,
        bytes,
    });
    let O::Loaded {
        document, stats, ..
    } = out
    else {
        panic!("box-reference.step must import (seam closed), got {out:?}");
    };
    is_canonical(&document);
    assert!(stats.parts >= 1, "at least one STEP part");
    assert!(stats.bodies >= 1, "at least one mapped shell body");
    let feat = &document.features[0];
    let tri = feat
        .parameters
        .get("triangle_count")
        .and_then(|v| v.as_f64());
    assert_eq!(tri, Some(12.0), "box-reference is a 12-triangle unit box");
}

#[test]
fn workbench_loads_committed_step_golden_structurally() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("oep-import")
        .join("tests/golden/box-reference-import.oepgeo.json");
    let text = std::fs::read_to_string(&dir).expect("committed golden");
    let doc = Document::from_json_pretty(&text).expect("golden reparses");
    is_canonical(&doc);
    assert_eq!(doc.parts.len(), 1);
    assert_eq!(doc.bodies.len(), 1);
    let bb = doc.bodies[0].bbox.expect("bbox");
    assert_eq!(bb.min, [0.0, 0.0, 0.0]);
    assert_eq!(bb.max, [1.0, 1.0, 1.0]);
}

// ---------------------------------------------------------------------------
// 5. PDM save path stubbed against the P8 API contract (round trip)
// ---------------------------------------------------------------------------

#[test]
fn workbench_pdm_save_and_open_round_trip() {
    let mut wb = Workbench::with_local_vault();
    let original = tetra_document();
    wb.attach_document(original.clone(), "tetra");

    let out = wb.run(WorkbenchAction::SaveToPdm {
        item_id: "TEST-01".to_string(),
        revision: "A".to_string(),
        summary: "initial publish".to_string(),
    });
    let O::SavedToPdm { revision } = out else {
        panic!("PDM save must succeed against the stub contract, got {out:?}");
    };
    assert_eq!(revision.item_id, "TEST-01");
    assert_eq!(revision.revision, "A");
    assert!(revision.blob_digest.is_some(), "content-addressed blob");

    // Saving the same content under a second revision is still one blob (equal
    // content addresses equal), which is the stub's honest dedup.
    let out = wb.run(WorkbenchAction::SaveToPdm {
        item_id: "TEST-01".to_string(),
        revision: "B".to_string(),
        summary: String::new(),
    });
    let O::SavedToPdm { revision: b } = out else {
        panic!("second PDM save must succeed, got {out:?}");
    };
    assert_eq!(
        b.blob_digest, revision.blob_digest,
        "content-addressed dedup"
    );

    // Open restores the exact canonical document.
    let out = wb.run(WorkbenchAction::OpenFromPdm {
        item_id: "TEST-01".to_string(),
        revision: "A".to_string(),
    });
    let O::OpenedFromPdm { document, .. } = out else {
        panic!("PDM open must restore the document, got {out:?}");
    };
    is_canonical(&document);
    assert_eq!(document, original, "round trip is identical");
}

#[test]
fn workbench_pdm_save_requires_a_loaded_document() {
    let mut wb = Workbench::with_local_vault();
    let out = wb.run(WorkbenchAction::SaveToPdm {
        item_id: "X".to_string(),
        revision: "A".to_string(),
        summary: String::new(),
    });
    assert!(
        matches!(out, O::Failed { .. }),
        "save with no loaded document must fail, got {out:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. Interchange save buttons are honest pending (not fabricated)
// ---------------------------------------------------------------------------

#[test]
fn workbench_interchange_save_buttons_are_honest_pending() {
    let mut wb = Workbench::with_local_vault();
    wb.attach_document(tetra_document(), "tetra");
    for format in [Interchange::Cgr, Interchange::ThreeDXml, Interchange::Step] {
        let out = wb.run(WorkbenchAction::Save { format });
        let O::InterchangeSavePending {
            format: tag,
            reason,
        } = out
        else {
            panic!("{format:?} save must be typed pending, got {out:?}");
        };
        assert_eq!(tag, format.tag());
        assert!(!reason.is_empty(), "reason names the owning lane");
    }
}

// ---------------------------------------------------------------------------
// Adapter-level check: the STEP seam is closed at the hub, not just the shell
// ---------------------------------------------------------------------------

#[test]
fn hub_step_adapter_is_real_not_reserved() {
    let outcome =
        oep_import::import_bytes(SourceKind::Step, &box_reference_step()).expect("no panic");
    assert!(
        matches!(outcome, ImportOutcome::Imported { .. }),
        "STEP adapter must produce Imported, got {outcome:?}"
    );
}
