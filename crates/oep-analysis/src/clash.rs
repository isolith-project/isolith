//! Assembly interference / clash detection across the placed instances
//! (the resolved mates graph).
//!
//! The assembly workbench ([`crate`] sibling `oep-assem`) solves a mate set
//! into instance placements and writes them back into the canonical
//! `oep-geo` document. This module walks the placed instances of an assembly
//! (recursively through nested sub-assemblies), composes their transforms to
//! world space, and reports every pair of solid bodies whose world volumes
//! **penetrate** — positive overlap volume on all three axes. For each pair
//! it reports the two instance chains, the overlap axis-aligned box, the
//! penetration location (the overlap box centroid), and the overlap volume in
//! the document's length unit (mm³ when the document is in mm).
//!
//! Surface-touching pairs (zero-volume contact) are NOT clashes: a positive
//! overlap volume on every axis is required, so perfectly coincident faces
//! never count as interference.

use oep_geo::math::{Mat4, Vec3};
use oep_geo::{BodyKind, BoundingBox, Document, InstanceSource, Transform};

/// Error type for the clash check. Total: never panics.
#[derive(Debug, Clone, PartialEq)]
pub enum ClashError {
    /// The assembly id does not exist in the document.
    AssemblyNotFound(String),
    /// An instance references a part/body that is missing or not a solid.
    MissingBody { instance: String, body: String },
    /// A nested assembly instance references an assembly that is missing
    /// (broken link).
    MissingAssembly { instance: String, assembly: String },
    /// An instance carries a non-finite transform (invalid geometry).
    NonFiniteTransform(String),
}

/// One reported penetrating pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    /// Instance chain that placed body A (innermost first).
    pub chain_a: Vec<String>,
    /// Instance chain that placed body B (innermost first).
    pub chain_b: Vec<String>,
    /// Part ids of the penetrating pair.
    pub part_a: String,
    pub part_b: String,
    /// Body ids of the penetrating pair.
    pub body_a: String,
    pub body_b: String,
    /// Overlap box in world coordinates (positive volume on all axes).
    pub overlap: BoundingBox,
    /// Penetration location: the centroid of the overlap box.
    pub location: [f64; 3],
    /// Overlap volume in cubic document-units (mm³ for a mm document).
    pub volume: f64,
}

/// A body placed in world space by a chain of instances.
#[derive(Debug, Clone)]
struct WorldBody {
    chain: Vec<String>,
    part_id: String,
    body_id: String,
    /// World-space AABB of the body's bounding box corners.
    world: BoundingBox,
}

fn transform_to_mat4(t: &Transform) -> Result<Mat4, ClashError> {
    if !t.components().iter().all(|x| x.is_finite()) {
        return Err(ClashError::NonFiniteTransform(format!(
            "{:?}",
            t.components()
        )));
    }
    Mat4::from_array16(*t.components()).map_err(|_| {
        ClashError::NonFiniteTransform("transform had a non-finite component".to_string())
    })
}

fn body_world_aabb(world: &Mat4, bbox: &BoundingBox) -> BoundingBox {
    let corners = [
        Vec3::new(bbox.min[0], bbox.min[1], bbox.min[2]),
        Vec3::new(bbox.max[0], bbox.min[1], bbox.min[2]),
        Vec3::new(bbox.max[0], bbox.max[1], bbox.min[2]),
        Vec3::new(bbox.min[0], bbox.max[1], bbox.min[2]),
        Vec3::new(bbox.min[0], bbox.min[1], bbox.max[2]),
        Vec3::new(bbox.max[0], bbox.min[1], bbox.max[2]),
        Vec3::new(bbox.max[0], bbox.max[1], bbox.max[2]),
        Vec3::new(bbox.min[0], bbox.max[1], bbox.max[2]),
    ];
    let mut mn = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut mx = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in corners.iter() {
        let w = world.transform_point(c);
        mn = Vec3::new(mn.x.min(w.x), mn.y.min(w.y), mn.z.min(w.z));
        mx = Vec3::new(mx.x.max(w.x), mx.y.max(w.y), mx.z.max(w.z));
    }
    BoundingBox::new([mn.x, mn.y, mn.z], [mx.x, mx.y, mx.z])
}

/// Walk the instances of one assembly depth-first, emitting the world-space
/// AABB of every solid body placed under it.
fn walk_assembly(
    doc: &Document,
    assembly: &oep_geo::Assembly,
    parent: &Mat4,
    chain: &mut Vec<String>,
    out: &mut Vec<WorldBody>,
) -> Result<(), ClashError> {
    for inst in &assembly.instances {
        let local = transform_to_mat4(&inst.transform)?;
        let world = parent.mul(&local);
        match &inst.source {
            InstanceSource::Part { id } => {
                let part = doc.parts.iter().find(|p| &p.id == id);
                let Some(part) = part else { continue };
                chain.push(inst.id.clone());
                for bid in &part.body_ids {
                    let Some(body) = doc.bodies.iter().find(|b| &b.id == bid) else {
                        return Err(ClashError::MissingBody {
                            instance: inst.id.clone(),
                            body: bid.clone(),
                        });
                    };
                    // Only volumetric solids can clash.
                    if body.kind != BodyKind::Solid {
                        continue;
                    }
                    let Some(bbox) = body.bbox else { continue };
                    out.push(WorldBody {
                        chain: chain.clone(),
                        part_id: id.clone(),
                        body_id: bid.clone(),
                        world: body_world_aabb(&world, &bbox),
                    });
                }
                chain.pop();
            }
            InstanceSource::Assembly { id } => {
                chain.push(inst.id.clone());
                let Some(sub) = doc.assemblies.iter().find(|a| &a.id == id) else {
                    return Err(ClashError::MissingAssembly {
                        instance: inst.id.clone(),
                        assembly: id.clone(),
                    });
                };
                walk_assembly(doc, sub, &world, chain, out)?;
                chain.pop();
            }
        }
    }
    Ok(())
}

/// Detect all penetrating pairs in an assembly.
///
/// Walks `assembly_id` depth-first (recursing through sub-assemblies),
/// computes each solid body's world AABB, and reports every body pair whose
/// overlap has positive volume on all three axes. Deterministic order: pairs
/// are returned sorted by the primary chain's instance ids, then the
/// secondary chain's.
pub fn detect_in_assembly(doc: &Document, assembly_id: &str) -> Result<Vec<Clash>, ClashError> {
    let assembly = doc
        .assemblies
        .iter()
        .find(|a| a.id == assembly_id)
        .ok_or_else(|| ClashError::AssemblyNotFound(assembly_id.to_string()))?;

    let mut bodies: Vec<WorldBody> = Vec::new();
    let mut chain: Vec<String> = Vec::new();
    walk_assembly(doc, assembly, &Mat4::identity(), &mut chain, &mut bodies)?;

    let mut clashes: Vec<Clash> = Vec::new();
    for i in 0..bodies.len() {
        for j in (i + 1)..bodies.len() {
            let a = &bodies[i];
            let b = &bodies[j];
            let Some(overlap) = overlap_box(&a.world, &b.world) else {
                continue;
            };
            let extents = [
                overlap.max[0] - overlap.min[0],
                overlap.max[1] - overlap.min[1],
                overlap.max[2] - overlap.min[2],
            ];
            // Zero-volume (surface contact) is not interference.
            if extents.iter().all(|e| *e > 0.0) {
                let volume = extents[0] * extents[1] * extents[2];
                clashes.push(Clash {
                    chain_a: a.chain.clone(),
                    chain_b: b.chain.clone(),
                    part_a: a.part_id.clone(),
                    part_b: b.part_id.clone(),
                    body_a: a.body_id.clone(),
                    body_b: b.body_id.clone(),
                    overlap,
                    location: [
                        (overlap.min[0] + overlap.max[0]) / 2.0,
                        (overlap.min[1] + overlap.max[1]) / 2.0,
                        (overlap.min[2] + overlap.max[2]) / 2.0,
                    ],
                    volume,
                });
            }
        }
    }

    // Deterministic ordering: sort by (chain_a lexicographic, chain_b).
    clashes.sort_by(|x, y| {
        x.chain_a
            .cmp(&y.chain_a)
            .then_with(|| x.chain_b.cmp(&y.chain_b))
            .then_with(|| x.body_a.cmp(&y.body_a))
            .then_with(|| x.body_b.cmp(&y.body_b))
    });
    Ok(clashes)
}

/// The intersection box of two axis-aligned boxes, or `None` when they do not
/// overlap with positive extent on every axis.
fn overlap_box(a: &BoundingBox, b: &BoundingBox) -> Option<BoundingBox> {
    let min = [
        a.min[0].max(b.min[0]),
        a.min[1].max(b.min[1]),
        a.min[2].max(b.min[2]),
    ];
    let max = [
        a.max[0].min(b.max[0]),
        a.max[1].min(b.max[1]),
        a.max[2].min(b.max[2]),
    ];
    if max[0] <= min[0] || max[1] <= min[1] || max[2] <= min[2] {
        return None;
    }
    Some(BoundingBox::new(min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mass::mass_tolerance;
    use oep_geo::{Assembly, Body, Part};

    fn box_doc(assembly_name: &str) -> Document {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p-a", "box-a").with_body("b-a"));
        doc.push_body(
            Body::new("b-a", "box-solid-a", BodyKind::Solid)
                .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])),
        );
        doc.push_assembly(
            Assembly::new("asm", assembly_name)
                .with_instance(
                    oep_geo::Instance::new(
                        "i-a",
                        "box-a",
                        InstanceSource::Part { id: "p-a".into() },
                    )
                    .with_transform(oep_geo::Transform::translation(0.0, 0.0, 0.0)),
                )
                .with_instance(
                    oep_geo::Instance::new(
                        "i-b",
                        "box-b-overlap",
                        InstanceSource::Part { id: "p-a".into() },
                    )
                    .with_transform(oep_geo::Transform::translation(5.0, 5.0, 5.0)),
                ),
        );
        doc
    }

    #[test]
    fn penetrating_pair_is_flagged() {
        let doc = box_doc("asm-clash");
        let clashes = detect_in_assembly(&doc, "asm").unwrap();
        assert_eq!(clashes.len(), 1);
        let c = &clashes[0];
        assert_eq!(c.body_a, "b-a");
        assert_eq!(c.body_b, "b-a");
        // Overlap box is [5,10] on every axis → volume 125 mm³, location (7.5,7.5,7.5).
        assert!(mass_tolerance().approx_eq(c.volume, 125.0));
        assert!(mass_tolerance().points_eq(c.location, [7.5, 7.5, 7.5]));
    }

    #[test]
    fn touching_pair_is_not_a_clash() {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p-a", "box-a").with_body("b-a"));
        doc.push_body(
            Body::new("b-a", "box-solid-a", BodyKind::Solid)
                .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])),
        );
        doc.push_assembly(
            Assembly::new("asm", "asm-touch")
                .with_instance(
                    oep_geo::Instance::new(
                        "i-a",
                        "box-a",
                        InstanceSource::Part { id: "p-a".into() },
                    )
                    .with_transform(oep_geo::Transform::translation(0.0, 0.0, 0.0)),
                )
                .with_instance(
                    oep_geo::Instance::new(
                        "i-b",
                        "box-b-touching",
                        InstanceSource::Part { id: "p-a".into() },
                    )
                    .with_transform(oep_geo::Transform::translation(10.0, 0.0, 0.0)),
                ),
        );
        let clashes = detect_in_assembly(&doc, "asm").unwrap();
        assert!(
            clashes.is_empty(),
            "surface contact is not a clash: {:?}",
            clashes
        );
    }

    #[test]
    fn missing_assembly_yields_error() {
        let doc = Document::new("mm");
        let err = detect_in_assembly(&doc, "nope");
        assert!(matches!(err, Err(ClashError::AssemblyNotFound(_))));
    }
}
