//! Scene: flatten an `oep-geo` document into renderable, selectable leaves.
//!
//! At P6-M1 a [`oep_geo::Body`] only carries an axis-aligned bounding box
//! (the B-Rep / geometry kernel slots land at P6-M2/P6-M3), so each visible
//! unit of the preview is a **box proxy**: the body's AABB placed by the full
//! instance-transform chain (assembly walk). A [`Primitive`] records the
//! identity needed for selection: the instance ids that placed it, the part
//! id, and the body id. [`build_scene`] walks every root assembly (and any
//! part not referenced by an instance) depth-first, composing transforms with
//! `oep_geo::math::Mat4`.

use oep_geo::math::{Mat4, Vec3};
use oep_geo::{Assembly, Document, InstanceSource};

/// Axis-aligned bounding box in any frame (world or local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl Aabb {
    /// Build a box from min and max corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Build a box covering an iterator of points; `None` when empty.
    pub fn from_points<'a>(iter: impl IntoIterator<Item = &'a Vec3>) -> Option<Self> {
        let mut it = iter.into_iter();
        let first = it.next()?;
        let mut min = *first;
        let mut max = *first;
        for p in it {
            min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        Some(Self { min, max })
    }

    /// The 8 corners of the box.
    pub fn corners(&self) -> [Vec3; 8] {
        let (mn, mx) = (self.min, self.max);
        [
            Vec3::new(mn.x, mn.y, mn.z),
            Vec3::new(mx.x, mn.y, mn.z),
            Vec3::new(mx.x, mx.y, mn.z),
            Vec3::new(mn.x, mx.y, mn.z),
            Vec3::new(mn.x, mn.y, mx.z),
            Vec3::new(mx.x, mn.y, mx.z),
            Vec3::new(mx.x, mx.y, mx.z),
            Vec3::new(mn.x, mx.y, mx.z),
        ]
    }

    /// Centre of the box.
    pub fn centroid(&self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Radius of the bounding sphere around the centroid.
    pub fn sphere_radius(&self) -> f64 {
        0.5 * self.min.sub(&self.max).norm()
    }

    /// Union of this box with another.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb::new(
            Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        )
    }
}

// Convenience helper: Vec3 from an `[f64; 3]`.
fn vec3_arr(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

/// A real triangle mesh attached to a primitive (world space).
///
/// The box-proxy era renders bodies as their AABB; the interactive editor
/// attaches the actual derived solid mesh ([`oep_geo::tessellate`]) here so
/// the viewport shows real faces and edges. `edges` are index pairs into
/// `positions` (boundary wires). When `triangles` is empty but `edges` is
/// not, the primitive is a pure wire overlay (e.g. a sketch drawn in the
/// viewport).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    /// Vertex positions `[x, y, z]` in world space.
    pub positions: Vec<[f32; 3]>,
    /// Indexed triangles into [`MeshData::positions`].
    pub triangles: Vec<[u32; 3]>,
    /// Boundary/overlay edge polylines (index pairs into positions).
    pub edges: Vec<[usize; 2]>,
}

/// One selectable, drawable leaf of the viewer scene.
#[derive(Debug, Clone)]
pub struct Primitive {
    /// Stable document-unique id.
    pub id: String,
    /// The part owning this body.
    pub part_id: String,
    /// The body rendered as a box proxy.
    pub body_id: String,
    /// Ordered instance chain that placed this body (empty for uninstanced).
    pub instance_ids: Vec<String>,
    /// Body-local AABB from the document.
    pub local: Aabb,
    /// World pose: composition of the full instance transform chain
    /// (column-major `Mat4`).
    pub transform: Mat4,
    /// `local.corners()` transformed to world space.
    pub world_corners: [Vec3; 8],
    /// Conservative world-space AABB of the transformed corners.
    pub world: Aabb,
    /// Real solid mesh (world space) when the editor attaches one; `None` for
    /// the classic box proxy.
    pub mesh: Option<MeshData>,
    /// Selection highlight: when `true` the renderer draws this primitive with
    /// the theme highlight colour (faces and edges); identity is still the
    /// structured [`crate::pick::Selection`], never pixel color.
    pub highlighted: bool,
}

impl Primitive {
    /// The world-space AABB.
    pub fn world_aabb(&self) -> Aabb {
        self.world
    }

    /// Build a mesh primitive (world space, identity pose) directly from a
    /// derived solid mesh, computing its AABB for picking/framing. The part
    /// owns `part_id`; the body owns `body_id`.
    pub fn from_mesh(
        id: impl Into<String>,
        part_id: impl Into<String>,
        body_id: impl Into<String>,
        mesh: MeshData,
    ) -> Self {
        let pts: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|p| Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
            .collect();
        let world = if pts.is_empty() {
            Aabb::new(Vec3::zero(), Vec3::zero())
        } else {
            Aabb::from_points(pts.iter()).expect("non-empty mesh bounds")
        };
        let local = world;
        let world_corners = world.corners();
        Self {
            id: id.into(),
            part_id: part_id.into(),
            body_id: body_id.into(),
            instance_ids: Vec::new(),
            local,
            transform: Mat4::identity(),
            world_corners,
            world,
            mesh: Some(mesh),
            highlighted: false,
        }
    }

    /// A pure wire overlay primitive (sketch points/lines drawn in the
    /// viewport): no filled faces, only the listed edges. A point is a
    /// zero-length edge; a segment is one edge.
    pub fn wire(
        id: impl Into<String>,
        part_id: impl Into<String>,
        points: &[[f32; 3]],
        edges: &[[usize; 2]],
    ) -> Self {
        let mesh = MeshData {
            positions: points.to_vec(),
            triangles: Vec::new(),
            edges: edges.to_vec(),
        };
        Self::from_mesh(id, part_id, "", mesh)
    }
}

/// The viewer scene: every body proxy plus the overall bounds.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// Renderable leaves.
    pub primitives: Vec<Primitive>,
    /// Union of all primitive world AABBs (empty scene -> `None`).
    pub bounds: Option<Aabb>,
}

/// Convert a document `Transform` to an `oep_geo::math::Mat4`.
///
/// The math core's [`Mat4::from_array16`] is column-major and places the
/// translation in the last column (`data[12..15]`); the P6-M1 instance
/// transform stores the placement with translation at exactly those elements
/// (the schema doc labels it row-major, but the math contract that the viewer
/// consumes treats the same 16 doubles as column-major affine, and the golden
/// bracket placement arrays verify this directly: feeding the raw array to
/// `from_array16` places F1 at (10,10,0) as expected). A general affine
/// matrix would need an explicit transpose; for the placement translation
/// matrices used here, direct pass-through is the verified contract.
fn transform_to_mat4(t: &oep_geo::Transform) -> Mat4 {
    Mat4::from_array16(*t.components()).expect("document transform had a non-finite component")
}

/// Emit one box proxy per body of `part_id`, posed at `transform`, tagged with
/// the current instance `chain`.
fn emit_part_bodies(
    doc: &Document,
    part_id: &str,
    transform: &Mat4,
    chain: &[String],
    out: &mut Vec<Primitive>,
) {
    let Some(body_ids) = doc
        .parts
        .iter()
        .find(|p| p.id == part_id)
        .map(|p| &p.body_ids)
    else {
        return;
    };
    for bid in body_ids {
        let Some(body) = doc.bodies.iter().find(|b| &b.id == bid) else {
            continue;
        };
        let Some(bbox) = body.bbox else {
            continue; // no geometry slot yet: nothing to draw
        };
        let local = Aabb::new(vec3_arr(bbox.min), vec3_arr(bbox.max));
        let local_corners = local.corners();
        let world_corners: [Vec3; 8] =
            std::array::from_fn(|i| transform.transform_point(&local_corners[i]));
        let world = Aabb::from_points(world_corners.iter()).expect("8 corners always bound");
        let mut id = format!("part:{part_id}:body:{bid}");
        if let Some(first_inst) = chain.first() {
            id.push(':');
            id.push_str(first_inst);
        }
        out.push(Primitive {
            id,
            part_id: part_id.to_string(),
            body_id: bid.clone(),
            instance_ids: chain.to_vec(),
            local,
            transform: *transform,
            world_corners,
            world,
            mesh: None,
            highlighted: false,
        });
    }
}

fn walk_assembly(
    doc: &Document,
    asm: &Assembly,
    parent: &Mat4,
    chain: &mut Vec<String>,
    out: &mut Vec<Primitive>,
) {
    for inst in &asm.instances {
        if !inst.visible {
            continue;
        }
        let m = parent.mul(&transform_to_mat4(&inst.transform));
        match &inst.source {
            InstanceSource::Part { id } => {
                chain.push(inst.id.clone());
                emit_part_bodies(doc, id, &m, chain, out);
                chain.pop();
            }
            InstanceSource::Assembly { id } => {
                if let Some(sub) = doc.assemblies.iter().find(|a| a.id == *id) {
                    chain.push(inst.id.clone());
                    walk_assembly(doc, sub, &m, chain, out);
                    chain.pop();
                }
            }
        }
    }
}

/// Which assembly ids are nested under another assembly's instance (i.e. are
/// NOT root assemblies).
fn nested_assembly_ids(doc: &Document) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    for asm in &doc.assemblies {
        for inst in &asm.instances {
            if let InstanceSource::Assembly { id } = &inst.source {
                set.insert(id.clone());
            }
        }
    }
    set
}

/// Which part ids are instanced anywhere (referenced by an instance).
fn instanced_part_ids(doc: &Document) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    for asm in &doc.assemblies {
        for inst in &asm.instances {
            if let InstanceSource::Part { id } = &inst.source {
                set.insert(id.clone());
            }
        }
    }
    set
}

/// Build the viewer [`Scene`] from a real `oep-geo` document.
///
/// Walks every root assembly (an assembly not nested inside another assembly)
/// depth-first, composing instance transforms along the chain; then emits any
/// part never referenced by an instance at the identity pose so stand-alone
/// parts still preview.
pub fn build_scene(doc: &Document) -> Scene {
    let mut primitives = Vec::new();
    let nested = nested_assembly_ids(doc);
    let mut chain = Vec::new();
    let identity = Mat4::identity();

    for asm in &doc.assemblies {
        if nested.contains(&asm.id) {
            continue; // already walked from its parent
        }
        walk_assembly(doc, asm, &identity, &mut chain, &mut primitives);
    }

    // Parts not instanced anywhere: preview them at identity.
    let instanced = instanced_part_ids(doc);
    for part in &doc.parts {
        if !instanced.contains(&part.id) {
            emit_part_bodies(doc, &part.id, &identity, &[], &mut primitives);
        }
    }

    let bounds = primitives
        .iter()
        .map(|p| p.world)
        .reduce(|a, b| a.union(&b));
    Scene { primitives, bounds }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_geo::{Body, BodyKind, Part};

    fn one_part_doc() -> Document {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p1", "part-one").with_body("b1"));
        doc.push_body(
            Body::new("b1", "body-one", BodyKind::Solid)
                .with_bbox(oep_geo::BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])),
        );
        doc
    }

    #[test]
    fn aabb_union_covers_both() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let b = Aabb::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));
        let u = a.union(&b);
        assert_eq!(u.min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(u.max, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn corners_are_eight_distinct_points() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0));
        let cs = a.corners();
        assert_eq!(cs.len(), 8);
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert_ne!(cs[i], cs[j]);
            }
        }
    }

    #[test]
    fn build_scene_from_part_without_assembly() {
        let doc = one_part_doc();
        let scene = build_scene(&doc);
        assert_eq!(scene.primitives.len(), 1);
        assert_eq!(scene.primitives[0].part_id, "p1");
        assert_eq!(scene.primitives[0].body_id, "b1");
        assert!(scene.bounds.is_some());
        assert_eq!(
            scene.primitives[0].world_aabb().max,
            Vec3::new(1.0, 1.0, 1.0)
        );
    }
}
