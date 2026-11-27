//! UI-E6 UI-NAV-CUBE: the 3D orientation cube beside the existing nav button
//! panel (wave E).
//!
//! This is the real 3D navigation cube the card adds to the editor viewport,
//! alongside the UI-D4 button panel (`nav.rs`) which stays exactly as it is.
//! Adopts the FreeCAD / Onshape / blender-view-cube convention: a small cube
//! in the viewport corner that shows the live model orientation, where a face
//! maps to the perpendicular standard view, an edge to the 45 degree view and
//! a corner to an isometric view.
//!
//! The cube is drawn in its own small viewport pass through the SAME renderer
//! the viewport already uses (`oep_viewer::render`), NOT a second renderer:
//! the cube scene is six coloured face quads built once (the resident-scene
//! idea from UI-D10 applied to this tiny pass) and re-rasterized only when the
//! camera yaw/pitch, the highlighted face or the pass size changed ([`PassKey`]
//! dirty key, mirroring the UI-D10 frame-key discipline); in steady state the
//! cached pixels are re-drawn with zero cube work. This is a second small draw
//! pass, never a second scene submission.
//!
//! The interaction is shaped by the US7782319B2 read (see the card body): a
//! clickable navigation cube reads on Autodesk's claim set while the patent is
//! in force, so the INTERACTION ships behind
//! [`settings::Settings::nav_cube_click_to_align`], DEFAULT OFF. Off, the cube
//! is a live orientation indicator (tracks the camera, labels its faces with
//! the shared view-name table, highlights the current face, and passes pointer
//! input straight through to the viewport). On, the full click/drag/snap
//! behaviour is available and tested. The re-check and flip are tracked for
//! April 2027 (vault `Topics/Isolith.md` decision #24).
//!
//! ## Wiring and placement
//!
//! The panel is registered in `panels/mod.rs` (`pub mod nav_cube;`) and driven
//! by ONE `nav_cube::update(...)` call at the END of `nav::update` (nav.rs is
//! this card's owned file; appending the call is the registration step, and
//! the button panel's own behaviour never changes). It draws in the viewport's
//! right column directly above the nav button panel, because the literal
//! top-right corner is where the D2 shading card lives: a 96 px cube there
//! overlaps the shading card's mode rows and swallows the frozen `gpu_shading`
//! clicks (measured; see the card proof). "Beside the existing button panel"
//! is the operative placement (the card title), and the two controls stay
//! independently reachable.
//!
//! ## Guards
//!
//! - UI-A3 / UI-A12: all ink from `oep_ui_theme` tokens or egui's built-in
//!   `Color32` constants; no raw hex, no `Color32::from_*`, no `strong()` here.
//! - UI-B1 mutation guard: this panel never writes a document; it only moves
//!   the viewport camera (through the runtime backend store, the same seam
//!   nav.rs uses) and publishes its own readouts.
//! - One `Pane` preserved: the cube is an `egui::Area` overlay, not a native
//!   panel, so the single-Pane AccessKit contract stays intact.

use eframe::egui;

use oep_geo::math::{Mat3, Quat, Vec3};
use oep_viewer::views::CubeFace;
use oep_viewer::{Camera, MeshData, Primitive, Scene};

use crate::app::App;
use crate::theme;

use super::settings::{self, Settings};

/// Context-data key under which the cube publishes its typed report every
/// frame (the acceptance harness reads it back with the same key).
pub const NAV_CUBE_REPORT_KEY: &str = "isolith.ui.viewport.nav_cube.report";

/// Context-data key under which the cube publishes its `String` oracle:
/// `click_to_align=<0|1> current=<face> hover=<target|none>
/// rect=<x0,y0,x1,y1>`.
pub const NAV_CUBE_ORACLE_KEY: &str = "isolith.ui.viewport.nav_cube.oracle";

/// The viewport backend's context-data key. Duplicated as a string constant
/// here (documented to match `viewport::VIEWPORT_STATE_KEY`) so this panel has
/// no dependency on the viewport module's private items; the tests already
/// treat the string as the shared seam contract (cf. `tests/nav.rs`).
pub const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";

/// Outer size of the corner cube at scale factor 1.0 (logical px), the card's
/// committed ~96 px, DPI-aware (the pass renders at `CUBE_SIZE * ppp`).
pub const CUBE_SIZE: f32 = 96.0;

/// Inset the cube honours from the viewport's right edge (logical px).
pub const CUBE_INSET: f32 = 8.0;

/// The D4 nav button panel's committed anchor (nav.rs `CARD_H = 220.0` and
/// its `10.0` gap); the cube sits 8 px clear above it. Matched to nav.rs so
/// the two right-column surfaces keep a fixed gap.
const NAV_CARD_H: f32 = 220.0;
const NAV_CARD_GAP: f32 = 10.0;

/// Corner-pick radius / edge-pick tolerance in pass pixels at scale 1.0.
const CORNER_RADIUS: f32 = 9.0;
const EDGE_TOLERANCE: f32 = 7.0;

/// The cube camera's long lens: the unit cube sits far from a tiny field of
/// view, so perspective foreshortening across the cube is `1/CUBE_DISTANCE` of
/// its on-screen extent, under one pixel at the ~96 px pass. This delivers the
/// card's orthographic intent through the existing renderer's camera path
/// (no second projection).
const CUBE_DISTANCE: f64 = 300.0;
const CUBE_NEAR: f64 = 100.0;
const CUBE_FAR: f64 = 600.0;

// ---------------------------------------------------------------------------
// Face geometry (pure, egui-free)
// ---------------------------------------------------------------------------

/// The outward unit normal of a cube face (independent of the renderer's box
/// winding, so hit-testing, labelling and the analytic current-face logic
/// share one table).
pub fn face_normal(face: CubeFace) -> Vec3 {
    match face {
        CubeFace::Front => Vec3::new(0.0, 0.0, 1.0),
        CubeFace::Back => Vec3::new(0.0, 0.0, -1.0),
        CubeFace::Left => Vec3::new(-1.0, 0.0, 0.0),
        CubeFace::Right => Vec3::new(1.0, 0.0, 0.0),
        CubeFace::Top => Vec3::new(0.0, 1.0, 0.0),
        CubeFace::Bottom => Vec3::new(0.0, -1.0, 0.0),
    }
}

/// A selectable feature of the orientation cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTarget {
    /// A cube face: maps to the perpendicular [`StandardView`].
    Face(CubeFace),
    /// An edge between two adjacent faces: maps to the 45 degree view along
    /// the bisector of the two face normals.
    Edge([CubeFace; 2]),
    /// A corner where three faces meet: maps to the isometric view along the
    /// trisector of the three face normals.
    Corner([CubeFace; 3]),
}

/// The 12 cube edges, each named by its two incident faces.
pub const EDGES: [[CubeFace; 2]; 12] = [
    [CubeFace::Front, CubeFace::Right],
    [CubeFace::Front, CubeFace::Left],
    [CubeFace::Front, CubeFace::Top],
    [CubeFace::Front, CubeFace::Bottom],
    [CubeFace::Back, CubeFace::Right],
    [CubeFace::Back, CubeFace::Left],
    [CubeFace::Back, CubeFace::Top],
    [CubeFace::Back, CubeFace::Bottom],
    [CubeFace::Right, CubeFace::Top],
    [CubeFace::Right, CubeFace::Bottom],
    [CubeFace::Left, CubeFace::Top],
    [CubeFace::Left, CubeFace::Bottom],
];

/// The 8 cube corners, each named by its three incident faces.
pub const CORNERS: [[CubeFace; 3]; 8] = [
    [CubeFace::Front, CubeFace::Right, CubeFace::Top],
    [CubeFace::Front, CubeFace::Right, CubeFace::Bottom],
    [CubeFace::Front, CubeFace::Left, CubeFace::Top],
    [CubeFace::Front, CubeFace::Left, CubeFace::Bottom],
    [CubeFace::Back, CubeFace::Right, CubeFace::Top],
    [CubeFace::Back, CubeFace::Right, CubeFace::Bottom],
    [CubeFace::Back, CubeFace::Left, CubeFace::Top],
    [CubeFace::Back, CubeFace::Left, CubeFace::Bottom],
];

impl NavTarget {
    /// Every concrete target: 6 faces + 12 edges + 8 corners = 26.
    pub const ALL: [NavTarget; 26] = {
        let mut all = [NavTarget::Face(CubeFace::Front); 26];
        let mut i = 0;
        let mut f = 0;
        while f < 6 {
            all[i] = NavTarget::Face(CubeFace::ALL[f]);
            i += 1;
            f += 1;
        }
        let mut e = 0;
        while e < EDGES.len() {
            all[i] = NavTarget::Edge(EDGES[e]);
            i += 1;
            e += 1;
        }
        let mut c = 0;
        while c < CORNERS.len() {
            all[i] = NavTarget::Corner(CORNERS[c]);
            i += 1;
            c += 1;
        }
        all
    };

    /// The unit direction from the cube centre TOWARD the eye this target
    /// points the camera at (the same `(cos p sin y, sin p, cos p cos y)`
    /// convention as [`StandardView::direction`]).
    pub fn eye_direction(&self) -> Vec3 {
        match self {
            NavTarget::Face(face) => face.view().direction(),
            NavTarget::Edge([a, b]) => face_normal(*a)
                .add(&face_normal(*b))
                .normalize()
                .expect("sum of distinct principal normals is never zero"),
            NavTarget::Corner([a, b, c]) => face_normal(*a)
                .add(&face_normal(*b))
                .add(&face_normal(*c))
                .normalize()
                .expect("sum of distinct principal normals is never zero"),
        }
    }

    /// The committed `(yaw, pitch)` this target lands the camera on. Faces
    /// reuse the button panel's exact [`StandardView`] angles (the SHARED
    /// view-parameter table, so the two controls cannot drift); edges and
    /// corners use the analytic direction.
    pub fn yaw_pitch(&self) -> (f64, f64) {
        match self {
            NavTarget::Face(face) => face.view().yaw_pitch(),
            other => {
                let d = other.eye_direction();
                // The camera eye offset is (cp*sy, sp, cp*cy), so
                // yaw = atan2(dx, dz) (sin yaw / cos yaw), pitch = asin(dy).
                (d.x.atan2(d.z), d.y.clamp(-1.0, 1.0).asin())
            }
        }
    }

    /// A short deterministic label naming the target (fixpoint for the hover
    /// readout and transcripts; every component name comes from the shared
    /// `CubeFace::label` table).
    pub fn label(&self) -> String {
        match self {
            NavTarget::Face(f) => f.label().to_owned(),
            NavTarget::Edge([a, b]) => format!("{}-{}", a.label(), b.label()),
            NavTarget::Corner([a, b, c]) => format!("{}-{}-{}", a.label(), b.label(), c.label()),
        }
    }

    /// Which face a pointer resolving to this target should pre-highlight: the
    /// corner's or edge's incident face most facing the eye.
    pub fn primary_face(&self) -> CubeFace {
        match self {
            NavTarget::Face(f) => *f,
            other => {
                let faces: &[CubeFace] = match other {
                    NavTarget::Edge(e) => e.as_slice(),
                    NavTarget::Corner(c) => c.as_slice(),
                    NavTarget::Face(_) => unreachable!(),
                };
                let d = other.eye_direction();
                faces
                    .iter()
                    .copied()
                    .max_by(|a, b| {
                        face_normal(*a)
                            .dot(&d)
                            .partial_cmp(&face_normal(*b).dot(&d))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .expect("every target names at least one face")
            }
        }
    }

    /// The canonical orientation quaternion this target documents, built
    /// independently from the target's pure geometry (shared face normals and
    /// view angles) so the landed camera can be verified against it to 1e-9.
    pub fn orientation(&self) -> Option<Quat> {
        // The camera looks FROM the standard-view eye TOWARD the target, i.e.
        // along the reverse of the eye direction.
        look_quaternion(self.eye_direction().neg())
    }
}

/// The face the cube marks as current for a camera at `(yaw, pitch)`: the face
/// whose outward normal is closest to the eye direction, via the shared
/// [`CubeFace::from_direction`] hit test.
pub fn current_face(yaw: f64, pitch: f64) -> CubeFace {
    let (cp, sp) = (pitch.cos(), pitch.sin());
    let (cy, sy) = (yaw.cos(), yaw.sin());
    CubeFace::from_direction(&Vec3::new(cp * sy, sp, cp * cy))
}

// ---------------------------------------------------------------------------
// Orientation quaternions
// ---------------------------------------------------------------------------

/// The unit look direction (eye -> target) of the orbit camera.
fn camera_look_dir(cam: &Camera) -> Option<Vec3> {
    cam.target.sub(&cam.eye()).normalize().ok()
}

/// The view-orientation quaternion for a unit look direction `f` (eye toward
/// target): right/up built from `f.cross((0,1,0))` exactly as the orbit camera
/// builds its view basis, then extracted from the proper rotation
/// `(s, u, -f)` (right, up, back: the camera's actual view basis, whose third
/// vector points FROM the target TOWARD the eye) with
/// [`Quat::from_rotation_matrix`]. Building the third column as `-f` (not
/// `f`) keeps the matrix right-handed (det +1) for every look direction, so
/// the quaternion is a genuine rotation for every committed view.
pub fn look_quaternion(f: Vec3) -> Option<Quat> {
    let up0 = Vec3::new(0.0, 1.0, 0.0);
    if f.cross(&up0).norm() < 1e-12 {
        return None; // look direction parallel to world up (never a committed view)
    }
    let s = f.cross(&up0).normalize().ok()?;
    let u = s.cross(&f);
    Some(Quat::from_rotation_matrix(Mat3::from_cols(s, u, f.neg())))
}

/// The orientation quaternion of a real orbit camera (from its own view
/// basis). The acceptance asserts a clicked target lands this EXACTLY equal
/// (within 1e-9) to the target's independent [`NavTarget::orientation`].
pub fn camera_quaternion(cam: &Camera) -> Option<Quat> {
    look_quaternion(camera_look_dir(cam)?)
}

// ---------------------------------------------------------------------------
// Cube scene + camera (the small resident pass)
// ---------------------------------------------------------------------------

/// One dense part id per face so the shared viewer palette gives each face a
/// distinct, deterministic colour.
const FACE_PARTS: [&str; 6] = [
    "orient-cube-front",
    "orient-cube-back",
    "orient-cube-left",
    "orient-cube-right",
    "orient-cube-top",
    "orient-cube-bottom",
];

/// The eight corners of the unit cube (`[-1,1]^3`), in the renderer's corner
/// order (mirror of `Aabb::corners`).
fn cube_corners() -> [Vec3; 8] {
    [
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ]
}

/// The four face-corner indices for each face in [`CubeFace::ALL`] order
/// (outward normal follows the corner order).
fn face_corner_quads() -> [[usize; 4]; 6] {
    [
        [4, 5, 6, 7], // Front (+Z)
        [0, 1, 2, 3], // Back (-Z)
        [0, 3, 7, 4], // Left (-X)
        [1, 5, 6, 2], // Right (+X)
        [3, 2, 6, 7], // Top (+Y)
        [0, 4, 5, 1], // Bottom (-Y)
    ]
}

fn face_index(face: CubeFace) -> usize {
    CubeFace::ALL
        .iter()
        .position(|f| *f == face)
        .expect("face is one of the six")
}

/// Build the resident cube scene: six flat coloured face quads (each its own
/// mesh primitive so each face gets its own palette colour and can be
/// highlighted independently), centred at the origin.
fn cube_scene() -> Scene {
    let corners = cube_corners();
    let quads = face_corner_quads();
    let mut primitives = Vec::with_capacity(6);
    for (face, part) in CubeFace::ALL.iter().zip(FACE_PARTS) {
        let idx = quads[face_index(*face)];
        let positions: Vec<[f32; 3]> = idx.iter().map(|&i| corner_f32(&corners[i])).collect();
        let triangles = vec![[0u32, 1, 2], [0u32, 2, 3]];
        let edges: Vec<[usize; 2]> = vec![[0, 1], [1, 2], [2, 3], [3, 0]];
        primitives.push(Primitive::from_mesh(
            format!("orient-cube-{}", face.label()),
            part,
            "",
            MeshData {
                positions,
                triangles,
                edges,
            },
        ));
    }
    Scene {
        primitives,
        bounds: Some(oep_viewer::Aabb::new(
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, 1.0),
        )),
    }
}

fn corner_f32(p: &Vec3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

/// The cube's dedicated camera: rotation from the main camera's yaw/pitch
/// only, no translation (target pinned at the origin), a long lens so the
/// pass reads as orthographic.
pub fn cube_camera(yaw: f64, pitch: f64) -> Camera {
    Camera {
        target: Vec3::zero(),
        yaw,
        pitch,
        distance: CUBE_DISTANCE,
        fov_y: 2.0 * (1.0 / (CUBE_DISTANCE * 0.72)).atan(),
        near: CUBE_NEAR,
        far: CUBE_FAR,
    }
}

// ---------------------------------------------------------------------------
// Projection + hit test (pure over pass-pixel space)
// ---------------------------------------------------------------------------

/// The cube's 8 corners and 6 face corner-index tables projected into
/// pass-pixel space: the same view-projection the render pass uses, so the
/// pointer hits exactly what is drawn.
#[derive(Clone, Debug)]
pub struct CubeProjection {
    /// Pass size in pixels (square).
    pub pass_px: u32,
    /// Projected corner positions in pass pixels.
    pub corners: [egui::Pos2; 8],
    /// Face -> its 4 corner indices (in `face_corner_quads` order).
    pub face_quads: [[usize; 4]; 6],
}

/// Project `world` into pass-pixel space under `cam` (the same NDC mapping
/// the renderer uses).
fn project_world(world: &Vec3, cam: &Camera, pass_px: u32) -> Option<egui::Pos2> {
    let vp = cam.view_projection(1.0);
    let h = vp.transform_point4(world);
    if h.w.abs() <= 1e-9 {
        return None;
    }
    let (nx, ny, nz) = (h.x / h.w, h.y / h.w, h.z / h.w);
    if !(-1.0..=1.0).contains(&nz) {
        return None;
    }
    Some(egui::pos2(
        ((nx * 0.5 + 0.5) * f64::from(pass_px)) as f32,
        ((0.5 - ny * 0.5) * f64::from(pass_px)) as f32,
    ))
}

/// Project the cube for `cam` into a `pass_px` square pass.
pub fn project_cube(cam: &Camera, pass_px: u32) -> Option<CubeProjection> {
    let mut corners = [egui::Pos2::ZERO; 8];
    for (i, p) in cube_corners().iter().enumerate() {
        corners[i] = project_world(p, cam, pass_px)?;
    }
    Some(CubeProjection {
        pass_px,
        corners,
        face_quads: face_corner_quads(),
    })
}

impl CubeProjection {
    fn corner(&self, i: usize) -> egui::Pos2 {
        self.corners[i]
    }

    /// A face's 4 projected points, in order.
    fn face(&self, face: CubeFace) -> [egui::Pos2; 4] {
        let q = self.face_quads[face_index(face)];
        [
            self.corner(q[0]),
            self.corner(q[1]),
            self.corner(q[2]),
            self.corner(q[3]),
        ]
    }

    /// The projected endpoints of an edge between two faces (their two shared
    /// cube corners).
    fn edge_endpoints(&self, faces: [CubeFace; 2]) -> [egui::Pos2; 2] {
        let qa = self.face_quads[face_index(faces[0])];
        let qb = self.face_quads[face_index(faces[1])];
        let mut shared = Vec::with_capacity(2);
        for &a in &qa {
            if qb.contains(&a) {
                shared.push(self.corner(a));
            }
        }
        debug_assert_eq!(shared.len(), 2, "adjacent faces share an edge");
        [shared[0], shared[1]]
    }

    /// The 3D cube-corner index where three faces meet.
    fn corner_index(&self, faces: [CubeFace; 3]) -> usize {
        let sets: Vec<Vec<usize>> = faces
            .iter()
            .map(|f| self.face_quads[face_index(*f)].to_vec())
            .collect();
        (0..8)
            .find(|i| sets.iter().all(|s| s.contains(i)))
            .expect("three faces of a cube meet at an existing corner")
    }

    /// The feature's canonical click point in pass pixels: face centroid,
    /// edge midpoint, or corner point.
    pub fn target_image_point(&self, target: &NavTarget) -> egui::Pos2 {
        match target {
            NavTarget::Face(f) => centroid4(&self.face(*f)),
            NavTarget::Edge(faces) => {
                let [a, b] = self.edge_endpoints(*faces);
                egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
            }
            NavTarget::Corner(faces) => self.corner(self.corner_index(*faces)),
        }
    }

    /// Resolve a click at pass-pixel `p` onto a target. Priority order is the
    /// standard one (corner > edge > face); within a kind, the candidate whose
    /// 3D feature lies nearest the camera along the eye direction wins, so a
    /// click on a covered (hidden) feature resolves to the feature actually
    /// shown under the pointer.
    pub fn resolve_target(&self, cam: &Camera, p: egui::Pos2) -> Option<NavTarget> {
        let eye = current_eye_dir(cam);
        let corners_3d = cube_corners();

        // 1. Corners within radius (nearest corner along the eye axis wins).
        let mut best_corner: Option<(f64, NavTarget)> = None;
        for faces in CORNERS {
            let idx = self.corner_index(faces);
            let pos = self.corner(idx);
            if pos.distance(p) <= corner_radius(self.pass_px) {
                let d = corners_3d[idx].dot(&eye);
                if best_corner.is_none_or(|(bd, _)| d > bd) {
                    best_corner = Some((d, NavTarget::Corner(faces)));
                }
            }
        }
        if let Some((_, t)) = best_corner {
            return Some(t);
        }

        // 2. Edges within tolerance of their projected segment (nearest edge
        //    along the eye axis wins).
        let mut best_edge: Option<(f64, NavTarget)> = None;
        for faces in EDGES {
            let [a, b] = self.edge_endpoints(faces);
            if seg_dist(p, a, b) <= edge_tolerance(self.pass_px) {
                let d = edge_axis_depth(faces, &eye);
                if best_edge.is_none_or(|(bd, _)| d > bd) {
                    best_edge = Some((d, NavTarget::Edge(faces)));
                }
            }
        }
        if let Some((_, t)) = best_edge {
            return Some(t);
        }

        // 3. Faces containing the point (nearest, i.e. most facing the eye).
        let mut best_face: Option<(f64, CubeFace)> = None;
        for f in CubeFace::ALL {
            if point_in_quad(p, &self.face(f)) {
                let d = face_normal(f).dot(&eye);
                if best_face.is_none_or(|(bd, _)| d > bd) {
                    best_face = Some((d, f));
                }
            }
        }
        best_face.map(|(_, f)| NavTarget::Face(f))
    }
}

/// The camera's eye direction (target -> eye) as a unit vector.
fn current_eye_dir(cam: &Camera) -> Vec3 {
    cam.eye()
        .sub(&cam.target)
        .normalize()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// Mean depth of an edge's two endpoints along the eye axis (the edge's
/// alignment with the camera, used to rank edge candidates).
fn edge_axis_depth(faces: [CubeFace; 2], eye: &Vec3) -> f64 {
    let c = cube_corners();
    let qa = face_corner_quads()[face_index(faces[0])];
    let qb = face_corner_quads()[face_index(faces[1])];
    let mut sum = 0.0;
    let mut n = 0;
    for &a in &qa {
        if qb.contains(&a) {
            sum += c[a].dot(eye);
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

/// Distance from `p` to the segment `a`-`b`.
fn seg_dist(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 1e-9 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn corner_radius(pass_px: u32) -> f32 {
    CORNER_RADIUS * pass_px as f32 / CUBE_SIZE
}

fn edge_tolerance(pass_px: u32) -> f32 {
    EDGE_TOLERANCE * pass_px as f32 / CUBE_SIZE
}

fn centroid4(q: &[egui::Pos2; 4]) -> egui::Pos2 {
    egui::pos2(
        (q[0].x + q[1].x + q[2].x + q[3].x) / 4.0,
        (q[0].y + q[1].y + q[2].y + q[3].y) / 4.0,
    )
}

/// Point-in-convex-quad test (all edge cross products share a sign).
fn point_in_quad(p: egui::Pos2, q: &[egui::Pos2; 4]) -> bool {
    let mut sign: Option<f32> = None;
    for i in 0..4 {
        let a = q[i];
        let b = q[(i + 1) % 4];
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross.abs() < 1e-4 {
            continue;
        }
        match sign {
            None => sign = Some(cross.signum()),
            Some(s) if s != cross.signum() => return false,
            _ => {}
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Pass cache (the resident-scene-style dirty key)
// ---------------------------------------------------------------------------

/// Dirty key for the cube pass: a change re-rasterizes, steady state reuses
/// the cached pixels (zero cube work when the camera is idle).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PassKey {
    yaw_bits: u64,
    pitch_bits: u64,
    pass_px: u32,
    highlight: u8, // 0 = none, else the highlighted face's CubeFace::ALL index + 1
}

fn pass_key(yaw: f64, pitch: f64, pass_px: u32, highlight: Option<CubeFace>) -> PassKey {
    PassKey {
        yaw_bits: yaw.to_bits(),
        pitch_bits: pitch.to_bits(),
        pass_px,
        highlight: highlight.map_or(0, |f| face_index(f) as u8 + 1),
    }
}

/// The stored cube pass pixels.
#[derive(Clone, Default)]
struct PassCache {
    key: Option<PassKey>,
    image: Option<RgbaImageRef>,
}

/// A thin owned handle to the rendered pass (re-rendered only on a new key).
#[derive(Clone)]
struct RgbaImageRef {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

fn cache_id() -> egui::Id {
    egui::Id::unique("isolith.ui.viewport.nav_cube.pass_cache")
}

fn scene_id() -> egui::Id {
    egui::Id::unique("isolith.ui.viewport.nav_cube.scene")
}

fn backend_id() -> egui::Id {
    egui::Id::unique(VIEWPORT_STATE_KEY)
}

fn report_id() -> egui::Id {
    egui::Id::unique(NAV_CUBE_REPORT_KEY)
}

fn oracle_id() -> egui::Id {
    egui::Id::unique(NAV_CUBE_ORACLE_KEY)
}

fn backend(
    ctx: &egui::Context,
) -> Option<std::sync::Arc<std::sync::Mutex<Box<dyn oep_viewer::ViewportBackend>>>> {
    ctx.data(|d| d.get_temp(backend_id()))
}

/// The resident cube scene, built once and kept in the context store.
fn scene_store(ctx: &egui::Context) -> std::sync::Arc<std::sync::Mutex<Scene>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(scene_id(), || {
            std::sync::Arc::new(std::sync::Mutex::new(cube_scene()))
        })
        .clone()
    })
}

/// Current main-camera yaw/pitch from the backend, if reachable.
fn read_orientation(ctx: &egui::Context) -> Option<(f64, f64)> {
    let arc = backend(ctx)?;
    let cam = arc.lock().ok()?.camera();
    Some((cam.yaw, cam.pitch))
}

/// Apply an orientation to the real viewport camera (keep target, distance,
/// fov; set exactly yaw/pitch; re-rasterize through the seam).
fn apply_orientation(ctx: &egui::Context, yaw: f64, pitch: f64) {
    if let Some(arc) = backend(ctx) {
        if let Ok(mut guard) = arc.lock() {
            let mut cam = guard.camera();
            cam.yaw = yaw;
            cam.pitch = pitch;
            guard.set_camera(cam);
        }
    }
}

/// The cube's screen rect inside `viewport` (right-aligned inset, directly
/// above the D4 nav button panel). Public so the acceptance harness can
/// compute clicks against exactly what the panel draws.
pub fn cube_rect(viewport: egui::Rect) -> egui::Rect {
    let size = egui::Vec2::splat(CUBE_SIZE);
    let right = viewport.max.x - CUBE_INSET;
    let bottom = viewport.max.y - (NAV_CARD_H + NAV_CARD_GAP) - CUBE_INSET;
    let min = egui::pos2(right - size.x, bottom - size.y);
    egui::Rect::from_min_max(min, min + size)
}

/// A plain-scalar snapshot of the cube, published every frame so the
/// acceptance harness can assert the exact numbers without reaching
/// app-private types: `(click_to_align, current_face_index, rect_x0, rect_y0,
/// rect_x1, rect_y1)`. A plain std tuple of scalars so the TypeId matches
/// across the app and test compilations (the same pattern `NavSnapshot` uses).
pub type NavCubeScalars = (bool, u8, f32, f32, f32, f32);

/// Read the cube's last published scalar report, if any (acceptance harness).
/// `current` decodes via [`CubeFace::ALL`]; `None` before the first frame.
#[allow(dead_code)]
pub fn report(ctx: &egui::Context) -> Option<NavCubeScalars> {
    ctx.data(|d| d.get_temp(report_id()))
}

/// Draw the orientation cube for this frame. Called ONCE per frame from the
/// end of `nav::update` (the registration call site; nav.rs is this card's
/// owned file and its button-panel behaviour is untouched).
pub fn update(ui: &egui::Ui, viewport: egui::Rect, scale: f32, _app: &mut App) {
    let ctx = ui.ctx();
    let settings: Settings = settings::read(ctx);
    let click_to_align = settings.nav_cube_click_to_align;

    let rect = cube_rect(viewport);
    let pass_px = ((CUBE_SIZE * scale).clamp(32.0, 512.0)).round() as u32;

    let (yaw, pitch) = read_orientation(ctx).unwrap_or((0.7, 0.5));
    let cam = cube_camera(yaw, pitch);
    let proj = project_cube(&cam, pass_px);

    let current = current_face(yaw, pitch);

    // Hover pre-highlight (flag on) or the current face (flag off).
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let over = pointer.is_some_and(|p| rect.contains(p));
    let hover_target: Option<String> = if click_to_align && over {
        pointer
            .and_then(|p| {
                let local = to_pass_px(p, rect, pass_px);
                proj.as_ref().and_then(|pr| pr.resolve_target(&cam, local))
            })
            .map(|t| t.label())
    } else {
        None
    };
    let hover_face: Option<CubeFace> = if click_to_align && over {
        pointer.and_then(|p| {
            let local = to_pass_px(p, rect, pass_px);
            proj.as_ref()
                .and_then(|pr| pr.resolve_target(&cam, local))
                .map(|t| t.primary_face())
        })
    } else {
        None
    };
    // Highlight: the would-be hover target (flag on), else the current face.
    let highlight_face = if click_to_align {
        hover_face.or(Some(current))
    } else {
        Some(current)
    };

    // The pass cache: re-rasterize only when the key changed. The scene Arc is
    // fetched in its OWN context-data scope first, so no egui `data_mut` is
    // ever nested inside another (the egui store RwLock would deadlock); the
    // short scene lock below is a plain std Mutex, not the egui store.
    let scene_arc = scene_store(ctx);
    let cache_key = pass_key(yaw, pitch, pass_px, highlight_face);
    let pass = ctx.data_mut(|d| {
        let cache: &mut PassCache = d.get_temp_mut_or_insert_with(cache_id(), PassCache::default);
        if cache.key != Some(cache_key) {
            let mut guard = scene_arc.lock().expect("cube scene lock held briefly");
            for prim in &mut guard.primitives {
                prim.highlighted = false;
            }
            if let Some(face) = highlight_face {
                let id = format!("orient-cube-{}", face.label());
                if let Some(prim) = guard.primitives.iter_mut().find(|p| p.id == id) {
                    prim.highlighted = true;
                }
            }
            let rendered = oep_viewer::render::render(&guard, &cam, &[], pass_px, pass_px);
            cache.key = Some(cache_key);
            cache.image = Some(RgbaImageRef {
                width: rendered.width,
                height: rendered.height,
                data: rendered.data,
            });
        }
        cache.image.clone()
    });

    // Publish the scalar report + the String oracle.
    ctx.data_mut(|d| {
        d.insert_temp(
            report_id(),
            (
                click_to_align,
                face_index(current) as u8,
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.max.y,
            ),
        );
    });
    let oracle = format!(
        "click_to_align={} current={} hover={} rect={:.0},{:.0},{:.0},{:.0}\n",
        i32::from(click_to_align),
        current.label(),
        hover_target.as_deref().unwrap_or("none"),
        rect.min.x,
        rect.min.y,
        rect.max.x,
        rect.max.y,
    );
    ctx.data_mut(|d| d.insert_temp(oracle_id(), oracle));

    // a11y surface: the caption, readout and the six face-name line, as
    // AccessKit labels. The area is ALWAYS non-interactive: pointer input is
    // handled by the raw-input interaction above the layer stack (see below),
    // so in BOTH flag states the cube never swallows viewport input through a
    // widget hit-test, and the labels register regardless of interactivity.
    // (An interactable Area here would also have fought egui's pivot/sizing,
    // see the card proof.)
    egui::Area::new(egui::Id::unique("isolith_nav_cube_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .pivot(egui::Align2::LEFT_TOP)
        .movable(false)
        .interactable(false)
        .show(ctx, |ui| {
            let mode = theme::mode_of(ui);
            let p = oep_ui_theme::Palette::resolve(mode);

            // Caption + current-orientation readout + the six face names, all
            // as AccessKit labels. Every name comes from the SHARED face-name
            // table (`CubeFace::label`), the same table the button panel's
            // "Cube <face>" composition uses, so the two controls cannot drift.
            // The names are prefixed "Orientation cube" so they stay unique in
            // the accessibility tree (the button panel and the nav quick
            // buttons own the bare face names; `get_by_label` in the existing
            // nav suite queries those singularly).
            ui.label(
                egui::RichText::new("Orientation cube")
                    .size(9.0)
                    .color(p.text_muted),
            );
            ui.label(
                egui::RichText::new(format!("Orientation cube viewing {}", current.label()))
                    .size(9.0)
                    .color(p.text_primary),
            );
            let faces_line = CubeFace::ALL
                .iter()
                .map(|f| f.label())
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(
                egui::RichText::new(format!("Orientation cube faces: {faces_line}"))
                    .size(9.0)
                    .color(p.text_muted),
            );
        });

    // Interaction (flag on): an interactive widget on the viewport layer at
    // the cube rect, added LAST in the frame so it shadows the viewport's own
    // click/drag allocations over the cube (a press on the cube never reaches
    // the datum-plane pick). The cube does NOT orbit from its own drag: the
    // viewport's single raw-input orbit lane moves the camera during the drag
    // (one orbit, never doubled); the cube contributes the click-to-align
    // (press + release without movement resolves the target) and the
    // drag-release snap to the nearest of the 26 documented orientations.
    if click_to_align {
        let response = ui.interact(
            rect,
            egui::Id::unique("isolith_nav_cube_interact"),
            egui::Sense::click_and_drag(),
        );
        if response.clicked() {
            if let Some(at) = response.interact_pointer_pos() {
                let local = to_pass_px(at, rect, pass_px);
                if let Some(target) = proj.as_ref().and_then(|pr| pr.resolve_target(&cam, local)) {
                    let (y, pp) = target.yaw_pitch();
                    apply_orientation(ctx, y, pp);
                }
            }
        }
        if response.drag_stopped() {
            snap_nearest(ctx);
        }
    }

    // Draw the cube shapes on the foreground layer at screen coordinates
    // (painter, not widgets: the short face labels and the triad never create
    // AccessKit nodes, so they can use the compact shared names freely).
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("isolith_nav_cube_shapes"),
    ));

    if let Some(pass) = pass {
        let image = to_semi_transparent(&pass, 210);
        let tex_id = ctx.load_texture_cached(
            "isolith_nav_cube",
            egui::Id::unique("isolith_nav_cube_tex_cache"),
            image,
            egui::TextureOptions::LINEAR,
        );
        painter.image(
            tex_id.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // Short face labels on the visible faces (visual only; the AccessKit names
    // live in the area labels above). Positioned from the SAME projection the
    // pass used.
    if let Some(proj) = &proj {
        let mode = theme::mode_of(ui);
        let p = oep_ui_theme::Palette::resolve(mode);
        let eye = current_eye_dir(&cam);
        for face in CubeFace::ALL {
            if face_normal(face).dot(&eye) <= 0.0 {
                continue; // back-facing: no label on the drawn cube
            }
            let c = proj.target_image_point(&NavTarget::Face(face));
            let local = egui::pos2(
                c.x / pass_px as f32 * rect.width(),
                c.y / pass_px as f32 * rect.height(),
            );
            let is_highlight = highlight_face == Some(face);
            let color = if is_highlight {
                p.accent
            } else {
                egui::Color32::WHITE
            };
            painter.text(
                rect.min + local.to_vec2(),
                egui::Align2::CENTER_CENTER,
                face.view().label(),
                egui::FontId::proportional(10.0),
                color,
            );
        }
    }

    // The axis triad at the cube's lower left (three coloured arrows, drawn
    // only, no click targets).
    draw_triad(
        &painter,
        rect.min + egui::vec2(0.0, rect.height() + 20.0),
        16.0,
        &cam,
        235,
    );
}

/// Convert a screen pointer position to pass-pixel space within the cube rect.
fn to_pass_px(screen: egui::Pos2, rect: egui::Rect, pass_px: u32) -> egui::Pos2 {
    egui::pos2(
        (screen.x - rect.min.x) / rect.width() * pass_px as f32,
        (screen.y - rect.min.y) / rect.height() * pass_px as f32,
    )
}

/// Draw the axis triad: three coloured arrows, each arm along the projected
/// world axis under the current camera, plus its letter.
fn draw_triad(painter: &egui::Painter, origin: egui::Pos2, len: f32, cam: &Camera, alpha: u8) {
    let up0 = Vec3::new(0.0, 1.0, 0.0);
    let Ok(f) = camera_look_dir(cam).ok_or(()) else {
        return;
    };
    let s = f
        .cross(&up0)
        .normalize()
        .expect("cube camera is off-vertical");
    let u = s.cross(&f);
    let axes = [
        (Vec3::new(1.0, 0.0, 0.0), egui::Color32::DARK_RED, "X"),
        (Vec3::new(0.0, 1.0, 0.0), egui::Color32::DARK_GREEN, "Y"),
        (Vec3::new(0.0, 0.0, 1.0), egui::Color32::DARK_BLUE, "Z"),
    ];
    let a_alpha = alpha;
    for (axis, color, letter) in axes {
        let a = axis.dot(&s) as f32;
        let b = -axis.dot(&u) as f32; // top-left screen origin flips y
        let mag = (a * a + b * b).sqrt();
        if mag < 1e-4 {
            continue; // axis pointing straight at / away from the eye
        }
        let dir = egui::vec2(a, b) / mag;
        let tip = origin + dir * len;
        let stroke_color = color.gamma_multiply(f32::from(a_alpha) / 255.0);
        painter.line_segment([origin, tip], egui::Stroke::new(2.0, stroke_color));
        let back = origin + dir * (len - 4.0);
        let perp = egui::vec2(-dir.y, dir.x);
        painter.line_segment(
            [tip, back + perp * 3.0],
            egui::Stroke::new(2.0, stroke_color),
        );
        painter.line_segment(
            [tip, back - perp * 3.0],
            egui::Stroke::new(2.0, stroke_color),
        );
        painter.text(
            tip + dir * 3.0,
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(9.0),
            stroke_color,
        );
    }
}

/// Convert the rendered (opaque, dark-backdrop) pass into a semi-transparent
/// egui colour image: backdrop pixels become fully transparent, cube pixels
/// keep their colour at `alpha` so the model shows through. Built via the
/// bytes API (no `Color32::from_*` and no hex: the UI-A3 guard stays green).
fn to_semi_transparent(pass: &RgbaImageRef, alpha: u8) -> egui::ColorImage {
    let [br, bg, bb] = oep_viewer::theme::BACKGROUND_RGB;
    let mut bytes = Vec::with_capacity(pass.data.len());
    for chunk in pass.data.as_chunks::<4>().0 {
        if chunk[0] == br && chunk[1] == bg && chunk[2] == bb {
            bytes.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            bytes.extend_from_slice(&[chunk[0], chunk[1], chunk[2], alpha]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([pass.width as usize, pass.height as usize], &bytes)
}

/// Snap the camera to the nearest of the 26 documented orientations (drag
/// release with the flag on).
fn snap_nearest(ctx: &egui::Context) {
    let Some((yaw, pitch)) = read_orientation(ctx) else {
        return;
    };
    let cam = cube_camera(yaw, pitch);
    let Some(base_q) = camera_quaternion(&cam) else {
        return;
    };
    let mut best: Option<(f64, NavTarget)> = None;
    for t in NavTarget::ALL {
        if let Some(q) = t.orientation() {
            let dot = base_q.dot(&q).abs().clamp(0.0, 1.0);
            let ang = 2.0 * dot.clamp(-1.0, 1.0).acos();
            if best.as_ref().is_none_or(|(ba, _)| ang < *ba) {
                best = Some((ang, t));
            }
        }
    }
    if let Some((_, t)) = best {
        let (y, p) = t.yaw_pitch();
        apply_orientation(ctx, y, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_viewer::views::StandardView;

    fn assert_quat_close(a: &Quat, b: &Quat, tol: f64, tag: &str) {
        for (ai, bi, name) in [
            (a.w, b.w, "w"),
            (a.x, b.x, "x"),
            (a.y, b.y, "y"),
            (a.z, b.z, "z"),
        ] {
            let d = (ai - bi).abs();
            assert!(
                d < tol,
                "{tag}: quat mismatch on {name}: {d} (a=({},{},{},{}) b=({},{},{},{}))",
                a.w,
                a.x,
                a.y,
                a.z,
                b.w,
                b.x,
                b.y,
                b.z
            );
        }
    }

    /// Edge and corner tables describe real cube features (each edge joins two
    /// corners at distance 2, each corner triple meets at one corner).
    #[test]
    fn edge_and_corner_tables_are_geometrically_sound() {
        let c = cube_corners();
        let q = face_corner_quads();
        for faces in EDGES {
            let qa = q[face_index(faces[0])];
            let qb = q[face_index(faces[1])];
            let shared: Vec<usize> = qa.iter().copied().filter(|i| qb.contains(i)).collect();
            assert_eq!(shared.len(), 2, "edge {:?} joins two corners", faces);
            let pa = c[shared[0]];
            let pb = c[shared[1]];
            assert!((pa.distance_to(&pb) - 2.0).abs() < 1e-12, "edge length 2");
        }
        for faces in CORNERS {
            let sets: Vec<Vec<usize>> = faces.iter().map(|f| q[face_index(*f)].to_vec()).collect();
            let hits: Vec<usize> = (0..8)
                .filter(|i| sets.iter().all(|s| s.contains(i)))
                .collect();
            assert_eq!(hits.len(), 1, "corner {:?} meets at one corner", faces);
            assert!((c[hits[0]].norm() - 3.0_f64.sqrt()).abs() < 1e-12);
        }
    }

    /// Edge and corner orientations are the exact analytic 45 degree /
    /// isometric directions.
    #[test]
    fn edge_and_corner_directions_are_analytic() {
        let s3 = 3.0_f64.sqrt();
        let e = NavTarget::Edge([CubeFace::Front, CubeFace::Right]);
        let d = e.eye_direction();
        let inv_s2 = 1.0 / 2.0_f64.sqrt();
        assert!((d.x - inv_s2).abs() < 1e-12 && d.y.abs() < 1e-12 && (d.z - inv_s2).abs() < 1e-12);
        let c = NavTarget::Corner([CubeFace::Front, CubeFace::Right, CubeFace::Top]);
        let d = c.eye_direction();
        for comp in [d.x, d.y, d.z] {
            assert!((comp - 1.0 / s3).abs() < 1e-12);
        }
        let (yaw, pitch) = c.yaw_pitch();
        assert!((yaw - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert!((pitch - (1.0 / s3).asin()).abs() < 1e-12);
    }

    /// Landing the camera on any target makes the camera quaternion equal the
    /// target's independent quaternion within 1e-9.
    #[test]
    fn camera_quaternion_matches_target_orientation_to_1e9() {
        for target in NavTarget::ALL {
            let (yaw, pitch) = target.yaw_pitch();
            let cam = Camera {
                target: Vec3::new(40.0, 20.0, 5.0),
                yaw,
                pitch,
                distance: 120.0,
                fov_y: 0.785,
                near: 0.1,
                far: 5000.0,
            };
            let cq = camera_quaternion(&cam).expect("camera basis");
            let tq = target.orientation().expect("target orientation");
            assert_quat_close(&cq, &tq, 1e-9, &format!("target {}", target.label()));
        }
    }

    /// The current-face marking equals the literal face each standard view
    /// names (printed numbers for the proof).
    #[test]
    fn current_face_matches_literal_standard_views() {
        let cases = [
            (StandardView::Front, CubeFace::Front),
            (StandardView::Back, CubeFace::Back),
            (StandardView::Left, CubeFace::Left),
            (StandardView::Right, CubeFace::Right),
            (StandardView::Top, CubeFace::Top),
            (StandardView::Bottom, CubeFace::Bottom),
        ];
        for (view, expected) in cases {
            let (yaw, pitch) = view.yaw_pitch();
            let got = current_face(yaw, pitch);
            assert_eq!(got, expected, "{view:?} marks {:?}", got);
            println!(
                "analytic: view={:?} yaw={:.12} pitch={:.12} current={:?} expected={:?} match={}",
                view,
                yaw,
                pitch,
                got,
                expected,
                got == expected
            );
        }
    }

    /// At the four isometric-corner cameras the cube marks the face whose
    /// outward normal is closest to the eye direction, against literals
    /// derived from the shared `CubeFace::from_direction` tie-break (an exact
    /// isometric direction ties among the three principal axes; the hit test
    /// resolves the tie toward +Y/-Y by the eye direction's sign).
    #[test]
    fn current_face_at_four_isometric_corners() {
        let corners = [
            (Vec3::new(1.0, 1.0, 1.0), CubeFace::Top),
            (Vec3::new(-1.0, 1.0, 1.0), CubeFace::Top),
            (Vec3::new(1.0, -1.0, 1.0), CubeFace::Bottom),
            (Vec3::new(-1.0, -1.0, 1.0), CubeFace::Bottom),
        ];
        for (dir, expected) in corners {
            let d = dir.normalize().expect("corner direction");
            let yaw = d.x.atan2(d.z);
            let pitch = d.y.clamp(-1.0, 1.0).asin();
            let got = current_face(yaw, pitch);
            assert_eq!(got, expected);
            println!(
                "analytic-corner: dir=({:.6},{:.6},{:.6}) yaw={:.12} pitch={:.12} current={:?} expected={:?} match={}",
                d.x, d.y, d.z, yaw, pitch, got, expected, got == expected
            );
        }
    }

    /// A pointer at a target's own projected point resolves to that target
    /// (corner at its corner point, face at its centroid); this is the pure
    /// resolver half of the flag-on acceptance, exercised headlessly at a
    /// general pose.
    #[test]
    fn resolve_picks_each_target_at_its_own_point() {
        let pass_px = 96;
        let cam = cube_camera(0.7, 0.5);
        let proj = project_cube(&cam, pass_px).expect("cube projects at a general pose");
        let mut exact = 0;
        for t in NavTarget::ALL {
            let pt = proj.target_image_point(&t);
            let resolved = proj.resolve_target(&cam, pt);
            if resolved == Some(t) {
                exact += 1;
            } else if resolved.is_some() {
                println!(
                    "resolve: {:?} at its own point resolves to {:?} (covered at this pose)",
                    t.label(),
                    resolved.unwrap().label()
                );
            }
        }
        // Faces are never covered at a general pose (their centroids sit in
        // their own quad and no corner/edge coincides), so all six plus every
        // corner resolve exactly.
        assert!(
            exact >= 6 + 8,
            "expected >= 14 exact self-resolutions, got {exact}"
        );
    }

    /// The projection maps cube corners into the pass and the front face
    /// centroid is dead-centre at the front view.
    #[test]
    fn projection_is_flush_and_centred() {
        let pass_px = 96;
        let proj = project_cube(&cube_camera(0.0, 0.0), pass_px).expect("front view projects");
        for (i, p) in proj.corners.iter().enumerate() {
            assert!(
                p.x >= -1.0
                    && p.x <= pass_px as f32 + 1.0
                    && p.y >= -1.0
                    && p.y <= pass_px as f32 + 1.0,
                "corner {i} inside the pass"
            );
        }
        let c = proj.target_image_point(&NavTarget::Face(CubeFace::Front));
        assert!((c.x - pass_px as f32 / 2.0).abs() < 8.0);
        assert!((c.y - pass_px as f32 / 2.0).abs() < 8.0);
    }

    /// The acceptance pose probe: posed at each target's own nudged base, a
    /// pointer at that target's projected point resolves to the target itself
    /// (the exact resolver path the flag-on clicks take). This runs the pure
    /// half of the acceptance so the kittest leg only proves the end-to-end
    /// click.
    #[test]
    fn each_target_resolves_at_its_own_pose() {
        let pass_px = 96;
        let mut bad = Vec::new();
        for target in NavTarget::ALL {
            let (y0, p0) = target.yaw_pitch();
            let base_pitch = if p0.abs() > 1.5 {
                p0 - p0.signum() * 0.11
            } else {
                p0 - 0.11
            };
            let base_yaw = y0 + 0.17;
            let cam = cube_camera(base_yaw, base_pitch);
            let proj = project_cube(&cam, pass_px).expect("cube projects at base pose");
            let pt = proj.target_image_point(&target);
            let got = proj.resolve_target(&cam, pt);
            if got != Some(target) {
                bad.push(format!(
                    "{} at its own pose resolves to {:?} (point {}, {})",
                    target.label(),
                    got.map(|t| t.label()),
                    pt.x,
                    pt.y
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "self-resolution failed at base poses:\n{}",
            bad.join("\n")
        );
    }

    /// The cube placement: the implemented rect is clear of the D2 shading
    /// card's committed rect (top-right) and sits above the nav button panel.
    #[test]
    fn cube_rect_is_clear_of_shading_card() {
        // A 1280x800 window with the committed default shell layout produces
        // this viewport rect (tree 240, props 300, palette 32, status 26).
        let viewport = egui::Rect::from_min_max(egui::pos2(240.0, 32.0), egui::pos2(980.0, 774.0));
        let cube = cube_rect(viewport);
        // The shading card's committed rect (mirror of shading::shading_rects).
        let shading_card = egui::Rect::from_min_size(
            egui::pos2(viewport.max.x - 148.0 - 8.0, viewport.min.y + 8.0),
            egui::vec2(148.0, 142.0),
        );
        assert!(
            !cube.intersects(shading_card),
            "cube at bottom-right must not overlap the top-right shading card"
        );
        // Right edge honoured: inset CUBE_INSET from the viewport right edge.
        assert!((viewport.max.x - cube.max.x - CUBE_INSET).abs() < 1e-3);
        // The cube's bottom clearances: above the nav card (viewport bottom).
        let nav_top = viewport.max.y - (NAV_CARD_H + NAV_CARD_GAP);
        assert!(cube.max.y <= nav_top - CUBE_INSET + 1e-3);
    }
}
