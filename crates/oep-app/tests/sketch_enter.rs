//! UI-B3 UI-SKETCH-ENTER: pick a datum plane and enter sketch (wave B).
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts on the
//! observable state exactly as the card's acceptance reads it:
//!
//! 1. Clicking a datum plane in the viewport enters sketch mode: the sketch
//!    `String` oracle flips to `mode=sketch` with the picked plane id and the
//!    grid layer on, the camera swings to look normal to the plane (forward
//!    x normal magnitude < 1e-6, plus yaw/pitch exactly the plane's normal
//!    orientation), the frame re-renders, and the model tree shows a sketch
//!    node naming the active plane.
//! 2. Escape leaves sketch mode with the document byte-untouched (the doc
//!    panel's `String` oracle is identical before and after).
//! 3. A click that does not land on a datum plane stays in normal mode.
//!
//! The camera normal is asserted numerically, never by screenshot. All math
//! here reads the app's real code: `sketch.rs` is pulled in verbatim via
//! `#[path]` (a second copy of the exact source the viewport panel ships), so
//! the click position is computed with the same world-to-panel projection the
//! panel uses, and `oep_viewer::pick` is the same seam the panel drives.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact sketch module source the viewport panel compiles, joined here so
// the acceptance can compute the click position and the expected camera swing
// with the app's own functions instead of a hand-rolled replica.
// This is a deliberate second `#[path]` include of the same file (the shipped
// copy arrives via `app.rs` as `app::panels::viewport::sketch`): rustc allows
// it, and clippy's `duplicate_mod` (a `-D warnings` escalation) is suppressed
// because the harness needs both the app's module tree AND its own math oracle.
// Dead-code is silenced for the copy-level items the harness does not call
// (same allowance as the `config`/`theme` copies above).
#[path = "../src/panels/sketch.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sketch;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

use egui::PointerButton;

use oep_viewer::Vec3;

/// The viewport panel's context-data key (must match
/// `viewport::VIEWPORT_STATE_KEY`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";
/// The sketch panel's report-oracle key (must match
/// `app::panels::viewport::sketch::SKETCH_REPORT_KEY`).
const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";
/// The doc panel's report-oracle key (must match
/// `app::panels::doc::DOC_REPORT_KEY`).
const DOC_REPORT_KEY: &str = "ui_a7_doc_report";

/// The panel's scalar snapshot tuple.
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The viewport region's full rect (and its centre / pixel height), read from
/// the app's own accessibility tree.
fn viewport_rect(harness: &Harness<'_, app::App>) -> egui::Rect {
    let node = harness
        .get_all_by_label("viewport")
        .next()
        .expect("viewport region present");
    let bounds = node
        .accesskit_node()
        .bounding_box()
        .expect("viewport region has explicit bounds");
    egui::Rect::from_min_max(
        egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
        egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
    )
}

/// Read the panel's last published scalar snapshot from egui context data.
fn current_snapshot(harness: &Harness<'_, app::App>) -> Option<Snapshot> {
    harness
        .ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

/// The sketch panel's `String` oracle, read back through the same context.
fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// The doc panel's `String` oracle (the document's on-disk identity).
fn doc_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(DOC_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of the sketch oracle, e.g. `("mode", "idle")`.
fn sk_field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
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

/// The camera's normalized forward vector (target - eye).
fn forward_of(s: &Snapshot) -> Vec3 {
    let cam = camera_from_snapshot(s);
    cam.target
        .sub(&cam.eye())
        .normalize()
        .expect("camera is never degenerate")
}

fn press(h: &Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
}

fn release(h: &Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
}

/// Click the mouse at `pos` over one frame (egui sees press then release).
/// One extra frame is stepped so the tree panel (which runs earlier in the
/// frame than the viewport) re-reads any sketch-state change it lags by a
/// single frame.
fn click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    press(h, pos);
    h.step();
    release(h, pos);
    h.step();
    h.step();
}

/// Enter sketch mode on the XY datum plane, exactly as the app resolves it:
/// project a world point on the XY plane to a panel click position, verify
/// with the panel's own pick seam that the round-trip ray picks XY, then
/// click there. Returns the viewport rect and the pre-click frame hash.
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> (egui::Rect, u64) {
    harness.run();

    let rect = viewport_rect(harness);
    let before = current_snapshot(harness).expect("first frame publishes a snapshot");
    let cam = camera_from_snapshot(&before);
    let scale = harness.ctx.pixels_per_point();

    // A world point on the XY plane, inside its rectangle and off the shared
    // scene centre so the ray picks XY unambiguously (nearest sheet).
    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let planes = sketch::datum_planes(&oep_viewer::CpuBackend::demo_scene());
    assert_eq!(
        planes
            .iter()
            .find(|p| p.id == "XY")
            .expect("XY plane defined")
            .point,
        cam.target,
        "the datum planes are built through the scene centre the camera frames"
    );

    let click_pos = sketch::world_to_panel(&cam, rect, scale, &p_world)
        .expect("P projects in front of the camera");
    assert!(
        rect.contains(click_pos),
        "the XY-plane click position must fall inside the viewport panel"
    );

    // Precondition: the round-trip ray at that click pixel picks XY through
    // the same pick seam the panel drives.
    let cx = ((click_pos.x - rect.min.x) * scale).max(0.0) as f64;
    let cy = ((click_pos.y - rect.min.y) * scale).max(0.0) as f64;
    let px = (rect.width() * scale).round().max(2.0) as f64;
    let py = (rect.height() * scale).round().max(2.0) as f64;
    let ray = oep_viewer::pick::unproject(&cam, cx, cy, px, py);
    let f = cam.target.sub(&cam.eye()).normalize().expect("camera");
    let picked = sketch::pick_plane(&planes, &ray, &f)
        .map(|p| p.id)
        .expect("the crafted click must pick a plane");
    assert_eq!(picked, "XY", "the crafted click must land on the XY plane");

    click(harness, click_pos);
    (rect, before.7)
}

/// Section 1 of the acceptance: click the plane, sketch mode becomes active,
/// the camera is normal to the plane within 1e-6, the grid layer turns on, the
/// frame re-renders, and the tree shows the sketch node.
#[test]
fn clicking_a_datum_plane_enters_sketch_and_swings_the_camera_normal() {
    let mut harness = real_app_harness();
    let (_rect, before_hash) = enter_xy_sketch(&mut harness);

    let report_after = sketch_report(&harness);
    assert_eq!(
        sk_field(&report_after, "mode").as_deref(),
        Some("sketch"),
        "sketch mode must be active after clicking the plane: {report_after:?}"
    );
    assert_eq!(
        sk_field(&report_after, "plane").as_deref(),
        Some("XY"),
        "the active plane must be the one clicked: {report_after:?}"
    );
    assert_eq!(
        sk_field(&report_after, "grid").as_deref(),
        Some("on"),
        "the sketch grid layer must be on in sketch mode: {report_after:?}"
    );
    let normal = sk_field(&report_after, "normal").expect("oracle reports the plane normal");
    let v: Vec<f64> = normal
        .split(',')
        .map(|s| s.parse::<f64>().expect("normal is numeric"))
        .collect();
    assert_eq!(v, vec![0.0, 0.0, 1.0], "XY plane normal is +Z");

    let after = current_snapshot(&harness).expect("snapshot after the click");

    // Camera normal within 1e-6: the forward vector is parallel to the plane
    // normal (cross magnitude ~0), asserted numerically.
    let f = forward_of(&after);
    let n = Vec3::new(0.0, 0.0, 1.0);
    assert!(
        f.cross(&n).norm() < 1e-6,
        "camera must look along the XY normal: forward={f:?} normal={n:?}"
    );
    // And exactly the orientation the panel computed from the plane normal.
    let (eyaw, epitch) = sketch::normal_orientation(&oep_viewer::Vec3::new(0.0, 0.0, 1.0));
    assert!(
        (after.0 - eyaw).abs() < 1e-9 && (after.1 - epitch).abs() < 1e-9,
        "yaw/pitch must be exactly the plane's normal orientation: \
         got ({}, {}) expected ({eyaw}, {epitch})",
        after.0,
        after.1
    );

    // The swung camera re-renders to a new frame hash (the pick visibly
    // changes the viewport).
    assert_ne!(
        after.7, before_hash,
        "entering sketch must re-render the viewport"
    );

    // The model tree shows a sketch node naming the active plane.
    let tree_has_plane = harness.get_all_by_label("XY (Top)").next().is_some();
    assert!(
        tree_has_plane,
        "the tree must show a sketch node for the active plane"
    );
    let tree_has_sketch = harness.get_all_by_label("Sketch").next().is_some();
    assert!(
        tree_has_sketch,
        "the tree must show the Sketch node while in sketch mode"
    );
}

/// Section 2: Escape leaves sketch mode without touching the document. The
/// document oracle (file / hash / dirty) is identical before and after.
#[test]
fn escape_leaves_sketch_mode_without_touching_the_document() {
    let mut harness = real_app_harness();
    let _rect = enter_xy_sketch(&mut harness);

    assert_eq!(
        sk_field(&sketch_report(&harness), "mode").as_deref(),
        Some("sketch"),
        "precondition: sketch mode active before Escape"
    );
    let doc_before = doc_report(&harness);
    assert!(
        doc_before.contains("file="),
        "doc oracle is live before Escape: {doc_before:?}"
    );

    harness.key_press(egui::Key::Escape);
    harness.step();
    // One more frame so the tree panel (running earlier in the frame than the
    // viewport) catches the now-idle sketch report before it is inspected.
    harness.step();

    let report = sketch_report(&harness);
    assert_eq!(
        sk_field(&report, "mode").as_deref(),
        Some("idle"),
        "Escape must leave sketch mode: {report:?}"
    );
    assert_eq!(
        sk_field(&report, "grid").as_deref(),
        Some("off"),
        "Escape must turn the sketch grid layer off: {report:?}"
    );
    assert_eq!(
        sk_field(&report, "plane").as_deref(),
        Some("-"),
        "no active plane after Escape: {report:?}"
    );
    assert_eq!(
        doc_report(&harness),
        doc_before,
        "Escape must leave the document untouched"
    );

    // The tree's sketch node disappears with sketch mode.
    assert!(
        harness.query_by_label("XY (Top)").is_none(),
        "the tree sketch node must vanish after Escape"
    );
}

/// Section 3: a click over the viewport that does not land on a datum plane
/// stays in normal mode — sketch mode is opt-in on a real plane pick, not a
/// side effect of any click.
#[test]
fn clicking_viewport_empty_space_stays_in_normal_mode() {
    let mut harness = real_app_harness();
    harness.run();

    let rect = viewport_rect(&harness);
    let before = current_snapshot(&harness).expect("snapshot before the click");

    // Pick a pixel inside the panel that the app's own pick seam proves lands
    // on no datum plane (the far top-right corner of the viewport), so the
    // assertion below is grounded rather than guessed.
    let scale = harness.ctx.pixels_per_point();
    let cam = camera_from_snapshot(&before);
    let planes = sketch::datum_planes(&oep_viewer::CpuBackend::demo_scene());
    let px = (rect.width() * scale).round().max(2.0) as f64;
    let py = (rect.height() * scale).round().max(2.0) as f64;

    let mut miss_pos = None;
    for (fx, fy) in [(0.96, 0.04), (0.98, 0.05), (0.97, 0.03), (0.95, 0.06)] {
        let pos = egui::pos2(
            rect.min.x + rect.width() * fx as f32,
            rect.min.y + rect.height() * fy as f32,
        );
        let cx = ((pos.x - rect.min.x) * scale).max(0.0) as f64;
        let cy = ((pos.y - rect.min.y) * scale).max(0.0) as f64;
        let ray = oep_viewer::pick::unproject(&cam, cx, cy, px, py);
        let f = cam.target.sub(&cam.eye()).normalize().expect("camera");
        if sketch::pick_plane(&planes, &ray, &f).is_none() {
            miss_pos = Some(pos);
            break;
        }
    }
    let miss_pos = miss_pos.expect("a viewport corner must miss the datum planes");

    click(&mut harness, miss_pos);

    let report = sketch_report(&harness);
    assert_eq!(
        sk_field(&report, "mode").as_deref(),
        Some("idle"),
        "a click off any datum plane must not enter sketch mode: {report:?}"
    );
    assert_eq!(
        sk_field(&report, "plane").as_deref(),
        Some("-"),
        "no active plane: {report:?}"
    );
    assert_eq!(
        sk_field(&report, "grid").as_deref(),
        Some("off"),
        "grid layer stays off: {report:?}"
    );

    // And the viewport camera is untouched (no accidental frame change).
    let after = current_snapshot(&harness).expect("snapshot after the click");
    assert_eq!(after.0, before.0, "yaw untouched");
    assert_eq!(after.1, before.1, "pitch untouched");
    assert_eq!(after.7, before.7, "frame hash untouched by an empty click");
}
