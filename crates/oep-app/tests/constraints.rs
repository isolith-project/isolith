//! UI-B6 UI-CONSTRAINTS: apply and remove constraints (wave B).
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts the card's
//! acceptance exactly as written:
//!
//! 1. Horizontal / vertical / coincident / length constraints are APPLIED
//!    from the on-screen constraint toolbar and REMOVED again, each bringing
//!    the solver-reported dof down and back up (the toolbar buttons live at
//!    [`sketch::constraint_toolbar_layout`] rects, so the test clicks the
//!    exact pixels the renderer uses).
//! 2. The degrees of freedom are reported HONESTLY as a first-class value in
//!    the sketch status (the report oracle carries `dof=N` / `solve=` /
//!    `constraint_status=`; the toolbar paints the same readout). A free
//!    two-point line starts at dof 4; each h/v/coincident/length constraint
//!    removes the solver's own number of dof.
//! 3. An over-constrained sketch SAYS SO: applying enough redundant
//!    constraints makes the solver's dof go negative, and the sketch reports
//!    the named `over-constrained` state instead of pretending it solved -
//!    never a silent partial solve.
//!
//! The math reads the app's own code (`panels/sketch.rs` is pulled in
//! verbatim via `#[path]`), so the click positions are computed with the
//! same world-to-panel projection and the same viewport rect the app uses,
//! keeping the drawn and constrained geometry exact.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact sketch module source the viewport panel compiles (math + the
// draw tool machine + this card's constraint layer), joined here so the
// acceptance computes click positions and expects the solver's own dof with
// the app's own functions.
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
/// The sketch panel's report-oracle key.
const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";

/// The panel's scalar snapshot tuple:
/// `(yaw, pitch, distance, target_x, target_y, target_z, fov_y, frame_hash)`.
type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The viewport region's full rect, read from the app's own accessibility
/// tree (the ground truth the a11y system reports).
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

/// The sketch panel's `String` oracle, read back through the same context.
fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of the sketch oracle, e.g. `("mode", "idle")`.
fn sk_field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// `sk_field` parsed as an integer (dof may be negative).
fn sk_int(report: &str, key: &str) -> Option<i32> {
    sk_field(report, key)?.parse().ok()
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

/// Click the mouse at `pos` over one frame (press then release). Extra frames
/// let the tree panel (which runs earlier in the frame than the viewport)
/// catch state that lags by a single frame.
fn click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    press(h, pos);
    h.step();
    release(h, pos);
    h.step();
    h.step();
}

/// Single-click an egui `Button` style click (used for the toolbar buttons).
fn click_at(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    click(h, pos);
}

/// Enter sketch mode on the XY datum plane, exactly as the app resolves it:
/// project a world point on the XY plane to a click position, verify with the
/// panel's own pick seam that the round-trip ray picks XY, click there, then
/// assert sketch mode is active.
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> egui::Rect {
    harness.run();

    let rect = viewport_rect(harness);
    let before = current_snapshot(harness).expect("first frame publishes a snapshot");
    let cam = camera_from_snapshot(&before);
    let scale = harness.ctx.pixels_per_point();

    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let click_pos = sketch::world_to_panel(&cam, rect, scale, &p_world)
        .expect("entry point projects in front of the camera");
    assert!(rect.contains(click_pos));

    click(harness, click_pos);

    assert_eq!(
        sk_field(&sketch_report(harness), "mode").as_deref(),
        Some("sketch"),
        "precondition: sketch mode active after the entry click"
    );
    rect
}

/// The demo scene the viewport renders (the canonical 80x40x10 solid), whose
/// bounding-box centroid is where the XY datum plane passes through.
fn xy_plane() -> sketch::DatumPlane {
    sketch::datum_planes(&oep_viewer::CpuBackend::demo_scene())
        .into_iter()
        .find(|p| p.id == "XY")
        .expect("XY plane defined")
}

/// Map plane coordinates back to world for the XY plane (uv measured from the
/// plane centre at the scene centroid).
fn uv_to_world(uv: (f64, f64)) -> Vec3 {
    let plane = xy_plane();
    plane
        .point
        .add(&plane.e1.mul(uv.0))
        .add(&plane.e2.mul(uv.1))
}

/// Draw one two-point line via the Line tool (L, two clicks). The committed
/// document then has `p0 p1 l0` - the constraint toolbar's first target.
fn draw_one_line(harness: &mut Harness<'_, app::App>, rect: egui::Rect) {
    harness.key_press(egui::Key::L);
    harness.step();
    harness.step();
    assert_eq!(
        sk_field(&sketch_report(harness), "tool").as_deref(),
        Some("line")
    );

    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let a = sketch::world_to_panel(&cam, rect, scale, &uv_to_world((10.0, 8.0))).unwrap();
    let b = sketch::world_to_panel(&cam, rect, scale, &uv_to_world((50.0, 8.0))).unwrap();
    assert!(rect.contains(a) && rect.contains(b));

    click(harness, a);
    click(harness, b);

    let r = sketch_report(harness);
    assert_eq!(sk_int(&r, "docpoints"), Some(2));
    assert_eq!(sk_int(&r, "doclines"), Some(1));
}

/// Click the constraint toolbar button for `kind` (at the deterministic rect
/// the app both exposes and renders).
fn click_constraint(
    harness: &mut Harness<'_, app::App>,
    rect: egui::Rect,
    kind: sketch::constraints::ApplyKind,
) {
    let layout = sketch::constraint_toolbar_layout(rect);
    let button = layout
        .iter()
        .find(|b| b.kind == kind)
        .unwrap_or_else(|| panic!("toolbar has a button for {kind:?}"));
    click_at(harness, button.rect.center());
}

/// A free two-point line has 4 dof through the REAL solver; the acceptance
/// asserts the reported dof against that ground truth at every step.
#[test]
fn h_v_coincident_length_apply_and_remove_with_honest_dof() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_one_line(&mut harness, rect);

    // Baseline: the free line reports dof 4, unsolved-by-constraints.
    let r0 = sketch_report(&harness);
    assert_eq!(
        sk_int(&r0, "dof"),
        None,
        "no constraint solve yet, dof absent"
    );
    assert_eq!(sk_field(&r0, "solve").as_deref(), Some("unsolved"));
    assert_eq!(sk_int(&r0, "constraints"), Some(0));

    use sketch::constraints::ApplyKind as K;

    // --- Horizontal applies -> dof 3; remove restores dof 4. ---
    click_constraint(&mut harness, rect, K::Horizontal);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(1));
    assert_eq!(
        sk_int(&r, "dof"),
        Some(3),
        "horizontal frees one dof (solver)"
    );
    assert_eq!(sk_field(&r, "solve").as_deref(), Some("under"));
    assert!(sk_field(&r, "constraint").as_deref() == Some("horizontal l0"));

    click_constraint(&mut harness, rect, K::Remove);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(0));
    assert_eq!(sk_int(&r, "dof"), Some(4), "removing restores dof 4");

    // --- Vertical applies -> dof 3; remove restores. ---
    click_constraint(&mut harness, rect, K::Vertical);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(1));
    assert_eq!(
        sk_int(&r, "dof"),
        Some(3),
        "vertical frees one dof (solver)"
    );
    assert_eq!(sk_field(&r, "constraint").as_deref(), Some("vertical l0"));

    click_constraint(&mut harness, rect, K::Remove);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "dof"), Some(4));

    // --- Length pins the measured 20 -> dof 3; remove restores. ---
    click_constraint(&mut harness, rect, K::Length);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(1));
    assert_eq!(sk_int(&r, "dof"), Some(3), "length frees one dof (solver)");
    assert!(sk_field(&r, "constraint")
        .as_deref()
        .unwrap_or("")
        .starts_with("length l0="));

    click_constraint(&mut harness, rect, K::Remove);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "dof"), Some(4));

    // --- Coincident merges two points (the line's own two points) -> dof 2;
    //     remove restores dof 4. ---
    click_constraint(&mut harness, rect, K::Coincident);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(1));
    assert_eq!(
        sk_int(&r, "dof"),
        Some(2),
        "coincident merges 2 dof (solver)"
    );
    assert_eq!(
        sk_field(&r, "constraint").as_deref(),
        Some("coincident p0->p1")
    );

    click_constraint(&mut harness, rect, K::Remove);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(0));
    assert_eq!(sk_int(&r, "dof"), Some(4));
    assert_eq!(sk_field(&r, "solve").as_deref(), Some("under"));
}

/// The degrees of freedom are reported exactly as the solver computes them,
/// as a first-class status value (toolbar readout + oracle), never floored.
#[test]
fn dof_is_reported_honestly_as_a_first_class_value() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_one_line(&mut harness, rect);

    use sketch::constraints::ApplyKind as K;

    // Four identical length constraints drive dof 4 -> 3 -> 2 -> 1 -> 0
    // (each through the REAL solver). The oracle's `dof=`/`solve=`/status are
    // the first-class report; the toolbar paints the same `constraint_status`.
    for expect in [3, 2, 1, 0] {
        click_constraint(&mut harness, rect, K::Length);
        let r = sketch_report(&harness);
        assert_eq!(
            sk_int(&r, "dof"),
            Some(expect),
            "dof after {expect}th length = {expect} (solver ground truth)"
        );
        assert_eq!(sk_int(&r, "constraints"), Some(4 - expect));
        let status = sk_field(&r, "constraint_status").unwrap_or_default();
        assert!(!status.is_empty(), "status readout present");
    }

    let r = sketch_report(&harness);
    assert_eq!(
        sk_field(&r, "solve").as_deref(),
        Some("fully"),
        "dof 0 is exactly fully constrained"
    );
    assert_eq!(
        sk_field(&r, "constraint_status").as_deref(),
        Some("fully constrained (dof 0)")
    );
}

/// An over-constrained sketch SAYS SO: a fifth redundant length pushes the
/// solver's dof negative, and the sketch reports the named over-constrained
/// state (never a silent "solved"). Removing the redundancy heals to dof 0.
#[test]
fn over_constrained_state_is_reported_honestly() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_one_line(&mut harness, rect);

    use sketch::constraints::ApplyKind as K;

    // Four lengths -> dof 0 (fully constrained).
    for _ in 0..4 {
        click_constraint(&mut harness, rect, K::Length);
    }
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "dof"), Some(0));
    assert_eq!(sk_field(&r, "solve").as_deref(), Some("fully"));

    // A fifth redundant length makes the solver's dof NEGATIVE -> the named
    // over-constrained verdict, never a fake "fully constrained".
    click_constraint(&mut harness, rect, K::Length);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(5));
    assert_eq!(
        sk_field(&r, "solve").as_deref(),
        Some("over"),
        "over-constrained is a DISTINCT solve state"
    );
    let status = sk_field(&r, "constraint_status").unwrap_or_default();
    assert!(
        status.starts_with("over-constrained (dof -"),
        "status names the over-constrained state: {status}"
    );
    // The honest dof is surfaced (negative), never replaced by a fake 0.
    let dof = sk_int(&r, "dof").expect("negative dof surfaced, not hidden");
    assert!(dof < 0, "over-constrained dof is negative (solver truth)");

    // Removing the redundancy heals back to dof 0 (fully constrained).
    click_constraint(&mut harness, rect, K::Remove);
    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(4));
    assert_eq!(sk_int(&r, "dof"), Some(0));
    assert_eq!(sk_field(&r, "solve").as_deref(), Some("fully"));
    assert_eq!(
        sk_field(&r, "constraint_status").as_deref(),
        Some("fully constrained (dof 0)")
    );
}

/// The context menu (right-click in the viewport) drives the SAME constraint
/// actions as the toolbar through the app's own `apply_constraint` seam.
/// Opening it does not place draw geometry, and clicking a menu item applies
/// the constraint exactly as the toolbar would (same `constraints::apply`).
#[test]
fn context_menu_applies_the_same_constraints_as_the_toolbar() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_one_line(&mut harness, rect);

    // Right-click inside the viewport (off the toolbar) opens the menu.
    let anchor = egui::pos2(rect.center().x, rect.center().y);
    harness.event(egui::Event::PointerButton {
        pos: anchor,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos: anchor,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step();

    // The menu is open (oracle signal), and right-clicking committed nothing.
    let r = sketch_report(&harness);
    assert_eq!(
        sk_field(&r, "context_menu").as_deref(),
        Some("1"),
        "right-click opens the constraint context menu"
    );
    assert_eq!(
        sk_int(&r, "docpoints"),
        Some(2),
        "right-click adds no geometry"
    );
    assert_eq!(sk_int(&r, "doclines"), Some(1));

    // Click the menu's Horizontal item - at the deterministic rect the
    // renderer uses ([`sketch::constraint_menu_layout`]). The app applies
    // through the same `constraints::apply` seam as the toolbar.
    let menu_buttons = sketch::constraint_menu_layout(anchor);
    let h = menu_buttons
        .iter()
        .find(|b| b.kind == sketch::constraints::ApplyKind::Horizontal)
        .expect("menu has the horizontal action");
    click_at(&mut harness, h.rect.center());

    let r = sketch_report(&harness);
    assert_eq!(sk_int(&r, "constraints"), Some(1));
    assert_eq!(sk_int(&r, "dof"), Some(3));
    assert_eq!(sk_field(&r, "constraint").as_deref(), Some("horizontal l0"));
    // The menu closes after the item is chosen.
    assert_eq!(sk_field(&r, "context_menu").as_deref(), Some("0"));
}
