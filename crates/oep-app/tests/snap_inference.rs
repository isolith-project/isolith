//! UI-B5 UI-SNAP-INFERENCE: endpoint, midpoint, centre, on-edge, intersection,
//! h/v (wave B).
//!
//! The acceptance as the card writes it: one test per inference type, each
//! asserting the **exact snapped coordinate** from a fixture sketch; the
//! inference **priority order** explicit and tested (endpoint beats on-edge,
//! etc.); and the **toggle** proves it disables inference entirely and is a
//! command token like any other.
//!
//! Two layers:
//!
//! 1. The pure engine - `sketch::snap::infer` over a deterministic fixture
//!    `SketchDoc` (10 points, 4 lines, 1 circle) - asserts the exact snapped
//!    plane coordinate and kind for each of the six inference types plus the
//!    h/v pair, and the priority pairs the card calls out.
//! 2. The real app through `egui_kittest` (same harness the wave's other
//!    cards use): while a draw tool is active the pointer snaps on hover -
//!    the oracle reports the firing inference and the exact snapped
//!    coordinate (`infer=`/`snapx=`/`snapy=`, the data that drives the glyph
//!    and its label) - and a click lands **exactly** on the snapped
//!    coordinate. Pressing the toggle (F8, the `sk:snap` command token) turns
//!    inference off entirely: the pointer reports no inference and a click
//!    lands on the raw coordinate.
//!
//! The math shares the app's own source (`panels/sketch.rs` pulled in verbatim
//! via `#[path]`, exactly like `sketch_draw.rs`), so click positions come from
//! the same world-to-panel projection the panel uses and the same activator
//! seam that resolves them.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact sketch module source the viewport panel compiles (the draw-tool
// machine + the snap engine + the interactive half), joined here so the
// acceptance shares the app's own functions. `duplicate_mod` is the same
// deliberate allowance the other wave-B targets make for their `#[path]` copy.
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

/// The snap radius the running app applies (half a sketch-grid cell).
const R: f64 = sketch::snap::SNAP_RADIUS;

// ---------------------------------------------------------------------------
// The fixture sketch (the "fixture sketch" of the acceptance). Every
// inference type this card names has an exact, non-ambiguous candidate.
// ---------------------------------------------------------------------------

/// Build the deterministic fixture sketch the per-type assertions infer over:
///
/// - points `p0 (0,0)`, `p1 (40,0)`, `p2 (0,30)` - the ends of `l0`/`l1`;
/// - `p3 (20,20)` - the centre of `c0` (a circle centre, not a line end);
/// - `p4 (25,-10)`, `p5 (25,40)` - the ends of `l2` (vertical `x=25`);
/// - `p6 (60,10)`, `p7 (70,5)` - free points carrying the h/v alignment
///   guides (no edge terminates there, so they never fire Endpoint);
/// - `p8 (5,5)`, `p9 (35,15)` - the ends of `l3`, a rotated diagonal used to
///   prove on-edge inference is not axis-parallel-only;
/// - `c0` - circle, centre `p3`, radius 10.
fn fixture() -> sketch::tools::SketchDoc {
    let mut doc = sketch::tools::SketchDoc::default();
    let p0 = doc.add_point(0.0, 0.0);
    let p1 = doc.add_point(40.0, 0.0);
    let p2 = doc.add_point(0.0, 30.0);
    let p3 = doc.add_point(20.0, 20.0);
    let p4 = doc.add_point(25.0, -10.0);
    let p5 = doc.add_point(25.0, 40.0);
    // Free points carrying the h/v alignment guides; no edge terminates here,
    // so no Endpoint candidate ever arises at them (asserted by the tests).
    doc.add_point(60.0, 10.0);
    doc.add_point(70.0, 5.0);
    let p8 = doc.add_point(5.0, 5.0);
    let p9 = doc.add_point(35.0, 15.0);
    doc.add_line(&p0, &p1); // l0: (0,0)-(40,0)
    doc.add_line(&p0, &p2); // l1: (0,0)-(0,30)
    doc.add_line(&p4, &p5); // l2: (25,-10)-(25,40); crosses l0 at (25,0)
    doc.add_line(&p8, &p9); // l3: diagonal (5,5)-(35,15)
    doc.add_circle(&p3, 10.0); // c0: centre (20,20)
    doc
}

/// Plane-coordinate equality within the float tolerance the engine's math
/// holds (projections, crossings, midpoints are exact to well below this).
fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

// ---------------------------------------------------------------------------
// One test per inference type, each asserting the exact snapped coordinate.
// ---------------------------------------------------------------------------

#[test]
fn endpoint_inference_snaps_to_the_exact_point() {
    let doc = fixture();
    // Probing off a line end but still closely following l0: the end of the
    // edge at (0,0) is the winning candidate, exactly.
    let hit = sketch::snap::infer(&doc, (2.0, 1.0), R).expect("endpoint in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Endpoint);
    assert!(close(hit.point, (0.0, 0.0)), "got {:?}", hit.point);
    assert!(hit.distance < R);
}

#[test]
fn midpoint_inference_snaps_to_the_exact_midpoint() {
    let doc = fixture();
    // Near l1's midpoint (0,15): the half-way point of the edge, exactly.
    let hit = sketch::snap::infer(&doc, (0.3, 15.2), R).expect("midpoint in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Midpoint);
    assert!(close(hit.point, (0.0, 15.0)), "got {:?}", hit.point);
}

#[test]
fn centre_inference_snaps_to_the_exact_circle_centre() {
    let doc = fixture();
    // Near c0's centre (20,20). The centre point is a committed point but it
    // terminates no line, so it infers as Centre - not Endpoint - and beats
    // the nearer on-edge candidate on l2 (priority is tested separately).
    let hit = sketch::snap::infer(&doc, (23.0, 20.5), R).expect("centre in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Centre);
    assert!(close(hit.point, (20.0, 20.0)), "got {:?}", hit.point);
}

#[test]
fn on_edge_inference_snaps_to_the_exact_closest_segment_point() {
    let doc = fixture();
    // Probing close to l0's row: the closest point on the segment, exactly
    // the perpendicular projection (14,0).
    let hit = sketch::snap::infer(&doc, (14.0, 0.4), R).expect("on-edge in range");
    assert_eq!(hit.kind, sketch::snap::Inference::OnEdge);
    assert!(close(hit.point, (14.0, 0.0)), "got {:?}", hit.point);
}

#[test]
fn on_edge_inference_works_on_a_rotated_segment() {
    let doc = fixture();
    // l3 is a rotated diagonal, so on-edge must project onto it exactly, not
    // just mirror the axis grid.
    let hit = sketch::snap::infer(&doc, (10.0, 7.0), R).expect("rotated on-edge in range");
    assert_eq!(hit.kind, sketch::snap::Inference::OnEdge);
    assert!(close(hit.point, (10.1, 6.7)), "got {:?}", hit.point);
}

#[test]
fn intersection_inference_snaps_to_the_exact_crossing() {
    let doc = fixture();
    // l0 (y=0) and l2 (x=25) cross at (25,0); the pointer near the crossing
    // snaps exactly there, even though the two edges themselves are nearer.
    let hit = sketch::snap::infer(&doc, (25.6, 0.4), R).expect("intersection in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Intersection);
    assert!(close(hit.point, (25.0, 0.0)), "got {:?}", hit.point);
}

#[test]
fn horizontal_alignment_snaps_to_the_exact_row() {
    let doc = fixture();
    // p6 (60,10) carries a horizontal guide through its row y=10; the pointer
    // near that row - but off every edge - snaps onto it exactly.
    let hit = sketch::snap::infer(&doc, (55.0, 10.2), R).expect("row guide in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Horizontal);
    assert!(close(hit.point, (55.0, 10.0)), "got {:?}", hit.point);
}

#[test]
fn vertical_alignment_snaps_to_the_exact_column() {
    let doc = fixture();
    // p7 (70,5) carries a vertical guide through its column x=70; the pointer
    // near that column snaps onto it exactly.
    let hit = sketch::snap::infer(&doc, (69.8, 25.0), R).expect("column guide in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Vertical);
    assert!(close(hit.point, (70.0, 25.0)), "got {:?}", hit.point);
}

// ---------------------------------------------------------------------------
// Priority order - explicit and tested (endpoint beats on-edge, etc.).
// ---------------------------------------------------------------------------

/// The card's named pair: probing where the on-edge candidate is *nearer*
/// than the endpoint must still snap to the endpoint - priority, not raw
/// proximity, picks the winner.
#[test]
fn endpoint_beats_on_edge_even_when_the_edge_is_nearer() {
    let doc = fixture();
    let pointer = (2.0, 1.0);
    let on_edge_d = sketch::tools::distance(pointer, (2.0, 0.0)); // 1.0
    let endpoint_d = sketch::tools::distance(pointer, (0.0, 0.0)); // ~2.24
    assert!(
        on_edge_d < endpoint_d,
        "precondition: the edge is nearer than the endpoint"
    );
    let hit = sketch::snap::infer(&doc, pointer, R).expect("endpoint in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Endpoint);
    assert!(close(hit.point, (0.0, 0.0)));
}

/// Intersection outranks the two edges that form it: a crossing is a more
/// specific pick than riding either edge.
#[test]
fn intersection_beats_on_edge_even_when_the_edge_is_nearer() {
    let doc = fixture();
    let pointer = (25.6, 0.4);
    let on_edge_d = sketch::tools::distance(pointer, (25.6, 0.0)); // 0.4
    let cross_d = sketch::tools::distance(pointer, (25.0, 0.0)); // ~0.72
    assert!(
        on_edge_d < cross_d,
        "precondition: an edge is nearer than the crossing"
    );
    let hit = sketch::snap::infer(&doc, pointer, R).expect("intersection in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Intersection);
    assert!(close(hit.point, (25.0, 0.0)));
}

/// Midpoint outranks the on-edge candidate of the same line.
#[test]
fn midpoint_beats_on_edge_even_when_the_edge_is_nearer() {
    let doc = fixture();
    let pointer = (0.3, 15.2);
    let on_edge_d = sketch::tools::distance(pointer, (0.0, 15.2)); // 0.3
    let mid_d = sketch::tools::distance(pointer, (0.0, 15.0)); // ~0.36
    assert!(
        on_edge_d < mid_d,
        "precondition: the edge is nearer than the midpoint"
    );
    let hit = sketch::snap::infer(&doc, pointer, R).expect("midpoint in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Midpoint);
    assert!(close(hit.point, (0.0, 15.0)));
}

/// Centre outranks the on-edge candidate of a line passing near the centre.
#[test]
fn centre_beats_on_edge_even_when_the_edge_is_nearer() {
    let doc = fixture();
    let pointer = (23.0, 20.5);
    let on_edge_d = sketch::tools::distance(pointer, (25.0, 20.5)); // 2.0
    let centre_d = sketch::tools::distance(pointer, (20.0, 20.0)); // ~3.04
    assert!(
        on_edge_d < centre_d,
        "precondition: the edge is nearer than the centre"
    );
    let hit = sketch::snap::infer(&doc, pointer, R).expect("centre in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Centre);
    assert!(close(hit.point, (20.0, 20.0)));
}

/// The two kinds along l0's own row compete at equal distance and the same
/// coordinate by design - the geometry inference (OnEdge) outranks the
/// ambient alignment guide (the card's h/v rank last).
#[test]
fn on_edge_beats_the_ambient_row_alignment() {
    let doc = fixture();
    // (14,0.4): on-edge projection (14,0) and the horizontal guide through the
    // row y=0 land on the same point at the same distance.
    let hit = sketch::snap::infer(&doc, (14.0, 0.4), R).expect("on-edge in range");
    assert_eq!(
        hit.kind,
        sketch::snap::Inference::OnEdge,
        "the geometry inference, not the guide, owns the row"
    );
    assert!(close(hit.point, (14.0, 0.0)));
}

/// At a shared corner (where two lines terminate at one point), Endpoint -
/// rank 0 - owns the point over the Intersection of the two segments.
#[test]
fn endpoint_beats_intersection_at_a_shared_corner() {
    let doc = fixture();
    let hit = sketch::snap::infer(&doc, (0.5, 0.5), R).expect("corner in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Endpoint);
    assert!(close(hit.point, (0.0, 0.0)));
}

/// Endpoint is a line end, not any committed point: the circle centre p3
/// fires Centre, and a free point (off every guide and edge) fires the h/v
/// guide - never Endpoint.
#[test]
fn endpoint_is_a_line_end_not_every_committed_point() {
    let doc = fixture();
    // At the circle centre exactly: Centre, never Endpoint.
    let hit = sketch::snap::infer(&doc, (20.0, 20.0), R).expect("centre in range");
    assert_eq!(hit.kind, sketch::snap::Inference::Centre);
    // A free point p6 (60,10) terminates no line: near it the only candidates
    // are its own h/v alignment guides, never an Endpoint.
    let hit = sketch::snap::infer(&doc, (61.0, 10.8), R).expect("free-point guide in range");
    assert_ne!(
        hit.kind,
        sketch::snap::Inference::Endpoint,
        "a free point is not a line end"
    );
    assert!(
        close(hit.point, (61.0, 10.0)),
        "row guide of the free point"
    );
}

// ---------------------------------------------------------------------------
// Engine edge lanes exercised by effective_uv's guards.
// ---------------------------------------------------------------------------

#[test]
fn empty_document_has_no_inference() {
    assert!(sketch::snap::infer(&sketch::tools::SketchDoc::default(), (5.0, 5.0), R).is_none());
}

/// The disabled lane: a non-positive snap radius turns inference off entirely
/// (the effective_uv guard also skips the empty-document doc read the same
/// way).
#[test]
fn non_positive_radius_disables_inference() {
    let doc = fixture();
    assert!(sketch::snap::infer(&doc, (2.0, 1.0), 0.0).is_none());
    assert!(sketch::snap::infer(&doc, (2.0, 1.0), -1.0).is_none());
}

// ---------------------------------------------------------------------------
// The toggle is a command token like any other.
// ---------------------------------------------------------------------------

#[test]
fn toggle_is_a_command_token_like_any_other() {
    // The registry's own validate step - the exact one the UI-B1 router runs
    // on every binding row, and the commit seam on every sketch command -
    // accepts the toggle's dispatch family head.
    assert_eq!(
        oep_shell::commands::validate_token(sketch::snap::SNAP_FAMILY),
        Ok(sketch::snap::SNAP_FAMILY),
        "the family head validates through the CLI/router step"
    );
    // The token is a proper `sk:`-family command line (`sk:point`, `sk:line`,
    // `sk:snap`), spelled so a later wiring card can register the engine-side
    // command record without a spelling change.
    assert_eq!(
        sketch::snap::SNAP_TOKEN.split(':').next(),
        Some(sketch::snap::SNAP_FAMILY),
        "the toggle token carries the same family as the draw commands"
    );
    assert!(sketch::snap::SNAP_TOKEN.ends_with("snap"));
    // The kittest toggle below drives exactly this token (the keypress maps to
    // it and the oracle reports the token name in `last=`).
    assert_eq!(sketch::SNAP_TOGGLE_KEY, egui::Key::F8);
}

// ---------------------------------------------------------------------------
// The real app through kittest: pointer snaps on hover (glyph + label data in
// the oracle), a click lands exactly on the snapped coordinate, and the
// toggle turns inference off entirely.
// ---------------------------------------------------------------------------

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

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

fn current_snapshot(harness: &Harness<'_, app::App>) -> Option<Snapshot> {
    harness
        .ctx
        .data_mut(|d| d.get_temp(egui::Id::unique(VIEWPORT_STATE_KEY)))
}

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

fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

fn sk_field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// `sk_field` parsed as a number (the oracle's `-` placeholder for "none"
/// parses to `None`).
fn sk_num(report: &str, key: &str) -> Option<f64> {
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

fn click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    press(h, pos);
    h.step();
    release(h, pos);
    h.step();
    h.step();
}

/// Enter sketch mode on the XY datum plane exactly as the app resolves it:
/// project a world point on the XY sheet to a click position, click, then
/// assert the app's own internal viewport rect matches the a11y bounds.
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> egui::Rect {
    harness.run();
    let rect = viewport_rect(harness);
    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera published"));
    let scale = harness.ctx.pixels_per_point();
    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let click_pos = sketch::world_to_panel(&cam, rect, scale, &p_world)
        .expect("entry point projects in front of the camera");
    assert!(rect.contains(click_pos));
    click(harness, click_pos);

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

/// The exact click position for a plane coordinate on the XY sheet, using the
/// running app's own camera and viewport rect.
fn panel_pos_at(harness: &Harness<'_, app::App>, rect: egui::Rect, uv: (f64, f64)) -> egui::Pos2 {
    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let pos = sketch::world_to_panel(&cam, rect, scale, &uv_to_world(uv)).expect("in front");
    assert!(rect.contains(pos), "plane {uv:?} projects inside the panel");
    pos
}

/// Press a plain key and let the app settle.
fn key(h: &mut Harness<'_, app::App>, k: egui::Key) {
    h.key_press(k);
    h.step();
    h.step();
}

/// Move the pointer to `pos` and let the app settle (so the cursor/hover
/// inference is refreshed and the oracle is current).
fn hover(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    h.step();
    h.step();
}

/// Draw the fixture line (0,0)-(40,0) on the XY sheet with the Line tool, so
/// the committed sketch document has real geometry for inference to run over.
fn draw_fixture_line(harness: &mut Harness<'_, app::App>, rect: egui::Rect) {
    key(harness, egui::Key::L);
    assert_eq!(
        sk_field(&sketch_report(harness), "tool").as_deref(),
        Some("line")
    );
    click(harness, panel_pos_at(harness, rect, (0.0, 0.0)));
    click(harness, panel_pos_at(harness, rect, (40.0, 0.0)));
    let r = sketch_report(harness);
    assert_eq!(
        sk_field(&r, "docpoints").and_then(|v| v.parse().ok()),
        Some(2_u64)
    );
    assert_eq!(
        sk_field(&r, "doclines").and_then(|v| v.parse().ok()),
        Some(1_u64)
    );
}

/// The committed point coordinates from the oracle (`point=x,y,id` lines).
fn committed_points(report: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    for l in report.lines() {
        if let Some(rest) = l.strip_prefix("point=") {
            let mut it = rest.split(',');
            let x: f64 = it.next().unwrap().trim().parse().unwrap();
            let y: f64 = it.next().unwrap().trim().parse().unwrap();
            let id = it.next().unwrap().trim().to_string();
            out.push((x, y, id));
        }
    }
    out
}

/// While the Point tool is active, hovering near (but not on) the endpoint
/// (40,0) snaps the pointer exactly onto it: the oracle reports the fired
/// inference (the glyph + label data) and the exact snapped coordinate, and a
/// click lands exactly on the endpoint - not on the hovered pixel.
#[test]
fn pointer_snaps_and_lands_on_the_endpoint_with_glyph_and_label() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_fixture_line(&mut harness, rect);

    key(&mut harness, egui::Key::P);
    assert_eq!(
        sk_field(&sketch_report(&harness), "tool").as_deref(),
        Some("point")
    );

    // Hover 2 plane-units off the endpoint (41.5, 0.5): closer to l0's row
    // than to the point itself, but the endpoint inference (rank 0) wins and
    // publishes the exact snapped coordinate.
    let hover_pos = panel_pos_at(&harness, rect, (41.5, 0.5));
    hover(&mut harness, hover_pos);

    let rep = sketch_report(&harness);
    assert_eq!(
        sk_field(&rep, "snap").as_deref(),
        Some("on"),
        "inference on by default"
    );
    assert_eq!(
        sk_field(&rep, "infer").as_deref(),
        Some("endpoint"),
        "the firing inference names the glyph/label"
    );
    let snapx = sk_num(&rep, "snapx").expect("snapx published");
    let snapy = sk_num(&rep, "snapy").expect("snapy published");
    assert!(
        (snapx - 40.0).abs() < 0.001 && (snapy - 0.0).abs() < 0.001,
        "snapped coordinate must be exactly the endpoint (40,0), got ({snapx}, {snapy})"
    );

    // The click lands exactly on the endpoint, not the hovered coordinate.
    click(&mut harness, hover_pos);
    let after = sketch_report(&harness);
    let points = committed_points(&after);
    assert_eq!(points.len(), 3, "fixture line + one snapped point");
    let (_x, _y, id) = &points[2];
    assert_eq!(id, "p2");
    assert!(
        (points[2].0 - 40.0).abs() < 0.001 && (points[2].1 - 0.0).abs() < 0.001,
        "the snapped point must be the endpoint (40,0), got {:?}",
        points[2]
    );
}

/// F8 - the `sk:snap` command token - toggles inference off entirely: the
/// pointer stops reporting an inference and a click lands on the raw hovered
/// coordinate instead of the endpoint.
#[test]
fn snapping_toggle_disables_inference_and_lands_raw_coordinates() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_fixture_line(&mut harness, rect);

    key(&mut harness, egui::Key::P);
    assert_eq!(
        sk_field(&sketch_report(&harness), "snap").as_deref(),
        Some("on")
    );

    // Toggle inference off through the command token key.
    key(&mut harness, sketch::SNAP_TOGGLE_KEY);
    let toggled = sketch_report(&harness);
    assert_eq!(
        sk_field(&toggled, "snap").as_deref(),
        Some("off"),
        "the toggle turns inference off entirely"
    );
    assert_eq!(sk_field(&toggled, "infer").as_deref(), Some("-"));
    assert_eq!(sk_field(&toggled, "snapx").as_deref(), Some("-"));
    assert_eq!(sk_field(&toggled, "snapy").as_deref(), Some("-"));
    assert!(
        sk_field(&toggled, "last")
            .map(|l| l.contains("snap off") && l.contains(sketch::snap::SNAP_TOKEN))
            .unwrap_or(false),
        "the toggle reports the command token it drove"
    );

    // The same hover that snapped before now reports nothing.
    let hover_pos = panel_pos_at(&harness, rect, (41.5, 0.5));
    hover(&mut harness, hover_pos);
    let rep = sketch_report(&harness);
    assert_eq!(
        sk_field(&rep, "infer").as_deref(),
        Some("-"),
        "no inference reported"
    );
    assert_eq!(sk_field(&rep, "snapx").as_deref(), Some("-"));

    // The click lands on the raw hovered coordinate, not the endpoint.
    click(&mut harness, hover_pos);
    let after = sketch_report(&harness);
    let points = committed_points(&after);
    assert_eq!(points.len(), 3);
    assert!(
        (points[2].0 - 41.5).abs() < 0.01 && (points[2].1 - 0.5).abs() < 0.01,
        "raw coordinate (41.5,0.5) lands un-snapped, got {:?}",
        points[2]
    );

    // Toggling back on restores inference for the same pointer.
    key(&mut harness, sketch::SNAP_TOGGLE_KEY);
    hover(&mut harness, hover_pos);
    let back_on = sketch_report(&harness);
    assert_eq!(sk_field(&back_on, "snap").as_deref(), Some("on"));
    assert_eq!(sk_field(&back_on, "infer").as_deref(), Some("endpoint"));
    let snapx = sk_num(&back_on, "snapx").expect("snapx published");
    let snapy = sk_num(&back_on, "snapy").expect("snapy published");
    assert!(
        (snapx - 40.0).abs() < 0.001 && (snapy - 0.0).abs() < 0.001,
        "restored inference snaps back to (40,0), got ({snapx}, {snapy})"
    );
}
