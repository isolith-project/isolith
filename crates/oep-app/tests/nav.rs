//! UI-D4 UI-NAV: view cube, standard views, section planes (wave D).
//!
//! Acceptance harness. Drives the REAL `app::App` through `egui_kittest` and
//! asserts, numerically:
//!
//! 1. **Each of the six cube faces lands the camera on the exact standard
//!    view.** Clicking a gizmo face (`Cube Front` ... `Cube Bottom`) sets the
//!    backend camera to the committed `(yaw, pitch)` of
//!    `oep_viewer::views::StandardView`; the test prints the resulting
//!    `(eye, target, up)` and asserts the eye equals the analytic expectation
//!    `target + direction(view) * distance` within `1e-9` (target and
//!    distance unchanged, and the frame re-renders to a new hash).
//! 2. **The Front / Top / Right / Iso quick buttons do the same**, asserted
//!    numerically.
//! 3. **The section slider's committed positions produce the committed
//!    visible-triangle counts.** The slider is placed (via the AccessKit
//!    `SetValue` action, i.e. without a mouse) at the three committed
//!    fractions `1.0 / 0.5 / 0.0`; the published `NavState` reports both the
//!    measured (renderer-clip emulation) and analytic closed-form counts, and
//!    the test prints all six numbers, asserts they are equal to each other
//!    and to the committed literals `12 / 10 / 2`, and non-increasing across
//!    the three positions.
//! 4. **The gizmo is reachable without a mouse:** every face and the quick
//!    buttons expose an accessible name (asserted by label), and the slider
//!    is driven by an AccessKit action rather than a pointer.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use oep_viewer::views::{CubeFace, StandardView};

/// The viewport panel's context-data key (must match
/// `viewport::VIEWPORT_STATE_KEY`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";
/// The nav panel's context-data key (must match `nav::NAV_STATE_KEY`).
const NAV_STATE_KEY: &str = "isolith.ui.viewport.nav";

/// The panel's scalar snapshot tuple:
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

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

/// Read the panel's last published scalar snapshot from egui context data.
fn current_snapshot(h: &Harness<'_, app::App>) -> Option<Snapshot> {
    h.ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

/// The slice of the nav panel's published snapshot the acceptance reads:
/// `(section, plane_offset, visible_triangles, analytic_triangles)`.
#[derive(Debug, Clone)]
struct NavState {
    section: f64,
    plane_offset: f64,
    visible_triangles: usize,
    analytic_triangles: usize,
}

/// Read the nav panel's published snapshot from egui context data (a plain
/// scalar tuple, exactly as the panel publishes it).
fn current_nav(h: &Harness<'_, app::App>) -> Option<NavState> {
    let raw: (f64, f64, usize, usize) = h
        .ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(NAV_STATE_KEY)))?;
    Some(NavState {
        section: raw.0,
        plane_offset: raw.1,
        visible_triangles: raw.2,
        analytic_triangles: raw.3,
    })
}

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// Click the widget with `label`, then run two more frames so the backend
/// rasterizes with the newly applied camera and the viewport publishes the
/// matching snapshot. Returns the pre-click camera/snapshot and the post
/// snapshot.
fn click_view_and_read(
    h: &mut Harness<'_, app::App>,
    label: &str,
) -> (oep_viewer::Camera, Snapshot, Snapshot) {
    let pre = current_snapshot(h).expect("a snapshot is published before the click");
    let pre_cam = camera_from_snapshot(&pre);

    h.get_by_label(label).click();
    h.step(); // frame that processes the click: nav applies the orientation
    h.step(); // frame that re-rasterizes and publishes the new camera

    let post = current_snapshot(h).expect("a snapshot is published after the click");
    (pre_cam, pre, post)
}

/// Assert the camera landed exactly on `view`: yaw/pitch equal the committed
/// angles, the orbit target and distance are unchanged, the eye equals the
/// analytic `target + direction * distance` within `1e-9`, and the frame
/// re-rendered to a new hash. Prints the resulting `(eye, target, up)` and
/// the expected eye so the acceptance sees the real numbers.
fn assert_exact_standard_view(
    tag: &str,
    label: &str,
    view: StandardView,
    pre: &oep_viewer::Camera,
    pre_snap: &Snapshot,
    post: &Snapshot,
) {
    let downshifted = view.eye_for(&pre.target, pre.distance);
    let up = StandardView::up();
    let cam = camera_from_snapshot(post);
    let (yaw, pitch) = view.yaw_pitch();

    println!(
        "{tag}: clicked {label:?} -> eye=({:.12}, {:.12}, {:.12}) target=({:.12}, {:.12}, {:.12}) up=({:.1}, {:.1}, {:.1}) | yaw={:.12} pitch={:.12} | expected_eye=({:.12}, {:.12}, {:.12})",
        cam.eye().x, cam.eye().y, cam.eye().z,
        cam.target.x, cam.target.y, cam.target.z,
        up.x, up.y, up.z,
        cam.yaw, cam.pitch,
        downshifted.x, downshifted.y, downshifted.z,
    );

    // The committed orientation is applied exactly.
    let yaw_err = (cam.yaw - yaw).abs();
    let pitch_err = (cam.pitch - pitch).abs();
    assert!(yaw_err < 1e-9, "{}: yaw {} != {}", tag, cam.yaw, yaw);
    assert!(
        pitch_err < 1e-9,
        "{}: pitch {} != {}",
        tag,
        cam.pitch,
        pitch
    );
    // The orbit target and distance are unchanged by an orientation switch.
    let target_err = cam.target.distance_to(&pre.target);
    let distance_err = (cam.distance - pre.distance).abs();
    assert!(target_err < 1e-9, "{}: target moved", tag);
    assert!(distance_err < 1e-9, "{}: distance moved", tag);
    // The camera eye equals the analytic expectation within the tolerance.
    let err = cam.eye().distance_to(&downshifted);
    assert!(
        err < 1e-9,
        "{}: eye error {:.3e} exceeds 1e-9 (eye={:?} expected={:?})",
        tag,
        err,
        cam.eye(),
        downshifted
    );
    // Up is the camera's fixed vector.
    assert!(up.x == 0.0 && up.y == 1.0 && up.z == 0.0);
    // Landing on a new standard view must re-render the frame.
    assert_ne!(
        post.7, pre_snap.7,
        "{}: frame must re-render on a new view",
        tag
    );
}

/// Each of the six cube faces lands the camera on the exact standard view:
/// printed `(eye, target, up)` equal to the analytic expectation within 1e-9.
#[test]
fn six_cube_faces_land_exact_standard_views() {
    let mut harness = real_app_harness();
    harness.run();

    for face in CubeFace::ALL {
        let label = format!("Cube {}", face.label());
        let (pre_cam, pre_snap, post) = click_view_and_read(&mut harness, &label);
        assert_exact_standard_view("face", &label, face.view(), &pre_cam, &pre_snap, &post);
    }
}

/// The Front / Top / Right / Iso quick buttons land the camera on the exact
/// standard view, asserted numerically like the cube faces.
#[test]
fn quick_buttons_land_exact_standard_views() {
    let mut harness = real_app_harness();
    harness.run();

    let cases = [
        ("Front", StandardView::Front),
        ("Top", StandardView::Top),
        ("Right", StandardView::Right),
        ("Iso", StandardView::Iso),
    ];
    for (label, view) in cases {
        let (pre_cam, pre_snap, post) = click_view_and_read(&mut harness, label);
        assert_exact_standard_view("button", label, view, &pre_cam, &pre_snap, &post);
    }
}

/// The section slider, at the three committed positions, produces three
/// visible-triangle counts that are non-increasing and equal to the analytic
/// counts (prints all six numbers).
#[test]
fn section_slider_counts_match_analytic_and_are_non_increasing() {
    let mut harness = real_app_harness();
    harness.run();

    let fractions = [1.0_f64, 0.5, 0.0];
    let literals = [12usize, 10, 2];
    let mut measured = Vec::new();
    let mut analytic = Vec::new();

    for (frac, expected) in fractions.iter().copied().zip(literals) {
        let slider = harness.get_by_label("Section");
        // Drive the slider through the AccessKit SetValue action - the
        // "reachable without a mouse" proof for the section control.
        slider.set_value_accesskit(frac);
        harness.step();
        harness.step();

        let nav = current_nav(&harness).expect("nav state published after slider set");
        println!(
            "section: fraction={:.3} plane_offset={:.3} visible_triangles={} analytic_triangles={}",
            nav.section, nav.plane_offset, nav.visible_triangles, nav.analytic_triangles
        );
        // The slider landed exactly where the accessibility action put it.
        assert!(
            (nav.section - frac).abs() < 1e-9,
            "slider fraction {} != requested {}",
            nav.section,
            frac
        );
        // The measured count equals the analytic count equals the committed
        // literal for this committed plane.
        assert_eq!(nav.visible_triangles, nav.analytic_triangles);
        assert_eq!(nav.visible_triangles, expected, "visible count at {frac}");
        assert_eq!(nav.analytic_triangles, expected, "analytic count at {frac}");
        measured.push(nav.visible_triangles);
        analytic.push(nav.analytic_triangles);
    }

    // The three published counts are non-increasing as the cut deepens.
    assert!(measured[0] >= measured[1] && measured[1] >= measured[2]);
    assert!(analytic[0] >= analytic[1] && analytic[1] >= analytic[2]);

    println!(
        "section sequence (fractions 1.0 -> 0.5 -> 0.0): visible {:?} analytic {:?}",
        measured, analytic
    );
}

/// The gizmo exposes an accessible name for every face, and the quick buttons
/// and section slider are present by label, so navigation is reachable
/// without a mouse.
#[test]
fn gizmo_and_controls_expose_accessible_names() {
    let mut harness = real_app_harness();
    harness.run();

    let labels = [
        "Cube Front",
        "Cube Back",
        "Cube Left",
        "Cube Right",
        "Cube Top",
        "Cube Bottom", // gizmo faces
        "Front",
        "Top",
        "Right",
        "Iso",     // quick-view buttons
        "Section", // section slider
    ];
    for label in labels {
        let found = harness.get_all_by_label(label).next().is_some();
        assert!(
            found,
            "nav control label {:?} missing from the AccessKit tree",
            label
        );
        println!("a11y: {label:?} present");
    }
}
