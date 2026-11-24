//! UI-C4 UI-ROLLBACK: suppress and roll the tree (wave C).
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts the card's acceptance
//! exactly as written:
//!
//! 1. Suppressing a feature changes the hash: a suppressed feature carries
//!    `suppressed` document state in its `parameters`, so the canonical
//!    `.oepdoc` JSON changes and the tree's published `hash` (the FNV-1a
//!    document fingerprint, the mesh-hash oracle this wave's panels use)
//!    moves. The row renders a visible suppression mark and the document-tree
//!    report counts it.
//! 2. The status bar reports the suppressed count: the tree publishes the
//!    count of suppressed features each frame and the status bar region shows
//!    "N suppressed".
//! 3. Rolling back hides downstream features: rolling the tree back to a
//!    feature (`roll` document state on that feature) hides every feature
//!    created after it (creation order = `document.features`). The row
//!    list re-derives live each frame, so the hidden rows disappear and the
//!    report carries `rolled=` and `roll=`; unrolling clears the roll point
//!    and the rows come back (same frame, no stale geometry).
//! 4. Round-trip: suppression and roll live in `Feature.parameters`, which
//!    serialize into the canonical `.oepdoc` the panel publishes every frame
//!    ([`tree::TREE_DOC_JSON_KEY`]). Re-seeding that exact JSON into a fresh
//!    harness (save -> reopen) replays the same suppression marks, roll
//!    position and hash.
//!
//! All assertions read the app's real state: the tree report oracle
//! ([`tree::TREE_REPORT_KEY`] = `ui_c1_tree_report`), the AccessKit tree (row
//! labels, buttons, suppression marks) and the status bar region text.

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

use egui::PointerButton;

/// The tree panel's report oracle key (must match `tree.rs`).
const TREE_REPORT_KEY: &str = "ui_c1_tree_report";
/// The tree panel's seed key (must match `tree.rs`).
const TREE_SEED_KEY: &str = "ui_c1_tree_seed";
/// The canonical-doc-JSON oracle key (must match `tree.rs`).
const TREE_DOC_JSON_KEY: &str = "ui_c1_tree_doc_json";

/// Build a canonical `.oepdoc` with ONE part owning one body and three
/// features in creation order (`f-1` -> `f-2` -> `f-3`), so a roll to `f-1`
/// has `f-2` and `f-3` downstream to hide.
fn seed_json() -> String {
    let mut doc = oep_import::Document::new("mm");
    doc.push_part(
        oep_import::Part::new("p1", "part-1")
            .with_body("b1")
            .with_feature("f-1")
            .with_feature("f-2")
            .with_feature("f-3"),
    );
    doc.push_body(oep_import::Body::new(
        "b1",
        "solid-1",
        oep_import::BodyKind::Solid,
    ));
    doc.push_feature(oep_import::Feature::new("f-1", "base", "Extrude"));
    doc.push_feature(oep_import::Feature::new("f-2", "boss", "Extrude"));
    doc.push_feature(oep_import::Feature::new("f-3", "slot", "Cut"));
    doc.to_json_pretty().expect("canonical seed JSON")
}

/// Build the harness around the real `app::App` at the product window size,
/// seeding `json` under the tree seed key BEFORE the first frame.
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

/// One field of the tree report, e.g. `("hash", "123")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The tree panel's `String` report, read back through the same context.
fn tree_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(TREE_REPORT_KEY)))
        .unwrap_or_default()
}

/// The live canonical `.oepdoc` JSON the panel publishes (what a Save would
/// write and a Reopen would read back).
fn live_doc_json(harness: &Harness<'_, app::App>) -> Option<String> {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(TREE_DOC_JSON_KEY)))
}

/// A node with exactly this label exists anywhere in the AccessKit tree.
fn has_label(harness: &Harness<'_, app::App>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// All rendered text in the region named `region` (like the A11 helper).
fn region_values(harness: &Harness<'_, app::App>, region: &str) -> Vec<String> {
    let region = harness
        .get_all_by_label(region)
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .expect("region present");
    region
        .children_recursive()
        .filter_map(|n| {
            let node = n.accesskit_node();
            if let Some(v) = node.value() {
                Some(v)
            } else {
                node.label()
            }
        })
        .collect()
}

/// Click the accessible node with exactly this label (the C1 click helper).
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness
        .query_by_label(label)
        .expect("node present in the AccessKit tree");
    let pos = node.rect().center();
    click_pos(harness, pos);
}

/// Click the node at `pos` over a few frames (hover -> press -> release ->
/// settle), the proven wave pattern.
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
    harness.run();
}

/// Section 1 + 2: suppressing a feature changes the hash, shows the
/// suppression mark on the row, and the status bar reports the suppressed
/// count.
#[test]
fn suppressing_a_feature_changes_hash_shows_the_mark_and_reports_the_count() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();

    // Baseline: three feature rows, nothing suppressed, nothing rolled.
    let r0 = tree_report(&harness);
    assert_eq!(field(&r0, "rows").as_deref(), Some("5"), "{r0}");
    assert_eq!(field(&r0, "suppressed").as_deref(), Some("0"), "{r0}");
    assert_eq!(field(&r0, "rolled").as_deref(), Some("0"), "{r0}");
    assert_eq!(field(&r0, "roll").as_deref(), Some("-"), "{r0}");
    let hash0 = field(&r0, "hash").unwrap();
    for label in ["base (Extrude)", "boss (Extrude)", "slot (Cut)"] {
        assert!(has_label(&harness, label), "row {label} must render");
    }
    assert!(
        region_values(&harness, "status bar")
            .join("\n")
            .contains("0 selected"),
        "status bar baseline"
    );

    // Select the middle feature and suppress it.
    click_label(&mut harness, "boss (Extrude)");
    click_label(&mut harness, "Suppress");

    let r1 = tree_report(&harness);
    assert_eq!(field(&r1, "suppressed").as_deref(), Some("1"), "{r1}");
    assert_ne!(
        field(&r1, "hash").unwrap(),
        hash0,
        "suppressing must change the document hash (mesh hash oracle)"
    );
    // The row re-derives with the honest suppression mark.
    assert!(
        has_label(&harness, "boss (Extrude) (suppressed)"),
        "suppressed row carries its mark"
    );
    assert!(
        !has_label(&harness, "boss (Extrude)"),
        "the unmarked label is replaced by the marked one"
    );
    // The status bar reports the suppressed count.
    let status = region_values(&harness, "status bar").join("\n");
    assert!(status.contains("1 suppressed"), "status bar: {status}");

    // Restoring returns the exact document (hash unchanged from baseline).
    click_label(&mut harness, "boss (Extrude) (suppressed)");
    click_label(&mut harness, "Restore");
    let r2 = tree_report(&harness);
    assert_eq!(field(&r2, "suppressed").as_deref(), Some("0"), "{r2}");
    assert_eq!(
        field(&r2, "hash").unwrap(),
        hash0,
        "restoring returns to the exact pre-suppress document hash"
    );
    assert!(
        has_label(&harness, "boss (Extrude)"),
        "restored row loses the mark"
    );
}

/// Suppress is undoable through the UI-B2 journal: one Undo after a suppress
/// restores the exact pre-suppress hash, and the mark disappears.
#[test]
fn suppress_is_a_journaled_undoable_edit() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();

    click_label(&mut harness, "slot (Cut)");
    let hash0 = field(&tree_report(&harness), "hash").unwrap();
    click_label(&mut harness, "Suppress");
    assert_eq!(
        field(&tree_report(&harness), "suppressed").as_deref(),
        Some("1")
    );
    assert_ne!(field(&tree_report(&harness), "hash").unwrap(), hash0);

    click_label(&mut harness, "Undo");
    let r_undo = tree_report(&harness);
    assert_eq!(
        field(&r_undo, "hash").unwrap(),
        hash0,
        "undo returns to the exact pre-suppress hash"
    );
    assert_eq!(
        field(&r_undo, "suppressed").as_deref(),
        Some("0"),
        "{r_undo}"
    );
    assert!(
        has_label(&harness, "slot (Cut)"),
        "the unmarked row is back after undo"
    );
}

/// Section 3: rolling back to a feature hides its downstream features (the
/// rows re-derive live, no stale geometry), and unrolling brings them back
/// while returning to the exact pre-roll hash.
#[test]
fn rolling_back_hides_downstream_and_unrolling_restores_it() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();

    let r0 = tree_report(&harness);
    let hash0 = field(&r0, "hash").unwrap();
    assert_eq!(field(&r0, "rows").as_deref(), Some("5"), "{r0}");

    // Roll to the FIRST feature: everything created after it - boss, slot -
    // is downstream and must hide.
    click_label(&mut harness, "base (Extrude)");
    click_label(&mut harness, "Roll to here");

    let r1 = tree_report(&harness);
    assert_eq!(field(&r1, "roll").as_deref(), Some("f-1"), "{r1}");
    assert_eq!(field(&r1, "rolled").as_deref(), Some("2"), "{r1}");
    assert_eq!(field(&r1, "rows").as_deref(), Some("3"), "{r1}");
    assert_ne!(
        field(&r1, "hash").unwrap(),
        hash0,
        "rolling back changes the document hash"
    );
    assert!(has_label(&harness, "base (Extrude)"), "roll point stays");
    // Downstream hidden features are gone from the live rows.
    assert!(
        !has_label(&harness, "boss (Extrude)"),
        "downstream boss hidden"
    );
    assert!(!has_label(&harness, "slot (Cut)"), "downstream slot hidden");
    // The status bar reports the rolled-back count.
    let status = region_values(&harness, "status bar").join("\n");
    assert!(status.contains("2 rolled back"), "status bar: {status}");

    // The roll point row stays selected (it is not hidden by its own roll).
    assert_eq!(
        field(&tree_report(&harness), "sel").as_deref(),
        Some("f-1"),
        "the roll point remains the selected row"
    );
    // The roll bar marker text renders in the tree.
    assert!(
        region_values(&harness, "model tree")
            .join("\n")
            .contains("roll bar"),
        "the roll bar marker renders in the tree"
    );

    // Unroll: the roll point clears, downstream rows come back, hash exact.
    click_label(&mut harness, "Unroll");
    let r2 = tree_report(&harness);
    assert_eq!(field(&r2, "roll").as_deref(), Some("-"), "{r2}");
    assert_eq!(field(&r2, "rolled").as_deref(), Some("0"), "{r2}");
    assert_eq!(field(&r2, "rows").as_deref(), Some("5"), "{r2}");
    assert_eq!(
        field(&r2, "hash").unwrap(),
        hash0,
        "unroll returns to the exact pre-roll document hash"
    );
    for label in ["boss (Extrude)", "slot (Cut)"] {
        assert!(has_label(&harness, label), "row {label} restored");
    }
}

/// Rolling to a feature that is NOT the first hides only the later features;
/// a roll point can move (rolling again to a later feature widens what is
/// visible).
#[test]
fn rolling_to_a_later_feature_hides_only_features_after_it() {
    let mut harness = seeded_harness(&seed_json());
    harness.run();

    // Roll to f-2: only f-3 is downstream now.
    click_label(&mut harness, "boss (Extrude)");
    click_label(&mut harness, "Roll to here");
    let r = tree_report(&harness);
    assert_eq!(field(&r, "roll").as_deref(), Some("f-2"), "{r}");
    assert_eq!(field(&r, "rolled").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "rows").as_deref(), Some("4"), "{r}");
    assert!(has_label(&harness, "base (Extrude)"), "upstream stays");
    assert!(has_label(&harness, "boss (Extrude)"), "roll point stays");
    assert!(!has_label(&harness, "slot (Cut)"), "downstream slot hidden");

    // Rolling to the LAST feature hides nothing downstream.
    click_label(&mut harness, "Unroll");
    click_label(&mut harness, "slot (Cut)");
    click_label(&mut harness, "Roll to here");
    let r2 = tree_report(&harness);
    assert_eq!(field(&r2, "roll").as_deref(), Some("f-3"), "{r2}");
    assert_eq!(field(&r2, "rolled").as_deref(), Some("0"), "{r2}");
    assert_eq!(field(&r2, "rows").as_deref(), Some("5"), "{r2}");
}

/// Section 4: suppression and roll are document state and round-trip through
/// save and reopen. Re-seeding the panel's own published canonical JSON into a
/// fresh harness replays the identical marks, roll position and hash.
#[test]
fn suppression_and_roll_round_trip_through_save_and_reopen() {
    // Build the suppressed+rolled state in harness A: suppress `slot`, then
    // roll the tree back to `base`. The reopen must replay the same marks.
    let mut harness_a = seeded_harness(&seed_json());
    harness_a.run();

    click_label(&mut harness_a, "slot (Cut)");
    click_label(&mut harness_a, "Suppress");
    let suppressed_mark = tree_report(&harness_a);
    assert_eq!(field(&suppressed_mark, "suppressed").as_deref(), Some("1"));

    click_label(&mut harness_a, "base (Extrude)");
    click_label(&mut harness_a, "Roll to here");
    let report_a = tree_report(&harness_a);
    assert_eq!(
        field(&report_a, "roll").as_deref(),
        Some("f-1"),
        "{report_a}"
    );
    assert_eq!(
        field(&report_a, "rolled").as_deref(),
        Some("2"),
        "{report_a}"
    );

    // The panel publishes the exact canonical `.oepdoc` JSON of its live
    // document every frame - what a Save writes and a Reopen reads back.
    let json_a = live_doc_json(&harness_a).expect("live canonical doc JSON");

    // Reopen: feed the exact saved JSON into a fresh harness (the file
    // round-trip; read_oepdoc hands back the same canonical text).
    let mut harness_b = seeded_harness(&json_a);
    harness_b.run();
    let report_b = tree_report(&harness_b);
    let json_b = live_doc_json(&harness_b).expect("reopened doc JSON");

    // The reopened document is byte-identical (canonical round trip), and the
    // report replays the same suppression count, roll position, downstream
    // count and hash - exactly what the card means by "document state".
    assert_eq!(json_a, json_b, "save/reopen must be canonical-stable");
    assert_eq!(
        field(&report_b, "suppressed").as_deref(),
        field(&report_a, "suppressed").as_deref(),
        "suppressed count survives reopen: {report_a} | {report_b}"
    );
    assert_eq!(
        field(&report_b, "roll").as_deref(),
        field(&report_a, "roll").as_deref(),
        "roll point survives reopen"
    );
    assert_eq!(
        field(&report_b, "rolled").as_deref(),
        field(&report_a, "rolled").as_deref(),
        "downstream hidden count survives reopen"
    );
    assert_eq!(
        field(&report_b, "hash").as_deref(),
        field(&report_a, "hash").as_deref(),
        "the reopened document hashes identically"
    );
}
