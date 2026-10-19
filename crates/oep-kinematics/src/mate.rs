//! The DMU-like mate/joint model and its residual evaluation.
//!
//! A mate expresses a *relative-motion constraint* between two frames (either
//! instance frames or the fixed [`crate::GROUND`] frame / a fixed base body).
//! The concrete joints -- [`Revolute`], [`Prismatic`], [`Spherical`], [`Rigid`]
//! -- mirror the classic assembly-joint vocabulary of DMU kinematics, but every
//! residual below is our own formulation over `oep-geo` math.

use oep_geo::math::Vec3;
use std::collections::BTreeMap;

use crate::pose::{orientation_residual, Pose};

/// Read-only snapshot of every frame referenced by the mates (movables,
/// fixed bases and the ground frame), keyed by body id.
pub type PoseMap = BTreeMap<String, Pose>;

/// A relative-motion constraint between two named frames.
#[derive(Debug, Clone)]
pub enum Mate {
    /// One rotational degree of freedom about a shared axis through a shared
    /// pivot point (the classic "hinge" / DMU `Revolute`).
    Revolute(Revolute),
    /// One translational degree of freedom along a shared axis with the
    /// orientation locked (a groove / DMU `Prismatic`).
    Prismatic(Prismatic),
    /// A shared pivot point, three rotational DOF free (a ball / DMU `Spherical`).
    Spherical(Spherical),
    /// Fully rigid relative pose (six DOF locked / DMU `Rigid`).
    Rigid(Rigid),
}

impl Mate {
    /// Number of scalar residuals this mate contributes to the solve.
    pub fn residual_len(&self) -> usize {
        match self {
            Mate::Revolute(m) => m.residual_len(),
            Mate::Prismatic(m) => m.residual_len(),
            Mate::Spherical(m) => m.residual_len(),
            Mate::Rigid(m) => m.residual_len(),
        }
    }

    /// The two referenced frame ids (for DOF/connectivity bookkeeping).
    pub fn frames(&self) -> (&str, &str) {
        match self {
            Mate::Revolute(m) => (&m.a, &m.b),
            Mate::Prismatic(m) => (&m.a, &m.b),
            Mate::Spherical(m) => (&m.a, &m.b),
            Mate::Rigid(m) => (&m.a, &m.b),
        }
    }

    /// Append this mate's residuals into `out`.
    pub fn push_residual(&self, poses: &PoseMap, out: &mut Vec<f64>) {
        let pa = pose_or_ground(poses, self.frames().0);
        let pb = pose_or_ground(poses, self.frames().1);
        match self {
            Mate::Revolute(m) => m.push_residual(&pa, &pb, out),
            Mate::Prismatic(m) => m.push_residual(&pa, &pb, out),
            Mate::Spherical(m) => m.push_residual(&pa, &pb, out),
            Mate::Rigid(m) => m.push_residual(&pa, &pb, out),
        }
    }
}

fn pose_or_ground(poses: &PoseMap, id: &str) -> Pose {
    if id == crate::GROUND {
        Pose::identity()
    } else {
        *poses
            .get(id)
            .unwrap_or_else(|| panic!("mate references an unknown frame id {id:?}"))
    }
}

/// Revolute (hinge): frame `a` shares a pivot point with frame `b` and its
/// axis stays aligned with `b`'s axis; one rotational DOF about the axis stays
/// free. Pivot and axis are given in each frame's local coordinates.
#[derive(Debug, Clone)]
pub struct Revolute {
    /// Movable frame id.
    pub a: String,
    /// Pivot point in `a`'s local frame.
    pub pivot_a: Vec3,
    /// Axis in `a`'s local frame.
    pub axis_a: Vec3,
    /// Reference frame id (may be [`crate::GROUND`] or a fixed base).
    pub b: String,
    /// Pivot point in `b`'s local frame.
    pub pivot_b: Vec3,
    /// Axis in `b`'s local frame.
    pub axis_b: Vec3,
}

impl Revolute {
    /// Build a revolute between `a` and `b` with the given local anchors.
    pub fn new(
        a: impl Into<String>,
        pivot_a: Vec3,
        axis_a: Vec3,
        b: impl Into<String>,
        pivot_b: Vec3,
        axis_b: Vec3,
    ) -> Self {
        Self {
            a: a.into(),
            pivot_a,
            axis_a,
            b: b.into(),
            pivot_b,
            axis_b,
        }
    }

    /// 6 residuals: pivot coincidence (3) + axis alignment (3).
    pub fn residual_len(&self) -> usize {
        6
    }

    fn push_residual(&self, pa: &Pose, pb: &Pose, out: &mut Vec<f64>) {
        let pivot_a_w = pa.transform_point(&self.pivot_a);
        let pivot_b_w = pb.transform_point(&self.pivot_b);
        let d = pivot_a_w.sub(&pivot_b_w);
        out.push(d.x);
        out.push(d.y);
        out.push(d.z);
        let axis_a_w = pa.transform_vector(&self.axis_a);
        let axis_b_w = pb.transform_vector(&self.axis_b);
        out.push(axis_a_w.x - axis_b_w.x);
        out.push(axis_a_w.y - axis_b_w.y);
        out.push(axis_a_w.z - axis_b_w.z);
    }
}

/// Prismatic (groove): frame `a` slides along an axis in frame `b`, with the
/// orientation locked to `b`. The axis is a line through `origin_b` with
/// direction `axis_b` (both in `b`'s local frame; `axis_b` is normalized at
/// construction).
#[derive(Debug, Clone)]
pub struct Prismatic {
    /// Movable frame id.
    pub a: String,
    /// Reference frame id (may be [`crate::GROUND`] or a fixed base).
    pub b: String,
    /// Slide direction in `b`'s local frame (unit).
    pub axis_b: Vec3,
    /// A point on the slide axis in `b`'s local frame.
    pub origin_b: Vec3,
    /// Two orthonormal unit vectors perpendicular to the slide axis, in `b`'s
    /// local frame; the across-translation residual projects onto them.
    perp_b: [Vec3; 2],
}

impl Prismatic {
    /// Build a prismatic joint for `a` sliding along the line through
    /// `origin_b` in direction `axis_b` of frame `b`.
    pub fn new(a: impl Into<String>, b: impl Into<String>, axis_b: Vec3, origin_b: Vec3) -> Self {
        let perp_b = perpendicular_basis(&axis_b);
        Self {
            a: a.into(),
            b: b.into(),
            axis_b,
            origin_b,
            perp_b,
        }
    }

    /// 5 residuals: orientation lock (3) + translation across the slide axis (2),
    /// leaving one translation DOF free along the axis.
    pub fn residual_len(&self) -> usize {
        5
    }

    fn push_residual(&self, pa: &Pose, pb: &Pose, out: &mut Vec<f64>) {
        // Orientation lock: relative rotation a->b must be identity.
        let rel = orientation_residual(&pa.orientation, &pb.orientation);
        out.push(rel.x);
        out.push(rel.y);
        out.push(rel.z);
        // Across-translation: the pivot separation, measured from the axis
        // anchor, must have zero component perpendicular to the slide axis.
        // Sliding along the axis leaves both across components zero, which is
        // the one remaining DOF.
        let anchor_w = pb.transform_point(&self.origin_b);
        let u_w = pb.transform_vector(&self.perp_b[0]);
        let v_w = pb.transform_vector(&self.perp_b[1]);
        let delta = pa.position.sub(&anchor_w);
        out.push(delta.dot(&u_w));
        out.push(delta.dot(&v_w));
    }
}

/// Two orthonormal unit vectors perpendicular to `axis`.
fn perpendicular_basis(axis: &Vec3) -> [Vec3; 2] {
    let a = axis.normalize().unwrap_or(Vec3::new(1.0, 0.0, 0.0));
    let seed = if a.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = a
        .cross(&seed)
        .normalize()
        .unwrap_or(Vec3::new(0.0, 1.0, 0.0));
    let v = a.cross(&u);
    [u, v]
}

/// Spherical (ball): a shared pivot point; three rotational DOF free.
#[derive(Debug, Clone)]
pub struct Spherical {
    /// Movable frame id.
    pub a: String,
    /// Pivot in `a`'s local frame.
    pub pivot_a: Vec3,
    /// Reference frame id.
    pub b: String,
    /// Pivot in `b`'s local frame.
    pub pivot_b: Vec3,
}

impl Spherical {
    /// Build a spherical joint between `a` and `b` at the given local pivots.
    pub fn new(a: impl Into<String>, pivot_a: Vec3, b: impl Into<String>, pivot_b: Vec3) -> Self {
        Self {
            a: a.into(),
            pivot_a,
            b: b.into(),
            pivot_b,
        }
    }

    /// 3 residuals (pivot coincidence).
    pub fn residual_len(&self) -> usize {
        3
    }

    fn push_residual(&self, pa: &Pose, pb: &Pose, out: &mut Vec<f64>) {
        let pivot_a_w = pa.transform_point(&self.pivot_a);
        let pivot_b_w = pb.transform_point(&self.pivot_b);
        let d = pivot_a_w.sub(&pivot_b_w);
        out.push(d.x);
        out.push(d.y);
        out.push(d.z);
    }
}

/// Rigid: frame `a` stays coincident with frame `b` (zero relative DOF).
#[derive(Debug, Clone)]
pub struct Rigid {
    /// Frame id being locked to `b`.
    pub a: String,
    /// Reference frame id.
    pub b: String,
}

impl Rigid {
    /// Build a rigid joint locking `a` to `b`.
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        Self {
            a: a.into(),
            b: b.into(),
        }
    }

    /// 6 residuals: orientation lock (3) + pivot coincidence (3).
    pub fn residual_len(&self) -> usize {
        6
    }

    fn push_residual(&self, pa: &Pose, pb: &Pose, out: &mut Vec<f64>) {
        let rel = orientation_residual(&pa.orientation, &pb.orientation);
        out.push(rel.x);
        out.push(rel.y);
        out.push(rel.z);
        let d = pa.position.sub(&pb.position);
        out.push(d.x);
        out.push(d.y);
        out.push(d.z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pose::rotvec_to_mat3;

    fn pose_at(x: f64, y: f64, z: f64) -> Pose {
        Pose {
            position: Vec3::new(x, y, z),
            orientation: rotvec_to_mat3(&Vec3::zero()),
        }
    }

    #[test]
    fn prismatic_leaves_axis_translation_free() {
        // A body sliding along world X on the groove line y=50, z=8 must have a
        // zero residual at any slide location, while a cross-axis offset must
        // show up as a (non-zero) across residual.
        let groove = Prismatic::new(
            "piston",
            crate::GROUND,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 50.0, 8.0),
        );
        // Ground reference = identity pose (b == GROUND).
        let ground = Pose::identity();
        for x in [100.0, 120.0, 140.0] {
            let mut r = Vec::new();
            groove.push_residual(&pose_at(x, 50.0, 8.0), &ground, &mut r);
            assert_eq!(r.len(), 5);
            for v in &r {
                assert!(v.abs() < 1e-9, "residual {v} at x={x}");
            }
        }
        // A piston lifted off the groove line must register an across residual.
        let mut r = Vec::new();
        groove.push_residual(&pose_at(120.0, 50.0, 30.0), &ground, &mut r);
        assert!(
            (r[3].abs() + r[4].abs()) > 1.0,
            "expected across residual, got {r:?}"
        );
    }

    #[test]
    fn revolute_pivot_residual() {
        // Two frames sharing a pivot at world (10, 20, 0) with Z axes aligned
        // must yield zero revolute residual; rotating one about Z keeps it zero.
        let hinge = Revolute::new(
            "a",
            Vec3::zero(),
            Vec3::new(0.0, 0.0, 1.0),
            "b",
            Vec3::zero(),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let rot = rotvec_to_mat3(&Vec3::new(0.0, 0.0, 0.7));
        let pose_a = Pose {
            position: Vec3::new(10.0, 20.0, 0.0),
            orientation: rot,
        };
        let pose_b = Pose {
            position: Vec3::new(10.0, 20.0, 0.0),
            orientation: oep_geo::math::Mat3::identity(),
        };
        let mut r = Vec::new();
        hinge.push_residual(&pose_a, &pose_b, &mut r);
        for v in r {
            assert!(v.abs() < 1e-9, "residual {v}");
        }
    }
}
