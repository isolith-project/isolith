//! The solver seam: a small, safe wrapper over the OEP permissive solver.
//!
//! SOLVER-REPLACE (program decision #16, `workstreams/P7-M3/solver-adr.md` v1.2.0)
//! replaced the vendored GPL-3.0 SolveSpace engine with an **own bounded
//! solver** behind this vendor-neutral seam. Everything above this module is
//! the OEP assembly workbench; the engine below is the damped Gauss-Newton
//! least-squares lane owned by `oep-kinematics` (P7-M3-T02), reused as the
//! ADR's designated fallback route. The crate no longer contains any
//! SolveSpace code, FFI, or vendored GPL tree.
//!
//! The seam's contract is unchanged from the adoption era: the workbench
//! builds a solver request (initial unknowns plus the residual function that
//! encodes the mate set), calls [`solve`], and reconstructs placements from
//! the converged unknowns. The engine is replaceable behind this boundary.

use crate::transform_math as tm;

/// Result codes the solver reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverStatus {
    /// Constraints satisfied.
    Okay,
    /// Constraints satisfied but some were redundant.
    RedundantOkay,
    /// No solution satisfies the constraints.
    Inconsistent,
    /// Iteration did not converge.
    DidntConverge,
    /// Too many unknowns.
    TooManyUnknowns,
    /// Any other/unknown result code.
    Unknown(i32),
}

impl SolverStatus {
    /// True when the solve produced a satisfied solution we can trust.
    pub fn is_success(self) -> bool {
        matches!(self, SolverStatus::Okay | SolverStatus::RedundantOkay)
    }
}

/// The residual mapping a workbench solve builds: `F: R^n -> R^m` from the
/// flat unknown vector to the mate residual vector.
pub type ResidualFn = Box<dyn FnMut(&[f64]) -> Vec<f64>>;

/// One solve request to the engine: the initial unknowns and the residual
/// function the workbench built from the mate set.
pub struct SolverRequest {
    /// Initial values of the unknowns. Layout is the workbench's contract:
    /// per moving instance, `[x, y, z, qw, qx, qy, qz]`.
    pub unknowns: Vec<f64>,
    /// The residual function `F: R^n -> R^m`; the solver minimizes `||F||^2`.
    /// The solver never stores or re-enters this box after `solve` returns.
    pub residual: ResidualFn,
}

impl SolverRequest {
    pub fn new(unknowns: Vec<f64>, residual: ResidualFn) -> Self {
        Self { unknowns, residual }
    }
}

/// The outcome of a solve.
#[derive(Debug, Clone)]
pub struct SolverOutcome {
    pub status: SolverStatus,
    /// Unconstrained degrees of freedom (unknowns minus Jacobian rank).
    pub dof: i32,
    /// Unknowns after the solve (in/out: solved values written back here).
    pub params: Vec<f64>,
}

/// Run the solver on `request`. Never panics on a solver failure; the outcome
/// carries the result code.
///
/// The engine is the program's own damped Gauss-Newton least-squares solver
/// from `oep-kinematics` (P7-M3-T02): finite-difference Jacobian, normal
/// equations, Levenberg damping, rank-based free-DOF report. A converged
/// solve with redundant constraint rows reports [`SolverStatus::RedundantOkay`]
/// (same "both count as success" semantics the workbench has always used).
pub fn solve(request: SolverRequest) -> SolverOutcome {
    use oep_kinematics::solver::{solve_least_squares, Solver};

    let n = request.unknowns.len();
    let mut residual = request.residual;
    let m = residual(&request.unknowns).len();

    let cfg = Solver {
        max_iterations: 1000,
        tol: 1e-15,
        lambda0: 1e-4,
    };
    let (x, report) = solve_least_squares(&mut residual, &request.unknowns, &cfg);

    let status = if report.converged {
        let rank = n.saturating_sub(report.free_dof);
        if rank < m {
            SolverStatus::RedundantOkay
        } else {
            SolverStatus::Okay
        }
    } else {
        // A bounded least-squares solve either converges or stalls; a
        // contradictory mate set lands here, so report it as inconsistent
        // when the residual is large, otherwise as a non-convergence.
        if report.max_residual > 1e-6 {
            SolverStatus::Inconsistent
        } else {
            SolverStatus::DidntConverge
        }
    };

    SolverOutcome {
        status,
        dof: report.free_dof as i32,
        params: x,
    }
}

/// Build the quaternion of the frame whose U and V basis vectors are given.
/// The result is normalized and uses the `[w, x, y, z]` convention shared
/// with [`tm::quat_to_matrix`]. For the identity frame this returns exactly
/// `[1, 0, 0, 0]`.
pub fn make_quaternion(u: [f64; 3], v: [f64; 3]) -> [f64; 4] {
    let u = tm::V3::from(u).normalized();
    let v = tm::V3::from(v);
    // Orthonormal right-handed frame: n = u x v, then re-orthogonalize v.
    let n = u.cross(v).normalized();
    let v = n.cross(u).normalized();
    rotation_to_quat(u, v, n)
}

/// Convert an orthonormal right-handed frame (columns `u`, `v`, `n`, the
/// images of the local +X/+Y/+Z axes) into a `[w, x, y, z]` quaternion.
/// Standard Shepperd-style branch selection so large-rotation cases stay
/// numerically well conditioned.
fn rotation_to_quat(u: tm::V3, v: tm::V3, n: tm::V3) -> [f64; 4] {
    // Rotation matrix, row-major:
    //   R = [u.x v.x n.x; u.y v.y n.y; u.z v.z n.z]
    let (r00, r01, r02) = (u.x, v.x, n.x);
    let (r10, r11, r12) = (u.y, v.y, n.y);
    let (r20, r21, r22) = (u.z, v.z, n.z);
    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        [0.25 / s, (r21 - r12) * s, (r02 - r20) * s, (r10 - r01) * s]
    } else if r00 > r11 && r00 > r22 {
        let s = 2.0 * (1.0 + r00 - r11 - r22).max(0.0).sqrt();
        let s = s.recip();
        [(r21 - r12) * s, 0.25 / s, (r01 + r10) * s, (r02 + r20) * s]
    } else if r11 > r22 {
        let s = 2.0 * (1.0 + r11 - r00 - r22).max(0.0).sqrt();
        let s = s.recip();
        [(r02 - r20) * s, (r01 + r10) * s, 0.25 / s, (r12 + r21) * s]
    } else {
        let s = 2.0 * (1.0 + r22 - r00 - r11).max(0.0).sqrt();
        let s = s.recip();
        [(r10 - r01) * s, (r02 + r20) * s, (r12 + r21) * s, 0.25 / s]
    }
}

/// Unit vector U (the image of the local +X axis) of a `[w, x, y, z]`
/// quaternion's rotation.
pub fn quaternion_u(q: [f64; 4]) -> [f64; 3] {
    let m = tm::quat_to_matrix(q);
    [m[0], m[3], m[6]]
}

/// Unit vector V (the image of the local +Y axis) of a `[w, x, y, z]`
/// quaternion's rotation.
pub fn quaternion_v(q: [f64; 4]) -> [f64; 3] {
    let m = tm::quat_to_matrix(q);
    [m[1], m[4], m[7]]
}

/// Unit vector N (the image of the local +Z axis) of a `[w, x, y, z]`
/// quaternion's rotation.
pub fn quaternion_n(q: [f64; 4]) -> [f64; 3] {
    let m = tm::quat_to_matrix(q);
    [m[2], m[5], m[8]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert!(SolverStatus::Okay.is_success());
        assert!(SolverStatus::RedundantOkay.is_success());
        assert!(!SolverStatus::Inconsistent.is_success());
        assert!(!SolverStatus::DidntConverge.is_success());
        assert!(!SolverStatus::TooManyUnknowns.is_success());
        assert!(!SolverStatus::Unknown(99).is_success());
    }

    #[test]
    fn quaternion_helpers_are_consistent() {
        // make_quaternion with the standard basis gives the identity quaternion.
        let q = make_quaternion([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(q, [1.0, 0.0, 0.0, 0.0]);
        let n = quaternion_n(q);
        assert!((n[0] - 0.0).abs() < 1e-12);
        assert!((n[1] - 0.0).abs() < 1e-12);
        assert!((n[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn make_quaternion_25deg_about_x() {
        // Rotating +Z by 25 degrees about +X; feed the images of X and Y
        // under that rotation as the basis.
        let th = 25.0_f64.to_radians();
        let (co, si) = (th.cos(), th.sin());
        let u = [1.0, 0.0, 0.0];
        let v = [0.0, co, si];
        let q = make_quaternion(u, v);
        // q = (cos(th/2), sin(th/2), 0, 0).
        assert!((q[0] - (th / 2.0).cos()).abs() < 1e-9, "q={q:?}");
        assert!((q[1] - (th / 2.0).sin()).abs() < 1e-9, "q={q:?}");
        assert!(q[2].abs() < 1e-9 && q[3].abs() < 1e-9, "q={q:?}");
        // The +Z image under a +25 deg rotation about X is (0, -sin, cos).
        let n = quaternion_n(q);
        assert!((n[0] - 0.0).abs() < 1e-9, "n={n:?}");
        assert!((n[1] + si).abs() < 1e-9, "n={n:?}");
        assert!((n[2] - co).abs() < 1e-9, "n={n:?}");
    }

    #[test]
    fn make_quaternion_half_turn_about_z() {
        // 180 degrees about +Z: u -> -X, v -> -Y, n -> +Z.
        let q = make_quaternion([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]);
        assert!((q[3] - 1.0).abs() < 1e-9, "q={q:?}");
        assert!(
            q[0].abs() < 1e-9 && q[1].abs() < 1e-9 && q[2].abs() < 1e-9,
            "q={q:?}"
        );
    }

    #[test]
    fn empty_request_trivially_succeeds() {
        // An empty system has nothing to satisfy; the engine reports OKAY
        // with zero degrees of freedom. The workbench guards real misuse.
        let req = SolverRequest::new(Vec::new(), Box::new(|_| Vec::new()));
        let out = solve(req);
        assert!(out.status.is_success());
        assert_eq!(out.dof, 0);
    }

    #[test]
    fn bounded_solve_finds_plane_intersection() {
        // Solve x = (a, b) with residuals [a-1, b-2] from a bad start.
        let req = SolverRequest::new(
            vec![-3.0, 5.0],
            Box::new(|x: &[f64]| vec![x[0] - 1.0, x[1] - 2.0]),
        );
        let out = solve(req);
        assert!(out.status.is_success(), "{out:?}");
        assert!((out.params[0] - 1.0).abs() < 1e-9, "{out:?}");
        assert!((out.params[1] - 2.0).abs() < 1e-9, "{out:?}");
    }

    #[test]
    fn params_roundtrip_identity() {
        let p = vec![1.0, 2.0, 3.0];
        let req = SolverRequest::new(p.clone(), Box::new(|x: &[f64]| vec![x[0] - 1.0]));
        let out = solve(req);
        assert_eq!(out.params.len(), 3);
        assert_eq!(out.params[0], 1.0);
    }
}
