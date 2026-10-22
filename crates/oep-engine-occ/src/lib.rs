//! oep-engine-occ: the Open CASCADE Technology (OCCT) 8.0.x geometry-kernel
//! binding behind the oep-geo seam (P6-M3-T04 / ADR v1.2.0).
//!
//! Scope (three operation families behind one seam):
//!   - exact B-Rep booleans: union / difference / intersection
//!     (BRepAlgoAPI_Fuse / Cut / Common)          -- see [`occ_boolean`]
//!   - kernel modeling ops: fillet, chamfer, and primitive creation box /
//!     cylinder / sphere (BRepFilletAPI, BRepPrimAPI) -- see [`occ_fillet`],
//!     [`occ_chamfer`], [`occ_make_box`], [`occ_make_cylinder`],
//!     [`occ_make_sphere`].
//!   - sweeps of a planar profile: extrude and revolve (BRepPrimAPI_MakePrism
//!     / MakeRevol) -- see [`occ_extrude`], [`occ_revolve`].
//!
//! Everything OCCT-shaped is private to this crate and its C++ glue
//! (`cpp/glue.cc`, the only TU that includes OCCT headers). The public surface
//! is plain Rust: session ids and shape tokens are `u64`, results come back as
//! [`ShapeInfo`], failures as typed [`EngineError`]s (`IsDone()`/status mapped
//! across the seam per ADR section 6). oep-geo is the only consumer of this
//! surface, reached through its own typed kernel module, so no OCCT identifier
//! ever appears in oep-geo public items (seam denial, ADR section 9).
//!
//! Version pin + build (ADR section 9): the pinned line is 8.0.x
//! (`OEP_OCCT_VERSION`, default `8.0.1`); the source is fetched + built as a
//! dependency by `scripts/kernel/{fetch,build}-occt.sh` -- nothing is vendored
//! into this repo. The crate's Rust surface compiles WITHOUT the `build-engine`
//! feature (the workspace default): every call then returns
//! [`EngineError::Unavailable`], so the workspace builds on hosts with no
//! C++/OCCT toolchain, while the kernel acceptance job (`oep-geo` feature
//! `kernel`, which enables `build-engine`) links the real OCCT.
//!
//! License/clean-room (ADR sections 7-8): OCCT is LGPL-2.1 + OCCT Exception
//! 1.0, kept as a separate dependency so crates we author stay Apache-2.0; the
//! P0-M2 legal pass writes the required OCCT notice before any release. This
//! crate binds the public OCCT API only -- it never uses OCCT's importer
//! toolbox as a back door to vendor-format reading.
#![deny(missing_docs)]

use std::error::Error;
use std::fmt;

/// Boolean operation code: union (BRepAlgoAPI_Fuse).
pub const OP_UNION: u8 = 0;
/// Boolean operation code: difference, a minus b (BRepAlgoAPI_Cut).
pub const OP_DIFFERENCE: u8 = 1;
/// Boolean operation code: intersection (BRepAlgoAPI_Common).
pub const OP_COMMON: u8 = 2;

/// Typed failure reported across the kernel seam. Never a panic: OCCT
/// `IsDone()`/error status is mapped here (ADR section 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The `build-engine` feature is off; the stub surface is active. No
    /// kernel engine is linked, so no operation can run.
    Unavailable,
    /// A caller-supplied argument was invalid (bad op code, out-of-range edge
    /// index, empty operand, non-solid subject, ...).
    InvalidArgument(String),
    /// The kernel algorithm ran but reported not-done (OCCT `IsDone()` false).
    AlgorithmNotDone(String),
    /// A degenerate parameter or result (zero radius / zero size, ...).
    Degenerate(String),
    /// The operation completed but yielded an empty shape (e.g. a difference
    /// whose tool fully contains the base solid).
    EmptyResult,
    /// A session id or shape token is not present in the engine registry.
    MissingSession(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Unavailable => {
                write!(f, "kernel engine unavailable (build-engine feature off)")
            }
            EngineError::InvalidArgument(m) => write!(f, "invalid kernel argument: {m}"),
            EngineError::AlgorithmNotDone(m) => write!(f, "kernel algorithm not done: {m}"),
            EngineError::Degenerate(m) => write!(f, "degenerate kernel parameter/result: {m}"),
            EngineError::EmptyResult => write!(f, "kernel operation produced an empty result"),
            EngineError::MissingSession(m) => write!(f, "missing kernel session/shape: {m}"),
        }
    }
}

impl Error for EngineError {}

/// Measured properties of a kernel shape, translated out of OCCT
/// (BRepGProp + topology counts) so the seam hands oep-geo plain numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeInfo {
    /// Volume of the solid (`BRepGProp::VolumeProperties(...).Mass()`).
    pub volume: f64,
    /// Surface area (`BRepGProp::SurfaceProperties(...).Mass()`).
    pub surface_area: f64,
    /// Number of faces (`TopAbs_FACE`).
    pub face_count: u64,
    /// Number of edges (`TopAbs_EDGE`).
    pub edge_count: u64,
    /// Number of vertices (`TopAbs_VERTEX`).
    pub vertex_count: u64,
}

/// Map a C++ status code to a typed [`EngineError`] (ADR section 6).
#[cfg(feature = "build-engine")]
fn code_to_error(code: i32, message: &str) -> EngineError {
    match code {
        1 => EngineError::InvalidArgument(message.to_string()),
        2 => EngineError::AlgorithmNotDone(message.to_string()),
        3 => EngineError::Degenerate(message.to_string()),
        4 => EngineError::EmptyResult,
        _ => EngineError::MissingSession(message.to_string()),
    }
}

// --- cxx bridge (only compiled when the real engine is requested) -------------

// FFI signatures follow the C++ glue; the 8-argument primitive builders are
// the bridge's natural shape and not a Rust design smell.
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "build-engine")]
#[cxx::bridge(namespace = "oep::occ")]
mod ffi {
    struct OpOut {
        ok: bool,
        code: i32,
        token: u64,
        message: String,
    }
    struct InfoOut {
        ok: bool,
        code: i32,
        volume: f64,
        area: f64,
        nfaces: u64,
        nedges: u64,
        nverts: u64,
        message: String,
    }
    struct EdgesOut {
        ok: bool,
        code: i32,
        edges: Vec<u64>,
        message: String,
    }
    struct BoxOut {
        ok: bool,
        code: i32,
        xmin: f64,
        ymin: f64,
        zmin: f64,
        xmax: f64,
        ymax: f64,
        zmax: f64,
        message: String,
    }
    unsafe extern "C++" {
        include!("cpp/glue.hh");

        fn occNewSession() -> u64;
        fn occDropSession(s: u64);

        fn occMakeBox(
            s: u64,
            x0: f64,
            y0: f64,
            z0: f64,
            x1: f64,
            y1: f64,
            z1: f64,
            tol: f64,
        ) -> OpOut;
        fn occMakeCylinder(s: u64, radius: f64, height: f64, tol: f64) -> OpOut;
        fn occMakeSphere(s: u64, radius: f64, tol: f64) -> OpOut;
        fn occBoolean(s: u64, op: i32, a: u64, b: u64, tol: f64) -> OpOut;
        fn occFilletChamfer(
            s: u64,
            mode: i32,
            subject: u64,
            edges: Vec<u64>,
            amount: f64,
            tol: f64,
        ) -> OpOut;
        fn occExtrude(s: u64, xy: Vec<f64>, height: f64, tol: f64) -> OpOut;
        fn occRevolve(s: u64, xy: Vec<f64>, angle: f64, tol: f64) -> OpOut;
        fn occEdges(s: u64, subject: u64) -> EdgesOut;
        fn occInfo(s: u64, token: u64) -> InfoOut;
        fn occBBox(s: u64, subject: u64) -> BoxOut;
        fn occTranslate(s: u64, subject: u64, dx: f64, dy: f64, dz: f64) -> OpOut;
        fn occMirrorPlane(s: u64, subject: u64, axis: i32, at: f64) -> OpOut;
        fn occCountShapes(s: u64) -> u64;
    }
}

// --- engine implementations: real (bridge + OCCT) or stub ----------------------

#[cfg(feature = "build-engine")]
mod engine {
    use super::ffi;
    use super::ffi::{BoxOut, EdgesOut, InfoOut, OpOut};
    use super::{code_to_error, EngineError, ShapeInfo};

    fn op(out: OpOut) -> Result<u64, EngineError> {
        if out.ok {
            Ok(out.token)
        } else {
            Err(code_to_error(out.code, &out.message))
        }
    }

    fn info_out(out: InfoOut) -> Result<ShapeInfo, EngineError> {
        if out.ok {
            Ok(ShapeInfo {
                volume: out.volume,
                surface_area: out.area,
                face_count: out.nfaces,
                edge_count: out.nedges,
                vertex_count: out.nverts,
            })
        } else {
            Err(code_to_error(out.code, &out.message))
        }
    }

    fn edges_out(out: EdgesOut) -> Result<Vec<u64>, EngineError> {
        if out.ok {
            Ok(out.edges)
        } else {
            Err(code_to_error(out.code, &out.message))
        }
    }

    fn box_out(out: BoxOut) -> Result<(f64, f64, f64, f64, f64, f64), EngineError> {
        if out.ok {
            Ok((out.xmin, out.ymin, out.zmin, out.xmax, out.ymax, out.zmax))
        } else {
            Err(code_to_error(out.code, &out.message))
        }
    }

    pub fn sys_new() -> Result<u64, EngineError> {
        Ok(ffi::occNewSession())
    }
    pub fn sys_drop(id: u64) {
        ffi::occDropSession(id);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn make_box(
        sid: u64,
        x0: f64,
        y0: f64,
        z0: f64,
        x1: f64,
        y1: f64,
        z1: f64,
        tol: f64,
    ) -> Result<u64, EngineError> {
        op(ffi::occMakeBox(sid, x0, y0, z0, x1, y1, z1, tol))
    }
    pub fn make_cylinder(sid: u64, radius: f64, height: f64, tol: f64) -> Result<u64, EngineError> {
        op(ffi::occMakeCylinder(sid, radius, height, tol))
    }
    pub fn make_sphere(sid: u64, radius: f64, tol: f64) -> Result<u64, EngineError> {
        op(ffi::occMakeSphere(sid, radius, tol))
    }
    pub fn boolean(sid: u64, op_code: u8, a: u64, b: u64, tol: f64) -> Result<u64, EngineError> {
        op(ffi::occBoolean(sid, i32::from(op_code), a, b, tol))
    }
    pub fn fillet(
        sid: u64,
        subject: u64,
        edge_indices: &[u64],
        radius: f64,
        tol: f64,
    ) -> Result<u64, EngineError> {
        op(ffi::occFilletChamfer(
            sid,
            0,
            subject,
            edge_indices.to_vec(),
            radius,
            tol,
        ))
    }
    pub fn chamfer(
        sid: u64,
        subject: u64,
        edge_indices: &[u64],
        distance: f64,
        tol: f64,
    ) -> Result<u64, EngineError> {
        op(ffi::occFilletChamfer(
            sid,
            1,
            subject,
            edge_indices.to_vec(),
            distance,
            tol,
        ))
    }
    pub fn extrude(sid: u64, coords: &[f64], height: f64, tol: f64) -> Result<u64, EngineError> {
        op(ffi::occExtrude(sid, coords.to_vec(), height, tol))
    }
    pub fn revolve(sid: u64, coords: &[f64], angle: f64, tol: f64) -> Result<u64, EngineError> {
        op(ffi::occRevolve(sid, coords.to_vec(), angle, tol))
    }
    pub fn bbox(sid: u64, subject: u64) -> Result<(f64, f64, f64, f64, f64, f64), EngineError> {
        box_out(ffi::occBBox(sid, subject))
    }
    pub fn translate(
        sid: u64,
        subject: u64,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> Result<u64, EngineError> {
        op(ffi::occTranslate(sid, subject, dx, dy, dz))
    }
    pub fn mirror_plane(sid: u64, subject: u64, axis: i32, at: f64) -> Result<u64, EngineError> {
        op(ffi::occMirrorPlane(sid, subject, axis, at))
    }
    pub fn edges(sid: u64, subject: u64) -> Result<Vec<u64>, EngineError> {
        edges_out(ffi::occEdges(sid, subject))
    }
    pub fn info(sid: u64, token: u64) -> Result<ShapeInfo, EngineError> {
        info_out(ffi::occInfo(sid, token))
    }
    pub fn count(sid: u64) -> u64 {
        ffi::occCountShapes(sid)
    }
}

#[cfg(not(feature = "build-engine"))]
mod engine {
    use super::{EngineError, ShapeInfo};

    fn err() -> EngineError {
        EngineError::Unavailable
    }

    pub fn sys_new() -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn sys_drop(_id: u64) {}
    #[allow(clippy::too_many_arguments)]
    pub fn make_box(
        _sid: u64,
        _x0: f64,
        _y0: f64,
        _z0: f64,
        _x1: f64,
        _y1: f64,
        _z1: f64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn make_cylinder(
        _sid: u64,
        _radius: f64,
        _height: f64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn make_sphere(_sid: u64, _radius: f64, _tol: f64) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn boolean(
        _sid: u64,
        _op_code: u8,
        _a: u64,
        _b: u64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn fillet(
        _sid: u64,
        _subject: u64,
        _edge_indices: &[u64],
        _radius: f64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn chamfer(
        _sid: u64,
        _subject: u64,
        _edge_indices: &[u64],
        _distance: f64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn extrude(
        _sid: u64,
        _coords: &[f64],
        _height: f64,
        _tol: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn revolve(_sid: u64, _coords: &[f64], _angle: f64, _tol: f64) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn bbox(_sid: u64, _subject: u64) -> Result<(f64, f64, f64, f64, f64, f64), EngineError> {
        Err(err())
    }
    pub fn translate(
        _sid: u64,
        _subject: u64,
        _dx: f64,
        _dy: f64,
        _dz: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn mirror_plane(
        _sid: u64,
        _subject: u64,
        _axis: i32,
        _at: f64,
    ) -> Result<u64, EngineError> {
        Err(err())
    }
    pub fn edges(_sid: u64, _subject: u64) -> Result<Vec<u64>, EngineError> {
        Err(err())
    }
    pub fn info(_sid: u64, _token: u64) -> Result<ShapeInfo, EngineError> {
        Err(err())
    }
    pub fn count(_sid: u64) -> u64 {
        0
    }
}

// --- public surface (feature-independent) ---------------------------------------

/// Create a new kernel session and return its opaque id.
pub fn occ_sys_new() -> Result<u64, EngineError> {
    engine::sys_new()
}

/// Release a kernel session and all shapes registered under it.
pub fn occ_sys_drop(id: u64) {
    engine::sys_drop(id);
}

/// Create an axis-aligned box solid from corner to corner; returns its token.
///
/// Tolerance is passed through to the kernel algorithm (caller-owned, P6-M1-T03).
#[allow(clippy::too_many_arguments)]
pub fn occ_make_box(
    sid: u64,
    x0: f64,
    y0: f64,
    z0: f64,
    x1: f64,
    y1: f64,
    z1: f64,
    tol: f64,
) -> Result<u64, EngineError> {
    engine::make_box(sid, x0, y0, z0, x1, y1, z1, tol)
}

/// Create a cylinder solid (axis +Z, base at z=0) of the given radius and
/// height; returns its token.
pub fn occ_make_cylinder(sid: u64, radius: f64, height: f64, tol: f64) -> Result<u64, EngineError> {
    engine::make_cylinder(sid, radius, height, tol)
}

/// Create a sphere solid centered at the origin with the given radius; returns
/// its token.
pub fn occ_make_sphere(sid: u64, radius: f64, tol: f64) -> Result<u64, EngineError> {
    engine::make_sphere(sid, radius, tol)
}

/// Exact B-Rep boolean of two registered shapes (`a` op `b`); returns the
/// result token. `op_code` is one of [`OP_UNION`], [`OP_DIFFERENCE`],
/// [`OP_COMMON`]. A result with no faces maps to [`EngineError::EmptyResult`].
pub fn occ_boolean(sid: u64, op_code: u8, a: u64, b: u64, tol: f64) -> Result<u64, EngineError> {
    engine::boolean(sid, op_code, a, b, tol)
}

/// Fillet the given edges (indices into the subject's own topology) of a solid
/// with the given radius; returns the new solid's token.
pub fn occ_fillet(
    sid: u64,
    subject: u64,
    edge_indices: &[u64],
    radius: f64,
    tol: f64,
) -> Result<u64, EngineError> {
    engine::fillet(sid, subject, edge_indices, radius, tol)
}

/// Chamfer the given edges (indices into the subject's own topology) of a solid
/// with the given symmetric distance; returns the new solid's token.
pub fn occ_chamfer(
    sid: u64,
    subject: u64,
    edge_indices: &[u64],
    distance: f64,
    tol: f64,
) -> Result<u64, EngineError> {
    engine::chamfer(sid, subject, edge_indices, distance, tol)
}

/// Extrude a closed planar XY profile (`coords` = x0,y0,x1,y1,... in the Z=0
/// plane, loop closes implicitly) by `height` along +Z; returns the new
/// solid's token.
pub fn occ_extrude(sid: u64, coords: &[f64], height: f64, tol: f64) -> Result<u64, EngineError> {
    engine::extrude(sid, coords, height, tol)
}

/// Revolve a closed planar XY profile (`coords` = x0,y0,x1,y1,... in the Z=0
/// plane, loop closes implicitly) about the Z axis by `angle` radians; returns
/// the new solid's token.
pub fn occ_revolve(sid: u64, coords: &[f64], angle: f64, tol: f64) -> Result<u64, EngineError> {
    engine::revolve(sid, coords, angle, tol)
}

/// List the edge indices of a registered shape (stable `TopExp_Explorer`
/// order) -- the edge references returned to callers for fillet/chamfer.
pub fn occ_edges(sid: u64, subject: u64) -> Result<Vec<u64>, EngineError> {
    engine::edges(sid, subject)
}

/// Axis-aligned bounding box of a registered shape as the closed
/// `(xmin, ymin, zmin, xmax, ymax, zmax)` extent (BRepBndLib + Bnd_Box).
pub fn occ_bbox(sid: u64, subject: u64) -> Result<(f64, f64, f64, f64, f64, f64), EngineError> {
    engine::bbox(sid, subject)
}

/// Rigid translation of a registered shape by `(dx, dy, dz)` (a new solid is
/// registered; the subject is untouched).
pub fn occ_translate(
    sid: u64,
    subject: u64,
    dx: f64,
    dy: f64,
    dz: f64,
) -> Result<u64, EngineError> {
    engine::translate(sid, subject, dx, dy, dz)
}

/// Mirror a registered shape about an axis-aligned plane. `axis` selects the
/// plane normal (0 = X through `at`, 1 = Y through `at`, 2 = Z through `at`);
/// returns the new solid's token.
pub fn occ_mirror_plane(sid: u64, subject: u64, axis: i32, at: f64) -> Result<u64, EngineError> {
    engine::mirror_plane(sid, subject, axis, at)
}

/// Measured properties (volume, surface area, face/edge/vertex counts) of a
/// registered shape.
pub fn occ_info(sid: u64, token: u64) -> Result<ShapeInfo, EngineError> {
    engine::info(sid, token)
}

/// Number of shapes currently held in a session (diagnostic/leak checking).
pub fn occ_count_shapes(sid: u64) -> u64 {
    engine::count(sid)
}

#[cfg(all(test, feature = "build-engine"))]
mod tests {
    use super::*;

    fn tol() -> f64 {
        1e-6
    }

    #[test]
    fn session_lifecycle_registers_and_drops() {
        let sid = occ_sys_new().expect("new session");
        assert_eq!(occ_count_shapes(sid), 0);
        occ_sys_drop(sid);
    }

    #[test]
    fn box_volume_face_count() {
        let sid = occ_sys_new().unwrap();
        let t = occ_make_box(sid, 0.0, 0.0, 0.0, 10.0, 20.0, 30.0, tol()).unwrap();
        let info = occ_info(sid, t).unwrap();
        assert!((info.volume - 6000.0).abs() < 1e-6);
        assert_eq!(info.face_count, 6);
        occ_sys_drop(sid);
    }

    #[test]
    fn degenerate_primitive_is_typed_error() {
        let sid = occ_sys_new().unwrap();
        let r = occ_make_sphere(sid, 0.0, tol());
        assert!(matches!(r, Err(EngineError::Degenerate(_))));
        occ_sys_drop(sid);
    }

    #[test]
    fn bad_session_is_typed_error() {
        let r = occ_make_box(999_999, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, tol());
        assert!(matches!(r, Err(EngineError::MissingSession(_))));
    }

    #[test]
    fn extrude_rectangle_volume() {
        let sid = occ_sys_new().unwrap();
        // Unit square footprint (mirrors the committed box-reference corpus
        // solid) swept by 1 -> volume 1.
        let coords = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        let t = occ_extrude(sid, &coords, 1.0, tol()).unwrap();
        let info = occ_info(sid, t).unwrap();
        assert!((info.volume - 1.0).abs() < 1e-9);
        assert_eq!(info.face_count, 6);
        occ_sys_drop(sid);
    }

    #[test]
    fn revolve_annulus_volume() {
        let sid = occ_sys_new().unwrap();
        // Rectangle (0.4,0)-(1.0,1.0) revolved a full turn about Z: an
        // annular solid of volume pi*(1.0^2 - 0.4^2)*1.0.
        let coords = [0.4, 0.0, 1.0, 0.0, 1.0, 1.0, 0.4, 1.0];
        let t = occ_revolve(sid, &coords, 2.0 * std::f64::consts::PI, tol()).unwrap();
        let info = occ_info(sid, t).unwrap();
        let expected = std::f64::consts::PI * (1.0 - 0.16) * 1.0;
        assert!(
            (info.volume - expected).abs() / expected < 1e-9,
            "revolve volume {} vs {expected}",
            info.volume
        );
        occ_sys_drop(sid);
    }

    #[test]
    fn degenerate_sweep_is_typed_error() {
        let sid = occ_sys_new().unwrap();
        // Two-point profile cannot form a face.
        let coords = [0.0, 0.0, 1.0, 0.0];
        let r = occ_extrude(sid, &coords, 1.0, tol());
        assert!(
            matches!(r, Err(EngineError::InvalidArgument(_))),
            "got {r:?}"
        );
        // Non-positive height.
        let square = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        let r2 = occ_extrude(sid, &square, 0.0, tol());
        assert!(matches!(r2, Err(EngineError::Degenerate(_))), "got {r2:?}");
        occ_sys_drop(sid);
    }
}
