//! The OEP assembly workbench: maps the canonical oep-geo assembly model
//! (P6-M1-T01) plus a mate set onto the solver, solves instance placements,
//! writes them back into the oep-geo document, and measures how well the
//! constraints are satisfied.
//!
//! Workbench flow ("a 2-part fixture assembles and constrains through the
//! workbench"):
//!
//! 1. A canonical oep-geo [`Document`] holds `parts`, `bodies` and one or
//!    more `assemblies` of [`Instance`]s, each carrying a 4x4 [`Transform`].
//! 2. The caller selects an assembly, an [`AnchorRegistry`] (or derives
//!    default anchors from each part's body bounding box), the instances to
//!    move, and a mate set ([`crate::mates`]).
//! 3. [`solve`] builds a bounded least-squares system in the solver
//!    (`crate::adopted`): every moving instance contributes a free rigid pose
//!    (part-origin position + unit quaternion) in its own group; mates become
//!    residual rows over those poses; fixed instances contribute constants.
//! 4. The engine solves the poses; solved placements are written back into
//!    the document (validated) and residuals are measured independently.
//!
//! The public API of this module (types and functions re-exported from
//! `lib.rs`) is the contract that SOLVER-REPLACE keeps unchanged.

use std::collections::HashMap;
use std::fmt;

use oep_geo::{Document, Instance, Transform, ValidationError};
use serde::{Deserialize, Serialize};

use crate::adopted::{self, SolverRequest, SolverStatus};
use crate::anchors::{anchors_for, Anchor, AnchorRegistry};
use crate::mates::{AnchorRef, Mate};
use crate::transform_math as tm;

/// A solution request for one assembly solve.
#[derive(Debug, Clone)]
pub struct SolveRequest {
    /// Assembly id whose instances are constrained.
    pub assembly: String,
    /// Mates to satisfy.
    pub mates: Vec<Mate>,
    /// Anchor registry (extensions/overrides for parts).
    pub anchors: AnchorRegistry,
    /// Instance ids to solve (their placements are recomputed).
    pub moving: Vec<String>,
}

impl SolveRequest {
    pub fn new(assembly: impl Into<String>, mates: Vec<Mate>, moving: Vec<String>) -> Self {
        Self {
            assembly: assembly.into(),
            mates,
            anchors: AnchorRegistry::new(),
            moving,
        }
    }
}

/// How a mate's residual is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MateResidualKind {
    /// Length residual, in document units.
    Distance,
    /// Angle residual, in radians.
    Angle,
}

/// Measured residual for one mate after a solve.
#[derive(Debug, Clone, PartialEq)]
pub struct MateResidual {
    /// Index into the mate list.
    pub mate_index: usize,
    /// Measured residual value (units or radians).
    pub value: f64,
    pub kind: MateResidualKind,
}

/// The outcome of a workbench solve.
#[derive(Debug, Clone)]
pub struct SolveReport {
    /// Solver status (success = Okay or RedundantOkay).
    pub status: SolverStatus,
    /// Unconstrained degrees of freedom reported by the engine.
    pub dof: i32,
    /// Per-moving-instance solved placements (already written into the doc).
    pub moved: Vec<(String, Transform)>,
    /// Independent measurements of how well each mate is satisfied.
    pub residuals: Vec<MateResidual>,
    /// Largest distance-type residual (document units).
    pub max_distance_error: f64,
    /// Largest angle-type residual (radians).
    pub max_angle_error: f64,
}

/// Errors raised by the workbench.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkbenchError {
    /// The named assembly does not exist in the document.
    AssemblyNotFound(String),
    /// An instance referenced by a mate does not exist in the assembly.
    InstanceNotFound(String),
    /// A mate references an anchor that is not on the instance's part.
    AnchorNotFound { instance: String, anchor: String },
    /// A moving instance has no usable anchors on its part.
    NoAnchors(String),
    /// Sub-assembly instances are not yet supported by the workbench solve.
    SubAssemblyNotSupported(String),
    /// The solver rejected the system.
    SolverFailed { status: SolverStatus, dof: i32 },
    /// A mate contradicts the moving/fixed split (e.g. fixing a moved part).
    ContradictoryFixed(String),
    /// Orientation mates require Z-axis anchors (documented limitation).
    NotZAxisAnchor { instance: String, anchor: String },
    /// A moving-instance anchor at a rigid offset from the part origin is not
    /// supported yet (lands with B-Rep faces in P5).
    RigidOffsetNotSupported { instance: String, anchor: String },
    /// A plane-role anchor on a moving instance is not supported yet.
    PlaneOnMovingNotSupported { instance: String, anchor: String },
    /// Wrote a transform that makes the oep-geo document invalid.
    DocumentInvalid(Vec<ValidationError>),
    /// (De)serialization failed.
    Serialization(String),
}

impl fmt::Display for WorkbenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkbenchError::AssemblyNotFound(id) => write!(f, "assembly not found: {id}"),
            WorkbenchError::InstanceNotFound(id) => write!(f, "instance not found: {id}"),
            WorkbenchError::AnchorNotFound { instance, anchor } => {
                write!(f, "anchor '{anchor}' not found on instance '{instance}'")
            }
            WorkbenchError::NoAnchors(id) => {
                write!(f, "no anchors available for instance '{id}'")
            }
            WorkbenchError::SubAssemblyNotSupported(id) => {
                write!(f, "sub-assembly instance '{id}' is not supported by the solver yet")
            }
            WorkbenchError::SolverFailed { status, dof } => {
                write!(f, "solver failed: {status:?} (dof={dof})")
            }
            WorkbenchError::ContradictoryFixed(id) => {
                write!(f, "instance '{id}' is both fixed and moving")
            }
            WorkbenchError::NotZAxisAnchor { instance, anchor } => write!(
                f,
                "orientation mate anchor '{anchor}' on instance '{instance}' is not a Z-axis anchor"
            ),
            WorkbenchError::RigidOffsetNotSupported { instance, anchor } => write!(
                f,
                "anchor '{anchor}' on moving instance '{instance}' is at a rigid offset; pose-anchored mates only (v1 limitation, lands with B-Rep faces)"
            ),
            WorkbenchError::PlaneOnMovingNotSupported { instance, anchor } => write!(
                f,
                "plane-role anchor '{anchor}' on moving instance '{instance}' is not supported (v1 limitation)"
            ),
            WorkbenchError::DocumentInvalid(errors) => {
                write!(f, "document invalid after solve: {} error(s)", errors.len())
            }
            WorkbenchError::Serialization(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

impl std::error::Error for WorkbenchError {}

/// A minimal serializable instance listing (workbench-facing view of the
/// oep-geo assembly model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub source_id: String,
    pub transform: Transform,
}

/// Read the oep-geo assembly model: list the instances of an assembly.
pub fn list_instances(document: &Document, assembly_id: &str) -> Option<Vec<InstanceInfo>> {
    let assembly = document.assemblies.iter().find(|a| a.id == assembly_id)?;
    Some(
        assembly
            .instances
            .iter()
            .map(|i| InstanceInfo {
                id: i.id.clone(),
                name: i.name.clone(),
                source_kind: match &i.source {
                    oep_geo::InstanceSource::Part { .. } => "part".to_string(),
                    oep_geo::InstanceSource::Assembly { .. } => "sub-assembly".to_string(),
                },
                source_id: i.source.referenced_id().to_string(),
                transform: i.transform.clone(),
            })
            .collect(),
    )
}

/// Load a canonical oep-geo document from JSON.
pub fn load_document(text: &str) -> Result<Document, WorkbenchError> {
    Document::from_json_pretty(text).map_err(|e| WorkbenchError::Serialization(e.to_string()))
}

/// Serialize a canonical oep-geo document to pretty JSON.
pub fn save_document(document: &Document) -> Result<String, WorkbenchError> {
    document
        .to_json_pretty()
        .map_err(|e| WorkbenchError::Serialization(e.to_string()))
}

/// Number of scalar unknowns per moving instance (position x,y,z + unit
/// quaternion w,x,y,z).
const UNKNOWNS_PER_BODY: usize = 7;

fn part_id_of(inst: &Instance) -> Result<&str, WorkbenchError> {
    match &inst.source {
        oep_geo::InstanceSource::Part { id } => Ok(id.as_str()),
        oep_geo::InstanceSource::Assembly { .. } => {
            Err(WorkbenchError::SubAssemblyNotSupported(inst.id.clone()))
        }
    }
}

/// Find an anchor by name on an instance (resolving the part and registry).
/// Returns a clone because resolved anchors are built on the fly.
fn find_anchor(
    document: &Document,
    registry: &AnchorRegistry,
    inst: &Instance,
    name: &str,
) -> Result<Anchor, WorkbenchError> {
    let part_id = part_id_of(inst)?;
    anchors_for(document, registry, part_id)
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(|| WorkbenchError::AnchorNotFound {
            instance: inst.id.clone(),
            anchor: name.to_string(),
        })
}

/// A fixed (non-moving) geometric quantity captured before the solve.
#[derive(Debug, Clone, Copy)]
enum PointRef {
    /// A world point that never moves.
    Fixed([f64; 3]),
    /// A point on a moving instance: body index + part-local offset.
    Moving { body: usize, local: [f64; 3] },
}

/// A fixed or moving direction used by orientation mates.
#[derive(Debug, Clone, Copy)]
enum DirRef {
    /// A world direction that never moves.
    Fixed([f64; 3]),
    /// A direction on a moving instance: body index + part-local direction.
    Moving { body: usize, local: [f64; 3] },
}

/// A plane (origin + normal) in world coordinates. The v1 workbench only
/// allows the plane role on fixed instances, so a plane is always a constant.
#[derive(Debug, Clone, Copy)]
struct PlaneRef {
    o: [f64; 3],
    n: [f64; 3],
}

/// One residual building block over the bounded unknown vector.
#[derive(Debug, Clone, Copy)]
enum Spec {
    /// A coincident mate: world(a) - world(b) = 0 (3 rows).
    Coincident { a: PointRef, b: PointRef },
    /// A point-on-plane mate (1 row).
    OnPlane { pt: PointRef, plane: PlaneRef },
    /// A signed plane-distance mate (1 row).
    PlaneDistance {
        pt: PointRef,
        plane: PlaneRef,
        distance: f64,
    },
    /// A parallel mate: cross(world(a), world(b)) = 0 (3 rows).
    Parallel { a: DirRef, b: DirRef },
}

/// One moving instance's rigid pose entry in the unknown vector.
#[derive(Debug, Clone)]
struct MovingEntry {
    id: String,
    /// Part-local origin of the pose-point anchor (usually the part origin).
    pose_local_origin: [f64; 3],
    /// Initial world position of the pose-point anchor.
    position: [f64; 3],
    /// Initial orientation, `[w, x, y, z]`.
    quat: [f64; 4],
}

/// Solve an assembly through the workbench and write the solved placements
/// back into the document.
pub fn solve(document: &mut Document, req: &SolveRequest) -> Result<SolveReport, WorkbenchError> {
    // 1. Locate assembly and instances.
    let assembly = document
        .assemblies
        .iter()
        .find(|a| a.id == req.assembly)
        .ok_or_else(|| WorkbenchError::AssemblyNotFound(req.assembly.clone()))?;
    let instance_by_id: HashMap<&str, &Instance> = assembly
        .instances
        .iter()
        .map(|i| (i.id.as_str(), i))
        .collect();

    for id in &req.moving {
        if !instance_by_id.contains_key(id.as_str()) {
            return Err(WorkbenchError::InstanceNotFound(id.clone()));
        }
    }

    let mut moving: HashMap<&str, &Instance> = HashMap::new();
    let mut fixed: HashMap<&str, &Instance> = HashMap::new();
    for inst in &assembly.instances {
        if req.moving.iter().any(|m| m == &inst.id) {
            moving.insert(inst.id.as_str(), inst);
        } else {
            fixed.insert(inst.id.as_str(), inst);
        }
    }
    for mate in &req.mates {
        if let Mate::Fixed { instance } = mate {
            if moving.contains_key(instance.as_str()) {
                return Err(WorkbenchError::ContradictoryFixed(instance.clone()));
            }
        }
    }

    // 2. MODEL: every moving instance is represented in the solver by its
    // rigid POSE only - a pose point (the anchor at the part origin) and a
    // pose normal (a +Z-axis anchor, e.g. "top"). Mates on a moving instance
    // constrain these pose quantities directly. This keeps the solved body
    // rigid: its bounded pose (position + unit quaternion) is its 6 degrees
    // of freedom. Offsetting rigid anchors (points not at the part origin,
    // planes on moving parts) are a documented v1 limitation that lands with
    // real B-Rep faces in P5.
    let mut pose_point: HashMap<String, String> = HashMap::new(); // instance -> pose point anchor name
    let mut pose_normal: HashMap<String, String> = HashMap::new(); // instance -> pose normal anchor name

    for inst in moving.values() {
        let part_id = part_id_of(inst)?;
        let anchors = anchors_for(document, &req.anchors, part_id);
        if anchors.is_empty() {
            return Err(WorkbenchError::NoAnchors(inst.id.clone()));
        }
        let pp = anchors
            .iter()
            .find(|a| a.origin.iter().all(|c| c.abs() < 1e-9))
            .map(|a| a.name.clone())
            .or_else(|| anchors.first().map(|a| a.name.clone()))
            .ok_or_else(|| WorkbenchError::NoAnchors(inst.id.clone()))?;
        let pn = ["top", "origin"]
            .iter()
            .find_map(|n| anchors.iter().find(|a| a.name == *n && a.is_z_axis()))
            .map(|a| a.name.clone())
            .or_else(|| {
                anchors
                    .iter()
                    .find(|a| a.is_z_axis())
                    .map(|a| a.name.clone())
            })
            .ok_or_else(|| WorkbenchError::NoAnchors(inst.id.clone()))?;
        pose_point.insert(inst.id.clone(), pp);
        pose_normal.insert(inst.id.clone(), pn);
    }

    // Build deterministic per-instance ordering so the unknown vector layout
    // (and thus debug output and reports) is stable across runs.
    let mut moving_ids: Vec<&str> = moving.keys().copied().collect();
    moving_ids.sort_unstable();

    // World pose of a moving instance (position of its pose anchor + quat).
    let world_pose = |inst: &Instance| -> ([f64; 3], [f64; 4]) {
        let u = tm::transform_rotate(&inst.transform, tm::V3::new(1.0, 0.0, 0.0)).normalized();
        let v = tm::transform_rotate(&inst.transform, tm::V3::new(0.0, 1.0, 0.0)).normalized();
        let quat = adopted::make_quaternion([u.x, u.y, u.z], [v.x, v.y, v.z]);
        (
            [inst.transform[12], inst.transform[13], inst.transform[14]],
            quat,
        )
    };

    let mut entries: Vec<MovingEntry> = Vec::new();
    for &id in &moving_ids {
        let inst = moving[id];
        let pp = &pose_point[id];
        let pp_anchor = find_anchor(document, &req.anchors, inst, pp)?;
        let (pos, quat) = world_pose(inst);
        entries.push(MovingEntry {
            id: id.to_string(),
            pose_local_origin: pp_anchor.origin,
            position: pos,
            quat,
        });
    }
    let body_of: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();

    // Resolve a point-role anchor reference.
    fn point_ref(
        document: &Document,
        registry: &AnchorRegistry,
        moving: &HashMap<&str, &Instance>,
        fixed: &HashMap<&str, &Instance>,
        body_of: &HashMap<&str, usize>,
        r: &AnchorRef,
    ) -> Result<PointRef, WorkbenchError> {
        if let Some(&body) = body_of.get(r.instance.as_str()) {
            let inst = moving.get(r.instance.as_str()).unwrap();
            let anchor = find_anchor(document, registry, inst, &r.anchor)?;
            if anchor.origin.iter().any(|c| c.abs() > 1e-9) {
                return Err(WorkbenchError::RigidOffsetNotSupported {
                    instance: r.instance.clone(),
                    anchor: r.anchor.clone(),
                });
            }
            Ok(PointRef::Moving {
                body,
                local: anchor.origin,
            })
        } else {
            let inst = fixed
                .get(r.instance.as_str())
                .ok_or_else(|| WorkbenchError::InstanceNotFound(r.instance.clone()))?;
            let anchor = find_anchor(document, registry, inst, &r.anchor)?;
            let wp = tm::transform_apply(&inst.transform, tm::V3::from(anchor.origin));
            Ok(PointRef::Fixed([wp.x, wp.y, wp.z]))
        }
    }

    // Resolve a plane-role anchor reference (fixed instances only).
    fn plane_ref(
        document: &Document,
        registry: &AnchorRegistry,
        moving: &HashMap<&str, &Instance>,
        fixed: &HashMap<&str, &Instance>,
        r: &AnchorRef,
    ) -> Result<PlaneRef, WorkbenchError> {
        if moving.contains_key(r.instance.as_str()) {
            return Err(WorkbenchError::PlaneOnMovingNotSupported {
                instance: r.instance.clone(),
                anchor: r.anchor.clone(),
            });
        }
        let inst = fixed
            .get(r.instance.as_str())
            .ok_or_else(|| WorkbenchError::InstanceNotFound(r.instance.clone()))?;
        let anchor = find_anchor(document, registry, inst, &r.anchor)?;
        let o = tm::transform_apply(&inst.transform, tm::V3::from(anchor.origin));
        let n = tm::transform_rotate(&inst.transform, tm::V3::from(anchor.normal)).normalized();
        Ok(PlaneRef {
            o: [o.x, o.y, o.z],
            n: [n.x, n.y, n.z],
        })
    }

    // Resolve a normal-role anchor reference.
    fn dir_ref(
        document: &Document,
        registry: &AnchorRegistry,
        moving: &HashMap<&str, &Instance>,
        fixed: &HashMap<&str, &Instance>,
        body_of: &HashMap<&str, usize>,
        r: &AnchorRef,
    ) -> Result<DirRef, WorkbenchError> {
        if let Some(&body) = body_of.get(r.instance.as_str()) {
            let inst = moving.get(r.instance.as_str()).unwrap();
            let anchor = find_anchor(document, registry, inst, &r.anchor)?;
            if !anchor.is_z_axis() {
                return Err(WorkbenchError::NotZAxisAnchor {
                    instance: r.instance.clone(),
                    anchor: r.anchor.clone(),
                });
            }
            Ok(DirRef::Moving {
                body,
                local: anchor.normal,
            })
        } else {
            let inst = fixed
                .get(r.instance.as_str())
                .ok_or_else(|| WorkbenchError::InstanceNotFound(r.instance.clone()))?;
            let anchor = find_anchor(document, registry, inst, &r.anchor)?;
            if !anchor.is_z_axis() {
                return Err(WorkbenchError::NotZAxisAnchor {
                    instance: r.instance.clone(),
                    anchor: r.anchor.clone(),
                });
            }
            let n = tm::transform_rotate(&inst.transform, tm::V3::from(anchor.normal)).normalized();
            Ok(DirRef::Fixed([n.x, n.y, n.z]))
        }
    }

    // 3. Map mates to residual specs.
    let mut specs: Vec<Spec> = Vec::new();
    for mate in &req.mates {
        match mate {
            Mate::Fixed { .. } => {}
            Mate::Coincident { a, b } => {
                let pa = point_ref(document, &req.anchors, &moving, &fixed, &body_of, a)?;
                let pb = point_ref(document, &req.anchors, &moving, &fixed, &body_of, b)?;
                specs.push(Spec::Coincident { a: pa, b: pb });
            }
            Mate::OnPlane { point, plane } => {
                let pt = point_ref(document, &req.anchors, &moving, &fixed, &body_of, point)?;
                let plane = plane_ref(document, &req.anchors, &moving, &fixed, plane)?;
                specs.push(Spec::OnPlane { pt, plane });
            }
            Mate::PlaneDistance {
                point,
                plane,
                distance,
            } => {
                let pt = point_ref(document, &req.anchors, &moving, &fixed, &body_of, point)?;
                let plane = plane_ref(document, &req.anchors, &moving, &fixed, plane)?;
                specs.push(Spec::PlaneDistance {
                    pt,
                    plane,
                    distance: *distance,
                });
            }
            Mate::Parallel { a, b } => {
                let na = dir_ref(document, &req.anchors, &moving, &fixed, &body_of, a)?;
                let nb = dir_ref(document, &req.anchors, &moving, &fixed, &body_of, b)?;
                specs.push(Spec::Parallel { a: na, b: nb });
            }
        }
    }

    // 4. Assemble the bounded unknown vector.
    let mut unknowns: Vec<f64> = Vec::with_capacity(entries.len() * UNKNOWNS_PER_BODY);
    for e in &entries {
        unknowns.extend_from_slice(&e.position);
        unknowns.extend_from_slice(&e.quat);
    }

    if std::env::var_os("OEP_DEBUG").is_some() {
        eprintln!("=== oep-assem solver system (bounded pose) ===");
        for e in &entries {
            eprintln!(
                "{} p=({:.6}, {:.6}, {:.6}) q=({:.6}, {:.6}, {:.6}, {:.6})",
                e.id,
                e.position[0],
                e.position[1],
                e.position[2],
                e.quat[0],
                e.quat[1],
                e.quat[2],
                e.quat[3]
            );
        }
        for s in &specs {
            eprintln!("spec: {s:?}");
        }
    }

    let nb_entries = entries.len();
    // The residual closure owns every value it reads: fixed world data is
    // captured as constants and moving data is addressed by body index into
    // the unknown vector. No borrow of `document` outlives `solve`.
    let residual = move |x: &[f64]| -> Vec<f64> {
        let mut out = Vec::new();
        // Unit-quaternion boundedness row per moving body.
        for k in 0..nb_entries {
            let off = k * UNKNOWNS_PER_BODY;
            let (w, x, y, z) = (x[off + 3], x[off + 4], x[off + 5], x[off + 6]);
            out.push(w * w + x * x + y * y + z * z - 1.0);
        }
        // Rotate a part-local vector by body k's current quaternion.
        let rot = |k: usize, v: [f64; 3]| -> [f64; 3] {
            let off = k * UNKNOWNS_PER_BODY;
            let m = tm::quat_to_matrix([x[off + 3], x[off + 4], x[off + 5], x[off + 6]]);
            [
                m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
                m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
                m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
            ]
        };
        // World point of a PointRef.
        let pt = |refn: PointRef| -> [f64; 3] {
            match refn {
                PointRef::Fixed(p) => p,
                PointRef::Moving { body, local } => {
                    let off = body * UNKNOWNS_PER_BODY;
                    let r = rot(body, local);
                    [r[0] + x[off], r[1] + x[off + 1], r[2] + x[off + 2]]
                }
            }
        };
        // World direction of a DirRef.
        let dir = |refn: DirRef| -> [f64; 3] {
            match refn {
                DirRef::Fixed(n) => n,
                DirRef::Moving { body, local } => rot(body, local),
            }
        };
        for spec in &specs {
            match spec {
                Spec::Coincident { a, b } => {
                    let wa = pt(*a);
                    let wb = pt(*b);
                    out.push(wa[0] - wb[0]);
                    out.push(wa[1] - wb[1]);
                    out.push(wa[2] - wb[2]);
                }
                Spec::OnPlane { pt: ptref, plane } => {
                    let wp = pt(*ptref);
                    let dp = [wp[0] - plane.o[0], wp[1] - plane.o[1], wp[2] - plane.o[2]];
                    out.push(dp[0] * plane.n[0] + dp[1] * plane.n[1] + dp[2] * plane.n[2]);
                }
                Spec::PlaneDistance {
                    pt: ptref,
                    plane,
                    distance,
                } => {
                    let wp = pt(*ptref);
                    let dp = [wp[0] - plane.o[0], wp[1] - plane.o[1], wp[2] - plane.o[2]];
                    out.push(
                        dp[0] * plane.n[0] + dp[1] * plane.n[1] + dp[2] * plane.n[2] - distance,
                    );
                }
                Spec::Parallel { a, b } => {
                    let wa = dir(*a);
                    let wb = dir(*b);
                    out.push(wa[1] * wb[2] - wa[2] * wb[1]);
                    out.push(wa[2] * wb[0] - wa[0] * wb[2]);
                    out.push(wa[0] * wb[1] - wa[1] * wb[0]);
                }
            }
        }
        out
    };

    let outcome = adopted::solve(SolverRequest::new(unknowns, Box::new(residual)));

    let mut moved: Vec<(String, Transform)> = Vec::new();
    if outcome.status.is_success() {
        // 5. Reconstruct solved placements.
        for (i, e) in entries.iter().enumerate() {
            let off = i * UNKNOWNS_PER_BODY;
            let world_origin = [
                outcome.params[off],
                outcome.params[off + 1],
                outcome.params[off + 2],
            ];
            let quat = [
                outcome.params[off + 3],
                outcome.params[off + 4],
                outcome.params[off + 5],
                outcome.params[off + 6],
            ];
            let new_t = tm::transform_from_solved_pose(e.pose_local_origin, world_origin, quat);
            moved.push((e.id.clone(), new_t));
        }

        // 6. Write back into the document.
        for (id, t) in &moved {
            let target = document
                .assemblies
                .iter_mut()
                .find(|a| a.id == req.assembly)
                .and_then(|a| a.instances.iter_mut().find(|i| &i.id == id))
                .ok_or_else(|| WorkbenchError::InstanceNotFound(id.clone()))?;
            target.transform = t.clone();
        }

        // 7. The written-back doc must stay canonical.
        let errors = document.validate();
        if !errors.is_empty() {
            return Err(WorkbenchError::DocumentInvalid(errors));
        }
    }

    // 8. Independent measurement of how well the mates are satisfied.
    let residuals = measure_residuals(document, &req.anchors, &req.mates)?;
    let mut max_distance_error = 0.0_f64;
    let mut max_angle_error = 0.0_f64;
    for r in &residuals {
        match r.kind {
            MateResidualKind::Distance => max_distance_error = max_distance_error.max(r.value),
            MateResidualKind::Angle => max_angle_error = max_angle_error.max(r.value),
        }
    }

    if !outcome.status.is_success() {
        return Err(WorkbenchError::SolverFailed {
            status: outcome.status,
            dof: outcome.dof,
        });
    }

    Ok(SolveReport {
        status: outcome.status,
        dof: outcome.dof,
        moved,
        residuals,
        max_distance_error,
        max_angle_error,
    })
}

/// Measure each mate's residual directly from the document's transforms after
/// a solve. Independent of the engine's own residual reporting.
pub fn measure_residuals(
    document: &Document,
    registry: &AnchorRegistry,
    mates: &[Mate],
) -> Result<Vec<MateResidual>, WorkbenchError> {
    let mut out = Vec::new();
    for (idx, mate) in mates.iter().enumerate() {
        let residual = match mate {
            Mate::Fixed { .. } => MateResidual {
                mate_index: idx,
                value: 0.0,
                kind: MateResidualKind::Distance,
            },
            Mate::Coincident { a, b } => {
                let pa = world_point(document, registry, a)?;
                let pb = world_point(document, registry, b)?;
                MateResidual {
                    mate_index: idx,
                    value: (pa - pb).norm(),
                    kind: MateResidualKind::Distance,
                }
            }
            Mate::OnPlane { point, plane } => {
                let r = signed_to_plane(document, registry, point, plane)?;
                MateResidual {
                    mate_index: idx,
                    value: r,
                    kind: MateResidualKind::Distance,
                }
            }
            Mate::PlaneDistance {
                point,
                plane,
                distance,
            } => {
                let r = signed_to_plane(document, registry, point, plane)?;
                MateResidual {
                    mate_index: idx,
                    value: (r - distance).abs(),
                    kind: MateResidualKind::Distance,
                }
            }
            Mate::Parallel { a, b } => {
                let na = world_normal(document, registry, a)?;
                let nb = world_normal(document, registry, b)?;
                let dot = na.dot(nb).clamp(-1.0, 1.0);
                let ang = dot.acos();
                let dev = ang.min(std::f64::consts::PI - ang);
                MateResidual {
                    mate_index: idx,
                    value: dev,
                    kind: MateResidualKind::Angle,
                }
            }
        };
        out.push(residual);
    }
    Ok(out)
}

fn world_point(
    document: &Document,
    registry: &AnchorRegistry,
    r: &AnchorRef,
) -> Result<tm::V3, WorkbenchError> {
    let inst = find_instance(document, &r.instance)?;
    let anchor = find_anchor(document, registry, inst, &r.anchor)?;
    Ok(tm::transform_apply(
        &inst.transform,
        tm::V3::from(anchor.origin),
    ))
}

fn world_normal(
    document: &Document,
    registry: &AnchorRegistry,
    r: &AnchorRef,
) -> Result<tm::V3, WorkbenchError> {
    let inst = find_instance(document, &r.instance)?;
    let anchor = find_anchor(document, registry, inst, &r.anchor)?;
    Ok(tm::transform_rotate(&inst.transform, tm::V3::from(anchor.normal)).normalized())
}

fn signed_to_plane(
    document: &Document,
    registry: &AnchorRegistry,
    point: &AnchorRef,
    plane: &AnchorRef,
) -> Result<f64, WorkbenchError> {
    let p = world_point(document, registry, point)?;
    let o = world_point(document, registry, plane)?;
    let n = world_normal(document, registry, plane)?;
    Ok((p - o).dot(n))
}

fn find_instance<'a>(document: &'a Document, id: &str) -> Result<&'a Instance, WorkbenchError> {
    for a in &document.assemblies {
        if let Some(i) = a.instances.iter().find(|i| i.id == id) {
            return Ok(i);
        }
    }
    Err(WorkbenchError::InstanceNotFound(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknowns_per_body_is_seven() {
        assert_eq!(UNKNOWNS_PER_BODY, 7);
    }
}
