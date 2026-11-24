//! UI-C5 UI-MEASURE: distance, angle, radius (wave C).
//!
//! The acceptance as the card writes it - "pick two entities and see the
//! measurement annotated on screen; values match the analytic goldens for the
//! bracket fixture and the annotation appears in the snapshot":
//!
//! 1. **Engine goldens** - the measurement MATH lives in the egui-free engine
//!    section of `panels/measure.rs` (the card's "geometry layer"), and the
//!    pure functions here assert the committed bracket fixture's analytic
//!    values within [`measure::MEASURE_TOLERANCE`]:
//!    - distance `p0 (0,0)` -> `p1 (40,0)` = 40,
//!    - distance `p0` -> `p2 (40,30)` = 50 (the 3-4-5 diagonal),
//!    - angle between `l0` (horizontal) and `l1` (vertical) = 90 degrees,
//!    - radius of `c0` = 10,
//!    - and an unsupported pair (point + line) is refused with a named error.
//! 2. **The real app, driven through kittest** - `M` opens the measure
//!    surface; picking two entities (the card's "pick two entities") publishes
//!    the measured kind + value in the oracle, and the value label (the card's
//!    "annotation") is rendered in the AccessKit tree - it appears **in the
//!    snapshot** the harness holds, exactly as the annotation text is painted
//!    on screen. Clear resets the picks, and the same three goldens are
//!    asserted end-to-end.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact measure panel source the app compiles (via viewport.rs), pulled
// in verbatim so the acceptance shares the app's own engine + keys + oracle.
#[path = "../src/panels/measure.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod measure;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use egui::PointerButton;

/// The measure panel's `String` report oracle key (must match `measure.rs`).
const REPORT_KEY: &str = "ui_c5_measure_report";
/// The measure surface toggle key (must match `measure.rs`).
const TOGGLE_KEY: egui::Key = egui::Key::M;

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// The measure oracle, read back through the same context.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of the oracle, e.g. `("kind", "distance")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// A label node that contains `needle` anywhere in the AccessKit tree (the
/// annotation value label, e.g. `distance point p0 -> point p1 = 40.000 mm`).
fn any_label_contains(harness: &Harness<'_, app::App>, needle: &str) -> bool {
    harness.query_all_by_label_contains(needle).next().is_some()
}

fn click_pos(harness: &mut Harness<'_, app::App>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step();
}

/// Click the accessible node whose label is exactly `label` (the entity pick
/// buttons are real widgets with unique labels).
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness.query_by_label(label).expect("node present");
    let pos = node.rect().center();
    click_pos(harness, pos);
}

/// Press the measure toggle key and let the app settle.
fn open_measure(harness: &mut Harness<'_, app::App>) {
    harness.key_press(TOGGLE_KEY);
    harness.step();
    harness.step();
}

// ---------------------------------------------------------------------------
// Section 1: the geometry layer - distance/angle/radius against analytic
// goldens for the committed bracket fixture, within the stated tolerance.
// ---------------------------------------------------------------------------

/// Distance `p0 -> p1` is exactly the 40-unit bracket width.
#[test]
fn engine_distance_matches_the_analytic_golden() {
    let (kind, value) = measure::measure(
        &measure::EntityRef::Point("p0".into()),
        &measure::EntityRef::Point("p1".into()),
    )
    .expect("p0-p1 is a supported measure");
    assert_eq!(kind, measure::MeasureKind::Distance);
    assert!(
        (value - 40.0).abs() < measure::MEASURE_TOLERANCE,
        "bracket width golden: got {value}"
    );
}

/// Distance `p0 -> p2` is the 3-4-5 diagonal (40x30 -> 50), also analytic.
#[test]
fn engine_distance_matches_the_diagonal_golden() {
    let (kind, value) = measure::measure(
        &measure::EntityRef::Point("p0".into()),
        &measure::EntityRef::Point("p2".into()),
    )
    .expect("p0-p2 supported");
    assert_eq!(kind, measure::MeasureKind::Distance);
    assert!(
        (value - 50.0).abs() < measure::MEASURE_TOLERANCE,
        "3-4-5 diagonal golden: got {value}"
    );
}

/// Angle between the perpendicular l0 / l1 is exactly 90 degrees.
#[test]
fn engine_angle_matches_the_analytic_golden() {
    let (kind, value) = measure::measure(
        &measure::EntityRef::Line("l0".into()),
        &measure::EntityRef::Line("l1".into()),
    )
    .expect("l0-l1 supported");
    assert_eq!(kind, measure::MeasureKind::Angle);
    assert!(
        (value - 90.0).abs() < measure::MEASURE_TOLERANCE,
        "perpendicular angle golden: got {value}"
    );
}

/// A non-perpendicular pair still measures exactly (atan2-based, analytic):
/// l1 (vertical) vs l2 (horizontal) is also 90; l0 vs a 45 band is not a
/// fixture pair, so we pin the two analytic right corners the fixture asserts.
#[test]
fn engine_angle_is_consistent_across_both_right_corners() {
    let (_, v_a) = measure::measure(
        &measure::EntityRef::Line("l0".into()),
        &measure::EntityRef::Line("l1".into()),
    )
    .unwrap();
    let (_, v_b) = measure::measure(
        &measure::EntityRef::Line("l1".into()),
        &measure::EntityRef::Line("l2".into()),
    )
    .unwrap();
    assert!((v_a - 90.0).abs() < measure::MEASURE_TOLERANCE, "v_a {v_a}");
    assert!((v_b - 90.0).abs() < measure::MEASURE_TOLERANCE, "v_b {v_b}");
}

/// Radius of the fixture circle is exactly 10.
#[test]
fn engine_radius_matches_the_analytic_golden() {
    let (kind, value) = measure::measure(
        &measure::EntityRef::Circle("c0".into()),
        &measure::EntityRef::Point("p0".into()),
    )
    .expect("a circle pick is supported");
    assert_eq!(kind, measure::MeasureKind::Radius);
    assert!(
        (value - 10.0).abs() < measure::MEASURE_TOLERANCE,
        "circle radius golden: got {value}"
    );
}

/// An unsupported pair (point + line) is a named refusal, never a fabricated
/// number.
#[test]
fn engine_refuses_an_unsupported_pair_honestly() {
    let err = measure::measure(
        &measure::EntityRef::Point("p0".into()),
        &measure::EntityRef::Line("l0".into()),
    )
    .expect_err("point+line is not a supported measurement");
    assert!(
        err.to_string().contains("two points")
            && err.to_string().contains("two lines")
            && err.to_string().contains("circle"),
        "the refusal names the supported pairs: {err}"
    );
}

/// The fixture's own entity rows (points + lines + circles) are the same
/// surface the panel picks from.
#[test]
fn fixture_exposes_points_lines_and_circles_in_document_order() {
    let rows = measure::entity_rows();
    assert_eq!(rows.len(), 9);
    assert_eq!(rows[0], measure::EntityRef::Point("p0".into()));
    assert_eq!(rows[4], measure::EntityRef::Line("l0".into()));
    assert_eq!(rows[8], measure::EntityRef::Circle("c0".into()));
}

// ---------------------------------------------------------------------------
// Section 2: the real app - pick two entities, the annotation appears in the
// snapshot and the values match the analytic goldens end-to-end.
// ---------------------------------------------------------------------------

/// `M` opens the measure surface; picking p0 then p1 measures distance 40 and
/// the annotation (the value label) is rendered in the AccessKit tree.
#[test]
fn picking_two_points_measures_distance_and_annotates_on_screen() {
    let mut harness = real_app_harness();
    harness.run();

    assert_eq!(field(&report(&harness), "open").as_deref(), Some("0"));

    open_measure(&mut harness);
    let r = report(&harness);
    assert_eq!(field(&r, "open").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "first").as_deref(), Some("-"));
    assert_eq!(field(&r, "second").as_deref(), Some("-"));

    // The card's "pick two entities": the two point rows.
    click_label(&mut harness, "point p0");
    assert_eq!(field(&report(&harness), "first").as_deref(), Some("p0"));
    assert_eq!(field(&report(&harness), "second").as_deref(), Some("-"));

    click_label(&mut harness, "point p1");
    let r = report(&harness);
    assert_eq!(field(&r, "first").as_deref(), Some("p0"));
    assert_eq!(field(&r, "second").as_deref(), Some("p1"));
    assert_eq!(field(&r, "kind").as_deref(), Some("distance"));
    assert_eq!(field(&r, "unit").as_deref(), Some("mm"));
    let value: f64 = field(&r, "value")
        .and_then(|v| v.parse().ok())
        .expect("value");
    assert!(
        (value - 40.0).abs() < measure::MEASURE_TOLERANCE,
        "distance must match the analytic golden end-to-end: {value}"
    );
    assert_eq!(field(&r, "error").as_deref(), Some("-"));

    // The annotation appears in the snapshot: the rendered value label is
    // present in the AccessKit tree, exactly as it is painted on screen.
    assert!(
        any_label_contains(&harness, "distance point p0 -> point p1 = 40.0"),
        "the annotation label must appear in the tree"
    );
}

/// Picking two perpendicular lines measures the 90-degree angle.
#[test]
fn picking_two_lines_measures_angle_and_annotates_on_screen() {
    let mut harness = real_app_harness();
    harness.run();
    open_measure(&mut harness);

    click_label(&mut harness, "line l0");
    click_label(&mut harness, "line l1");

    let r = report(&harness);
    assert_eq!(field(&r, "kind").as_deref(), Some("angle"));
    assert_eq!(field(&r, "unit").as_deref(), Some("deg"));
    let value: f64 = field(&r, "value")
        .and_then(|v| v.parse().ok())
        .expect("value");
    assert!(
        (value - 90.0).abs() < measure::MEASURE_TOLERANCE,
        "value {value}"
    );
    assert!(any_label_contains(
        &harness,
        "angle line l0 -> line l1 = 90.0"
    ));
}

/// Picking a circle measures its radius.
#[test]
fn picking_a_circle_measures_radius_and_annotates_on_screen() {
    let mut harness = real_app_harness();
    harness.run();
    open_measure(&mut harness);

    click_label(&mut harness, "circle c0");
    click_label(&mut harness, "point p0"); // second pick, radius of c0

    let r = report(&harness);
    assert_eq!(field(&r, "kind").as_deref(), Some("radius"));
    assert_eq!(field(&r, "unit").as_deref(), Some("mm"));
    let value: f64 = field(&r, "value")
        .and_then(|v| v.parse().ok())
        .expect("value");
    assert!(
        (value - 10.0).abs() < measure::MEASURE_TOLERANCE,
        "value {value}"
    );
    assert!(any_label_contains(
        &harness,
        "radius circle c0 -> point p0 = 10.0"
    ));
}

/// An unsupported pair through the real surface is refused honestly (the
/// annotation says so, never a fabricated number).
#[test]
fn real_surface_refuses_point_plus_line_honestly() {
    let mut harness = real_app_harness();
    harness.run();
    open_measure(&mut harness);

    click_label(&mut harness, "point p0");
    click_label(&mut harness, "line l0");

    let r = report(&harness);
    assert_eq!(field(&r, "kind").as_deref(), Some("-"));
    assert_eq!(field(&r, "value").as_deref(), Some("-"));
    let err = field(&r, "error").unwrap_or_default();
    assert!(
        err.contains("two points") && err.contains("two lines") && err.contains("circle"),
        "honest named refusal expected, got: {err}"
    );
}

/// `Clear` resets the picks so the next measurement starts fresh.
#[test]
fn clear_resets_the_picks() {
    let mut harness = real_app_harness();
    harness.run();
    open_measure(&mut harness);

    click_label(&mut harness, "point p0");
    click_label(&mut harness, "point p1");
    assert_eq!(field(&report(&harness), "second").as_deref(), Some("p1"));

    click_label(&mut harness, "Clear");
    let r = report(&harness);
    assert_eq!(field(&r, "first").as_deref(), Some("-"));
    assert_eq!(field(&r, "second").as_deref(), Some("-"));
    assert_eq!(field(&r, "kind").as_deref(), Some("-"));

    // And it measures again: p0 -> p2 is the 50 diagonal.
    click_label(&mut harness, "point p0");
    click_label(&mut harness, "point p2");
    let r = report(&harness);
    let value: f64 = field(&r, "value")
        .and_then(|v| v.parse().ok())
        .expect("value");
    assert!(
        (value - 50.0).abs() < measure::MEASURE_TOLERANCE,
        "value {value}"
    );
}

/// Closing the surface with `M` hides it (the snapshot returns to the plain
/// viewport) and drops the picks.
#[test]
fn toggling_closed_hides_the_surface_and_drops_picks() {
    let mut harness = real_app_harness();
    harness.run();

    open_measure(&mut harness);
    click_label(&mut harness, "point p0");
    assert_eq!(field(&report(&harness), "first").as_deref(), Some("p0"));

    open_measure(&mut harness); // toggle closed
    let r = report(&harness);
    assert_eq!(field(&r, "open").as_deref(), Some("0"));
    assert_eq!(field(&r, "first").as_deref(), Some("-"));
}
