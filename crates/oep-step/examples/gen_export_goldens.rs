//! P4-M3-T01 golden generator: export the isolith unit box part and a small
//! assembly to STEP files.
//!
//! Used by `tests/test_step_export_freecad_acceptance.py` (independently from
//! pytest) so a real FreeCAD can re-import the exporter's output. The goldens
//! it writes are committed under `tests/golden/` for CI-stable verification;
//! the same documents are built in-line in `tests/export_contract.rs`.
//!
//! Usage:  cargo run -p oep-step --example gen_export_goldens -- <outdir>
//! (outdir defaults to `crates/oep-step/tests/golden` when omitted.)

use oep_geo::{
    Assembly, Body, BodyKind, BoundingBox, Document, Instance, InstanceSource, Part, Transform,
};
use oep_step::export::{write_document, BodyMesh, ExportOpts};
use std::path::PathBuf;

/// The isolith CC0-1.0 unit box (8 verts / 12 tris, bbox [0,0,0]..[1,1,1]).
fn box_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let positions = vec![
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 3, 1],
        [4, 5, 6],
        [5, 7, 6],
        [2, 6, 4],
        [2, 0, 6],
        [3, 5, 7],
        [3, 1, 5],
        [5, 1, 2],
        [5, 2, 4],
        [0, 7, 3],
        [0, 6, 7],
    ];
    (positions, triangles)
}

fn box_document() -> Document {
    let mut doc = Document::new("mm");
    let body = Body::new("body-1", "box-body", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let part = Part::new("part-1", "box").with_body("body-1");
    doc.push_body(body);
    doc.push_part(part);
    doc
}

fn assembly_document() -> Document {
    let mut doc = Document::new("mm");
    let body = Body::new("plate-body", "plate-body", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let part = Part::new("part-plate", "plate").with_body("plate-body");
    let i1 = Instance::new(
        "inst-1",
        "plate-1",
        InstanceSource::Part {
            id: "part-plate".into(),
        },
    )
    .with_transform(Transform::translation(0.0, 0.0, 0.0));
    let i2 = Instance::new(
        "inst-2",
        "plate-2",
        InstanceSource::Part {
            id: "part-plate".into(),
        },
    )
    .with_transform(Transform::translation(0.0, 0.0, 20.0));
    let asm = Assembly::new("assembly-1", "assembly-1")
        .with_instance(i1)
        .with_instance(i2);
    doc.push_body(body);
    doc.push_part(part);
    doc.push_assembly(asm);
    doc
}

fn opts(file_name: &str) -> ExportOpts {
    ExportOpts {
        file_name: file_name.to_string(),
        product_name: "isolith-export".to_string(),
        version: "0.1.0".to_string(),
        org: "Isolith OEP".to_string(),
        system: "oep-step".to_string(),
    }
}

fn main() {
    let out_dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests/golden"));

    let (positions, triangles) = box_mesh();

    let box_doc = box_document();
    let box_meshes = vec![BodyMesh {
        body_id: "body-1",
        positions: &positions,
        triangles: &triangles,
    }];
    let bytes = write_document(&box_doc, &box_meshes, &opts("box-part.step")).expect("box export");
    std::fs::write(out_dir.join("box-part.step"), &bytes).expect("write box golden");

    let asm_doc = assembly_document();
    let asm_meshes = vec![BodyMesh {
        body_id: "plate-body",
        positions: &positions,
        triangles: &triangles,
    }];
    let bytes =
        write_document(&asm_doc, &asm_meshes, &opts("assembly.step")).expect("assembly export");
    std::fs::write(out_dir.join("assembly.step"), &bytes).expect("write assembly golden");

    println!("goldens written to {}", out_dir.display());
}
