//! P3 adapter — 3DXML → canonical [`Document`].
//!
//! Uses the 3DXML reader's clean-room surface: container branch detection +
//! outer ZIP + `Manifest.xml` BOM (`oep_3dxml::bom`), the `<Model_3dxml>`
//! product-structure parse (`oep_3dxml::rep`) and the tessellated-ASCII
//! `.3DRep` mesh reader (`oep_3dxml::mesh`). The adapter maps the read
//! structures onto the canonical `oep_geo::Document`:
//!
//! - every `Reference3D` node → a `Part` (`oep-part-<node-id>`);
//! - every `ReferenceRep` whose associated `.3DRep` member yields a mesh →
//!   a `Solid` `Body` (bbox from the mesh vertices), owned by the part whose
//!   node the matching `InstanceRep` aggregates back to;
//! - the `ProductStructure` root → a top-level `Assembly` with one `Instance`
//!   per root-level `Instance3D` (`RelativeMatrix` → the canonical row-major
//!   4x4 transform, translation in the bottom row);
//! - anything unreadable is a warning on the report, never a panic; a document
//!   with structure but unreadable geometry still imports (structure is real).

use crate::{ImportError, ImportOutcome, ImportReport, SourceKind};
use oep_3dxml::container::{read_member, ContainerKind};
use oep_geo::{
    Assembly, Body, BodyKind, BoundingBox, Document, Instance, InstanceSource, Part, Transform,
};

/// Parse a `<RelativeMatrix>` (12 doubles, row-major 4x3) into the canonical
/// row-major 4x4 [`Transform`] (16 doubles, translation in elements 12/13/14,
/// affine w=1, matching `oep_geo`/P6-M1-T01).
fn matrix_4x3_to_transform(v: &[f64]) -> Transform {
    let mut t = [0.0f64; 16];
    // Upper-left 3x3 (rows 0-2).
    for r in 0..3 {
        for c in 0..3 {
            t[r * 4 + c] = if r * 3 + c < v.len() {
                v[r * 3 + c]
            } else {
                0.0
            };
        }
    }
    // Bottom row: [tx, ty, tz, 1].
    t[12] = v.get(9).copied().unwrap_or(0.0);
    t[13] = v.get(10).copied().unwrap_or(0.0);
    t[14] = v.get(11).copied().unwrap_or(0.0);
    t[15] = 1.0;
    Transform(t)
}

/// Compute an exact axis-aligned bbox from f32 positions.
fn positions_bbox(positions: &[[f64; 3]]) -> BoundingBox {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in positions {
        for i in 0..3 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    if !min[0].is_finite() {
        return BoundingBox::new([0.0; 3], [0.0; 3]);
    }
    BoundingBox::new(min, max)
}

fn first_mesh_bbox(text: &str) -> Option<BoundingBox> {
    let meshes = oep_3dxml::mesh::parse_rep(text).ok()?;
    let m = meshes.first()?;
    Some(positions_bbox(
        &m.positions()
            .iter()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
            .collect::<Vec<_>>(),
    ))
}

/// Import a 3DXML container (ZIP archive or standalone) into the canonical
/// [`Document`].
pub fn import_bytes(bytes: &[u8]) -> Result<ImportOutcome, ImportError> {
    let mut report = ImportReport::new(SourceKind::ThreeDXml);

    let bom = match oep_3dxml::bom::parse(bytes) {
        Ok(b) => b,
        Err(e) => {
            return Ok(ImportOutcome::Skipped {
                source: SourceKind::ThreeDXml,
                reason: format!("3DXML container parse failed: {e:?}"),
            });
        }
    };

    // Locate the primary model document.
    let model_bytes: Vec<u8> = match bom.kind {
        ContainerKind::StandaloneXml => bytes.to_vec(),
        ContainerKind::ZipArchive => {
            let root_member = bom
                .manifest
                .as_ref()
                .map(|m| m.root.clone())
                .unwrap_or_else(|| "*.3dxml".to_string());
            match read_member(bytes, &root_member) {
                Ok(mb) => mb,
                Err(e) => {
                    return Ok(ImportOutcome::Skipped {
                        source: SourceKind::ThreeDXml,
                        reason: format!("could not read model document {root_member:?}: {e:?}"),
                    });
                }
            }
        }
        ContainerKind::Unknown => {
            return Ok(ImportOutcome::Skipped {
                source: SourceKind::ThreeDXml,
                reason: "not a recognized 3DXML container".to_string(),
            });
        }
    };

    let model: oep_3dxml::rep::ModelRep = match oep_3dxml::rep::parse_model(&model_bytes) {
        Ok(m) => m,
        Err(e) => {
            return Ok(ImportOutcome::Skipped {
                source: SourceKind::ThreeDXml,
                reason: format!("model document parse failed: {e:?}"),
            });
        }
    };

    let mut doc = Document::new("mm");

    // 1. Parts from Reference3D nodes.
    let root_id = model.structure.root_id.clone().unwrap_or_default();
    let root_name = model
        .structure
        .nodes
        .iter()
        .find(|n| n.id == root_id)
        .map(|n| n.name.clone())
        .unwrap_or_else(|| "assembly".to_string());
    let node_to_part: std::collections::HashMap<String, String> = model
        .structure
        .nodes
        .iter()
        .map(|n| {
            let pid = format!("oep-part-{}", n.id);
            (n.id.clone(), pid.clone())
        })
        .collect();
    for n in &model.structure.nodes {
        let pid = node_to_part.get(&n.id).expect("node in map");
        doc.push_part(Part::new(pid.clone(), n.name.clone()));
    }

    // 2. Bodies from zigReferenceRep meshes, owned via InstanceRep wiring.
    //    InstanceRep { is_aggregated_by: node_id, is_instance_of: rep_id }.
    let rep_to_node: std::collections::HashMap<String, String> = model
        .structure
        .instance_reps
        .iter()
        .filter_map(|ir| {
            ir.is_instance_of
                .as_ref()
                .zip(ir.is_aggregated_by.as_ref())
                .map(|(rep, node)| (rep.clone(), node.clone()))
        })
        .collect();
    for r in &model.structure.references {
        let Some(member) = r.member_name() else {
            report
                .warnings
                .push(format!("rep {} has no 3DXML member", r.id));
            continue;
        };
        let text = match read_member(bytes, &member) {
            Ok(mb) => String::from_utf8_lossy(&mb).into_owned(),
            Err(_) => {
                report
                    .warnings
                    .push(format!("rep {} member {member:?} unreadable", r.id));
                continue;
            }
        };
        let Some(bbox) = first_mesh_bbox(&text) else {
            report.warnings.push(format!(
                "rep {} member {member:?} has no polygonal mesh",
                r.id
            ));
            continue;
        };
        let body_id = format!("oep-body-{}", r.id);
        let body = Body::new(
            &body_id,
            r.name.as_deref().unwrap_or(&r.id),
            BodyKind::Solid,
        )
        .with_bbox(bbox);
        // Wire to the owning part when the InstanceRep graph is unambiguous.
        if let Some(node) = rep_to_node.get(&r.id) {
            if let Some(pid) = node_to_part.get(node) {
                if let Some(p) = doc.parts.iter_mut().find(|p| &p.id == pid) {
                    p.body_ids.push(body_id.clone());
                }
            }
        }
        doc.push_body(body);
    }

    // 3. Assembly from the product-structure root + root-level instances.
    let mut assembly = Assembly::new(format!("oep-assembly-{root_id}"), root_name);
    for inst in &model.structure.instances {
        let is_root_child = inst.is_aggregated_by.as_deref() == Some(root_id.as_str());
        let Some(child_node) = inst.is_instance_of.as_ref() else {
            continue;
        };
        let Some(pid) = node_to_part.get(child_node) else {
            continue;
        };
        let transform = inst
            .relative_matrix
            .as_deref()
            .map(matrix_4x3_to_transform)
            .unwrap_or_else(Transform::identity);
        let iid = format!("oep-inst-{}", inst.id);
        let inn = Instance::new(
            iid,
            inst.name.clone(),
            InstanceSource::Part { id: pid.clone() },
        )
        .with_transform(transform);
        if is_root_child {
            assembly = assembly.with_instance(inn);
        } else {
            report.warnings.push(format!(
                "instance {} aggregates to non-root node {:?}; kept detached",
                inst.id,
                inst.is_aggregated_by.as_deref().unwrap_or("?")
            ));
        }
    }
    doc.push_assembly(assembly);

    report.parts = doc.parts.len();
    report.bodies = doc.bodies.len();
    report.assemblies = doc.assemblies.len();
    report.instances = doc.assemblies.iter().map(|a| a.instances.len()).sum();
    Ok(ImportOutcome::Imported {
        document: doc,
        report,
    })
}
