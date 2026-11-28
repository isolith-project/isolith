//! UI-F2 UI-VIEWS: view placement tools - base, projected, section and detail.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts the card acceptance exactly
//! as written, each clause with printed numbers from the real panel:
//!
//! 1. Place a base view, then a projected view: their axes stay aligned
//!    (assert the alignment coordinate equal within 1e-6, print both).
//! 2. Dragging the base view moves the projected view with it by exactly the
//!    same delta (real pointer drag through the harness, printed).
//! 3. A section view of the committed bracket shows hatching: the hatch line
//!    count is non-zero and equal to the analytic count for the cut plane
//!    (print both).
//! 4. A detail view shows the magnified region at exactly the requested scale
//!    (assert the rendered extents against the analytic extents, print both).
//!
//! The panel is seeded with NO override on the first frame: it opens the
//! committed bracket drawing through `oep_drafting::builder::drive_bracket_drawing`
//! (the existing drafting seam), so the geometry every number below traces to
//! is the engine's own committed bracket - never a second fixture.
//!
//! The harness uses a TALLER window (1280x1800) than the F1 sheet tests, the
//! same convention the E-wave tests use: the drafting region is the lower 48%
//! of the properties column, and the taller acceptance window brings the whole
//! scrollable drafting content (sheet controls + the views tools) into the
//! visible area so every widget is clicked directly.

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

/// The views panel's `String` report oracle key (must match `views.rs`).
const VIEWS_REPORT_KEY: &str = "ui_f2_views_report";

/// Build the harness around the real app at the taller acceptance window (see
/// module doc). No seed: the drafting panel opens the committed bracket
/// drawing on its first frame.
fn harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([1280.0, 1800.0])
        .build_eframe(|_cc| app::App::new(false))
}

/// The views panel's `String` report oracle, read back through the same
/// context store the panel writes to.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(VIEWS_REPORT_KEY)))
        .unwrap_or_default()
}

/// One report field, e.g. `("base_cx", "148.500000000")`.
fn field(report_text: &str, key: &str) -> Option<String> {
    report_text
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// Click a labelled node (button or chip) exactly, through the real input path.
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness
        .get_all_by_label(label)
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("label {label:?} not found in tree"));
    node.click();
    harness.step();
}

/// Pointer-drag a labelled drag handle by `(dx, dy)` logical points, through
/// egui's real pointer path (hover, press, moves, release) - the same
/// technique the E2 mate-tool acceptance uses to drag a component.
fn drag_label(harness: &mut Harness<'_, app::App>, label: &str, dx: f32, dy: f32) {
    let node = harness
        .get_all_by_label(label)
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("drag handle {label:?} not found"));
    let start = node.rect().center();
    harness.hover_at(start);
    harness.step();
    harness.drag_at(start);
    harness.step();
    let steps = 4;
    for i in 1..=steps {
        let f = i as f32 / steps as f32;
        harness.hover_at(egui::Pos2::new(start.x + dx * f, start.y + dy * f));
        harness.step();
    }
    harness.drop_at(egui::Pos2::new(start.x + dx, start.y + dy));
    harness.step();
}

/// 1 + 2. BASE THEN PROJECTED: axes stay aligned; dragging the base moves the
/// projected view with it by exactly the same delta. Both numbers are printed.
#[test]
fn base_and_projected_stay_aligned_and_follow_the_base_drag_exactly() {
    let mut harness = harness();
    harness.run();

    // Place the base Front view.
    click_label(&mut harness, "Place base front");
    harness.step();
    // Place the projected Top view (aligned to the base per the sheet's
    // third-angle convention through the layout seam).
    click_label(&mut harness, "Place projected top");
    harness.step();

    let r = report(&harness);
    println!("after place base + projected top:\n{r}");
    assert_eq!(field(&r, "has_base").as_deref(), Some("1"));
    assert_eq!(field(&r, "has_proj").as_deref(), Some("1"));

    // Clause 1: the alignment coordinate (cx of the TOP/Front vertical stack)
    // must be equal within 1e-6 - both printed.
    let base_cx: f64 = field(&r, "base_cx").unwrap().parse().unwrap();
    let proj_cx: f64 = field(&r, "proj_cx").unwrap().parse().unwrap();
    println!("ALIGN: base_cx = {base_cx}  proj_cx = {proj_cx}");
    assert!(
        (proj_cx - base_cx).abs() < 1e-6,
        "axes must stay aligned: |{proj_cx} - {base_cx}| < 1e-6"
    );

    // Clause 2: a real pointer drag on the base chip. After the drag:
    // the projected view moved by exactly the same delta (both printed).
    let before = report(&harness);
    let base_dx_before: f64 = field(&before, "drag_base_dx").unwrap().parse().unwrap();
    let base_dy_before: f64 = field(&before, "drag_base_dy").unwrap().parse().unwrap();
    let proj_dx_before: f64 = field(&before, "drag_proj_dx").unwrap().parse().unwrap();
    let proj_dy_before: f64 = field(&before, "drag_proj_dy").unwrap().parse().unwrap();
    println!(
        "pre-drag deltas: base=({base_dx_before:.9}, {base_dy_before:.9}) proj=({proj_dx_before:.9}, {proj_dy_before:.9})"
    );

    // Drag the base chip: 80 px right + 30 px down (logical points). MM_PER_PT
    // is 0.9; the panel maps pointer points to paper mm with that exact factor.
    drag_label(&mut harness, "drag base oep-view-base-1", 80.0, 30.0);

    let r = report(&harness);
    println!("after base drag:\n{r}");
    let base_dx: f64 = field(&r, "drag_base_dx").unwrap().parse().unwrap();
    let base_dy: f64 = field(&r, "drag_base_dy").unwrap().parse().unwrap();
    let proj_dx: f64 = field(&r, "drag_proj_dx").unwrap().parse().unwrap();
    let proj_dy: f64 = field(&r, "drag_proj_dy").unwrap().parse().unwrap();
    println!(
        "DRAG DELTA: base moved ({base_dx:.9}, {base_dy:.9}) mm, projected moved ({proj_dx:.9}, {proj_dy:.9}) mm"
    );
    assert!(
        (base_dx - proj_dx).abs() < 1e-6 && (base_dy - proj_dy).abs() < 1e-6,
        "projected must move with the base by exactly the same delta"
    );
    // The base really moved (the drag mapped to a non-zero paper delta).
    assert!(
        base_dx.abs() > 1e-9,
        "the drag must have moved the base (delta=({base_dx}, {base_dy}))"
    );

    // Alignment is preserved after the drag (both cx still equal).
    let r = report(&harness);
    let base_cx_after: f64 = field(&r, "base_cx").unwrap().parse().unwrap();
    let proj_cx_after: f64 = field(&r, "proj_cx").unwrap().parse().unwrap();
    assert!(
        (proj_cx_after - base_cx_after).abs() < 1e-6,
        "axes must stay aligned after the drag"
    );
}

/// 3. A section view of the committed bracket shows hatching: the hatch line
///    count is non-zero and EQUAL to the analytic count for the cut plane
///    (both printed).
#[test]
fn section_view_shows_hatching_equal_to_the_analytic_count() {
    let mut harness = harness();
    harness.run();

    click_label(&mut harness, "Place base front");
    harness.step();
    click_label(&mut harness, "Place section");
    harness.step();

    let r = report(&harness);
    println!("after placing section view:\n{r}");
    let rendered: usize = field(&r, "section_hatch").unwrap().parse().unwrap();
    let analytic: usize = field(&r, "section_hatch_analytic")
        .unwrap()
        .parse()
        .unwrap();
    println!("SECTION HATCH: rendered = {rendered}  analytic = {analytic}");
    assert!(rendered > 0, "hatch count must be non-zero");
    assert_eq!(
        rendered, analytic,
        "section hatch count must equal the analytic count for the cut plane"
    );

    // The section region is the committed bracket's width x depth at the cut.
    let region = field(&r, "section_region").unwrap();
    println!("section region (mm) = {region}");
    assert_eq!(region, "40x6");
}

/// 4. A detail view shows the magnified region at exactly the requested scale:
///    the rendered extents equal the analytic extents (both printed).
#[test]
fn detail_view_magnifies_region_at_exactly_the_requested_scale() {
    let mut harness = harness();
    harness.run();

    click_label(&mut harness, "Place base front");
    harness.step();
    click_label(&mut harness, "Place detail");
    harness.step();

    let r = report(&harness);
    println!("after placing detail view:\n{r}");
    let scale: f64 = field(&r, "detail_scale").unwrap().parse().unwrap();
    let render_w: f64 = field(&r, "detail_render_w").unwrap().parse().unwrap();
    let render_h: f64 = field(&r, "detail_render_h").unwrap().parse().unwrap();
    let analytic_w: f64 = field(&r, "detail_analytic_w").unwrap().parse().unwrap();
    let analytic_h: f64 = field(&r, "detail_analytic_h").unwrap().parse().unwrap();
    println!(
        "DETAIL: requested scale={scale} rendered=({render_w}, {render_h}) analytic=({analytic_w}, {analytic_h})"
    );
    assert!(
        (render_w - analytic_w).abs() < 1e-9 && (render_h - analytic_h).abs() < 1e-9,
        "rendered extents must equal the analytic extents at exactly the requested scale"
    );
    // Magnified: the rendered extents exceed the source model extents
    // (front 80x6 magnified at scale 0.5 -> 160x12).
    assert!(
        render_w > 80.0,
        "detail must magnify the region: {render_w} > 80"
    );
    assert_eq!((render_w, render_h), (160.0, 12.0));
}

/// The views tools live in the drafting region's scroll area and their report
/// oracle is populated every frame (the placement surface is reachable).
#[test]
fn views_panel_publishes_report_with_placement_surface_reachable() {
    let mut harness = harness();
    harness.run();

    let r = report(&harness);
    println!("fresh views report:\n{r}");
    assert_eq!(field(&r, "placed").as_deref(), Some("0"));
    assert!(
        field(&r, "has_base").is_some(),
        "report has_base field present"
    );

    // The placement tools are reachable as real UI buttons.
    for label in [
        "Place base front",
        "Place projected top",
        "Place projected right",
        "Place section",
        "Place detail",
    ] {
        let node = harness
            .get_all_by_label(label)
            .collect::<Vec<_>>()
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("tool {label:?} not found"));
        println!("tool {label:?} reachable at rect {}", node.rect());
    }
}
