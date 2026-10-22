//! Exploded-view generation: deterministic separation of an assembly along
//! its structure / mate hierarchy, driven by an interactive pull distance.
//!
//! The editor surface (oep-shell + oep-viewer) exposes a pull-distance
//! control; this module is the deterministic generator behind it. Given an
//! assembly, each child instance at hierarchy *depth* `d` is offset outwards
//! from the assembly reference point along the radial direction away from the
//! root, scaled by `pull_distance * d`. The offsets are a pure function of
//! (document, assembly, pull_distance): same inputs → same offsets (the
//! committed golden in `tests/golden/exploded-oepgeo.json` is byte-locked by
//! the acceptance test).
//!
//! The result is a new `oep-geo` `Document` whose instance transforms carry
//! the exploded placements, so the existing viewer/scene pipeline renders it
//! unchanged. Determinism is a hard property: iteration order is the stable
//! document order, and no non-deterministic collection is used.

use oep_geo::math::{Mat4, Vec3};
use oep_geo::{BoundingBox, Document, InstanceSource, Transform};

/// Error type for exploded-view generation. Total: never panics.
#[derive(Debug, Clone, PartialEq)]
pub enum ExplodeError {
    /// The assembly id does not exist in the document.
    AssemblyNotFound(String),
    /// A referenced sub-assembly is missing.
    MissingAssembly { instance: String, assembly: String },
    /// A body has no bounding box, so its centroid cannot be computed.
    NoBoundingBox(String),
    /// A non-finite pull distance was supplied.
    NonFinitePull(f64),
}

/// One exploded placement: an instance and its new transform.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplodedPlacement {
    /// Instance id.
    pub instance: String,
    /// Hierarchy depth (root children are depth 1).
    pub depth: usize,
    /// The outward unit direction in world space along which the instance was
    /// pulled.
    pub direction: [f64; 3],
    /// The applied offset vector in document units.
    pub offset: [f64; 3],
    /// The new world/augmented placement transform.
    pub transform: Transform,
}

/// A resolved instance pose: its world centroid and depth in the hierarchy.
struct Posed {
    instance: String,
    depth: usize,
    world_centroid: Vec3,
}

fn bbox_centroid(bbox: &BoundingBox) -> Vec3 {
    Vec3::new(
        (bbox.min[0] + bbox.max[0]) / 2.0,
        (bbox.min[1] + bbox.max[1]) / 2.0,
        (bbox.min[2] + bbox.max[2]) / 2.0,
    )
}

/// Deterministically explode an assembly at `pull_distance` (document units).
///
/// The reference point is the centroid of all solid-body world centroids in
/// the assembly (its "center of geometry"). Every instance is displaced
/// radially outward from that point by `pull_distance × depth` along the unit
/// direction from the reference to the instance's own world centroid. The
/// returned placements carry the full new local transform for each instance
/// (the placement transform is augmented by the applied world offset).
pub fn explode(
    doc: &Document,
    assembly_id: &str,
    pull_distance: f64,
) -> Result<Vec<ExplodedPlacement>, ExplodeError> {
    if !pull_distance.is_finite() {
        return Err(ExplodeError::NonFinitePull(pull_distance));
    }
    let assembly = doc
        .assemblies
        .iter()
        .find(|a| a.id == assembly_id)
        .ok_or_else(|| ExplodeError::AssemblyNotFound(assembly_id.to_string()))?;

    // Collect every solid body's world centroid, instance chain, and depth.
    let mut posed: Vec<Posed> = Vec::new();
    let mut reference = Vec3::new(0.0, 0.0, 0.0);
    let mut count = 0usize;
    collect_posed(
        doc,
        assembly,
        &Mat4::identity(),
        1,
        &mut posed,
        &mut reference,
        &mut count,
    )?;

    // Pull apart: for each posed instance compute the radial offset.
    let mut result: Vec<ExplodedPlacement> = Vec::new();
    for p in posed {
        let from_ref = p.world_centroid.sub(&reference);
        let dir = normalized_or_else(from_ref, Vec3::new(1.0, 0.0, 0.0));
        let magnitude = pull_distance * p.depth as f64;
        let offset = dir.mul(magnitude);
        // New world pose = old world pose + offset (pure translation), then
        // mapped back into the local frame is not required: the surface just
        // needs the augmented placement. We keep the original placement
        // transform and add the world offset as a post translation so the
        // existing viewer (which composes placement transforms) renders the
        // exploded scene directly.
        let base = doc
            .assemblies
            .iter()
            .find(|a| a.id == assembly_id)
            .and_then(|a| a.instances.iter().find(|i| i.id == p.instance))
            .map(|i| i.transform.clone())
            .unwrap_or_else(Transform::identity);
        let mut t = *base.components();
        t[12] += offset.x;
        t[13] += offset.y;
        t[14] += offset.z;
        result.push(ExplodedPlacement {
            instance: p.instance,
            depth: p.depth,
            direction: [dir.x, dir.y, dir.z],
            offset: [offset.x, offset.y, offset.z],
            transform: Transform(t),
        });
    }
    Ok(result)
}

/// Collect the world centroid of every solid body under an assembly,
/// recursively through sub-assemblies with the full world transform chain
/// composed, and accumulate the reference centroid.
fn collect_posed(
    doc: &Document,
    assembly: &oep_geo::Assembly,
    parent_world: &Mat4,
    depth: usize,
    out: &mut Vec<Posed>,
    reference: &mut Vec3,
    count: &mut usize,
) -> Result<(), ExplodeError> {
    for inst in &assembly.instances {
        let local = transform_to_mat4(&inst.transform)?;
        let world = parent_world.mul(&local);
        match &inst.source {
            InstanceSource::Part { id } => {
                let Some(part) = doc.parts.iter().find(|p| &p.id == id) else {
                    continue;
                };
                for bid in &part.body_ids {
                    let Some(body) = doc.bodies.iter().find(|b| &b.id == bid) else {
                        continue;
                    };
                    let Some(bbox) = body.bbox else { continue };
                    let world_centroid = world.transform_point(&bbox_centroid(&bbox));
                    *reference = reference.add(&world_centroid);
                    *count += 1;
                    out.push(Posed {
                        instance: inst.id.clone(),
                        depth,
                        world_centroid,
                    });
                }
            }
            InstanceSource::Assembly { id } => {
                let Some(sub) = doc.assemblies.iter().find(|a| &a.id == id) else {
                    return Err(ExplodeError::MissingAssembly {
                        instance: inst.id.clone(),
                        assembly: id.clone(),
                    });
                };
                collect_posed(doc, sub, &world, depth + 1, out, reference, count)?;
            }
        }
    }
    Ok(())
}

fn transform_to_mat4(t: &Transform) -> Result<Mat4, ExplodeError> {
    Mat4::from_array16(*t.components())
        .map_err(|_| ExplodeError::NonFinitePull(t.components().iter().cloned().sum::<f64>()))
}

/// Apply exploded placements back onto a document, producing a renderable
/// exploded assembly: the returned `Document` keeps the exploded instance
/// transforms so the existing viewer/scene pipeline draws the separated
/// assembly unchanged.
///
/// Deterministic: the output document is a pure function of the input
/// document and the placements.
pub fn apply_placements(
    doc: &Document,
    assembly_id: &str,
    placements: &[ExplodedPlacement],
) -> Result<Document, ExplodeError> {
    let assembly = doc
        .assemblies
        .iter()
        .find(|a| a.id == assembly_id)
        .ok_or_else(|| ExplodeError::AssemblyNotFound(assembly_id.to_string()))?;

    let assembly_idx = doc
        .assemblies
        .iter()
        .position(|a| a.id == assembly_id)
        .expect("found above");

    let mut out = doc.clone();
    for p in placements {
        let target = out
            .assemblies
            .get_mut(assembly_idx)
            .and_then(|a| a.instances.iter_mut().find(|i| i.id == p.instance))
            .ok_or_else(|| ExplodeError::AssemblyNotFound(p.instance.clone()))?;
        target.transform = p.transform.clone();
    }
    let _ = assembly; // used for the existence check above
    Ok(out)
}

/// Unit vector of `v`, or `fallback` when `v` has (near) zero norm.
fn normalized_or_else(v: Vec3, fallback: Vec3) -> Vec3 {
    let n = v.norm();
    if !n.is_finite() || n <= f64::MIN_POSITIVE {
        fallback
    } else {
        v.mul(1.0 / n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mass::mass_tolerance;
    use oep_geo::{Assembly, Body, BodyKind, Part};

    fn two_part_doc() -> Document {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p-a", "base").with_body("b-a"));
        doc.push_part(Part::new("p-b", "cap").with_body("b-b"));
        doc.push_body(
            Body::new("b-a", "base-solid", BodyKind::Solid)
                .with_bbox(BoundingBox::new([-10.0, -10.0, -5.0], [10.0, 10.0, 5.0])),
        );
        doc.push_body(
            Body::new("b-b", "cap-solid", BodyKind::Solid)
                .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])),
        );
        doc.push_assembly(
            Assembly::new("asm", "asm")
                .with_instance(
                    oep_geo::Instance::new(
                        "i-base",
                        "base",
                        InstanceSource::Part { id: "p-a".into() },
                    )
                    .with_transform(Transform::translation(0.0, 0.0, 0.0)),
                )
                .with_instance(
                    oep_geo::Instance::new(
                        "i-cap",
                        "cap",
                        InstanceSource::Part { id: "p-b".into() },
                    )
                    .with_transform(Transform::translation(0.0, 0.0, 30.0)),
                ),
        );
        doc
    }

    #[test]
    fn deterministic_offsets_along_hierarchy() {
        let doc = two_part_doc();
        let a = explode(&doc, "asm", 10.0).unwrap();
        let b = explode(&doc, "asm", 10.0).unwrap();
        assert_eq!(a, b, "explode must be deterministic");
        assert_eq!(a.len(), 2);
        // Both parts are depth 1.
        assert_eq!(a[0].depth, 1);
        assert_eq!(a[1].depth, 1);
        // The offset magnitude must be pull_distance * depth = 10.
        let mag0 = Vec3::new(a[0].offset[0], a[0].offset[1], a[0].offset[2]).norm();
        assert!(mass_tolerance().approx_eq(mag0, 10.0));
        // Nonzero pull → instances visibly displaced.
        assert!(a[0].offset != [0.0, 0.0, 0.0]);
    }

    #[test]
    fn zero_pull_is_identity_offsets() {
        let doc = two_part_doc();
        let a = explode(&doc, "asm", 0.0).unwrap();
        for p in &a {
            assert!(mass_tolerance()
                .approx_eq(Vec3::new(p.offset[0], p.offset[1], p.offset[2]).norm(), 0.0));
        }
    }

    #[test]
    fn missing_assembly_and_nonfinite_pull() {
        let doc = Document::new("mm");
        assert!(matches!(
            explode(&doc, "nope", 5.0),
            Err(ExplodeError::AssemblyNotFound(_))
        ));
        let doc = two_part_doc();
        assert!(matches!(
            explode(&doc, "asm", f64::NAN),
            Err(ExplodeError::NonFinitePull(_))
        ));
    }
}
