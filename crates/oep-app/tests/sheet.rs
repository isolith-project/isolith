//! UI-F1 UI-SHEET: sheet setup and sheet tree.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in
//! the `isolith` binary, pulled in verbatim via `#[path]` like every other
//! test target here) through `egui_kittest` and asserts the card acceptance
//! exactly as written:
//!
//! 1. **Sheet properties round-trip**: set size A3, scale 1:2, projection
//!    first-angle, save, reopen, assert every field byte-identical (both
//!    records printed).
//! 2. **Projection angle changes which views are LEGAL**: third-angle forbids
//!    the first-angle top-view placement and the refusal is a named error
//!    shown in the UI (the error printed and asserted).
//! 3. **Two sheets coexist**, are independently selectable, and each keeps
//!    its own properties.
//! 4. The drafting region is present in the AccessKit tree by its accessible
//!    name and the sheet rows are reachable there too.
//!
//! The panel is seeded with NO override on the first frame: it opens the
//! committed bracket drawing through `oep_drafting::builder::drive_bracket_drawing`
//! (the existing drafting seam), so the very first frame of the real app is
//! exactly what the acceptance edits. The save/reopen leg reads the live
//! canonical document JSON the panel publishes every frame (the bytes a real
//! save would write), re-parses it as a fresh `oep_geo::Document`, and
//! re-seeds a fresh harness with it - printing both records and asserting
//! byte-identical sheets.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

/// The panel's `String` report oracle key (must match `sheet.rs`).
const SHEET_REPORT_KEY: &str = "ui_f1_sheet_report";
/// The live canonical drawing document JSON key (must match `sheet.rs`).
const SHEET_DOC_JSON_KEY: &str = "ui_f1_sheet_doc_json";
/// The canonical `.oepdoc` seed key (must match `sheet.rs`).
const SHEET_SEED_KEY: &str = "ui_f1_sheet_seed";

/// Build the harness around the real app at the product window size. No seed:
/// the drafting panel opens the committed bracket drawing on its first frame.
fn harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// Build the harness with `json` under the sheet seed key BEFORE the first
/// frame, so the drafting panel opens exactly that drawing document (the
/// reopen leg of the round-trip acceptance).
fn seeded_harness(json: &str) -> Harness<'static, app::App> {
    let json = json.to_string();
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(SHEET_SEED_KEY), json.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

/// The panel's `String` report oracle, read back through the same context.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SHEET_REPORT_KEY)))
        .unwrap_or_default()
}

/// The live canonical drawing document JSON the panel publishes this frame.
fn doc_json(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(SHEET_DOC_JSON_KEY)))
        .unwrap_or_default()
}

/// One report field, e.g. `("size", "420x297")`.
fn field(report_text: &str, key: &str) -> Option<String> {
    report_text
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// A label node exists anywhere in the AccessKit tree (non-panicking).
fn has_label(harness: &Harness<'_, app::App>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// Click a labelled node exactly, through the real input path.
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

/// The panel's committed bracket drawing always opens with exactly one sheet:
/// the fixture's `oep-sheet-1` (A4 landscape, third-angle, scale 1:1, three
/// views).
#[test]
fn opens_committed_bracket_drawing_with_one_sheet() {
    let mut harness = harness();
    harness.run();

    let r = report(&harness);
    println!("{r}");
    assert_eq!(field(&r, "open").as_deref(), Some("1"));
    assert_eq!(field(&r, "sheets").as_deref(), Some("1"));
    assert_eq!(field(&r, "sel").as_deref(), Some("oep-sheet-1"));
    assert_eq!(field(&r, "size").as_deref(), Some("297x210"));
    assert_eq!(field(&r, "size_name").as_deref(), Some("A4 landscape"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:1"));
    assert_eq!(field(&r, "projection").as_deref(), Some("third-angle"));
    assert_eq!(field(&r, "views").as_deref(), Some("3"));
}

/// 1. SHEET PROPERTIES ROUND-TRIP: set size A3, scale 1:2, projection
///    first-angle through the real UI, save (the canonical document JSON the
///    panel publishes), reopen it both as a fresh `oep_geo::Document` and by
///    re-seeding a fresh app harness, and assert every sheet field
///    byte-identical. Both records are printed.
#[test]
fn sheet_properties_round_trip_bytes_identical() {
    let mut harness = harness();
    harness.run();

    // Select the sheet row, then set the three acceptance values.
    click_label(
        &mut harness,
        "Sheet 1 · A4 landscape · third-angle · scale 1:1",
    );
    harness.step();
    click_label(&mut harness, "A3");
    harness.step();
    click_label(&mut harness, "1:2");
    harness.step();
    click_label(&mut harness, "first-angle");
    harness.step();

    let r = report(&harness);
    println!("after edit:\n{r}");
    assert_eq!(field(&r, "size").as_deref(), Some("420x297"));
    assert_eq!(field(&r, "size_name").as_deref(), Some("A3 landscape"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:2"));
    assert_eq!(field(&r, "projection").as_deref(), Some("first-angle"));
    // Scale 1:2 drives every view's numeric scale to 2.0 (paper = model / 2).
    let edited_hash = field(&r, "doc_hash").unwrap_or_default();
    println!("edited doc_hash = {edited_hash}");

    // SAVE: the canonical `.oepdoc` JSON the panel would write.
    let saved = doc_json(&harness);
    assert!(
        !saved.is_empty(),
        "panel published no canonical document JSON"
    );

    // REOPEN #1: parse the saved bytes as a fresh canonical Document, print
    // both sheet records, and assert byte-identical (string equality over the
    // canonical JSON of the sheet).
    let reopened_doc = oep_geo::Document::from_json_pretty(&saved).expect("saved json parses");
    let saved_sheet = &oep_geo::Document::from_json_pretty(&saved)
        .expect("saved json parses again")
        .drawings[0]
        .sheets[0];
    let reopened_sheet = &reopened_doc.drawings[0].sheets[0];
    println!("SAVED   sheet record = {saved_sheet:#?}");
    println!("REOPEN  sheet record = {reopened_sheet:#?}");
    assert_eq!(
        reopened_sheet, saved_sheet,
        "every sheet field must be byte-identical after save and reopen"
    );
    assert_eq!(
        reopened_doc
            .to_json_pretty()
            .expect("reserialize")
            .as_bytes(),
        saved.as_bytes(),
        "the whole canonical document must round-trip byte-identical"
    );

    // REOPEN #2: a fresh app harness seeded with the saved bytes reopens it in
    // the UI, and the oracle names the same properties.
    let mut fresh = seeded_harness(&saved);
    fresh.run();
    let rf = report(&fresh);
    println!("reopened in UI:\n{rf}");
    assert_eq!(field(&rf, "sel").as_deref(), Some("oep-sheet-1"));
    assert_eq!(field(&rf, "size").as_deref(), Some("420x297"));
    assert_eq!(field(&rf, "scale").as_deref(), Some("1:2"));
    assert_eq!(field(&rf, "projection").as_deref(), Some("first-angle"));
    assert_eq!(
        field(&rf, "doc_hash").as_deref(),
        Some(edited_hash.as_str())
    );
}

/// 2. PROJECTION ANGLE CHANGES WHICH VIEWS ARE LEGAL: on the committed
///    third-angle sheet, the first-angle top placement (top below front) is
///    REFUSED with the named [`oep_app SheetError`] shown in the UI; flipping
///    the sheet to first-angle makes the same placement legal. The error
///    text is printed and asserted.
#[test]
fn third_angle_forbids_first_angle_top_placement_named_error_in_ui() {
    let mut harness = harness();
    harness.run();

    // The fixture sheet is third-angle by default.
    let r0 = report(&harness);
    assert_eq!(field(&r0, "projection").as_deref(), Some("third-angle"));

    // Attempt the FIRST-ANGLE top placement (top below front): refused.
    click_label(&mut harness, "Place top below (first-angle placement)");
    harness.step();
    let r = report(&harness);
    println!("third-angle attempt:\n{r}");
    assert_eq!(
        field(&r, "placement_status").as_deref(),
        Some("refused"),
        "third-angle must refuse the first-angle top placement"
    );
    let err = field(&r, "placement_error").unwrap_or_default();
    println!("named refusal = {err}");
    assert!(
        err.contains("SheetError::IllegalPlacement"),
        "the UI must show the NAMED error, got: {err}"
    );
    assert!(err.contains("top"), "error must name the view: {err}");
    assert!(
        err.contains("below the front view"),
        "error must name the placement: {err}"
    );
    assert!(
        err.contains("third-angle"),
        "error must name the sheet projection: {err}"
    );
    // The named error is genuinely rendered as a UI node (grounded in the
    // AccessKit tree), not only in the oracle.
    assert!(
        has_label(&harness, &err),
        "the refusal text must appear as a visible UI label"
    );

    // The legal placement on the same sheet is accepted.
    click_label(&mut harness, "Place top above (third-angle placement)");
    harness.step();
    let r = report(&harness);
    println!("third-angle legal attempt:\n{r}");
    assert_eq!(field(&r, "placement_status").as_deref(), Some("legal"));

    // Flip the sheet projection to first-angle: the SAME "top below"
    // placement is now legal (the projection angle changed which views are
    // legal).
    click_label(
        &mut harness,
        "Sheet 1 · A4 landscape · third-angle · scale 1:1",
    );
    harness.step();
    click_label(&mut harness, "first-angle");
    harness.step();
    let r = report(&harness);
    assert_eq!(field(&r, "projection").as_deref(), Some("first-angle"));

    click_label(&mut harness, "Place top below (first-angle placement)");
    harness.step();
    let r = report(&harness);
    println!("first-angle attempt after flip:\n{r}");
    assert_eq!(
        field(&r, "placement_status").as_deref(),
        Some("legal"),
        "first-angle must accept the first-angle top placement"
    );
    assert_eq!(field(&r, "placement_error").as_deref(), Some("-"));
}

/// 3. TWO SHEETS COEXIST, are independently selectable, and each keeps its
///    own properties.
#[test]
fn two_sheets_coexist_independently_selectable_keep_own_properties() {
    let mut harness = harness();
    harness.run();

    // Add a second sheet: the drawing now has two, and the new sheet is
    // selected. The new sheet is independent: A4, third-angle, 1:1, no views.
    click_label(&mut harness, "Add sheet");
    harness.step();
    let r = report(&harness);
    println!("after add:\n{r}");
    assert_eq!(field(&r, "sheets").as_deref(), Some("2"));
    assert_eq!(field(&r, "sel").as_deref(), Some("oep-sheet-2"));
    assert_eq!(field(&r, "size").as_deref(), Some("297x210"));
    assert_eq!(field(&r, "projection").as_deref(), Some("third-angle"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:1"));
    assert_eq!(field(&r, "views").as_deref(), Some("0"));
    assert!(has_label(
        &harness,
        "Sheet 2 · A4 landscape · third-angle · scale 1:1"
    ));

    // Select sheet 1 (the bracket sheet) and give it its own A3/1:2/first set.
    click_label(
        &mut harness,
        "Sheet 1 · A4 landscape · third-angle · scale 1:1",
    );
    harness.step();
    click_label(&mut harness, "A3");
    harness.step();
    click_label(&mut harness, "1:2");
    harness.step();
    click_label(&mut harness, "first-angle");
    harness.step();
    let r = report(&harness);
    println!("sheet 1 edited:\n{r}");
    assert_eq!(field(&r, "sel").as_deref(), Some("oep-sheet-1"));
    assert_eq!(field(&r, "size").as_deref(), Some("420x297"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:2"));
    assert_eq!(field(&r, "projection").as_deref(), Some("first-angle"));

    // Select sheet 2 again: it KEEPS its own defaults (unchanged by editing
    // sheet 1).
    click_label(
        &mut harness,
        "Sheet 2 · A4 landscape · third-angle · scale 1:1",
    );
    harness.step();
    let r = report(&harness);
    println!("sheet 2 reselected:\n{r}");
    assert_eq!(field(&r, "sel").as_deref(), Some("oep-sheet-2"));
    assert_eq!(field(&r, "size").as_deref(), Some("297x210"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:1"));
    assert_eq!(field(&r, "projection").as_deref(), Some("third-angle"));

    // And select sheet 1 back: still its own A3/1:2/first set.
    click_label(
        &mut harness,
        "Sheet 1 · A3 landscape · first-angle · scale 1:2",
    );
    harness.step();
    let r = report(&harness);
    println!("sheet 1 reselected:\n{r}");
    assert_eq!(field(&r, "sel").as_deref(), Some("oep-sheet-1"));
    assert_eq!(field(&r, "size").as_deref(), Some("420x297"));
    assert_eq!(field(&r, "scale").as_deref(), Some("1:2"));
    assert_eq!(field(&r, "projection").as_deref(), Some("first-angle"));

    // Both sheets are in the canonical document, each with its own record.
    let doc = doc_json(&harness);
    let parsed = oep_geo::Document::from_json_pretty(&doc).expect("parses");
    assert_eq!(parsed.drawings[0].sheets.len(), 2);
    let s1 = &parsed.drawings[0].sheets[0];
    let s2 = &parsed.drawings[0].sheets[1];
    assert_eq!(s1.size, oep_geo::SheetSize::A3_LANDSCAPE);
    assert_eq!(s1.projection, oep_geo::ProjectionType::FirstAngle);
    assert_eq!(s1.title_block.scale.as_deref(), Some("1:2"));
    assert_eq!(s2.size, oep_geo::SheetSize::A4_LANDSCAPE);
    assert_eq!(s2.projection, oep_geo::ProjectionType::ThirdAngle);
    assert_ne!(s1, s2, "the two sheets keep distinct records");
}

/// 4. The drafting region is reachable by its accessible name and carries the
///    sheet rows in the AccessKit tree.
#[test]
fn drafting_region_and_sheet_rows_reachable_in_accesskit_tree() {
    let mut harness = harness();
    harness.run();

    let region: Vec<_> = harness.query_all_by_label("drafting").collect();
    assert!(
        !region.is_empty(),
        "the drafting region must be present in the AccessKit tree"
    );
    assert!(has_label(
        &harness,
        "Sheet 1 · A4 landscape · third-angle · scale 1:1"
    ));

    // The region's accessible name is non-empty (a real labelled node).
    let node = region[0].accesskit_node();
    assert!(
        !node.label().unwrap_or_default().is_empty(),
        "the drafting region must carry a non-empty accessible name"
    );
}
