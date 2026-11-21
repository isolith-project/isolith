//! UI-B4 UI-SKETCH-DRAW: point, line, polyline, rectangle, circle (wave B).
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts the card's
//! acceptance exactly as written:
//!
//! 1. Four clicks on the Rectangle tool produce a closed rectangle that lands
//!    in the document as **four points and four lines** (the last line closes
//!    back to the first point), with the correct plane coordinates at the
//!    click sites, committed through the UI-B2 journal (one entry per discrete
//!    `sk:point` / `sk:line` command).
//! 2. The preview is **tool state only**: after one/two/three clicks the
//!    document still holds nothing (`docpoints=0`, `doclines=0`) while the
//!    preview steps (`preview_points`, `preview_lines`) grow. Only the
//!    completing fourth click commits.
//! 3. Escape semantics: one Escape cancels exactly one step (the preview
//!    shrinks, the tool stays); a second Escape leaves the tool; a third
//!    Escape leaves sketch mode - and the document is untouched throughout.
//!
//! The math here reads the app's own code (`sketch.rs` is pulled in verbatim
//! via `#[path]`), so the click positions are computed with the same
//! world-to-panel projection the panel uses and the same viewport rect the
//! app resolves the click with, keeping the drawn coordinates exact.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact sketch module source the viewport panel compiles (math + the
// draw tool machine), joined here so the acceptance computes click positions
// and expected ids with the app's own functions. `duplicate_mod` is the same
// deliberate allowance sketch_enter.rs makes for its second `#[path]` copy.
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
/// The doc panel's report-oracle key.
const DOC_REPORT_KEY: &str = "ui_a7_doc_report";

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

/// The doc panel's `String` oracle (the on-disk document identity).
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

/// `sk_field` parsed as a number.
fn sk_num(report: &str, key: &str) -> Option<u64> {
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

/// Enter sketch mode on the XY datum plane, exactly as the app resolves it:
/// project a world point on the XY plane to a click position, verify with the
/// panel's own pick seam that the round-trip ray picks XY, click there, then
/// assert that the app's own viewport-rect derivation matches the AccessKit
/// bounds (so the clicks below land where the app expects them).
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> egui::Rect {
    harness.run();

    let rect = viewport_rect(harness);
    let before = current_snapshot(harness).expect("first frame publishes a snapshot");
    let cam = camera_from_snapshot(&before);
    let scale = harness.ctx.pixels_per_point();

    // The click position for a plane-entry world point.
    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let click_pos = sketch::world_to_panel(&cam, rect, scale, &p_world)
        .expect("entry point projects in front of the camera");
    assert!(rect.contains(click_pos));

    click(harness, click_pos);

    // The app's internal viewport rect == the a11y bounds (any drift here
    // would mis-resolve the draw clicks below).
    let internal = sketch::viewport_rect(&harness.ctx);
    assert!(
        (internal.min.x - rect.min.x).abs() < 0.01
            && (internal.min.y - rect.min.y).abs() < 0.01
            && (internal.width() - rect.width()).abs() < 0.01
            && (internal.height() - rect.height()).abs() < 0.01,
        "app viewport rect {internal:?} must match the a11y bounds {rect:?}"
    );

    assert_eq!(
        sk_field(&sketch_report(harness), "mode").as_deref(),
        Some("sketch"),
        "precondition: sketch mode active after the entry click"
    );
    rect
}

/// The demo scene the viewport renders (the canonical 80x40x10 solid), whose
/// bounding-box centroid is where the XY datum plane passes through. The
/// three datum planes are built through that centroid, so a click resolves to
/// plane coordinates measured from it.
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

/// The plane coordinates this suite draws (kept inside the XY sheet's ±56×±28
/// bounds for the empty demo scene).
fn rectangle_uv_corners() -> [(f64, f64); 4] {
    [(30.0, 10.0), (50.0, 10.0), (50.0, 22.0), (30.0, 22.0)]
}

/// The world-space corners of the rectangle this suite draws, ON the XY plane
/// (so the app resolves each click back to exactly the targeted coordinates).
fn rectangle_corners() -> [Vec3; 4] {
    rectangle_uv_corners().map(uv_to_world)
}

/// Project each rectangle corner to an exact click position with the app's
/// own world-to-panel math (same camera + rect as the running app).
fn rectangle_click_positions(harness: &Harness<'_, app::App>, rect: egui::Rect) -> [egui::Pos2; 4] {
    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera published"));
    let scale = harness.ctx.pixels_per_point();
    let corners = rectangle_corners();
    let mut out: Vec<egui::Pos2> = Vec::new();
    for world in corners {
        let pos = sketch::world_to_panel(&cam, rect, scale, &world).expect("corner in front");
        assert!(
            rect.contains(pos),
            "corner {world:?} projects inside the panel"
        );
        out.push(pos);
    }
    [out[0], out[1], out[2], out[3]]
}

/// Select the Rectangle tool with the plain `R` key.
fn select_rectangle(h: &mut Harness<'_, app::App>) {
    h.key_press(egui::Key::R);
    h.step();
    h.step();
}

/// Section 1 of the acceptance: four clicks produce a closed rectangle that
/// lands in the document as four points and four lines, committed through
/// the journal as validated `sk:point`/`sk:line` commands.
#[test]
fn four_clicks_build_a_closed_rectangle_in_the_document() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    let doc_before = doc_report(&harness);

    select_rectangle(&mut harness);
    assert_eq!(
        sk_field(&sketch_report(&harness), "tool").as_deref(),
        Some("rectangle"),
        "R selects the rectangle tool"
    );

    let positions = rectangle_click_positions(&harness, rect);
    for (i, pos) in positions.iter().enumerate() {
        assert_eq!(
            sk_num(&sketch_report(&harness), "docpoints"),
            Some(0),
            "the preview is never committed mid-gesture (before click {i})"
        );
        click(&mut harness, *pos);
    }

    let report = sketch_report(&harness);
    assert_eq!(
        sk_num(&report, "docpoints"),
        Some(4),
        "four points landed in the document"
    );
    assert_eq!(
        sk_num(&report, "doclines"),
        Some(4),
        "four lines landed in the document"
    );
    assert_eq!(
        sk_num(&report, "circles"),
        Some(0),
        "no circles from a rectangle"
    );
    assert_eq!(
        sk_num(&report, "steps"),
        Some(0),
        "the completed gesture's preview is consumed"
    );
    assert_eq!(
        sk_num(&report, "journal"),
        Some(8),
        "four sk:point + four sk:line commands journaled"
    );

    // The four committed points are exactly the four targeted plane
    // coordinates (the app resolved the clicks through the same unproject
    // seam back onto the XY plane, relative to the plane centre).
    let point_lines: Vec<(f64, f64)> = report
        .lines()
        .filter_map(|l| l.strip_prefix("point="))
        .map(|l| {
            let mut it = l.split(',');
            (
                it.next().unwrap().trim().parse().unwrap(),
                it.next().unwrap().trim().parse().unwrap(),
            )
        })
        .collect();
    assert_eq!(point_lines.len(), 4, "oracle lists the committed points");
    let want = rectangle_uv_corners();
    for (i, (x, y)) in point_lines.iter().enumerate() {
        assert!(
            (x - want[i].0).abs() < 0.001 && (y - want[i].1).abs() < 0.001,
            "point {i} must be the click site ({}, {}) got ({x}, {y})",
            want[i].0,
            want[i].1
        );
    }

    // The four lines close the loop: l0 p0-p1, l1 p1-p2, l2 p2-p3, l3 p3-p0.
    let line_ids: Vec<(String, String)> = report
        .lines()
        .filter_map(|l| l.strip_prefix("lineseg="))
        .map(|l| {
            let (_, rest) = l.split_once(':').unwrap();
            let (a, b) = rest.split_once("->").unwrap();
            (a.trim().to_string(), b.trim().to_string())
        })
        .collect();
    assert_eq!(line_ids.len(), 4, "oracle lists the committed lines");
    assert_eq!(
        line_ids,
        vec![
            ("p0".to_string(), "p1".to_string()),
            ("p1".to_string(), "p2".to_string()),
            ("p2".to_string(), "p3".to_string()),
            ("p3".to_string(), "p0".to_string()),
        ],
        "four lines, the last closing back to the first point"
    );

    // The sketch document is its own canonical document; the visible part
    // document (doc panel) is untouched by a sketch draw this wave.
    assert_eq!(
        doc_report(&harness),
        doc_before,
        "sketch geometry does not mutate the part document oracle"
    );
}

/// Section 2 of the acceptance: the preview is never committed until the
/// gesture completes - after one/two/three rectangle clicks the document is
/// still empty while the preview steps grow, and only the fourth commits.
#[test]
fn preview_is_never_committed_until_the_rectangle_completes() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    select_rectangle(&mut harness);

    let positions = rectangle_click_positions(&harness, rect);

    // First click: one preview anchor, nothing committed.
    click(&mut harness, positions[0]);
    let r1 = sketch_report(&harness);
    assert_eq!(sk_num(&r1, "steps"), Some(1));
    assert_eq!(sk_num(&r1, "preview_points"), Some(1));
    assert_eq!(sk_num(&r1, "preview_lines"), Some(0));
    assert_eq!(
        sk_num(&r1, "docpoints"),
        Some(0),
        "first click commits nothing"
    );
    assert_eq!(sk_num(&r1, "doclines"), Some(0));
    assert_eq!(sk_num(&r1, "journal"), Some(0));

    // Second click: two preview anchors and one preview edge; still empty.
    click(&mut harness, positions[1]);
    let r2 = sketch_report(&harness);
    assert_eq!(sk_num(&r2, "steps"), Some(2));
    assert_eq!(sk_num(&r2, "preview_points"), Some(2));
    assert_eq!(sk_num(&r2, "preview_lines"), Some(1));
    assert_eq!(
        sk_num(&r2, "docpoints"),
        Some(0),
        "second click commits nothing"
    );
    assert_eq!(sk_num(&r2, "doclines"), Some(0));

    // Third click: three anchors, two edges; still empty.
    click(&mut harness, positions[2]);
    let r3 = sketch_report(&harness);
    assert_eq!(sk_num(&r3, "steps"), Some(3));
    assert_eq!(sk_num(&r3, "preview_points"), Some(3));
    assert_eq!(sk_num(&r3, "preview_lines"), Some(2));
    assert_eq!(
        sk_num(&r3, "docpoints"),
        Some(0),
        "third click commits nothing"
    );
    assert_eq!(sk_num(&r3, "doclines"), Some(0));

    // Completing fourth click: the whole rectangle lands at once.
    click(&mut harness, positions[3]);
    let r4 = sketch_report(&harness);
    assert_eq!(
        sk_num(&r4, "steps"),
        Some(0),
        "preview consumed on completion"
    );
    assert_eq!(sk_num(&r4, "docpoints"), Some(4));
    assert_eq!(sk_num(&r4, "doclines"), Some(4));
    assert_eq!(sk_num(&r4, "journal"), Some(8));
}

/// Section 3 of the acceptance: one Escape cancels exactly one step (the
/// preview shrinks, the tool stays, nothing is committed); a second Escape
/// leaves the tool; a third Escape leaves sketch mode - and the document
/// oracle is untouched throughout.
#[test]
fn escape_cancels_one_step_then_a_second_leaves_the_tool() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    select_rectangle(&mut harness);

    let positions = rectangle_click_positions(&harness, rect);
    click(&mut harness, positions[0]);
    click(&mut harness, positions[1]);
    click(&mut harness, positions[2]);
    let doc_before = doc_report(&harness);
    assert_eq!(sk_num(&sketch_report(&harness), "steps"), Some(3));

    // First Escape: exactly one step is cancelled (3 -> 2), the tool stays.
    harness.key_press(egui::Key::Escape);
    harness.step();
    harness.step();
    let r1 = sketch_report(&harness);
    assert_eq!(sk_num(&r1, "steps"), Some(2), "one step cancelled, not all");
    assert_eq!(sk_num(&r1, "preview_points"), Some(2));
    assert_eq!(
        sk_field(&r1, "tool").as_deref(),
        Some("rectangle"),
        "the tool stays after one Escape"
    );
    assert_eq!(
        sk_num(&r1, "docpoints"),
        Some(0),
        "cancelling commits nothing"
    );
    assert_eq!(sk_num(&r1, "doclines"), Some(0));

    // Second Escape: leaves the tool.
    harness.key_press(egui::Key::Escape);
    harness.step();
    harness.step();
    let r2 = sketch_report(&harness);
    assert_eq!(sk_field(&r2, "tool").as_deref(), Some("-"), "tool left");
    assert_eq!(sk_num(&r2, "steps"), Some(0));

    // Third Escape: leaves sketch mode entirely.
    harness.key_press(egui::Key::Escape);
    harness.step();
    harness.step();
    let r3 = sketch_report(&harness);
    assert_eq!(
        sk_field(&r3, "mode").as_deref(),
        Some("idle"),
        "sketch exited"
    );

    // The document is untouched by all three Escapes.
    assert_eq!(doc_report(&harness), doc_before);
}

/// The line tool path: two clicks land two points and the line between them
/// (a second tool working through the same journal seam).
#[test]
fn line_tool_commits_two_points_and_the_line() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);

    harness.key_press(egui::Key::L);
    harness.step();
    harness.step();
    assert_eq!(
        sk_field(&sketch_report(&harness), "tool").as_deref(),
        Some("line")
    );

    let cam = camera_from_snapshot(&current_snapshot(&harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let a = sketch::world_to_panel(&cam, rect, scale, &uv_to_world((5.0, 8.0))).unwrap();
    let b = sketch::world_to_panel(&cam, rect, scale, &uv_to_world((35.0, 8.0))).unwrap();
    assert!(rect.contains(a) && rect.contains(b));

    click(&mut harness, a);
    assert_eq!(sk_num(&sketch_report(&harness), "docpoints"), Some(0));
    click(&mut harness, b);

    let r = sketch_report(&harness);
    assert_eq!(sk_num(&r, "docpoints"), Some(2));
    assert_eq!(sk_num(&r, "doclines"), Some(1));
    assert_eq!(sk_num(&r, "journal"), Some(3)); // 2 points + 1 line
}

/// The point tool path: one click commits a single point.
#[test]
fn point_tool_commits_one_point() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);

    harness.key_press(egui::Key::P);
    harness.step();
    harness.step();
    assert_eq!(
        sk_field(&sketch_report(&harness), "tool").as_deref(),
        Some("point")
    );

    let cam = camera_from_snapshot(&current_snapshot(&harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let pos = sketch::world_to_panel(&cam, rect, scale, &uv_to_world((10.0, 12.0))).unwrap();
    assert!(rect.contains(pos));

    click(&mut harness, pos);
    let r = sketch_report(&harness);
    assert_eq!(sk_num(&r, "docpoints"), Some(1));
    assert_eq!(sk_num(&r, "doclines"), Some(0));
    assert_eq!(sk_num(&r, "journal"), Some(1));
}

/// The polyline path: an open chain stays pure preview until Enter commits
/// the placed vertices and the chain of lines.
#[test]
fn polyline_commits_the_chain_on_enter() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);

    harness.key_press(egui::Key::N);
    harness.step();
    harness.step();
    assert_eq!(
        sk_field(&sketch_report(&harness), "tool").as_deref(),
        Some("polyline")
    );

    let cam = camera_from_snapshot(&current_snapshot(&harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let pts: Vec<egui::Pos2> = [(5.0, 8.0), (25.0, 8.0), (25.0, 18.0)]
        .map(|uv| sketch::world_to_panel(&cam, rect, scale, &uv_to_world(uv)).unwrap())
        .to_vec();
    for p in &pts {
        assert!(rect.contains(*p));
    }

    for p in &pts {
        click(&mut harness, *p);
    }
    let mid = sketch_report(&harness);
    assert_eq!(
        sk_num(&mid, "steps"),
        Some(3),
        "chain previewed, not committed"
    );
    assert_eq!(sk_num(&mid, "docpoints"), Some(0));

    harness.key_press(egui::Key::Enter);
    harness.step();
    harness.step();

    let after = sketch_report(&harness);
    assert_eq!(sk_num(&after, "docpoints"), Some(3));
    assert_eq!(sk_num(&after, "doclines"), Some(2));
    assert_eq!(sk_num(&after, "journal"), Some(5)); // 3 points + 2 lines
    assert_eq!(sk_num(&after, "steps"), Some(0));
}
