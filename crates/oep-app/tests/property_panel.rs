//! UI-C2 UI-PROPERTY-PANEL: per-feature parameter editor (wave C).
//!
//! Drives the REAL `app::App` (the same source shipped in the `isolith`
//! binary, pulled in verbatim via `#[path]` as every oep-app test does)
//! through `egui_kittest` and asserts the card's acceptance exactly as
//! written:
//!
//! 1. The properties panel edits the SELECTED feature's parameters (depth,
//!    angle, direction, symmetric) with a LIVE PREVIEW: selecting the seeded
//!    Extrude feature shows the committed depth, and editing the depth field
//!    re-derives the solid through the UI-B8 derivation lane (`features.rs`
//!    copied in as `feature_preview`) so the volume moves predictably (a
//!    fixed profile at depth d has volume area*d, so doubling d doubles the
//!    volume) while the document hash and journal stay untouched (preview is
//!    not a commit).
//! 2. An INVALID value is refused: it leaves the document untouched (hash and
//!    journal byte-identical) and NAMES the field (e.g. `depth`) and reason.
//! 3. Apply and Cancel DIFFER: Apply commits exactly one journal entry and
//!    moves `doc_hash`; Cancel restores the committed set with the journal and
//!    hash unchanged, so the two are distinguishable by their effect.
//!
//! All assertions read the app's real state: the C2 report oracle
//! ([`properties::PARAM_REPORT_KEY`] = `ui_c2_property_report`), the shared
//! selection oracle (`isolith.ui.tree.selection`), the AccessKit tree (rows,
//! text inputs by rect) and the sketch report (which carries the committed
//! profile the preview derives from).

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact properties-panel source the app compiles, included here so the
// acceptance computes the widget rects, the oracle keys, the validation rule
// and the expected volume from the app's own code (the same `#[path]` copy
// pattern the B4/B7/B8 tests use). `duplicate_mod` is the same allowance.
#[path = "../src/panels/properties.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod properties;

// The exact sketch module source the app compiles (datum planes, world-to-panel
// projection) so the test enters sketch and draws a closed profile the same
// way B4/B8 do.
#[path = "../src/panels/sketch.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sketch;

// The B8 feature-from-sketch engine copy, used to compute the EXPECTED preview
// volume from the committed geometry + depth (exactly what the panel reads) so
// the test never hardcodes the profile area.
#[path = "../src/panels/features.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod feature_preview;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

use egui::PointerButton;

use oep_viewer::Vec3;

/// The viewport panel's context-data key (must match `viewport.rs`).
const VIEWPORT_STATE_KEY: &str = "isolith.ui.viewport.backend";
/// The sketch panel's report-oracle key (must match `sketch.rs`).
const SKETCH_REPORT_KEY: &str = "ui_b3_sketch_report";
/// The tree panel's seed key (must match `tree.rs`).
const TREE_SEED_KEY: &str = "ui_c1_tree_seed";
/// The C2 parameter-editor report key (must match `properties.rs`).
const PARAM_REPORT_KEY: &str = "ui_c2_property_report";

type Snapshot = (f64, f64, f64, f64, f64, f64, f64, u64);

/// Build the harness around the real `app::App` at the product window size,
/// seeding the live document the tree (and the property panel) will show with
/// `json` (a canonical `.oepdoc`) BEFORE the first frame.
fn seeded_harness(json: &str) -> Harness<'static, app::App> {
    let json = json.to_string();
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(TREE_SEED_KEY), json.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

/// The canonical `.oepdoc` JSON of a document holding one part owning one body
/// and one `Extrude` feature named `pad` with committed solid parameters.
fn seed_json() -> String {
    let mut doc = oep_import::Document::new("mm");
    doc.push_part(
        oep_import::Part::new("p1", "part-1")
            .with_body("body-1")
            .with_feature("feat-1"),
    );
    doc.push_body(oep_import::Body::new(
        "body-1",
        "solid-1",
        oep_import::BodyKind::Solid,
    ));
    doc.push_feature(
        oep_import::Feature::new("feat-1", "pad", "Extrude")
            .with_param("depth", 10.0)
            .with_param("angle", std::f64::consts::FRAC_PI_2)
            .with_param("direction", "+Z")
            .with_param("symmetric", false),
    );
    doc.to_json_pretty().expect("canonical seed JSON")
}

/// One field of the C2 report, e.g. `("volume", "2400.0000")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The C2 parameter-editor report, read back through the same context.
fn param_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(PARAM_REPORT_KEY)))
        .unwrap_or_default()
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

/// The viewport region's full rect, read from the app's own accessibility
/// tree (the probe B4/B7/B8 use).
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

/// The properties region's full rect (the probe B7 uses).
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

/// The sketch report, read back through the same context.
fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// The viewport scalar snapshot (camera + frame hash).
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

/// Enter sketch mode on the XY datum plane (same entry path B8 uses).
fn enter_xy_sketch(harness: &mut Harness<'_, app::App>) -> egui::Rect {
    harness.run();
    let rect = viewport_rect(harness);
    let cam = camera_from_snapshot(&current_snapshot(harness).expect("first frame snapshot"));
    let scale = harness.ctx.pixels_per_point();
    let p_world = Vec3::new(52.0, 22.0, 5.0);
    let click_pos =
        sketch::world_to_panel(&cam, rect, scale, &p_world).expect("entry point projects");
    assert!(rect.contains(click_pos), "entry click inside viewport");
    click(harness, click_pos);
    assert_eq!(
        field(&sketch_report(harness), "mode").as_deref(),
        Some("sketch"),
        "precondition: sketch mode active after the entry click"
    );
    rect
}

/// The XY datum plane of the demo scene (where the sketch grid sits).
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

/// Draw a closed rectangle (the B4 Rectangle tool: R, four clicks) of given
/// plane-space width/height. The committed sketch then has 4 points / 4 lines
/// forming a closed loop - the profile the preview derives.
fn draw_rectangle(harness: &mut Harness<'_, app::App>, rect: egui::Rect, w: f64, h: f64) {
    harness.key_press(egui::Key::R);
    harness.step();
    harness.step();
    assert_eq!(
        field(&sketch_report(harness), "tool").as_deref(),
        Some("rectangle")
    );
    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let corners = [
        (30.0, 10.0),
        (30.0 + w, 10.0),
        (30.0 + w, 10.0 + h),
        (30.0, 10.0 + h),
    ];
    for (u, v) in corners {
        let world = uv_to_world((u, v));
        let pos = sketch::world_to_panel(&cam, rect, scale, &world)
            .expect("corner projects in front of the camera");
        assert!(rect.contains(pos), "corner ({u},{v}) projected inside");
        click(harness, pos);
    }
    let r = sketch_report(harness);
    assert_eq!(field(&r, "docpoints").as_deref(), Some("4"));
    assert_eq!(field(&r, "doclines").as_deref(), Some("4"));
}

/// Click the tree row with exactly this label (the C1 click helper).
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness
        .query_by_label(label)
        .expect("row label present in the AccessKit tree");
    let pos = node.rect().center();
    click(harness, pos);
}

/// Set the depth (or angle) parameter field's text through the accessibility
/// SetValue action, found by rect overlap with the C2 widget rect the app
/// uses. Returns the resulting C2 report.
fn set_param(harness: &mut Harness<'_, app::App>, target: egui::Rect, text: &str) -> String {
    let input = harness
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| {
            let r = n.rect();
            r.max.x >= target.min.x
                && r.min.x <= target.max.x
                && r.max.y >= target.min.y
                && r.min.y <= target.max.y
        })
        .expect("C2 parameter TextInput present inside its widget rect");
    input.set_text_value_accesskit(text);
    harness.step();
    harness.step();
    harness.run();
    param_report(harness)
}

/// The expected derived-solid volume the panel's preview lane returns for the
/// committed sketch profile at `depth` (area * depth - the B8 build_clone
/// lane over the SAME geometry the panel reads). Computed from the app's own
/// code, never a test literal.
fn expected_volume(harness: &Harness<'_, app::App>, depth: f64) -> f64 {
    let sk = feature_preview::committed_geometry(&sketch_report(harness)).expect("closed profile");
    let session = feature_preview::build_clone(&sk, feature_preview::FeatureKind::Extrude, depth)
        .expect("preview clone derives");
    session.props().volume.expect("derived solid has a volume")
}

/// Section 1 + 2: selecting the editable feature loads its parameters, and the
/// LIVE PREVIEW follows an edited depth predictably (volume = area * depth)
/// without touching the document or journal.
#[test]
fn selecting_the_feature_edits_depth_with_a_predictable_live_preview() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();

    // A committed closed profile for the preview to derive from.
    let viewport = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, viewport, 20.0, 12.0); // area 240

    // Select the seeded Extrude feature row in the tree.
    click_label(&mut harness, "pad (Extrude)");

    let r = param_report(&harness);
    assert_eq!(field(&r, "sel").as_deref(), Some("feat-1"), "{r}");
    assert_eq!(field(&r, "kind").as_deref(), Some("Extrude"), "{r}");
    assert_eq!(field(&r, "editable").as_deref(), Some("1"), "{r}");
    assert_eq!(
        field(&r, "depth").as_deref(),
        Some("10"),
        "committed depth loaded"
    );
    assert_eq!(field(&r, "direction").as_deref(), Some("+Z"));
    assert_eq!(field(&r, "error").as_deref(), Some("-"), "{r}");

    // The preview reflects the committed depth: area 240 * 10 = 2400.
    assert_eq!(field(&r, "preview").as_deref(), Some("1"), "{r}");
    let v0: f64 = field(&r, "volume").and_then(|v| v.parse().ok()).unwrap();
    let expected0 = expected_volume(&harness, 10.0);
    assert!(
        (v0 - expected0).abs() < 1e-6,
        "preview volume {v0} vs derived {expected0}"
    );
    let hash0 = field(&r, "doc_hash").unwrap();
    let journal0 = field(&r, "journal").unwrap();

    // Edit depth to 20 -> volume doubles predictably (area 240 * 20 = 4800).
    let props = properties_rect(&harness);
    let rects = properties::param_widget_rects(props);
    let r = set_param(&mut harness, rects.depth, "20");
    let v1: f64 = field(&r, "volume").and_then(|v| v.parse().ok()).unwrap();
    let expected1 = expected_volume(&harness, 20.0);
    assert!(
        (v1 - expected1).abs() < 1e-6,
        "depth 20 preview volume {v1} vs derived {expected1}"
    );
    assert!(
        (v1 - 2.0 * v0).abs() < 1e-6,
        "depth doubled -> volume doubled predictably: {v0} -> {v1}"
    );
    assert_eq!(
        field(&r, "doc_hash").as_deref(),
        Some(hash0.as_str()),
        "preview must not touch the document"
    );
    assert_eq!(
        field(&r, "journal").as_deref(),
        Some(journal0.as_str()),
        "preview must not journal"
    );
}

/// Section 2: an invalid depth is refused. The document and journal are
/// untouched and the refusal NAMES the field and the reason.
#[test]
fn invalid_depth_is_refused_naming_field_and_reason() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();
    let viewport = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, viewport, 20.0, 12.0);
    click_label(&mut harness, "pad (Extrude)");

    let r0 = param_report(&harness);
    let hash0 = field(&r0, "doc_hash").unwrap();
    let journal0 = field(&r0, "journal").unwrap();

    let props = properties_rect(&harness);
    let rects = properties::param_widget_rects(props);
    let r = set_param(&mut harness, rects.depth, "-5");

    assert_eq!(
        field(&r, "error_field").as_deref(),
        Some("depth"),
        "the refusal names the field: {r}"
    );
    let reason = field(&r, "error").unwrap_or_default();
    assert!(
        reason.contains("positive"),
        "the refusal names the reason: {reason}"
    );
    assert_eq!(
        field(&r, "doc_hash").as_deref(),
        Some(hash0.as_str()),
        "invalid value leaves the document untouched"
    );
    assert_eq!(
        field(&r, "journal").as_deref(),
        Some(journal0.as_str()),
        "invalid value journals nothing"
    );

    // A non-numeric depth is refused the same way (field + reason named).
    let r = set_param(&mut harness, rects.depth, "abc");
    assert_eq!(
        field(&r, "error_field").as_deref(),
        Some("depth"),
        "non-numeric depth refuses with the field named: {r}"
    );
    assert!(
        field(&r, "error").unwrap_or_default().contains("number"),
        "non-numeric depth names the reason"
    );
    assert_eq!(
        field(&r, "doc_hash").as_deref(),
        Some(hash0.as_str()),
        "non-numeric depth leaves the document untouched"
    );
}

/// Section 2b: direction accepts only the axis tokens; anything else is a
/// named refusal that leaves the document untouched.
#[test]
fn invalid_direction_is_refused_naming_the_field() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();
    let viewport = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, viewport, 20.0, 12.0);
    click_label(&mut harness, "pad (Extrude)");

    let r0 = param_report(&harness);
    let hash0 = field(&r0, "doc_hash").unwrap();
    let journal0 = field(&r0, "journal").unwrap();

    let props = properties_rect(&harness);
    let rects = properties::param_widget_rects(props);
    let r = set_param(&mut harness, rects.direction, "sideways");

    assert_eq!(
        field(&r, "error_field").as_deref(),
        Some("direction"),
        "refusal names the direction field: {r}"
    );
    assert_eq!(
        field(&r, "doc_hash").as_deref(),
        Some(hash0.as_str()),
        "invalid direction leaves the document untouched"
    );
    assert_eq!(
        field(&r, "journal").as_deref(),
        Some(journal0.as_str()),
        "invalid direction journals nothing"
    );
}

/// Section 3: Apply commits EXACTLY ONE journal entry and moves `doc_hash`;
/// Cancel restores the committed set and leaves both unchanged. The two
/// behaviours are therefore distinguishable by their effect.
#[test]
fn apply_commits_one_journal_entry_and_cancel_discards() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();
    let viewport = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, viewport, 20.0, 12.0);
    click_label(&mut harness, "pad (Extrude)");

    let props = properties_rect(&harness);
    let rects = properties::param_widget_rects(props);

    // Baseline: committed depth 10, no edits.
    let r0 = param_report(&harness);
    let hash0 = field(&r0, "doc_hash").unwrap();
    assert_eq!(field(&r0, "journal").as_deref(), Some("0"), "{r0}");
    assert_eq!(field(&r0, "app_depth").as_deref(), Some("10.0000"), "{r0}");

    // Edit depth to 20 and Apply -> exactly one journal entry, hash moves, the
    // committed set updates to depth 20.
    set_param(&mut harness, rects.depth, "20");
    click(&mut harness, rects.apply.center());
    let r_applied = param_report(&harness);
    assert_eq!(
        field(&r_applied, "journal").as_deref(),
        Some("1"),
        "Apply commits exactly one journal entry: {r_applied}"
    );
    let hash1 = field(&r_applied, "doc_hash").unwrap();
    assert_ne!(hash1, hash0, "Apply changed the document");
    assert_eq!(
        field(&r_applied, "app_depth").as_deref(),
        Some("20.0000"),
        "committed depth updated: {r_applied}"
    );
    assert_eq!(field(&r_applied, "depth").as_deref(), Some("20"));

    // The journaled edit is the one and only entry (still exactly 1 after a
    // further frame - Apply must not double-commit).
    harness.run();
    let r_idle = param_report(&harness);
    assert_eq!(
        field(&r_idle, "journal").as_deref(),
        Some("1"),
        "Apply commits exactly one entry, not more: {r_idle}"
    );

    // Edit depth to 30, then Cancel -> drafts restore to the committed 20, the
    // journal stays at 1 and the document hash is unchanged (Cancel discards).
    set_param(&mut harness, rects.depth, "30");
    click(&mut harness, rects.cancel.center());
    let r_cancel = param_report(&harness);
    assert_eq!(
        field(&r_cancel, "journal").as_deref(),
        Some("1"),
        "Cancel must not add a journal entry: {r_cancel}"
    );
    assert_eq!(
        field(&r_cancel, "doc_hash").as_deref(),
        Some(hash1.as_str()),
        "Cancel must not change the document"
    );
    assert_eq!(
        field(&r_cancel, "depth").as_deref(),
        Some("20"),
        "Cancel restores the committed depth draft: {r_cancel}"
    );
    assert_eq!(
        field(&r_cancel, "app_depth").as_deref(),
        Some("20.0000"),
        "committed set still depth 20: {r_cancel}"
    );
    let v_after_cancel: f64 = field(&r_cancel, "volume")
        .and_then(|v| v.parse().ok())
        .unwrap();
    let expected20 = expected_volume(&harness, 20.0);
    assert!(
        (v_after_cancel - expected20).abs() < 1e-6,
        "after Cancel the preview follows the committed depth 20: {v_after_cancel}"
    );
}

/// Section 4: a body row (not a feature) is honestly NOT editable - no fake
/// parameter fields for a non-feature selection.
#[test]
fn selecting_a_body_shows_no_editable_parameters() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();
    click_label(&mut harness, "solid-1 (Body)");

    let r = param_report(&harness);
    assert_eq!(field(&r, "sel").as_deref(), Some("body-1"), "{r}");
    assert_eq!(field(&r, "editable").as_deref(), Some("0"), "{r}");
    assert_eq!(field(&r, "kind").as_deref(), Some(""), "{r}");
}
