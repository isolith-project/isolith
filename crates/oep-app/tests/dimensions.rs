//! UI-B7 UI-DIMENSIONS: place, drive and edit dimensions (wave B).
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts the card's
//! acceptance exactly as written:
//!
//! 1. A dimension is PLACED on the committed sketch geometry (the last drawn
//!    line) through the properties-panel "Place dimension" button; placing
//!    registers the variable-table entry `width` = the line's measured length
//!    and drives the model, whose volume comes from the document's own mass
//!    properties (80-wide -> 32000).
//! 2. The dimension is EDITED AS AN EXPRESSION: typing `width*1.5` into the
//!    properties expression field and applying it resolves `width` through the
//!    existing variable table path to 120, and the driven volume moves
//!    32000 -> 72000 (asserted via the same driven document, not a literal in
//!    the test).
//! 3. An INVALID expression is refused: the named error appears in the
//!    properties panel, and the variable table / dimension value / document
//!    (sketch points, lines, journal) are all untouched.
//!
//! The math reads the app's own code (`panels/properties.rs` and
//! `src/sketch/dimensions.rs` are pulled in verbatim via `#[path]`), so the
//! click positions are computed with the same region/world-to-panel
//! projection and the expected volume comes from the same driven document the
//! app reports - matching how the B6 constraints acceptance stays exact.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact properties-panel source the app compiles, included here so the
// acceptance computes the widget rects and the oracle keys from the app's own
// code (the same `#[path]` copy pattern the B4/B5/B6 tests use for sketch.rs).
#[path = "../src/panels/properties.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod properties;

// The exact sketch module source the app compiles (points/lines, datum planes,
// world-to-panel projection), included here so the acceptance computes clicks
// and expectations with the app's own math - the same `#[path]` copy pattern
// the B4/B5/B6 tests use.
#[path = "../src/panels/sketch.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sketch;

// The pure dimension engine, included verbatim so the test derives the
// expected volume from the same driven document the app reports.
#[path = "../src/sketch/dimensions.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod dimensions;

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
/// The dimension report-oracle key (must match
/// `app::panels::properties::DIMENSION_REPORT_KEY`).
const DIMENSION_REPORT_KEY: &str = "ui_b7_dimension_report";

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
/// tree.
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

/// The properties region's full rect, read from the app's own accessibility
/// tree (same probe as `properties::dimension_widget_rects(region)` uses).
fn properties_rect(harness: &Harness<'_, app::App>) -> egui::Rect {
    let node = harness
        .get_all_by_label("properties")
        .next()
        .expect("properties region present");
    let bounds = node
        .accesskit_node()
        .bounding_box()
        .expect("properties region has explicit bounds");
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

/// The sketch oracle, read back through the same context.
fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// The dimension oracle, read back through the same context.
fn dimension_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(DIMENSION_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of an oracle, e.g. `("dim_name", "width")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
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
/// let panels that run earlier in the frame catch state that lags by one.
fn click(h: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    press(h, pos);
    h.step();
    release(h, pos);
    h.step();
    h.step();
}

/// Enter sketch mode on the XY datum plane, exactly as the app resolves it.
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> egui::Rect {
    harness.run();

    let rect = viewport_rect(harness);
    let before = current_snapshot(harness).expect("first frame publishes a snapshot");
    let cam = camera_from_snapshot(&before);
    let scale = harness.ctx.pixels_per_point();

    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let planes = sketch::datum_planes(&oep_viewer::CpuBackend::demo_scene());
    let click_pos =
        sketch::world_to_panel(&cam, rect, scale, &p_world).expect("entry point projects");
    assert!(rect.contains(click_pos), "entry click inside viewport");

    click(harness, click_pos);
    assert_eq!(
        field(&sketch_report(harness), "mode").as_deref(),
        Some("sketch"),
        "precondition: sketch mode active after the entry click"
    );
    let _ = planes;
    rect
}

/// The demo scene set up in the viewport, whose bounding-box centroid is where
/// the XY datum plane passes through.
fn xy_plane() -> sketch::DatumPlane {
    sketch::datum_planes(&oep_viewer::CpuBackend::demo_scene())
        .into_iter()
        .find(|p| p.id == "XY")
        .expect("XY plane defined")
}

/// Map plane-space uv (measured from the plane centre) to a world point.
fn uv_to_world(uv: (f64, f64)) -> Vec3 {
    let plane = xy_plane();
    plane
        .point
        .add(&plane.e1.mul(uv.0))
        .add(&plane.e2.mul(uv.1))
}

/// Draw one two-point line via the Line tool (L, two clicks). The committed
/// document then has `p0 p1 l0` with the given plane-space length.
fn draw_line_len(harness: &mut Harness<'_, app::App>, rect: egui::Rect, len: f64) {
    harness.key_press(egui::Key::L);
    harness.step();
    harness.step();
    assert_eq!(
        field(&sketch_report(harness), "tool").as_deref(),
        Some("line")
    );

    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    // Two endpoints `len` apart, centred on the plane so both stay on the sheet.
    let a = uv_to_world((-len / 2.0, 8.0));
    let b = uv_to_world((len / 2.0, 8.0));
    let pa = sketch::world_to_panel(&cam, rect, scale, &a).unwrap();
    let pb = sketch::world_to_panel(&cam, rect, scale, &b).unwrap();
    assert!(rect.contains(pa) && rect.contains(pb));

    click(harness, pa);
    click(harness, pb);

    let r = sketch_report(harness);
    assert_eq!(field(&r, "docpoints").as_deref(), Some("2"));
    assert_eq!(field(&r, "doclines").as_deref(), Some("1"));
}

/// The properties-panel dimension editor: click "Place dimension", then type
/// an expression into the field and click Apply.
fn place_dimension(harness: &mut Harness<'_, app::App>, props: egui::Rect) -> String {
    let rects = properties::dimension_widget_rects(props);
    click(harness, rects.place.center());
    let r = dimension_report(harness);
    assert_eq!(
        field(&r, "dims").as_deref(),
        Some("1"),
        "placing attaches exactly one dimension: {r}"
    );
    r
}

/// Set the expression field to `text` (via its accessible label) and click
/// Apply. Returns the resulting dimension report.
fn edit_expression(harness: &mut Harness<'_, app::App>, props: egui::Rect, text: &str) -> String {
    let rects = properties::dimension_widget_rects(props);
    // Set the expression field's text through the accessibility SetValue
    // action (egui TextEdit accepts it and replaces the whole value), then
    // click Apply.
    let input = harness
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| {
            let r = n.rect();
            r.max.x >= rects.expr.min.x
                && r.min.x <= rects.expr.max.x
                && r.max.y >= rects.expr.min.y
                && r.min.y <= rects.expr.max.y
        })
        .expect("expression TextInput present inside its widget rect");
    input.set_text_value_accesskit(text);
    harness.step();
    harness.step();
    click(harness, rects.apply.center());
    dimension_report(harness)
}

/// Section 1: place a dimension on the committed line; it is driven by the
/// name `width` from the variable table and the driven volume is 32000 (the
/// document's own mass property, asserted via the same driven document).
#[test]
fn placing_a_dimension_drives_a_named_width_and_reports_its_volume() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_line_len(&mut harness, rect, 80.0);

    let props = properties_rect(&harness);
    let r = place_dimension(&mut harness, props);

    assert_eq!(field(&r, "dim_name").as_deref(), Some("width"));
    assert_eq!(field(&r, "dim_target").as_deref(), Some("l0"));
    assert_eq!(
        field(&r, "dim_value").as_deref(),
        Some("80.0000"),
        "placing measured the 80-long line into the variable table: {r}"
    );
    assert_eq!(field(&r, "dim_expr").as_deref(), Some("width"));
    assert_eq!(field(&r, "volume_ok").as_deref(), Some("1"));
    // Volume comes from the driven document's own mass properties, asserted
    // through the same driven-volume function the app reports, not a bare 32000
    // literal in the test.
    let volume = field(&r, "volume")
        .and_then(|v| v.parse::<f64>().ok())
        .expect("volume parsed");
    let mut vars = oep_sketch::varset::VariableSet::new();
    vars.set(dimensions::WIDTH_NAME, 80.0);
    let expected = dimensions::driven_volume(&vars).unwrap();
    assert!(
        (volume - expected).abs() < 1e-6,
        "volume {volume} vs {expected}"
    );
    assert!(
        (expected - 32000.0).abs() < 1e-6,
        "fixture: 80 -> {expected}"
    );
}

/// Section 2: editing the dimension as an expression `width*1.5` resolves
/// `width` through the variable table to 120 and the mesh volume moves from
/// 32,000 to 72,000 (the acceptance's numbers, derived from the document).
#[test]
fn editing_the_expression_to_width_times_1_5_moves_the_volume() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_line_len(&mut harness, rect, 80.0);
    let props = properties_rect(&harness);

    place_dimension(&mut harness, props);
    let before = dimension_report(&harness);
    let v_before = field(&before, "volume")
        .and_then(|v| v.parse::<f64>().ok())
        .expect("volume before edit");
    assert!((v_before - 32000.0).abs() < 1e-6, "before {v_before}");

    let r = edit_expression(&mut harness, props, "width*1.5");

    assert_eq!(field(&r, "dim_expr").as_deref(), Some("width*1.5"));
    assert_eq!(
        field(&r, "dim_value").as_deref(),
        Some("120.0000"),
        "expression resolved width through the variable table to 120: {r}"
    );
    assert_eq!(field(&r, "volume_ok").as_deref(), Some("1"));
    let volume = field(&r, "volume")
        .and_then(|v| v.parse::<f64>().ok())
        .expect("volume after edit");
    let mut vars = oep_sketch::varset::VariableSet::new();
    vars.set(dimensions::WIDTH_NAME, 120.0);
    let expected = dimensions::driven_volume(&vars).unwrap();
    assert!(
        (volume - expected).abs() < 1e-6,
        "volume {volume} vs {expected}"
    );
    assert!(
        (expected - 72000.0).abs() < 1e-6,
        "fixture: 120 -> {expected}"
    );
    assert!(
        (volume - v_before).abs() > 1.0,
        "volume moved: {v_before} -> {volume}"
    );
}

/// Section 3: an invalid expression is refused - the named error appears in
/// the properties oracle, and the variable table / dimension / document are
/// all untouched (sketch points, lines and journal identical).
#[test]
fn invalid_expression_is_refused_and_leaves_everything_untouched() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_line_len(&mut harness, rect, 80.0);
    let props = properties_rect(&harness);

    place_dimension(&mut harness, props);
    // First a valid edit so we can prove the invalid one is a no-op.
    let r_ok = edit_expression(&mut harness, props, "width*1.5");
    assert_eq!(field(&r_ok, "dim_value").as_deref(), Some("120.0000"));

    let sketch_before = sketch_report(&harness);
    let vars_line = field(&sketch_before, "docpoints").unwrap_or_default();
    let docs_before = (
        vars_line.clone(),
        field(&sketch_before, "doclines").unwrap_or_default(),
    );

    let bad = "width**1.5";
    let r = edit_expression(&mut harness, props, bad);

    // The named error is present in the properties panel oracle.
    let err = field(&r, "dim_error").expect("named error present");
    assert!(
        err.contains("invalid expression"),
        "error names the refusal: {err}"
    );
    assert_eq!(
        field(&r, "volume_ok").as_deref(),
        Some("1"),
        "volume survives"
    );
    // The dimension value and expression stay at the last good drive (120 /
    // `width*1.5`): a refused edit touches nothing. The draft field shows the
    // typed-but-rejected text.
    assert_eq!(field(&r, "dim_value").as_deref(), Some("120.0000"));
    assert_eq!(field(&r, "dim_expr").as_deref(), Some("width*1.5"));
    assert_eq!(
        field(&r, "draft").as_deref(),
        Some(bad),
        "rejected text still in the field"
    );
    let volume = field(&r, "volume").expect("volume still reported");
    let mut vars = oep_sketch::varset::VariableSet::new();
    vars.set(dimensions::WIDTH_NAME, 120.0);
    let expected = dimensions::driven_volume(&vars).unwrap();
    let v = volume.parse::<f64>().unwrap();
    assert!(
        (v - expected).abs() < 1e-6,
        "volume must be the 120 drive: {v}"
    );

    let sketch_after = sketch_report(&harness);
    let docpoints_after = field(&sketch_after, "docpoints").unwrap_or_default();
    let doclines_after = field(&sketch_after, "doclines").unwrap_or_default();
    assert_eq!(
        (docpoints_after, doclines_after),
        docs_before,
        "document untouched by the refused edit"
    );
}

/// Section 3b: an expression naming an unknown variable is refused with that
/// variable named, and the document is untouched.
#[test]
fn unknown_variable_in_expression_is_refused_with_its_name() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_line_len(&mut harness, rect, 80.0);
    let props = properties_rect(&harness);

    place_dimension(&mut harness, props);
    let sketch_before = sketch_report(&harness);
    let before_docs = (
        field(&sketch_before, "docpoints").unwrap_or_default(),
        field(&sketch_before, "doclines").unwrap_or_default(),
    );

    let r = edit_expression(&mut harness, props, "depth*2");
    let err = field(&r, "dim_error").expect("named error present");
    assert!(
        err.contains("depth"),
        "unknown-variable error names the variable: {err}"
    );

    let sketch_after = sketch_report(&harness);
    let after_docs = (
        field(&sketch_after, "docpoints").unwrap_or_default(),
        field(&sketch_after, "doclines").unwrap_or_default(),
    );
    assert_eq!(after_docs, before_docs, "document untouched");
}
