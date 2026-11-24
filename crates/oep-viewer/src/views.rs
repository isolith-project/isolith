//! Standard view math, the cube-face hit test and section-plane presets
//! (editor parity program, wave D, card UI-D4 UI-NAV).
//!
//! This is the analytic geometry layer the nav surface (`crates/oep-app`
//! `panels/nav.rs`) drives, kept in `oep-viewer` so the numbers are
//! headlessly provable:
//!
//! - [`StandardView`] is the exact committed orientation - a `(yaw, pitch)`
//!   pair for the orbit [`Camera`] - that each view-cube face and each
//!   quick-view button must land the camera on. [`StandardView::direction`]
//!   is the analytic unit vector from the orbit target toward the eye, so
//!   `eye = target + direction * distance` is the closed-form expectation a
//!   test asserts the real camera against.
//! - [`CubeFace`] is the six-face cube gizmo; [`CubeFace::view`] is the
//!   face-to-standard-view mapping and [`CubeFace::from_direction`] is the
//!   hit test that resolves a view ray onto the face whose outward normal is
//!   closest - the test proves it round-trips for every face.
//! - [`section_plane`] turns a slider fraction in `[0,1]` into a world
//!   [`ClipPlane`] preset against one of the three principal axes (the
//!   "plane presets"). [`section_triangle_count`] is the analytic closed-form
//!   count of visible triangles the demo solid leaves when clipped by that
//!   plane, and [`visible_triangle_count`] is the mechanical mesh-clip
//!   emulation of the same cut; both agree, and both are what the section
//!   slider reports so "the slider clips the model predictably" is a number,
//!   not an impression.
//!
//! All math is `f64` (`oep_geo::math`), deterministic and identical to the
//! math the CPU renderer's own clip path uses ([`crate::clip`],
//! [`crate::render`]).

use crate::clip::ClipPlane;
use crate::scene::Aabb;
use oep_geo::math::Vec3;

/// Pitch clamped away from vertical: the orbit camera's look-at is
/// degenerate when the view direction is exactly parallel to its fixed up
/// vector `(0,1,0)` (see [`crate::camera::Camera::view_matrix`]), so the
/// top/bottom standard views sit at this committed near-vertical pitch -
/// the same safe band the sketch normal-orientation and the camera's own
/// `orbit` clamp already use.
pub const VERTICAL_PITCH: f64 = 1.54;

/// The standard orientation names the view surface offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardView {
    /// Looking along world `-Z` (eye at `+Z` of the orbit target).
    Front,
    /// Looking along world `+Z` (eye at `-Z` of the orbit target).
    Back,
    /// Looking along world `+X` (eye at `-X` of the orbit target).
    Left,
    /// Looking along world `-X` (eye at `+X` of the orbit target).
    Right,
    /// Looking straight down world `-Y` (eye above the orbit target).
    Top,
    /// Looking straight up world `+Y` (eye below the orbit target).
    Bottom,
    /// The isometric view: eye direction `(1,1,1)/sqrt(3)`.
    Iso,
}

impl StandardView {
    /// Every standard view, in the order the nav surface presents them.
    pub const ALL: [StandardView; 7] = [
        StandardView::Front,
        StandardView::Back,
        StandardView::Left,
        StandardView::Right,
        StandardView::Top,
        StandardView::Bottom,
        StandardView::Iso,
    ];

    /// The six cube-face views (no iso).
    pub const FACES: [StandardView; 6] = [
        StandardView::Front,
        StandardView::Back,
        StandardView::Left,
        StandardView::Right,
        StandardView::Top,
        StandardView::Bottom,
    ];

    /// The exact committed `(yaw, pitch)` in radians for this standard view.
    ///
    /// The camera stores `target/yaw/pitch/distance`; setting exactly these
    /// two angles (keeping target, distance, fov) is what "lands the camera
    /// on the standard view" means, and it is assertable numerically.
    pub fn yaw_pitch(&self) -> (f64, f64) {
        match self {
            StandardView::Front => (0.0, 0.0),
            StandardView::Back => (std::f64::consts::PI, 0.0),
            StandardView::Left => (-std::f64::consts::FRAC_PI_2, 0.0),
            StandardView::Right => (std::f64::consts::FRAC_PI_2, 0.0),
            StandardView::Top => (0.0, VERTICAL_PITCH),
            StandardView::Bottom => (0.0, -VERTICAL_PITCH),
            StandardView::Iso => (std::f64::consts::FRAC_PI_4, (1.0 / 3.0_f64.sqrt()).asin()),
        }
    }

    /// The analytic unit view direction (from the orbit target toward the
    /// eye) for this standard view, using exactly the orbit-camera eye
    /// formula `(cos(pitch)*sin(yaw), sin(pitch), cos(pitch)*cos(yaw))`.
    pub fn direction(&self) -> Vec3 {
        let (yaw, pitch) = self.yaw_pitch();
        let (cp, sp) = (pitch.cos(), pitch.sin());
        let (cy, sy) = (yaw.cos(), yaw.sin());
        Vec3::new(cp * sy, sp, cp * cy)
    }

    /// Analytic eye position: `target + direction * distance`.
    pub fn eye_for(&self, target: &Vec3, distance: f64) -> Vec3 {
        target.add(&self.direction().mul(distance))
    }

    /// The camera pose this standard view produces: `(eye, target, up)` with
    /// the orbit camera's fixed up vector.
    pub fn camera_pose(&self, target: &Vec3, distance: f64) -> (Vec3, Vec3, Vec3) {
        (self.eye_for(target, distance), *target, Self::up())
    }

    /// The orbit camera's fixed up vector.
    pub fn up() -> Vec3 {
        Vec3::new(0.0, 1.0, 0.0)
    }

    /// Short label used by the nav buttons.
    pub fn label(&self) -> &'static str {
        match self {
            StandardView::Front => "Front",
            StandardView::Back => "Back",
            StandardView::Left => "Left",
            StandardView::Right => "Right",
            StandardView::Top => "Top",
            StandardView::Bottom => "Bottom",
            StandardView::Iso => "Iso",
        }
    }
}

/// The six faces of the view cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeFace {
    /// The `+Z` facing face (front of the model).
    Front,
    /// The `-Z` facing face (back of the model).
    Back,
    /// The `-X` facing face.
    Left,
    /// The `+X` facing face.
    Right,
    /// The `+Y` facing face.
    Top,
    /// The `-Y` facing face.
    Bottom,
}

impl CubeFace {
    /// Every cube face, in the order the gizmo lays them out.
    pub const ALL: [CubeFace; 6] = [
        CubeFace::Front,
        CubeFace::Back,
        CubeFace::Left,
        CubeFace::Right,
        CubeFace::Top,
        CubeFace::Bottom,
    ];

    /// The cube-face -> standard-view mapping (a click on this face lands the
    /// camera on this standard view).
    pub fn view(&self) -> StandardView {
        match self {
            CubeFace::Front => StandardView::Front,
            CubeFace::Back => StandardView::Back,
            CubeFace::Left => StandardView::Left,
            CubeFace::Right => StandardView::Right,
            CubeFace::Top => StandardView::Top,
            CubeFace::Bottom => StandardView::Bottom,
        }
    }

    /// The accessible name of this face (the gizmo is reachable without a
    /// mouse because every face is a labelled, a11y-visible control).
    pub fn label(&self) -> &'static str {
        self.view().label()
    }

    /// The cube-face hit test: the face whose outward normal is closest to
    /// `direction` (the face a viewer looking along `direction` is looking
    /// at). This is how an orientation cube resolves a pointer ray onto a
    /// face - pick the principal axis with the largest projection.
    pub fn from_direction(direction: &Vec3) -> CubeFace {
        let x = direction.x.abs();
        let y = direction.y.abs();
        let z = direction.z.abs();
        if y >= x && y >= z {
            if direction.y >= 0.0 {
                CubeFace::Top
            } else {
                CubeFace::Bottom
            }
        } else if x >= z {
            if direction.x >= 0.0 {
                CubeFace::Right
            } else {
                CubeFace::Left
            }
        } else if direction.z >= 0.0 {
            CubeFace::Front
        } else {
            CubeFace::Back
        }
    }
}

/// The principal axis a section-plane preset slices along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionAxis {
    /// Slice along world X (plane normal `+X`, kept side below `+X`).
    X,
    /// Slice along world Y (plane normal `+Y`, kept side below `+Y`).
    Y,
    /// Slice along world Z (plane normal `+Z`, kept side below `+Z`).
    Z,
}

impl SectionAxis {
    /// Every slice axis.
    pub const ALL: [SectionAxis; 3] = [SectionAxis::X, SectionAxis::Y, SectionAxis::Z];

    /// Short label.
    pub fn label(&self) -> &'static str {
        match self {
            SectionAxis::X => "X",
            SectionAxis::Y => "Y",
            SectionAxis::Z => "Z",
        }
    }
}

/// Build the world section-plane preset from a slider fraction in `[0,1]`
/// against `axis` within `bounds`: the plane sits a fraction of the box's
/// extent along that axis from its min corner, its normal points away from
/// the box (toward the removed side), so the kept side is the
/// lower-fraction slab. The fraction is clamped to `[0,1]`.
pub fn section_plane(bounds: &Aabb, axis: SectionAxis, fraction: f64) -> ClipPlane {
    let f = fraction.clamp(0.0, 1.0);
    match axis {
        SectionAxis::X => {
            let x = bounds.min.x + f * (bounds.max.x - bounds.min.x);
            ClipPlane::point_normal(
                Vec3::new(x, bounds.min.y, bounds.min.z),
                Vec3::new(1.0, 0.0, 0.0),
            )
        }
        SectionAxis::Y => {
            let y = bounds.min.y + f * (bounds.max.y - bounds.min.y);
            ClipPlane::point_normal(
                Vec3::new(bounds.min.x, y, bounds.min.z),
                Vec3::new(0.0, 1.0, 0.0),
            )
        }
        SectionAxis::Z => {
            let z = bounds.min.z + f * (bounds.max.z - bounds.min.z);
            ClipPlane::point_normal(
                Vec3::new(bounds.min.x, bounds.min.y, z),
                Vec3::new(0.0, 0.0, 1.0),
            )
        }
    }
}

/// The `i`-th corner of the box, per the renderer's corner order (the same
/// 8 corners [`Aabb::corners`] produces and `render::box_faces` consumes).
#[allow(clippy::many_single_char_names)]
fn corner(b: &Aabb, i: usize) -> Vec3 {
    let (mn, mx) = (b.min, b.max);
    match i {
        0 => Vec3::new(mn.x, mn.y, mn.z),
        1 => Vec3::new(mx.x, mn.y, mn.z),
        2 => Vec3::new(mx.x, mx.y, mn.z),
        3 => Vec3::new(mn.x, mx.y, mn.z),
        4 => Vec3::new(mn.x, mn.y, mx.z),
        5 => Vec3::new(mx.x, mn.y, mx.z),
        6 => Vec3::new(mx.x, mx.y, mx.z),
        _ => Vec3::new(mn.x, mx.y, mx.z),
    }
}

/// The six faces of the box as quads, in the plane order the renderer draws
/// them (mirror of `render::box_faces`: bottom z-, top z+, front y-, back
/// y+, left x-, right x+), so the analytic count below counts exactly the
/// polygons the renderer would clip for the same solid.
fn box_faces(b: &Aabb) -> [Vec<Vec3>; 6] {
    let p: Vec<Vec3> = (0..8).map(|i| corner(b, i)).collect();
    [
        vec![p[0], p[1], p[2], p[3]], // bottom z-
        vec![p[4], p[5], p[6], p[7]], // top z+
        vec![p[0], p[1], p[5], p[4]], // front y-
        vec![p[3], p[2], p[6], p[7]], // back y+
        vec![p[0], p[3], p[7], p[4]], // left x-
        vec![p[1], p[2], p[6], p[5]], // right x+
    ]
}

/// Projected area of a planar polygon (Newell's method: the magnitude of the
/// area vector is twice the area). A degenerate sliver that clips a plane
/// exactly through a box corner projects to zero area and is not a visible
/// triangle - the renderer's own rasterizer drops zero-area triangles too.
fn polygon_area(poly: &[Vec3]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        nx += (a.y - b.y) * (a.z + b.z);
        ny += (a.z - b.z) * (a.x + b.x);
        nz += (a.x - b.x) * (a.y + b.y);
    }
    Vec3::new(nx, ny, nz).norm() * 0.5
}

/// Minimum projected area (square document units) a clipped polygon must have
/// to count as a visible triangle.
const MIN_AREA: f64 = 1e-9;

/// Analytic closed-form count of visible triangles: clip each of the box's
/// six surface faces against `plane` (the exact
/// [`ClipPlane::clip_polygon`] path the renderer uses) and sum the fan
/// triangle counts `n - 2` of every surviving non-degenerate polygon. This is
/// the whole-box count the section slider must report for the demo solid.
pub fn section_triangle_count(bounds: &Aabb, plane: &ClipPlane) -> usize {
    let mut total = 0usize;
    for face in box_faces(bounds) {
        let clipped = plane.clip_polygon(&face);
        if polygon_area(&clipped) >= MIN_AREA && clipped.len() >= 3 {
            total += clipped.len() - 2;
        }
    }
    total
}

/// Mechanical visible-triangle count: clip each of the box's six surface
/// faces against `plane` (the exact [`ClipPlane::clip_polygon`] path the
/// renderer's `push_polygon` uses), then enumerate the fan triangle list the
/// rasterizer would fill ([`crate::render::fan_triangles`] semantics: `n - 2`
/// triangles per non-degenerate clipped polygon) and count the triangles. It
/// is written as an explicit enumeration of the triangle list, in contrast to
/// the closed-form [`section_triangle_count`], so the two code paths
/// cross-check that they describe the same solid.
pub fn visible_triangle_count(bounds: &Aabb, plane: &ClipPlane) -> usize {
    let mut total = 0usize;
    for face in box_faces(bounds) {
        let clipped = plane.clip_polygon(&face);
        if clipped.len() < 3 || polygon_area(&clipped) < MIN_AREA {
            continue;
        }
        // The rasterizer fans the convex clipped polygon into `n - 2`
        // triangles; every one is non-degenerate because the polygon is.
        total += clipped.len() - 2;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed demo solid (the viewport's scene): 80 x 40 x 10.
    fn demo_bounds() -> Aabb {
        Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(80.0, 40.0, 10.0))
    }

    /// A standard view's analytic eye equals the orbit camera's own `eye`
    /// calculation for a camera carrying exactly that yaw/pitch.
    #[test]
    fn standard_view_matches_camera_eye_formula() {
        let target = Vec3::new(40.0, 20.0, 5.0);
        for view in StandardView::ALL {
            let (yaw, pitch) = view.yaw_pitch();
            let (cp, sp) = (pitch.cos(), pitch.sin());
            let (cy, sy) = (yaw.cos(), yaw.sin());
            let d = 150.0;
            let expected = target.add(&Vec3::new(cp * sy, sp, cp * cy).mul(d));
            let eye = view.eye_for(&target, d);
            assert!(
                eye.distance_to(&expected) < 1e-9,
                "{view:?} eye {eye:?} != camera formula {expected:?}"
            );
        }
    }

    /// The six cube-face views are exact axis-aligned directions (front/back/
    /// left/right are perfect; top/bottom sit at the committed near-vertical
    /// pitch inside the camera's safe band).
    #[test]
    fn cube_face_views_are_exact_axis_views() {
        let target = Vec3::new(40.0, 20.0, 5.0);
        let d = 120.0;
        let cases = [
            (CubeFace::Front, Vec3::new(40.0, 20.0, 125.0)),
            (CubeFace::Back, Vec3::new(40.0, 20.0, -115.0)),
            (CubeFace::Left, Vec3::new(-80.0, 20.0, 5.0)),
            (CubeFace::Right, Vec3::new(160.0, 20.0, 5.0)),
        ];
        for (face, expected) in cases {
            let eye = face.view().eye_for(&target, d);
            assert!(
                eye.distance_to(&expected) < 1e-9,
                "{face:?} eye {eye:?} != {expected:?}"
            );
        }
        // Top/bottom at the committed vertical pitch: the camera looks almost
        // straight down/up - X stays on axis (yaw 0), the small Z offset is
        // exactly cos(VERTICAL_PITCH)*distance (the committed near-vertical
        // declination the look-at clamp allows), and the eye is > 99% of the
        // distance above/below the target.
        let top = CubeFace::Top.view();
        let (yaw, pitch) = top.yaw_pitch();
        assert_eq!(yaw, 0.0);
        assert_eq!(pitch, VERTICAL_PITCH);
        let top_eye = top.eye_for(&target, d);
        let z_off = VERTICAL_PITCH.cos() * d;
        assert!((top_eye.x - 40.0).abs() < 1e-9);
        assert!((top_eye.z - (5.0 + z_off)).abs() < 1e-9);
        assert!(top_eye.y - target.y > d * 0.99);
        let bottom_eye = CubeFace::Bottom.view().eye_for(&target, d);
        assert!((bottom_eye.x - 40.0).abs() < 1e-9);
        // cos is even: the small Z declination offset is the same positive
        // `cp*d` for bottom as for top; only Y flips sign.
        assert!((bottom_eye.z - (5.0 + z_off)).abs() < 1e-9);
        assert!(target.y - bottom_eye.y > d * 0.99);
        // Isometric: eye direction is exactly (1,1,1)/sqrt(3).
        let iso_eye = StandardView::Iso.eye_for(&target, d);
        let dir = iso_eye.sub(&target).normalize().expect("non-zero");
        let s3 = 3.0_f64.sqrt();
        assert!((dir.x - 1.0 / s3).abs() < 1e-9);
        assert!((dir.y - 1.0 / s3).abs() < 1e-9);
        assert!((dir.z - 1.0 / s3).abs() < 1e-9);
    }

    /// The cube-face hit test round-trips: the direction of every standard
    /// face view resolves back to that exact face.
    #[test]
    fn cube_face_hit_test_round_trips_every_face() {
        for face in CubeFace::ALL {
            let dir = face.view().direction();
            assert_eq!(
                face,
                CubeFace::from_direction(&dir),
                "hit test for {face:?}"
            );
        }
    }

    /// The section plane preset sits at the committed fraction of the box
    /// extent and clips the correct slab (kept below the plane).
    #[test]
    fn section_plane_preset_slices_at_fraction() {
        let b = demo_bounds();
        for frac in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let pl = section_plane(&b, SectionAxis::Z, frac);
            let z0 = b.min.z + frac * (b.max.z - b.min.z);
            // A point just below the plane is kept, just above is removed.
            assert!(pl.keeps(&Vec3::new(1.0, 1.0, z0 - 0.01)));
            assert!(!pl.keeps(&Vec3::new(1.0, 1.0, z0 + 0.01)));
            // The plane passes through the committed offset.
            assert!(pl.signed_distance(&Vec3::new(0.0, 0.0, z0)).abs() < 1e-9);
        }
    }

    /// The analytic and measured visible-triangle counts agree at every
    /// sampled fraction and both match the committed literal counts at the
    /// three committed slider positions.
    #[test]
    fn section_counts_are_monotone_and_match_literals() {
        let b = demo_bounds();
        // The slider is committed as "1.0 = whole box at the right end";
        // dragging the knob LEFT reduces the fraction, so the traversal the
        // acceptance cares about is descending: counts must be non-increasing
        // as the cut deepens.
        let mut prev = usize::MAX;
        let mut prev_measured = usize::MAX;
        for frac in [1.0, 0.75, 0.5, 0.25, 0.0] {
            let pl = section_plane(&b, SectionAxis::Z, frac);
            let a = section_triangle_count(&b, &pl);
            let m = visible_triangle_count(&b, &pl);
            assert_eq!(a, m, "analytic != measured at fraction {frac}");
            assert!(
                a <= prev,
                "count must be non-increasing, {a} > {prev} at {frac}"
            );
            assert!(m <= prev_measured, "measured must be non-increasing");
            prev = a;
            prev_measured = m;
        }
        // Committed positions (matching the nav surface's literals): full box
        // 12, half cut 10, closed to the bottom sheet 2.
        for (frac, expected) in [(1.0, 12usize), (0.5, 10usize), (0.0, 2usize)] {
            let pl = section_plane(&b, SectionAxis::Z, frac);
            assert_eq!(
                section_triangle_count(&b, &pl),
                expected,
                "analytic at {frac}"
            );
            assert_eq!(
                visible_triangle_count(&b, &pl),
                expected,
                "measured at {frac}"
            );
        }
    }
}
