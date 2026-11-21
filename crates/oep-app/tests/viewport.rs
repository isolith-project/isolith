//! UI-A5 UI-VIEWPORT-CPU: the viewport seam and the live frame.
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts on the CPU
//! viewport's observable state:
//!
//! 1. A left-drag over the viewport orbits the camera by EXACTLY the numeric
//!    delta the panel maps per point (`dx * ORBIT_RAD_PER_PX` yaw), and the
//!    rendered frame hash changes with it.
//! 2. A middle-drag pans the orbit target exactly where
//!    `Camera::pan(dx*world_per_pixel, dy*world_per_pixel)` puts it, and the
//!    frame hash changes.
//! 3. A zoom gesture changes the camera distance by the exact gesture factor,
//!    and the frame hash changes.
//!
//! The panel is compiled here as `app::panels::viewport`, exactly as it ships;
//! the fixture reads the scalar snapshot the panel publishes into the egui
//! context data under `VIEWPORT_STATE_KEY`, so nothing reaches into `app`'s
//! private state.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

use egui::PointerButton;

/// The panel's context-data key (must match `viewport::VIEWPORT_STATE_KEY`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";
/// The panel's orbit sensitivity (must match `viewport::ORBIT_RAD_PER_PX`).
const ORBIT_RAD_PER_PX: f64 = 0.005;

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The centre and pixel height of the viewport region, read from the app's own
/// accessibility tree (the region the panel draws into).
fn viewport_geometry(harness: &Harness<'_, app::App>) -> (egui::Pos2, f64) {
    let node = harness
        .get_all_by_label("viewport")
        .next()
        .expect("viewport region present");
    let bounds = node
        .accesskit_node()
        .bounding_box()
        .expect("viewport region has explicit bounds");
    let center = egui::pos2(
        (bounds.x0 + bounds.x1) as f32 * 0.5,
        (bounds.y0 + bounds.y1) as f32 * 0.5,
    );
    (center, bounds.y1 - bounds.y0)
}

/// The panel's scalar snapshot tuple.
type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// Read the panel's last published scalar snapshot from egui context data.
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
fn current_snapshot(harness: &Harness<'_, app::App>) -> Option<Snapshot> {
    harness
        .ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

fn press(h: &Harness<'_, app::App>, pos: egui::Pos2, button: PointerButton) {
    h.event(egui::Event::PointerButton {
        pos,
        button,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
}

fn release(h: &Harness<'_, app::App>, pos: egui::Pos2, button: PointerButton) {
    h.event(egui::Event::PointerButton {
        pos,
        button,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
}

/// Rebuild the exact camera the panel holds from the published scalars.
fn camera_from_snapshot(s: &Snapshot) -> oep_viewer::Camera {
    oep_viewer::Camera {
        target: oep_viewer::Vec3::new(s.3, s.4, s.5),
        yaw: s.0,
        pitch: s.1,
        distance: s.2,
        fov_y: s.6,
        near: 0.1,
        far: 5000.0,
    }
}

/// A left drag of `dx = 80` orbits by exactly `80 * ORBIT_RAD_PER_PX` radians
/// of yaw (pitch unchanged), and the frame hash changes.
#[test]
fn left_drag_orbits_camera_numerically_and_changes_frame_hash() {
    let mut harness = real_app_harness();
    harness.run();

    let (start, _) = viewport_geometry(&harness);
    let before = current_snapshot(&harness).expect("first frame publishes a snapshot");

    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    press(&harness, start, egui::PointerButton::Primary);
    harness.step();
    // Move in two steps so egui commits both frames as drag displacement.
    let mid = egui::pos2(start.x + 40.0, start.y);
    let end = egui::pos2(start.x + 80.0, start.y);
    harness.event(egui::Event::PointerMoved(mid));
    harness.step();
    harness.event(egui::Event::PointerMoved(end));
    harness.step();
    release(&harness, end, egui::PointerButton::Primary);
    harness.step();

    let after = current_snapshot(&harness).expect("panel publishes after drag");

    let expected = 80.0 * ORBIT_RAD_PER_PX;
    assert!(
        (after.0 - before.0 - expected).abs() < 1e-9,
        "yaw must move by exactly dx*ORBIT_RAD_PER_PX: before={} after={} expected_delta={}",
        before.0,
        after.0,
        expected
    );
    assert!(
        (after.1 - before.1).abs() < 1e-9,
        "horizontal drag must not change pitch"
    );
    assert_ne!(
        after.7, before.7,
        "a new camera must re-render to a new frame hash"
    );
}

/// A middle-drag pans the orbit target exactly where `Camera::pan` puts it (the
/// panel's own mapping, recomputed here), and the frame hash changes.
#[test]
fn middle_drag_pans_target_exactly_and_changes_frame_hash() {
    let mut harness = real_app_harness();
    harness.run();

    let (start, panel_px_height) = viewport_geometry(&harness);
    let before = current_snapshot(&harness).expect("snapshot before pan");

    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    press(&harness, start, egui::PointerButton::Middle);
    harness.step();
    // Diagonal drag: 60px right, 30px down (screen-y positive is downward).
    let end = egui::pos2(start.x + 60.0, start.y + 30.0);
    harness.event(egui::Event::PointerMoved(end));
    harness.step();
    release(&harness, end, egui::PointerButton::Middle);
    harness.step();

    let after = current_snapshot(&harness).expect("snapshot after pan");

    // The panel pans via `cam.pan(dx*wpp, dy*wpp)` where wpp is
    // `2*distance*tan(fov_y/2) / panel_height_px`. Recompute that exact same
    // operation from the pre-pan camera and compare the resulting target
    // component-wise — a true numeric delta, not just "something moved".
    let mut expected_cam = camera_from_snapshot(&before);
    let wpp = (2.0 * before.2 * (before.6 * 0.5).tan()) / panel_px_height;
    expected_cam.pan(60.0 * wpp, 30.0 * wpp);

    assert!(
        (after.3 - expected_cam.target.x).abs() < 1e-6
            && (after.4 - expected_cam.target.y).abs() < 1e-6
            && (after.5 - expected_cam.target.z).abs() < 1e-6,
        "target must land exactly where Camera::pan(dx*wpp, dy*wpp) puts it: \
         after=({}, {}, {}) expected=({}, {}, {})",
        after.3,
        after.4,
        after.5,
        expected_cam.target.x,
        expected_cam.target.y,
        expected_cam.target.z
    );
    assert_ne!(after.7, before.7, "pan must re-render to a new frame hash");
}

/// A zoom gesture (zoom_delta = 1.2, i.e. zoom in) shrinks the distance to
/// exactly `before / 1.2` (kept above the camera's `near*2` distance clamp so
/// the ratio stays exact) and the frame hash changes.
#[test]
fn zoom_changes_camera_distance_numerically_and_changes_frame_hash() {
    let mut harness = real_app_harness();
    harness.run();

    let (center, _) = viewport_geometry(&harness);
    harness.event(egui::Event::PointerMoved(center));
    harness.step();

    let before = current_snapshot(&harness).expect("snapshot before zoom");

    let factor = 1.2_f64;
    harness.event(egui::Event::Zoom(factor as f32)); // 1.2x zoom in => distance shrinks
    harness.step();

    let after = current_snapshot(&harness).expect("snapshot after zoom");

    assert!(
        (after.2 - before.2 / factor).abs() < 1e-4,
        "zoom must scale distance by approximately the gesture factor: before={} after={} expected={}",
        before.2,
        after.2,
        before.2 / factor
    );
    assert_ne!(after.7, before.7, "zoom must re-render to a new frame hash");
}
