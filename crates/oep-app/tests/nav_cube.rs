//! UI-E6 UI-NAV-CUBE: 3D orientation cube beside the existing button panel
//! (wave E).
//!
//! Acceptance harness. Drives the REAL `app::App` through `egui_kittest` and
//! asserts, numerically:
//!
//! 1. **Analytic current-face marking.** For each of the six standard views
//!    plus the four isometric corners, the face the cube marks as current
//!    equals the face the camera's view command produces (the module-level
//!    analytic tests print the numbers; this file also drives the real nav
//!    buttons and reads the published cube report live).
//! 2. **Flag-off click-through.** With `nav_cube_click_to_align` OFF (the
//!    default) a drag that starts inside the cube's rect reaches the viewport
//!    controls: the camera orbits by exactly the drag delta times
//!    `ORBIT_RAD_PER_PX`, proving the cube neither swallows the orbit nor
//!    captures the pick surface.
//! 3. **Flag-on click alignment.** With the flag ON, clicking each of the 26
//!    targets (6 faces, 12 edges, 8 corners) lands the camera quaternion
//!    EXACTLY on the target's documented orientation within 1e-9, reusing the
//!    assertion style UI-D4 established (printed per-target numbers).
//! 4. **Flag-on drag with snap.** Dragging the cube orbits the camera; on
//!    release the camera snaps onto one of the 26 documented orientations
//!    (asserted to 1e-9 against the nearest one).
//! 5. **Reachable without a mouse.** The cube exposes accessible labels for
//!    the six faces (from the shared view-name table) and the
//!    current-orientation readout; the existing nav-suite labels stay
//!    unique and clickable (122 assertions over in `tests/nav.rs`).

// The pure geometry + settings modules are included once at this crate root
// (so the pure analytic tests reach `nav_cube::*`/`settings::*` without the
// whole shell) and a second time inside the `mod app` tree (the real panel the
// harness drives). Both loads are the same files; the duplication is
// intentional, so clippy's `duplicate_mod` is allowed here.
#![allow(clippy::duplicate_mod)]

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The cube's module and its settings surface, included at the test crate root
// (crate-root modules, so `nav_cube`'s `super::settings` resolves and the pure
// geometry + flag are reachable by public path without duplicating the whole
// shell). The cube only touches `App` as a type, and the published
// context-data keys it shares with the real app are plain strings, so the
// harness drives the same live panel state.
#[path = "../src/panels/nav_cube.rs"]
pub mod nav_cube;

#[path = "../src/settings.rs"]
pub mod settings;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use oep_geo::math::Vec3;
use oep_viewer::views::{CubeFace, StandardView};

use nav_cube::NavTarget;

/// The viewport backend's context-data key (must match
/// `nav_cube::VIEWPORT_STATE_KEY`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";

fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// Read the live viewport backend camera from the context store (the same
/// `Arc<Mutex<Box<dyn ViewportBackend>>>` the viewport panel owns).
fn set_camera(h: &Harness<'_, app::App>, yaw: f64, pitch: f64) {
    h.ctx.data_mut(|d| {
        if let Some(arc) = d
            .get_temp::<std::sync::Arc<std::sync::Mutex<Box<dyn oep_viewer::ViewportBackend>>>>(
                egui::Id::unique(VIEWPORT_STATE_KEY),
            )
        {
            if let Ok(mut guard) = arc.lock() {
                let mut cam = guard.camera();
                cam.yaw = yaw;
                cam.pitch = pitch;
                guard.set_camera(cam);
            }
        }
    });
}

/// The live backend camera (yaw, pitch), if reachable.
fn read_pose(h: &Harness<'_, app::App>) -> Option<(f64, f64)> {
    h.ctx.data_mut(|d| {
        let arc: std::sync::Arc<std::sync::Mutex<Box<dyn oep_viewer::ViewportBackend>>> =
            d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY))?;
        let cam = arc.lock().ok()?.camera();
        Some((cam.yaw, cam.pitch))
    })
}

/// Set the backend camera, then run frames until the cube publishes a report
/// for that pose.
fn pose_and_settle(h: &mut Harness<'_, app::App>, yaw: f64, pitch: f64) {
    set_camera(h, yaw, pitch);
    for _ in 0..4 {
        h.step();
    }
}

/// The screen point where `target` projects at the CURRENT camera pose, using
/// the same cube camera / projection the panel draws with, mapped into the
/// cube's published screen rect.
fn target_screen_point(h: &Harness<'_, app::App>, target: &NavTarget) -> egui::Pos2 {
    let report = cube_report(h);
    let (yaw, pitch) = read_pose(h).expect("backend pose reachable");
    let pass_px = (nav_cube::CUBE_SIZE * h.ctx.pixels_per_point()).round() as u32;
    let cam = nav_cube::cube_camera(yaw, pitch);
    let proj = nav_cube::project_cube(&cam, pass_px).expect("cube projects at this pose");
    let img = proj.target_image_point(target);
    egui::pos2(
        report.rect.min.x + img.x / pass_px as f32 * report.rect.width(),
        report.rect.min.y + img.y / pass_px as f32 * report.rect.height(),
    )
}

/// A clean click (move, press, release) at `pos`, then two more frames so the
/// panel applies and the backend re-rasterises.
fn click_at(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.step();
}

fn quat_close(a: &oep_geo::math::Quat, b: &oep_geo::math::Quat, tol: f64) -> bool {
    [a.w - b.w, a.x - b.x, a.y - b.y, a.z - b.z]
        .iter()
        .all(|d| d.abs() < tol)
}

/// The cube's scalar report decoded into a small struct (the published value
/// is a plain std tuple so the harness TypeId matches the app's copy).
struct CubeReport {
    click_to_align: bool,
    current: CubeFace,
    rect: egui::Rect,
}

fn cube_report(h: &Harness<'_, app::App>) -> CubeReport {
    let (click, idx, x0, y0, x1, y1) = nav_cube::report(&h.ctx).expect("cube report published");
    CubeReport {
        click_to_align: click,
        current: CubeFace::ALL[idx as usize],
        rect: egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),
    }
}

/// Analytic leg: the current-face marking for the six standard views plus the
/// four isometric corners, printed as numbers (numbers in the proof, not a
/// sentence). Runs the pure marking logic AND drives the real app: set each
/// view through the nav buttons and read the published cube report.
#[test]
fn analytic_current_face_table_standard_views_and_isometric_corners() {
    let mut harness = real_app_harness();
    harness.run();

    // Six standard views, driven through the real nav quick buttons.
    let views = [
        ("Front", StandardView::Front, CubeFace::Front),
        ("Back", StandardView::Back, CubeFace::Back),
        ("Left", StandardView::Left, CubeFace::Left),
        ("Right", StandardView::Right, CubeFace::Right),
        ("Top", StandardView::Top, CubeFace::Top),
        ("Bottom", StandardView::Bottom, CubeFace::Bottom),
    ];
    for (label, view, expected) in views {
        // The nav panel owns the quick "Front"/"Back"/... buttons by label
        // (edge buttons carry the "Cube " prefix); the cube's own labels are
        // unique strings so no singular get resolves to two nodes.
        let button = if matches!(
            view,
            StandardView::Front | StandardView::Top | StandardView::Right
        ) {
            label.to_owned()
        } else {
            format!("Cube {label}")
        };
        harness.get_by_label(&button).click();
        harness.step();
        harness.step();
        let report = cube_report(&harness);
        let (yaw, pitch) = view.yaw_pitch();
        println!(
            "analytic: view={label} yaw={yaw:.12} pitch={pitch:.12} current={:?} expected={:?} match={}",
            report.current,
            expected,
            report.current == expected
        );
        assert_eq!(report.current, expected, "{label} marks the committed face");
    }

    // Four isometric corners (the +Z tetrad), set directly on the backend.
    // An exact isometric direction ties among the three principal axes, so the
    // current face follows the shared from_direction tie-break toward +Y/-Y.
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
        pose_and_settle(&mut harness, yaw, pitch);
        let report = cube_report(&harness);
        println!(
            "analytic-corner: dir=({:.6},{:.6},{:.6}) yaw={yaw:.12} pitch={pitch:.12} current={:?} expected={:?} match={}",
            d.x, d.y, d.z, report.current, expected, report.current == expected
        );
        assert_eq!(
            report.current, expected,
            "corner ({}, {}, {})",
            d.x, d.y, d.z
        );
    }
}

/// Flag-off leg: with the interaction flag OFF (the shipped default), a drag
/// that starts inside the cube's rect reaches the viewport controls: the
/// camera orbits by exactly `dx * ORBIT_RAD_PER_PX`, the same numeric mapping
/// the viewport test asserts. No orbit or pick is swallowed.
#[test]
fn flag_off_drag_inside_cube_reaches_viewport_orbit() {
    const ORBIT_RAD_PER_PX: f64 = 0.005;
    let mut harness = real_app_harness();
    harness.run();

    // Default flag state is off, and the cube must report it.
    let report = cube_report(&harness);
    assert!(
        !report.click_to_align,
        "the interaction flag must default OFF (patent-conservative)"
    );

    // Drag horizontally across the cube's own rect.
    let center = report.rect.center();
    let dx = 60.0;
    let start = egui::pos2(center.x - dx / 2.0, center.y);
    let end = egui::pos2(center.x + dx / 2.0, center.y);
    let before = read_pose(&harness).expect("pose before drag");

    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    let mid = egui::pos2(center.x, center.y);
    harness.event(egui::Event::PointerMoved(mid));
    harness.step();
    harness.event(egui::Event::PointerMoved(end));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();

    let after = read_pose(&harness).expect("pose after drag");
    let expected_delta = f64::from(dx) * ORBIT_RAD_PER_PX;
    assert!(
        (after.0 - before.0 - expected_delta).abs() < 1e-9,
        "cube must not swallow the orbit drag: yaw moved {} rad, expected {expected_delta}",
        after.0 - before.0
    );
    assert!(
        (after.1 - before.1).abs() < 1e-9,
        "horizontal drag through the cube must not change pitch"
    );
    println!(
        "flag-off: drag {start:?} -> {end:?} orbited yaw {:.12} -> {:.12} (delta {:.12} == 60 * 0.005)",
        before.0, after.0, after.0 - before.0
    );
    // And a clean click inside the cube rect must not move the camera at all
    // (no swallowed pick/orbit on a click either).
    let before2 = read_pose(&harness).expect("pose before click");
    click_at(&mut harness, center);
    let after2 = read_pose(&harness).expect("pose after click");
    assert!((after2.0 - before2.0).abs() < 1e-9 && (after2.1 - before2.1).abs() < 1e-9);
    let report = cube_report(&harness);
    println!(
        "flag-off: click at cube {:?} passed through (camera pose unchanged), report.click_to_align={}",
        center,
        report.click_to_align
    );
}

/// Flag-on leg: clicking each face, edge and corner lands the camera
/// quaternion on the target's documented orientation within 1e-9. Each target
/// is posed frontmost first (its own committed orientation nudged slightly),
/// so the click both PROVES resolution and MOVES the camera onto the exact
/// orientation.
#[test]
fn flag_on_click_each_face_edge_corner_lands_documented_orientation() {
    let mut harness = real_app_harness();
    harness.run();

    settings::write(
        &harness.ctx,
        &settings::Settings {
            nav_cube_click_to_align: true,
        },
    );
    harness.step();

    let mut exact = 0usize;
    for target in NavTarget::ALL {
        let (y0, p0) = target.yaw_pitch();
        // A small deterministic nudge from the committed pose: the target
        // stays frontmost at its projected point, and the click must actually
        // move the camera back onto the committed orientation.
        let base_pitch = if p0.abs() > 1.5 {
            p0 - p0.signum() * 0.11
        } else {
            p0 - 0.11
        };
        let base_yaw = y0 + 0.17;
        pose_and_settle(&mut harness, base_yaw, base_pitch);

        let pos = target_screen_point(&harness, &target);
        click_at(&mut harness, pos);
        let after = read_pose(&harness).expect("pose after click");

        // Compare the landed camera quaternion to the target's independent
        // orientation quaternion within 1e-9.
        let cam = oep_viewer::Camera {
            target: oep_geo::math::Vec3::new(40.0, 20.0, 5.0),
            yaw: after.0,
            pitch: after.1,
            distance: 120.0,
            fov_y: 0.785,
            near: 0.1,
            far: 5000.0,
        };
        let landed = nav_cube::camera_quaternion(&cam).expect("landed camera basis");
        let expected = target.orientation().expect("target orientation");
        let ok = quat_close(&landed, &expected, 1e-9);
        println!(
            "flag-on click: target={:<16} pos=({:5.1},{:5.1}) landed=({:.12},{:.12}) quat=({:.12},{:.12},{:.12},{:.12}) expected=({:.12},{:.12},{:.12},{:.12}) match={}",
            target.label(),
            pos.x,
            pos.y,
            after.0,
            after.1,
            landed.w,
            landed.x,
            landed.y,
            landed.z,
            expected.w,
            expected.x,
            expected.y,
            expected.z,
            ok
        );
        assert!(
            ok,
            "target {} did not land (landed yaw/pitch {:.12}/{:.12})",
            target.label(),
            after.0,
            after.1
        );
        exact += if ok { 1 } else { 0 };
    }
    assert_eq!(
        exact, 26,
        "all 26 targets must land on their documented view"
    );
}

/// Flag-on drag with snap: dragging the cube orbits the camera in real time,
/// and releasing it snaps onto the nearest documented orientation (verified to
/// 1e-9 against the expected nearest target).
#[test]
fn flag_on_drag_orbits_and_release_snaps() {
    let mut harness = real_app_harness();
    harness.run();
    settings::write(
        &harness.ctx,
        &settings::Settings {
            nav_cube_click_to_align: true,
        },
    );
    harness.step();

    let report = cube_report(&harness);
    let center = report.rect.center();
    let start = egui::pos2(center.x - 10.0, center.y - 10.0);
    let end = egui::pos2(center.x + 14.0, center.y + 8.0);

    let before = read_pose(&harness).expect("pose before drag");
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerMoved(end));
    harness.step();
    let mid_pose = read_pose(&harness).expect("pose mid-drag");
    harness.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    for _ in 0..4 {
        harness.step();
    }
    let after = read_pose(&harness).expect("pose after release");

    // Mid-drag: the cube orbited the camera (yaw moved with the pointer).
    assert!(
        (mid_pose.0 - before.0).abs() > 1e-6,
        "dragging the cube must orbit the camera in real time"
    );

    // After release: snapped onto one of the 26 documented orientations.
    let cam = oep_viewer::Camera {
        target: oep_geo::math::Vec3::new(40.0, 20.0, 5.0),
        yaw: after.0,
        pitch: after.1,
        distance: 120.0,
        fov_y: 0.785,
        near: 0.1,
        far: 5000.0,
    };
    let landed = nav_cube::camera_quaternion(&cam).expect("camera basis after snap");
    let nearest: Option<(f64, NavTarget)> = NavTarget::ALL
        .iter()
        .filter_map(|t| t.orientation().map(|q| (t, q)))
        .map(|(t, q)| {
            let d = q.dot(&landed).abs().clamp(-1.0, 1.0);
            (d.acos(), *t)
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    assert!(nearest.is_some(), "an orientation must exist to snap onto");
    let (ang, t) = nearest.expect("nearest target present");
    let expected = t.orientation().expect("target orientation");
    assert!(
        quat_close(&landed, &expected, 1e-9),
        "after release the camera must rest exactly on the nearest documented \
         orientation ({}) at 1e-9, landed=({},{},{},{}) expected=({},{},{},{})",
        t.label(),
        landed.w,
        landed.x,
        landed.y,
        landed.z,
        expected.w,
        expected.x,
        expected.y,
        expected.z
    );
    println!(
        "flag-on drag: id pos {:?} -> {:?}, snapped to {} at angle {:.6} rad; landed=({:.12},{:.12})",
        start,
        end,
        t.label(),
        ang,
        after.0,
        after.1
    );
}

/// Accessible labels: the cube exposes the six face names (from the SHARED
/// view-name table) and the current-orientation readout, all as unique
/// strings that do not collide with the nav panel's own labels.
#[test]
fn cube_exposes_face_names_and_orientation_readout() {
    let mut harness = real_app_harness();
    harness.run();

    harness.get_by_label("Orientation cube");
    harness.get_by_label("Orientation cube faces: Front, Back, Left, Right, Top, Bottom");
    harness.get_by_label("Orientation cube viewing Front");

    // The nav panel's own labels stay singularly queryable (no new collisions
    // introduced by the cube): a quick button and a cube face button.
    harness.get_by_label("Front");
    harness.get_by_label("Cube Front");
    harness.get_by_label("Iso");

    println!("a11y: orientation cube labels present; nav labels stay singular");
}
