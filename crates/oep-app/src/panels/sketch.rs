//! Sketch mode state and the sketch grid layer (wave B, card UI-B3 UI-SKETCH-ENTER).
//!
//! Entering sketch mode is a viewport behaviour (UI-B3 picks a datum plane on
//! click and swings the camera normal to it), but the *state* and the *grid
//! layer* it drives live here so the viewport panel, the model tree and the
//! test harness can share them without touching `app.rs`:
//!
//! - [`SketchState`] is stored in the egui context data store, the same
//!   cross-panel seam the viewport backend and the doc panel use. A plain
//!   `String` oracle under [`SKETCH_REPORT_KEY`] is published every frame so
//!   the model tree (`tree.rs`) and the `--test sketch_enter` harness can read
//!   the mode and the active plane through a cross-`#[path]`-copy-safe type.
//! - The three datum planes ([`datum_planes`]) are the principal world planes
//!   through the scene centre. [`pick_plane`] resolves the clicked ray (built
//!   with the viewport's existing pick seam [`oep_viewer::pick::unproject`])
//!   to the nearest plane under the cursor; no second picker is introduced.
//! - [`normal_orientation`] swings the camera to look exactly along a plane's
//!   normal while keeping the fixed world-up (`oep_viewer`'s look-at uses
//!   up = (0,1,0)) from degenerating.
//! - [`grid_overlay`] projects the active plane's sketch grid onto the panel
//!   so the viewport paints the grid layer on top of the rendered frame.
//!
//! Escape leaves sketch mode without touching a document: the viewport panel
//! consumes Escape only while a plane is active and writes [`SketchState`]
//! back with `plane = None`; no document code is reached.

use eframe::egui;

use oep_viewer::pick::Ray;
use oep_viewer::{Scene, Vec3};

/// Context-data key under which the sketch [`SketchState`] struct lives
/// (viewport panel and this module only; a custom type is not safe across
/// `#[path]` copies, so the tree and tests read the `String` oracle instead).
pub const SKETCH_STATE_KEY: &str = "ui_b3_sketch_state";
/// Context-data key under which the sketch `String` oracle is published.
/// Read back by `tree.rs` and `tests/sketch_enter.rs` as a plain `String`.
pub const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";

/// Stable ids of the three principal datum planes.
pub const PLANE_XY: &str = "XY";
pub const PLANE_XZ: &str = "XZ";
pub const PLANE_YZ: &str = "YZ";

/// Spacing (world units) between sketch grid lines on the active plane.
pub const SKETCH_GRID_SPACING: f64 = 10.0;

/// A pickable, bounded datum plane in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatumPlane {
    /// Stable id (`XY`, `XZ` or `YZ`).
    pub id: &'static str,
    /// Human label shown in the tree and status bar.
    pub label: &'static str,
    /// Plane centre point (through the scene centre).
    pub point: Vec3,
    /// First in-plane unit axis.
    pub e1: Vec3,
    /// Second in-plane unit axis.
    pub e2: Vec3,
    /// Unit plane normal (`= e1 x e2`).
    pub normal: Vec3,
    /// Half size along `e1`.
    pub half_w: f64,
    /// Half size along `e2`.
    pub half_h: f64,
}

/// The three principal world datum planes through the scene's bounding-box
/// centre, each sized to cover the model's cross-section with a 40% margin so
/// the sheets are easy to click from the framed camera. An empty scene falls
/// back to origin planes at the demo model's footprint so picking still works.
pub fn datum_planes(scene: &Scene) -> Vec<DatumPlane> {
    let (c, mx, my, mz) = match &scene.bounds {
        Some(b) => {
            let ext = b.max.sub(&b.min);
            (b.centroid(), ext.x * 0.7, ext.y * 0.7, ext.z * 0.7)
        }
        None => (Vec3::zero(), 56.0, 28.0, 7.0),
    };
    vec![
        DatumPlane {
            id: PLANE_XY,
            label: "XY (Top)",
            point: c,
            e1: Vec3::new(1.0, 0.0, 0.0),
            e2: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            half_w: mx,
            half_h: my,
        },
        DatumPlane {
            id: PLANE_XZ,
            label: "XZ (Front)",
            point: c,
            e1: Vec3::new(1.0, 0.0, 0.0),
            e2: Vec3::new(0.0, 0.0, -1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            half_w: mx,
            half_h: mz,
        },
        DatumPlane {
            id: PLANE_YZ,
            label: "YZ (Right)",
            point: c,
            e1: Vec3::new(0.0, 1.0, 0.0),
            e2: Vec3::new(0.0, 0.0, 1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            half_w: my,
            half_h: mz,
        },
    ]
}

/// Ray-plane intersection returning the ray parameter `t` when the hit lies
/// inside the plane's rectangle; `None` on a miss, a glancing (parallel) ray
/// or a hit behind the ray origin.
fn plane_ray_hit(plane: &DatumPlane, ray: &Ray) -> Option<f64> {
    let denom = ray.dir.dot(&plane.normal);
    if denom.abs() < 1e-12 {
        return None; // ray parallel to the plane
    }
    let t = plane.point.sub(&ray.origin).dot(&plane.normal) / denom;
    if t < 0.0 {
        return None; // plane behind the ray
    }
    let hit = ray.at(t);
    let d1 = hit.sub(&plane.point).dot(&plane.e1);
    let d2 = hit.sub(&plane.point).dot(&plane.e2);
    if d1.abs() <= plane.half_w && d2.abs() <= plane.half_h {
        Some(t)
    } else {
        None
    }
}

/// The datum plane a ray picks: the nearest hit under the cursor, with an
/// exact-tie tiebreak to the sheet most normal to the view (the one the user
/// is looking at) so a click through the shared scene centre is deterministic.
pub fn pick_plane<'a>(
    planes: &'a [DatumPlane],
    ray: &Ray,
    view_dir: &Vec3,
) -> Option<&'a DatumPlane> {
    let mut best: Option<(&'a DatumPlane, f64, f64)> = None;
    for p in planes {
        let Some(t) = plane_ray_hit(p, ray) else {
            continue;
        };
        let frontal = p.normal.dot(view_dir).abs();
        let take = match best {
            None => true,
            Some((_, bt, bf)) => {
                if (t - bt).abs() > 1e-9 {
                    t < bt
                } else {
                    frontal > bf
                }
            }
        };
        if take {
            best = Some((p, t, frontal));
        }
    }
    best.map(|(p, _, _)| p)
}

/// Yaw/pitch that make the camera look exactly along world `-n` (a plane
/// normal), so the view is normal to the plane. `pitch` is clamped away from
/// exactly +/- 90 deg so `oep_viewer`'s fixed world-up look-at (up = (0,1,0))
/// never degenerates on a vertical plane normal.
pub fn normal_orientation(n: &Vec3) -> (f64, f64) {
    let pitch = n.y.clamp(-1.0, 1.0).asin().clamp(-1.54, 1.54);
    let yaw = n.x.atan2(n.z);
    (yaw, pitch)
}

/// Which datum plane is currently being sketched on.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivePlane {
    /// Plane id (`XY`, `XZ` or `YZ`).
    pub id: String,
    /// Human label.
    pub label: String,
    /// Unit plane normal `(x, y, z)`.
    pub normal: (f64, f64, f64),
}

/// Sketch mode state, stored in the egui context data store.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchState {
    /// `Some(plane)` while sketching on a datum plane; `None` in normal
    /// modelling mode. The sketch grid layer is active exactly when this is
    /// `Some`.
    pub plane: Option<ActivePlane>,
}

impl SketchState {
    /// True while a datum plane is active (sketch mode on, grid layer on).
    pub fn in_sketch(&self) -> bool {
        self.plane.is_some()
    }
}

/// Read the sketch state from the egui context data store.
pub fn read_state(ctx: &egui::Context) -> SketchState {
    ctx.data(|d| d.get_temp::<SketchState>(egui::Id::unique(SKETCH_STATE_KEY)))
        .unwrap_or_default()
}

/// The `String` oracle for the current sketch state: the cross-`#[path]`-copy
/// -safe form the model tree and test harness parse.
pub fn report(state: &SketchState) -> String {
    match &state.plane {
        None => "mode=idle\nplane=-\ngrid=off\n".to_string(),
        Some(p) => format!(
            "mode=sketch\nplane={}\nlabel={}\nnormal={},{},{}\ngrid=on\n",
            p.id, p.label, p.normal.0, p.normal.1, p.normal.2
        ),
    }
}

/// Persist the sketch state: the struct (for the viewport panel) plus the
/// plain `String` oracle (for the tree and the test harness).
pub fn write_state(ctx: &egui::Context, state: &SketchState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(SKETCH_STATE_KEY), state.clone()));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY), report(state)));
}

/// Project a world point onto the viewport panel in logical points (top-left
/// origin), mirroring the picker's unproject divide. `None` behind the near
/// plane (w <= 0). Public so the test harness can turn a world point on a
/// datum plane into the exact click position the panel would resolve.
pub fn world_to_panel(
    cam: &oep_viewer::Camera,
    panel: egui::Rect,
    scale: f32,
    p: &Vec3,
) -> Option<egui::Pos2> {
    let px = (panel.width() * scale).round().max(2.0) as f64;
    let py = (panel.height() * scale).round().max(2.0) as f64;
    let vp = cam.view_projection(px / py);
    let a = vp.to_array16();
    let cw = a[3] * p.x + a[7] * p.y + a[11] * p.z + a[15];
    if cw <= 0.0 {
        return None;
    }
    let iw = 1.0 / cw;
    let cx = (a[0] * p.x + a[4] * p.y + a[8] * p.z + a[12]) * iw;
    let cy = (a[1] * p.x + a[5] * p.y + a[9] * p.z + a[13]) * iw;
    let nx = cx * 0.5 + 0.5;
    let ny = 1.0 - (cy * 0.5 + 0.5); // screen y grows downward
    Some(egui::pos2(
        panel.min.x + (nx * px / f64::from(scale)) as f32,
        panel.min.y + (ny * py / f64::from(scale)) as f32,
    ))
}

/// The sketch grid layer for the active plane, pre-projected to panel points.
#[derive(Debug, Clone, Default)]
pub struct GridOverlay {
    /// The four projected plane corners (the highlight outline).
    pub corners: Vec<egui::Pos2>,
    /// Projected grid segments inside the plane rectangle.
    pub segments: Vec<(egui::Pos2, egui::Pos2)>,
    /// The projected plane centre (pick-marker anchor).
    pub centre: egui::Pos2,
}

/// Compute the sketch grid + highlight geometry for a plane over the panel.
/// `None` when the plane would be drawn behind the camera.
pub fn grid_overlay(
    plane: &DatumPlane,
    cam: &oep_viewer::Camera,
    panel: egui::Rect,
    scale: f32,
) -> Option<GridOverlay> {
    let corner = |u: f64, v: f64| {
        world_to_panel(
            cam,
            panel,
            scale,
            &plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(v)),
        )
    };
    let (w, h) = (plane.half_w, plane.half_h);
    let corners: Vec<egui::Pos2> = [(-w, -h), (w, -h), (w, h), (-w, h)]
        .iter()
        .map(|&(u, v)| corner(u, v))
        .collect::<Option<_>>()?;
    let centre = corner(0.0, 0.0)?;

    let mut segments = Vec::new();
    let mut add_line = |a: Vec3, b: Vec3| {
        if let (Some(pa), Some(pb)) = (
            world_to_panel(cam, panel, scale, &a),
            world_to_panel(cam, panel, scale, &b),
        ) {
            segments.push((pa, pb));
        }
    };
    let n1 = (plane.half_w / SKETCH_GRID_SPACING).floor() as i64;
    let n2 = (plane.half_h / SKETCH_GRID_SPACING).floor() as i64;
    for i in -n1..=n1 {
        let u = i as f64 * SKETCH_GRID_SPACING;
        let a = plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(-h));
        let b = plane.point.add(&plane.e1.mul(u)).add(&plane.e2.mul(h));
        add_line(a, b);
    }
    for j in -n2..=n2 {
        let v = j as f64 * SKETCH_GRID_SPACING;
        let a = plane.point.add(&plane.e2.mul(v)).add(&plane.e1.mul(-w));
        let b = plane.point.add(&plane.e2.mul(v)).add(&plane.e1.mul(w));
        add_line(a, b);
    }

    Some(GridOverlay {
        corners,
        segments,
        centre,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_viewer::pick::unproject;
    use oep_viewer::Camera;

    /// Each plane's normal is the cross of its in-plane axes (the invariant
    /// `grid_overlay` and the camera swing rely on).
    #[test]
    fn every_plane_has_consistent_axes() {
        let scene = oep_viewer::CpuBackend::demo_scene();
        for p in datum_planes(&scene) {
            let n = p.e1.cross(&p.e2);
            assert!(
                n.sub(&p.normal).norm() < 1e-12,
                "{} normal must equal e1 x e2",
                p.id
            );
            assert!((p.e1.norm() - 1.0).abs() < 1e-12);
            assert!((p.e2.norm() - 1.0).abs() < 1e-12);
        }
    }

    /// A centre-ish click (the default framed camera looks at the scene
    /// centre, where all three sheets cross) picks the sheet most normal to
    /// the view, and the chosen plane is deterministic.
    #[test]
    fn nearest_and_most_frontal_plane_wins() {
        let scene = oep_viewer::CpuBackend::demo_scene();
        let planes = datum_planes(&scene);
        let cam = Camera::default();
        let f = cam.target.sub(&cam.eye()).normalize().expect("camera");
        let ray = unproject(&cam, 400.0, 300.0, 800.0, 600.0);
        let picked = pick_plane(&planes, &ray, &f).expect("centre click picks a plane");
        let most_frontal = planes
            .iter()
            .max_by(|a, b| {
                a.normal
                    .dot(&f)
                    .abs()
                    .partial_cmp(&b.normal.dot(&f).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("planes are non-empty");
        assert_eq!(picked.id, most_frontal.id);
        assert_eq!(
            picked.id, PLANE_XY,
            "from the default view the top sheet faces the camera"
        );
    }

    /// normal_orientation turns the camera to look exactly along the normal:
    /// for the safe (non-vertical) normals the forward vector is parallel to
    /// the plane normal within tight tolerance.
    #[test]
    fn orientation_looks_along_the_normal() {
        for n in [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(-1.0, 0.0, 0.0),
        ] {
            let (yaw, pitch) = normal_orientation(&n);
            let cam = Camera {
                yaw,
                pitch,
                ..Camera::default()
            };
            let f = cam
                .target
                .sub(&cam.eye())
                .normalize()
                .expect("non-degenerate camera");
            let cross_mag = f.cross(&n).norm();
            assert!(
                cross_mag < 1e-6,
                "camera forward must be normal to the plane (cross={cross_mag:e})"
            );
        }
    }

    /// The vertical plane normal stays safe: orientation never points the
    /// fixed-up camera straight along the world Y axis (which would make the
    /// look-at degenerate); it clamps within the camera's own safe band.
    #[test]
    fn vertical_normal_never_degenerates() {
        let (yaw, pitch) = normal_orientation(&Vec3::new(0.0, 1.0, 0.0));
        assert!(pitch.abs() < 1.55, "pitch clamped off vertical");
        let cam = Camera {
            yaw,
            pitch,
            ..Camera::default()
        };
        let f = cam
            .target
            .sub(&cam.eye())
            .normalize()
            .expect("still not degenerate");
        let _ = cam.view_matrix(); // must not panic
        assert!(
            f.cross(&Vec3::new(0.0, 1.0, 0.0)).norm() > 1e-3,
            "forward must keep a safe angle away from the world up axis"
        );
    }
}
