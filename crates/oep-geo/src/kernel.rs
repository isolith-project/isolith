//! P6-M3-T04 / ADR v1.2.0: the geometry-kernel seam.
//!
//! Every kernel-shaped operation on the OEP model lives here as a typed,
//! kernel-neutral function: exact B-Rep booleans ([`solid_boolean`]), fillet /
//! chamfer ([`solid_fillet`], [`solid_chamfer`]), primitive creation
//! ([`make_box`], [`make_cylinder`], [`make_sphere`]), and sweep profile
//! creation ([`solid_extrude`], [`solid_revolve`]). Callers name, construct,
//! and pattern-match ONLY the oep-geo types defined in this module; the engine
//! crate (`oep-engine-occ`, the OCCT binding) is an implementation detail
//! behind the `kernel` feature, and tolerance is passed by the caller, never
//! baked in (P6-M1-T03).
//!
//! Seam rules (ADR sections 5 and 9, enforced in CI by
//! `tests/test_kernel_seam_guardrails.py`):
//!   - No OCCT identifier (`TopoDS_`, `Handle_`, `BRepAlgo`, `BRepFilletAPI`,
//!     `BRepPrimAPI`, `gp_`) may appear in a public item of oep-geo. Everything
//!     here is plain oep-geo.
//!   - Failure is typed ([`BooleanError`], [`KernelError`]), never a panic.
//!   - With `build-engine`/OCCT unavailable the seam stays typed and returns
//!     [`KernelError::EngineUnavailable`] / [`BooleanError::EngineUnavailable`].

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

use crate::body::BoundingBox;

/// The three exact B-Rep boolean operations (union, difference, intersection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BooleanOp {
    /// Union of the operands (BRepAlgoAPI_Fuse behind the seam).
    Union,
    /// Operand 0 minus the rest (BRepAlgoAPI_Cut behind the seam).
    Difference,
    /// Common material of all operands (BRepAlgoAPI_Common behind the seam).
    Intersection,
}

/// The mirror plane normal for [`solid_mirror`]: one of the three model axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirrorAxis {
    /// Mirror about the plane normal to X at coordinate `at`.
    X,
    /// Mirror about the plane normal to Y at coordinate `at`.
    Y,
    /// Mirror about the plane normal to Z at coordinate `at`.
    Z,
}

impl MirrorAxis {
    /// The engine's plane-normal code (0 = X, 1 = Y, 2 = Z).
    pub(crate) fn code(self) -> i32 {
        match self {
            MirrorAxis::X => 0,
            MirrorAxis::Y => 1,
            MirrorAxis::Z => 2,
        }
    }
}

/// Caller-owned geometric tolerance (P6-M1-T03: tolerance lives in oep-geo,
/// never baked into the kernel). A finite, strictly positive length.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Tolerance(f64);

impl Tolerance {
    /// A validated tolerance; `None` for non-finite or non-positive values.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && value > 0.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// The raw length.
    pub fn get(&self) -> f64 {
        self.0
    }
}

impl Default for Tolerance {
    /// A conservative default linear tolerance for model-scale geometry.
    fn default() -> Self {
        Self(1e-6)
    }
}

/// An opaque handle to one kernel session. Constructed with [`sys_new`]; its
/// interior is a session id meaningful only to the engine behind the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysHandle {
    id: u64,
}

impl SysHandle {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

/// A reference to one edge of a kernel solid, used to name fillet/chamfer
/// edges. Edge indices are stable within a session for a given solid
/// (deterministic kernel topology order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeRef {
    solid: u64,
    index: u64,
}

impl EdgeRef {
    /// The session token of the owning solid.
    pub fn solid_token(&self) -> u64 {
        self.solid
    }

    /// The edge index within that solid.
    pub fn index(&self) -> u64 {
        self.index
    }
}

/// Measured properties of a solid, translated across the seam as plain
/// numbers: the quantities the acceptance contract asserts against.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolidMeasures {
    /// Material volume of the solid.
    pub volume: f64,
    /// Surface (boundary) area.
    pub surface_area: f64,
    /// Number of boundary faces.
    pub face_count: u64,
    /// Number of edges.
    pub edge_count: u64,
    /// Number of vertices.
    pub vertex_count: u64,
}

impl SolidMeasures {
    /// A zeroed measure set (for the engine-unavailable / degenerate path).
    pub fn empty() -> Self {
        Self {
            volume: 0.0,
            surface_area: 0.0,
            face_count: 0,
            edge_count: 0,
            vertex_count: 0,
        }
    }
}

/// A solid produced or consumed by the kernel seam.
///
/// The public fields are pure oep-geo (id + measured properties). The session
/// token inside is engine-local: it lets the same solid feed a later operation
/// in the same session (e.g. fillet a boolean result) without ever exposing a
/// kernel type here. Tokens are not meaningful across sessions (ADR section 5:
/// the engine is replaceable behind this unchanged interface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Solid {
    /// Document-useful identifier of the solid.
    pub id: String,
    /// Kernel-measured properties.
    pub measures: SolidMeasures,
    /// Engine-session-local reference (absent when created without a session).
    #[serde(skip)]
    token: Option<u64>,
}

impl Solid {
    pub(crate) fn token(&self) -> Option<u64> {
        self.token
    }
}

/// One vertex of a planar wire profile in its own 2D plane.
///
/// Sweep operations ([`solid_extrude`], [`solid_revolve`]) take a closed
/// profile loop as `&[Point2D]`; the polygon implicitly closes back to the
/// first point. The meaning of the two coordinates depends on the sweep:
///
/// - [`solid_extrude`]: the profile lies in the model Z = 0 plane (`x`, `y`
///   unchanged) and sweeps along +Z;
/// - [`solid_revolve`]: the profile is a meridian cross-section in the XZ
///   half-plane — `x` is the radial distance (>= 0) from the Z rotation axis
///   and `y` is the axial height — and the sweep rotates about the Z axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    /// Profile X coordinate / radial distance (finite, double-everywhere;
    /// non-negative for revolve).
    pub x: f64,
    /// Profile Y coordinate / axial height (finite, double-everywhere).
    pub y: f64,
}

impl Point2D {
    /// A raw profile vertex from its (x, y) coordinates.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Validate a sweep profile: at least three distinct, finite points forming a
/// closed loop in the Z = 0 plane with positive signed area.
fn validate_profile(profile: &[Point2D]) -> Result<(), KernelError> {
    if profile.len() < 3 {
        return Err(KernelError::InvalidArgument(format!(
            "profile needs at least 3 points, got {}",
            profile.len()
        )));
    }
    for (i, p) in profile.iter().enumerate() {
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(KernelError::InvalidArgument(format!(
                "profile point {i} is not finite"
            )));
        }
    }
    // Shoelace signed area: collapse the implicit closing edge. A zero (or
    // non-finite) area profile is degenerate for a sweep.
    let mut twice_area = 0.0f64;
    for i in 0..profile.len() {
        let a = profile[i];
        let b = profile[(i + 1) % profile.len()];
        twice_area += a.x * b.y - b.x * a.y;
    }
    if !twice_area.is_finite() || twice_area.abs() <= f64::EPSILON {
        return Err(KernelError::InvalidArgument(
            "profile has zero (degenerate) swept area".to_string(),
        ));
    }
    Ok(())
}

/// Typed failure of a boolean operation (ADR section 6: never a panic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanError {
    /// No kernel engine is linked (the `kernel` feature is off).
    EngineUnavailable,
    /// An operand was not a live kernel solid (missing session token).
    InvalidOperand(String),
    /// The boolean algorithm ran but reported not-done.
    AlgorithmNotDone(String),
    /// The boolean inputs/parameters were degenerate.
    Degenerate(String),
    /// The operation completed but produced no material.
    EmptyResult,
    /// The session or an operand shape was missing from the engine.
    MissingShape(String),
}

impl fmt::Display for BooleanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BooleanError::EngineUnavailable => {
                write!(f, "kernel engine unavailable (kernel feature off)")
            }
            BooleanError::InvalidOperand(m) => write!(f, "invalid boolean operand: {m}"),
            BooleanError::AlgorithmNotDone(m) => write!(f, "boolean algorithm not done: {m}"),
            BooleanError::Degenerate(m) => write!(f, "degenerate boolean input: {m}"),
            BooleanError::EmptyResult => write!(f, "boolean produced an empty result"),
            BooleanError::MissingShape(m) => write!(f, "missing kernel shape/session: {m}"),
        }
    }
}

impl Error for BooleanError {}

/// Typed failure of a kernel modeling operation (fillet, chamfer, primitive)
/// or of session management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// No kernel engine is linked (the `kernel` feature is off).
    EngineUnavailable,
    /// A caller-supplied argument was invalid.
    InvalidArgument(String),
    /// The kernel algorithm ran but reported not-done.
    AlgorithmNotDone(String),
    /// The parameters/geometry were degenerate.
    Degenerate(String),
    /// The operation produced no material.
    EmptyResult,
    /// The session or a shape was missing from the engine.
    MissingShape(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::EngineUnavailable => {
                write!(f, "kernel engine unavailable (kernel feature off)")
            }
            KernelError::InvalidArgument(m) => write!(f, "invalid kernel argument: {m}"),
            KernelError::AlgorithmNotDone(m) => write!(f, "kernel algorithm not done: {m}"),
            KernelError::Degenerate(m) => write!(f, "degenerate kernel parameter: {m}"),
            KernelError::EmptyResult => write!(f, "kernel operation produced an empty result"),
            KernelError::MissingShape(m) => write!(f, "missing kernel shape/session: {m}"),
        }
    }
}

impl Error for KernelError {}

// ---------------------------------------------------------------------------
// Engine path: real engine (feature `kernel`) or typed stub.
// ---------------------------------------------------------------------------

/// A kernel-produced shape plus its measured properties, translated out of the
/// engine so the seam can hand oep-geo a plain [`Solid`].
struct Measured {
    token: u64,
    measures: SolidMeasures,
}

fn with_token(token: u64, measures: SolidMeasures, sys: &SysHandle) -> Solid {
    Solid {
        id: format!("oep-solid-{}-{}", sys.id(), token),
        measures,
        token: Some(token),
    }
}

fn require_token(solid: &Solid) -> Result<u64, KernelError> {
    solid
        .token()
        .ok_or_else(|| KernelError::InvalidArgument("solid has no kernel token".to_string()))
}

#[cfg(feature = "kernel")]
fn to_measures(info: oep_engine_occ_iface::ShapeInfo) -> SolidMeasures {
    SolidMeasures {
        volume: info.volume,
        surface_area: info.surface_area,
        face_count: info.face_count,
        edge_count: info.edge_count,
        vertex_count: info.vertex_count,
    }
}

/// Feature-independent engine interface so the public seam functions below are
/// identical whether or not the real engine is linked.
mod engine_iface {
    use super::{BooleanError, KernelError, SysHandle};

    /// Allocate a fresh session id.
    pub fn sys_new() -> Result<u64, KernelError> {
        #[cfg(feature = "kernel")]
        {
            oep_engine_occ::occ_sys_new().map_err(super::kernel_err)
        }
        #[cfg(not(feature = "kernel"))]
        {
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn sys_drop(sys: &SysHandle) {
        #[cfg(feature = "kernel")]
        oep_engine_occ::occ_sys_drop(sys.id());
        #[cfg(not(feature = "kernel"))]
        let _ = sys;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn make_box(
        sys: &SysHandle,
        x0: f64,
        y0: f64,
        z0: f64,
        x1: f64,
        y1: f64,
        z1: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_make_box(sys.id(), x0, y0, z0, x1, y1, z1, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, x0, y0, z0, x1, y1, z1, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn make_cylinder(
        sys: &SysHandle,
        radius: f64,
        height: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_make_cylinder(sys.id(), radius, height, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, radius, height, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn make_sphere(
        sys: &SysHandle,
        radius: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_make_sphere(sys.id(), radius, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, radius, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn boolean(
        sys: &SysHandle,
        op: super::BooleanOp,
        a: u64,
        b: u64,
        tol: f64,
    ) -> Result<super::Measured, BooleanError> {
        #[cfg(feature = "kernel")]
        {
            let code = match op {
                super::BooleanOp::Union => oep_engine_occ::OP_UNION,
                super::BooleanOp::Difference => oep_engine_occ::OP_DIFFERENCE,
                super::BooleanOp::Intersection => oep_engine_occ::OP_COMMON,
            };
            let token = oep_engine_occ::occ_boolean(sys.id(), code, a, b, tol)
                .map_err(super::boolean_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::boolean_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, op, a, b, tol);
            Err(BooleanError::EngineUnavailable)
        }
    }

    pub fn edges(sys: &SysHandle, subject: u64) -> Result<Vec<u64>, KernelError> {
        #[cfg(feature = "kernel")]
        {
            oep_engine_occ::occ_edges(sys.id(), subject).map_err(super::kernel_err)
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn fillet(
        sys: &SysHandle,
        subject: u64,
        edge_indices: &[u64],
        radius: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_fillet(sys.id(), subject, edge_indices, radius, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject, edge_indices, radius, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn chamfer(
        sys: &SysHandle,
        subject: u64,
        edge_indices: &[u64],
        distance: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_chamfer(sys.id(), subject, edge_indices, distance, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject, edge_indices, distance, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn extrude(
        sys: &SysHandle,
        profile: &[super::Point2D],
        height: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let coords: Vec<f64> = profile.iter().flat_map(|p| [p.x, p.y]).collect();
            let token = oep_engine_occ::occ_extrude(sys.id(), &coords, height, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, profile, height, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    pub fn revolve(
        sys: &SysHandle,
        profile: &[super::Point2D],
        angle: f64,
        tol: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let coords: Vec<f64> = profile.iter().flat_map(|p| [p.x, p.y]).collect();
            let token = oep_engine_occ::occ_revolve(sys.id(), &coords, angle, tol)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, profile, angle, tol);
            Err(KernelError::EngineUnavailable)
        }
    }

    /// Axis-aligned bounding box of a registered shape as `[xmin, ymin, zmin,
    /// xmax, ymax, zmax]`.
    pub fn bbox(sys: &SysHandle, subject: u64) -> Result<[f64; 6], KernelError> {
        #[cfg(feature = "kernel")]
        {
            let (x0, y0, z0, x1, y1, z1) =
                oep_engine_occ::occ_bbox(sys.id(), subject).map_err(super::kernel_err)?;
            Ok([x0, y0, z0, x1, y1, z1])
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject);
            Err(KernelError::EngineUnavailable)
        }
    }

    /// Rigid translation of a registered shape by `(dx, dy, dz)` (new token).
    pub fn translate(
        sys: &SysHandle,
        subject: u64,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_translate(sys.id(), subject, dx, dy, dz)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject, dx, dy, dz);
            Err(KernelError::EngineUnavailable)
        }
    }

    /// Mirror a registered shape about an axis-aligned plane (axis code 0/1/2,
    /// offset `at` along that axis); returns the new token.
    pub fn mirror(
        sys: &SysHandle,
        subject: u64,
        axis: i32,
        at: f64,
    ) -> Result<super::Measured, KernelError> {
        #[cfg(feature = "kernel")]
        {
            let token = oep_engine_occ::occ_mirror_plane(sys.id(), subject, axis, at)
                .map_err(super::kernel_err)?;
            let info = oep_engine_occ::occ_info(sys.id(), token).map_err(super::kernel_err)?;
            Ok(super::Measured {
                token,
                measures: super::to_measures(info),
            })
        }
        #[cfg(not(feature = "kernel"))]
        {
            let _ = (sys, subject, axis, at);
            Err(KernelError::EngineUnavailable)
        }
    }
}

/// Map an engine error to a [`KernelError`] (typed across the seam).
#[cfg(feature = "kernel")]
pub(crate) fn kernel_err(e: oep_engine_occ_iface::EngineError) -> KernelError {
    use oep_engine_occ_iface::EngineError as E;
    match e {
        E::Unavailable => KernelError::EngineUnavailable,
        E::InvalidArgument(m) => KernelError::InvalidArgument(m),
        E::AlgorithmNotDone(m) => KernelError::AlgorithmNotDone(m),
        E::Degenerate(m) => KernelError::Degenerate(m),
        E::EmptyResult => KernelError::EmptyResult,
        E::MissingSession(m) => KernelError::MissingShape(m),
    }
}

/// Map an engine error to a [`BooleanError`] (typed across the seam).
#[cfg(feature = "kernel")]
pub(crate) fn boolean_err(e: oep_engine_occ_iface::EngineError) -> BooleanError {
    use oep_engine_occ_iface::EngineError as E;
    match e {
        E::Unavailable => BooleanError::EngineUnavailable,
        E::InvalidArgument(m) => BooleanError::InvalidOperand(m),
        E::AlgorithmNotDone(m) => BooleanError::AlgorithmNotDone(m),
        E::Degenerate(m) => BooleanError::Degenerate(m),
        E::EmptyResult => BooleanError::EmptyResult,
        E::MissingSession(m) => BooleanError::MissingShape(m),
    }
}

// ---------------------------------------------------------------------------
// Public seam (feature-independent).
// ---------------------------------------------------------------------------

/// Open a new kernel session and return its handle. Without the `kernel`
/// feature the seam stays typed and returns [`KernelError::EngineUnavailable`].
pub fn sys_new() -> Result<SysHandle, KernelError> {
    let id = engine_iface::sys_new()?;
    Ok(SysHandle { id })
}

/// Close a kernel session, releasing its shapes. No-op on a dropped session.
pub fn sys_drop(sys: &SysHandle) {
    engine_iface::sys_drop(sys);
}

/// Create an axis-aligned box solid from corner `(x0,y0,z0)` to corner
/// `(x1,y1,z1)`.
#[allow(clippy::too_many_arguments)]
pub fn make_box(
    sys: &SysHandle,
    x0: f64,
    y0: f64,
    z0: f64,
    x1: f64,
    y1: f64,
    z1: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    let m = engine_iface::make_box(sys, x0, y0, z0, x1, y1, z1, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Create a cylinder solid (axis Z, base at z=0) of the given radius and height.
pub fn make_cylinder(
    sys: &SysHandle,
    radius: f64,
    height: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    let m = engine_iface::make_cylinder(sys, radius, height, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Create a sphere solid centered at the origin of the given radius.
pub fn make_sphere(
    sys: &SysHandle,
    radius: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    let m = engine_iface::make_sphere(sys, radius, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Exact B-Rep boolean over two or more solids (left-folded), returning the
/// resulting solid's measures and a token for chaining in the same session.
pub fn solid_boolean(
    op: BooleanOp,
    sys: &SysHandle,
    operands: &[Solid],
    tolerance: Tolerance,
) -> Result<Solid, BooleanError> {
    if operands.len() < 2 {
        return Err(BooleanError::InvalidOperand(format!(
            "need at least two operands, got {}",
            operands.len()
        )));
    }
    let mut acc = require_token_boolean(&operands[0])?;
    for operand in &operands[1..] {
        let other = require_token_boolean(operand)?;
        let m = engine_iface::boolean(sys, op, acc, other, tolerance.get())?;
        acc = m.token;
    }
    // Final result token is the last fold's token; re-measure it.
    let info = engine_iface_info(sys, acc)?;
    Ok(with_token(acc, info, sys))
}

/// List the edges of a solid as [`EdgeRef`]s, suitable for fillet/chamfer.
pub fn solid_edges(
    sys: &SysHandle,
    subject: &Solid,
    _tolerance: Tolerance,
) -> Result<Vec<EdgeRef>, KernelError> {
    let token = require_token(subject)?;
    let idx = engine_iface::edges(sys, token)?;
    Ok(idx
        .into_iter()
        .map(|i| EdgeRef {
            solid: token,
            index: i,
        })
        .collect())
}

/// Fillet the selected edges of a solid by `radius`; returns the new solid.
pub fn solid_fillet(
    sys: &SysHandle,
    edge_refs: &[EdgeRef],
    radius: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    if edge_refs.is_empty() {
        return Err(KernelError::InvalidArgument(
            "fillet needs at least one edge".to_string(),
        ));
    }
    let subject = edge_refs[0].solid;
    if edge_refs.iter().any(|e| e.solid != subject) {
        return Err(KernelError::InvalidArgument(
            "all fillet edges must belong to the same solid".to_string(),
        ));
    }
    let indices: Vec<u64> = edge_refs.iter().map(|e| e.index).collect();
    let m = engine_iface::fillet(sys, subject, &indices, radius, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Chamfer the selected edges of a solid by symmetric `distance`; returns the
/// new solid.
pub fn solid_chamfer(
    sys: &SysHandle,
    edge_refs: &[EdgeRef],
    distance: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    if edge_refs.is_empty() {
        return Err(KernelError::InvalidArgument(
            "chamfer needs at least one edge".to_string(),
        ));
    }
    let subject = edge_refs[0].solid;
    if edge_refs.iter().any(|e| e.solid != subject) {
        return Err(KernelError::InvalidArgument(
            "all chamfer edges must belong to the same solid".to_string(),
        ));
    }
    let indices: Vec<u64> = edge_refs.iter().map(|e| e.index).collect();
    let m = engine_iface::chamfer(sys, subject, &indices, distance, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Extrude a closed planar profile (XY, Z = 0) by `height` along +Z into a
/// new solid. `height` must be finite and strictly positive. The profile must
/// have at least three finite points with non-zero swept area; the loop closes
/// implicitly back to the first point (ADR v1.2.0 adds the sweep to the seam).
pub fn solid_extrude(
    sys: &SysHandle,
    profile: &[Point2D],
    height: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    validate_profile(profile)?;
    if !height.is_finite() || height <= 0.0 {
        return Err(KernelError::InvalidArgument(format!(
            "extrude height must be finite and positive, got {height}"
        )));
    }
    let m = engine_iface::extrude(sys, profile, height, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Revolve a meridian profile about the Z axis by `angle` radians into a new
/// solid. `angle` must be finite and strictly positive (a full turn is 2π).
/// The profile loop is a cross-section in the XZ half-plane: each [`Point2D`]
/// carries (radial distance from the axis, axial height); the loop closes
/// implicitly and must stay on the non-negative radial side (ADR v1.2.0 adds
/// the sweep to the seam).
pub fn solid_revolve(
    sys: &SysHandle,
    profile: &[Point2D],
    angle: f64,
    tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    validate_profile(profile)?;
    if !angle.is_finite() || angle <= 0.0 {
        return Err(KernelError::InvalidArgument(format!(
            "revolve angle must be finite and positive, got {angle}"
        )));
    }
    // Radial distance cannot be negative (the meridian must stay in the
    // non-negative XZ half-plane; the kernel rejects a loop that touches the
    // axis for a full turn, as a typed error).
    if profile.iter().any(|p| p.x < 0.0) {
        return Err(KernelError::InvalidArgument(
            "revolve profile crosses the negative radial side".to_string(),
        ));
    }
    let m = engine_iface::revolve(sys, profile, angle, tolerance.get())?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Measure the axis-aligned bounding box of a solid as a closed corner box
/// (`BRepBndLib`/`Bnd_Box` behind the seam). Shapes have exact planar extents;
/// the caller-owned tolerance is carried for seam-signature consistency.
pub fn solid_bbox(
    sys: &SysHandle,
    subject: &Solid,
    _tolerance: Tolerance,
) -> Result<BoundingBox, KernelError> {
    let token = require_token(subject)?;
    let [x0, y0, z0, x1, y1, z1] = engine_iface::bbox(sys, token)?;
    Ok(BoundingBox::new([x0, y0, z0], [x1, y1, z1]))
}

/// Rigidly translate a solid by `(dx, dy, dz)` (by copy; the subject is
/// untouched) into a new solid in the same session. The caller-owned
/// tolerance is carried for seam-signature consistency (translation is a
/// rigid placement, so the engine needs no fuzzy tolerance here).
pub fn solid_translate(
    sys: &SysHandle,
    subject: &Solid,
    dx: f64,
    dy: f64,
    dz: f64,
    _tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    if ![dx, dy, dz].iter().all(|v| v.is_finite()) {
        return Err(KernelError::InvalidArgument(
            "translate vector must be finite".to_string(),
        ));
    }
    let token = require_token(subject)?;
    let m = engine_iface::translate(sys, token, dx, dy, dz)?;
    Ok(with_token(m.token, m.measures, sys))
}

/// Mirror a solid about an axis-aligned plane (by copy): the plane whose
/// normal is the chosen [`MirrorAxis`] passes through coordinate `at` along
/// that axis. Returns the mirrored solid in the same session. The caller-owned
/// tolerance is carried for seam-signature consistency (mirror is a rigid
/// placement, so the engine needs no fuzzy tolerance here).
pub fn solid_mirror(
    sys: &SysHandle,
    subject: &Solid,
    axis: MirrorAxis,
    at: f64,
    _tolerance: Tolerance,
) -> Result<Solid, KernelError> {
    if !at.is_finite() {
        return Err(KernelError::InvalidArgument(
            "mirror plane offset must be finite".to_string(),
        ));
    }
    let token = require_token(subject)?;
    let m = engine_iface::mirror(sys, token, axis.code(), at)?;
    Ok(with_token(m.token, m.measures, sys))
}

fn require_token_boolean(solid: &Solid) -> Result<u64, BooleanError> {
    solid.token().ok_or_else(|| {
        BooleanError::InvalidOperand(format!("solid {:?} has no kernel token", solid.id))
    })
}

fn engine_iface_info(sys: &SysHandle, token: u64) -> Result<SolidMeasures, BooleanError> {
    #[cfg(feature = "kernel")]
    {
        let info = oep_engine_occ::occ_info(sys.id(), token).map_err(boolean_err)?;
        Ok(to_measures(info))
    }
    #[cfg(not(feature = "kernel"))]
    {
        let _ = (sys, token);
        Err(BooleanError::EngineUnavailable)
    }
}

// When the kernel feature is on this module re-exports the engine's plain
// result types so the seam's error mappers and measure converter type-check
// against them without naming the engine crate all over the surface.
#[cfg(feature = "kernel")]
mod oep_engine_occ_iface {
    pub use oep_engine_occ::{EngineError, ShapeInfo};
}
