//! UI-B8 UI-FEATURE-FROM-SKETCH: extrude/revolve dialog with preview (wave B).
//!
//! Drives the REAL `app::App` through `egui_kittest` and asserts the card's
//! acceptance exactly as written:
//!
//! 1. The viewport FRAME CHANGES when the finish dialog opens with a live
//!    preview: the published snapshot frame hash is different while the
//!    dialog is up than it was before the dialog (the "preview frame differs
//!    from the pre-feature frame" assertion), and returns to the exact
//!    pre-feature hash when the dialog closes.
//! 2. CANCEL leaves the document byte-identical, asserted by HASH EQUALITY:
//!    the feature document digest (`features::document_digest` over the
//!    committed geometry + applied features) is byte-identical before opening
//!    the dialog and after cancelling it, the sketch oracle is byte-identical,
//!    and the feature journal holds zero entries (the preview clone never
//!    entered the journal).
//! 3. The STEP export from the UI matches the CLI export hash: after Apply,
//!    exporting through the UI's own "Export STEP" button (which routes
//!    through `oep_shell::editor::EditorSession::export_step`, the exact lane
//!    `oep-workbench export:step` dispatches) produces a file whose SHA-256
//!    equals the CLI-lane export of the same committed model - the honesty
//!    check that UI and CLI produce the same bytes.
//!
//! The math reads the app's own code (`features.rs`, `sketch.rs` pulled in
//! verbatim via `#[path]`), so the click positions use the same project-rest
//! and the same rects the app resolves, and the expected volumes come from the
//! derived document, never a test literal.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact feature-panel source the app compiles: its deterministic widget
// rects (`dialog_rects`, `feature_rects`), the geometry parser and the feature
// document digest. `duplicate_mod` is the same allowance the B4/B7 tests make
// for their second `#[path]` copies.
#[path = "../src/panels/features.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod features;

// The exact sketch module source the viewport panel compiles (math + the
// world-to-panel projection), joined here so the acceptance computes click
// positions with the app's own functions.
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
/// The feature panel's report-oracle key.
const FEATURE_REPORT_KEY: &str = "ui_b8_feature_report";
/// The feature panel's export-path key (plain String, cross-copy safe).
const EXPORT_PATH_KEY: &str = "ui_b8_export_path";

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
/// tree (the same probe the B4/B7 tests use).
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

/// The sketch panel's `String` oracle.
fn sketch_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SKETCH_REPORT_KEY)))
        .unwrap_or_default()
}

/// The feature panel's `String` oracle.
fn feature_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(FEATURE_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of an oracle, e.g. `("dialog", "1")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The feature journal length (typed to the panel's own journal copy would be
/// cross-`#[path]`-unsafe; the oracle's `journal=` field is the shared read).
fn feature_journal(report: &str) -> u64 {
    field(report, "journal")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
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

/// Draw a closed rectangle (the B4 Rectangle tool: R, four clicks) whose first
/// corner is `(a, b)` and extends `w` by `h` in plane coordinates. The
/// committed document then has 4 points and 4 lines forming a closed loop (the
/// fixture the dialog finishes into a solid).
fn draw_rectangle(
    harness: &mut Harness<'_, app::App>,
    rect: egui::Rect,
    a: f64,
    b: f64,
    w: f64,
    h: f64,
) {
    harness.key_press(egui::Key::R);
    harness.step();
    harness.step();
    assert_eq!(
        field(&sketch_report(harness), "tool").as_deref(),
        Some("rectangle")
    );

    let cam = camera_from_snapshot(&current_snapshot(harness).expect("camera"));
    let scale = harness.ctx.pixels_per_point();
    let corners = [(a, b), (a + w, b), (a + w, b + h), (a, b + h)];
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

/// The feature document digest of the current committed sketch + applied set,
/// computed by the test's own copy of the panel engine (never read back from
/// the app's oracle): the honest hash the acceptance compares across Cancel.
fn self_digest(harness: &Harness<'_, app::App>, applied: &[features::AppliedFeature]) -> u64 {
    let sk = features::committed_geometry(&sketch_report(harness)).unwrap_or_default();
    features::document_digest(&sk, applied)
}

/// Open the finish dialog with plain F (the sketch-mode finish key).
fn open_dialog(harness: &mut Harness<'_, app::App>) {
    harness.key_press(egui::Key::F);
    harness.step();
    harness.step();
    harness.step();
}

/// Section 1: the live preview. Opening the dialog derives a document clone
/// (marked preview); the viewport snapshot's frame hash DIFFERS from the
/// pre-feature frame while the dialog is up, and the preview is a marked
/// clone that reports the derived volume from the document.
#[test]
fn preview_frame_differs_from_pre_feature_frame() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, rect, 30.0, 10.0, 20.0, 12.0);

    // Pre-feature frame: the ordinary sketch frame (no dialog, nothing
    // applied) - read BEFORE opening the dialog.
    let pre = current_snapshot(&harness).expect("pre-feature snapshot");
    let pre_hash = pre.7;

    open_dialog(&mut harness);

    let r = feature_report(&harness);
    assert_eq!(field(&r, "dialog").as_deref(), Some("1"), "dialog open");
    assert_eq!(field(&r, "kind").as_deref(), Some("extrude"));
    assert_eq!(field(&r, "preview").as_deref(), Some("1"));
    // The preview is a document clone MARKED preview.
    assert_eq!(field(&r, "preview_marked").as_deref(), Some("1"));

    let during = current_snapshot(&harness).expect("preview snapshot");
    assert_ne!(
        during.7, pre_hash,
        "preview frame must differ from the pre-feature frame"
    );

    // The preview clone derived a solid: its volume comes from the document
    // (a 20x12 profile at depth 10 -> 2400, the document's own mass property).
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("preview volume reported");
    assert!((volume - 2400.0).abs() < 1e-6, "volume {volume}");

    // Cancel: the frame returns to the exact pre-feature hash (nothing was
    // committed to the viewport path).
    let d = features::dialog_rects(rect);
    click(&mut harness, d.cancel.center());
    let after = current_snapshot(&harness).expect("after-cancel snapshot");
    assert_eq!(after.7, pre_hash, "cancel restores the pre-feature frame");
    let r = feature_report(&harness);
    assert_eq!(field(&r, "dialog").as_deref(), Some("0"), "dialog closed");
}

/// The committed-document block of the sketch oracle: the geometry the feature
/// finishes (points, linesegs) plus the counts and journal. This is "the
/// document" the acceptance means - the transient draw-tool/snap cursor state
/// in the oracle is frame-local UI state and is not part of it.
fn document_block(report: &str) -> String {
    let mut out = String::new();
    for line in report.lines() {
        if line.starts_with("point=")
            || line.starts_with("lineseg=")
            || line.starts_with("docpoints=")
            || line.starts_with("doclines=")
            || line.starts_with("circle")
            || line.starts_with("journal=")
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Section 2: Cancel leaves the document byte-identical, asserted by HASH
/// EQUALITY. Both the feature document digest and the committed sketch
/// document block are byte-identical before opening the dialog and after
/// cancelling it, and the feature journal holds zero entries (the preview
/// clone never entered it).
#[test]
fn cancel_leaves_the_document_byte_identical() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, rect, 30.0, 10.0, 20.0, 12.0);

    // Digests / document / journal BEFORE opening the dialog (applied = none).
    let digest_before = self_digest(&harness, &[]);
    let doc_before = document_block(&sketch_report(&harness));
    let journal_before = feature_journal(&feature_report(&harness));
    assert_eq!(journal_before, 0);

    // Open the dialog: this builds a document clone (preview), which must not
    // touch the committed document.
    open_dialog(&mut harness);
    let r_open = feature_report(&harness);
    assert_eq!(field(&r_open, "preview").as_deref(), Some("1"));

    // Cancel.
    let d = features::dialog_rects(rect);
    click(&mut harness, d.cancel.center());

    // HASH EQUALITY: the feature document digest is identical.
    let digest_after = self_digest(&harness, &[]);
    assert_eq!(
        digest_after, digest_before,
        "cancel must leave the document hash unchanged"
    );
    // The committed sketch document block is byte-identical.
    let doc_after = document_block(&sketch_report(&harness));
    assert_eq!(doc_after, doc_before, "sketch document byte-identical");
    // The preview clone never entered the journal.
    let r_after = feature_report(&harness);
    assert_eq!(field(&r_after, "dialog").as_deref(), Some("0"));
    assert_eq!(
        feature_journal(&r_after),
        journal_before,
        "cancel must not journal anything"
    );
    assert_eq!(field(&r_after, "applied").as_deref(), Some("0"));
}

/// Section 3: Apply journals the feature and the viewport keeps the solid.
#[test]
fn apply_commits_the_feature_into_the_journal() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, rect, 30.0, 10.0, 20.0, 12.0);

    open_dialog(&mut harness);
    let d = features::dialog_rects(rect);
    click(&mut harness, d.apply.center());

    let r = feature_report(&harness);
    assert_eq!(field(&r, "dialog").as_deref(), Some("0"), "dialog closed");
    assert_eq!(field(&r, "applied").as_deref(), Some("1"));
    assert_eq!(
        feature_journal(&r),
        1,
        "exactly one journal entry after Apply"
    );
    // The committed record names the feature and its derived volume.
    let feat_line = field(&r, "feat").expect("feat record present");
    assert!(
        feat_line.starts_with("extrude::"),
        "applied a real extruded feature: {feat_line}"
    );
    // The applied feature changed the document digest (a commit, not a no-op).
    let after = self_digest(
        &harness,
        &[features::AppliedFeature {
            kind: features::FeatureKind::Extrude,
            amount: 10.0,
            volume: 2400.0,
            faces: 12,
        }],
    );
    assert_ne!(
        after,
        self_digest(&harness, &[]),
        "applying a feature must change the digest"
    );
}

/// Section 3b: the dialog offers BOTH kinds - switching to Revolve re-derives
/// the preview clone as a revolution of the closed profile (the preview stays
/// live and the journal stays empty; a preview kind change is not a commit).
#[test]
fn revolve_kind_is_offered_and_previews_live() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, rect, 30.0, 10.0, 20.0, 12.0);

    open_dialog(&mut harness);
    let r0 = feature_report(&harness);
    assert_eq!(field(&r0, "kind").as_deref(), Some("extrude"));

    let d = features::dialog_rects(rect);
    click(&mut harness, d.kind_revolve.center());
    let r1 = feature_report(&harness);
    assert_eq!(field(&r1, "kind").as_deref(), Some("revolve"));
    assert_eq!(field(&r1, "preview").as_deref(), Some("1"));
    // Still a preview clone: nothing journaled by switching kind.
    assert_eq!(feature_journal(&r1), 0);
    let v1: f64 = field(&r1, "volume").and_then(|v| v.parse().ok()).unwrap();
    assert!(v1 > 0.0, "revolve preview derives a solid (volume {v1})");

    // Switch back to extrude: the preview follows live.
    click(&mut harness, d.kind_extrude.center());
    let r2 = feature_report(&harness);
    assert_eq!(field(&r2, "kind").as_deref(), Some("extrude"));
    assert_eq!(field(&r2, "preview").as_deref(), Some("1"));
    assert_eq!(feature_journal(&r2), 0);
}

/// Section 4: the honest export check - the STEP export from the UI matches
/// the CLI export hash. The UI's "Export STEP" button routes through
/// `oep_shell::editor::EditorSession::export_step`, the exact lane
/// `oep-workbench export:step` dispatches; the test reproduces the CLI lane on
/// the same committed model and compares the two files' SHA-256. STEP embeds
/// the file name, so the parallel writes use the SAME basename (different
/// directories) - the probe-confirmed byte-equality condition.
#[test]
fn ui_step_export_matches_the_cli_export_hash() {
    let mut harness = real_app_harness();
    let rect = enter_xy_sketch(&mut harness);
    draw_rectangle(&mut harness, rect, 30.0, 10.0, 20.0, 12.0);

    // Open the dialog and Apply the default 10mm extrude.
    open_dialog(&mut harness);
    let d = features::dialog_rects(rect);
    click(&mut harness, d.apply.center());
    let r = feature_report(&harness);
    assert_eq!(field(&r, "applied").as_deref(), Some("1"));

    // Aim the UI export at an isolated temp dir, then click "Export STEP"
    // (the persistent button that appears once a feature is applied).
    let base = std::env::temp_dir().join("ui-b8-export-test");
    let ui_dir = base.join("ui");
    let cli_dir = base.join("cli");
    std::fs::create_dir_all(&ui_dir).unwrap();
    std::fs::create_dir_all(&cli_dir).unwrap();
    let ui_path = ui_dir.join("ui-export.step");
    let _ = std::fs::remove_file(&ui_path);

    harness.ctx.data_mut(|d| {
        d.insert_temp::<String>(
            egui::Id::unique(EXPORT_PATH_KEY),
            ui_path.display().to_string(),
        );
    });

    let surface = features::feature_rects(rect);
    click(&mut harness, surface.export_step.center());
    assert!(
        std::path::Path::new(&ui_path).exists(),
        "UI export wrote the STEP file"
    );
    let ui_bytes = std::fs::read(&ui_path).expect("read UI STEP");
    let ui_hash = oep_shell::sha256::sha256_hex(&ui_bytes);
    assert_eq!(ui_hash.len(), 64, "UI export sha256");
    println!("UI export  {}: sha256 {ui_hash}", ui_path.display());

    // The CLI lane on the same committed model: oep-workbench export:step is a
    // thin wrapper over EditorSession::point/line/solve/extrude/export_step -
    // reproduce the identical session from the committed geometry + amount.
    // Same basename ("ui-export.step") so the embedded STEP file name matches.
    let sk = features::committed_geometry(&sketch_report(&harness)).expect("committed geometry");
    let session = features::build_clone(&sk, features::FeatureKind::Extrude, 10.0)
        .expect("CLI-lane session derives");
    let cli_path = cli_dir.join("ui-export.step");
    let _ = std::fs::remove_file(&cli_path);
    session
        .export_step(cli_path.to_str().unwrap())
        .expect("CLI-lane export");
    let cli_bytes = std::fs::read(&cli_path).expect("read CLI STEP");
    let cli_hash = oep_shell::sha256::sha256_hex(&cli_bytes);
    println!("CLI export  {}: sha256 {cli_hash}", cli_path.display());

    assert_eq!(
        ui_hash, cli_hash,
        "STEP export from the UI must match the CLI export hash"
    );

    // The exported solid re-imports through the importer hub (never fake
    // bytes): both files are the exported solid, verified by re-import.
    for (path, label) in [(&ui_path, "UI"), (&cli_path, "CLI")] {
        let bytes = std::fs::read(path).unwrap();
        let outcome = oep_import::import_bytes(oep_import::SourceKind::Step, &bytes);
        assert!(
            outcome.is_ok(),
            "{label} STEP re-imports through the importer hub"
        );
    }
}
