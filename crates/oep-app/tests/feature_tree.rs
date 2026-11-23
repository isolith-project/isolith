//! UI-C1 UI-FEATURE-TREE: the tree is the document (wave C).
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts the acceptance exactly as
//! the card reads it:
//!
//! 1. Rows come from the live document: the tree derives part / body /
//!    feature rows from the live model document each frame (the committed demo
//!    STEP opens on the first frame), and a seeded document containing Sketch
//!    and VariableSet features renders all five row kinds the card names.
//! 2. Selection is one shared id: clicking a feature row publishes the node's
//!    document id under the shared selection key, the selected row is painted
//!    selected, and the status bar reads the shared id ("1 selected").
//! 3. Rename persists: renaming a row writes the entity's `name` into the
//!    document (the row re-derives with the new label; the document hash
//!    moves), and the edit is journaled (undoable).
//! 4. Delete asks for confirmation and is undoable: Delete shows a confirm
//!    prompt and touches nothing until confirmed; confirming removes the row
//!    (hash changes) and Undo restores the document hash EXACTLY (the UI-B2
//!    journal's canonical-snapshot contract).
//!
//! All assertions read the live app's real state: the tree panel's `String`
//! oracle ([`tree::TREE_REPORT_KEY`] = `ui_c1_tree_report`), the shared
//! selection oracle ([`tree::TREE_SELECTION_KEY`]), the AccessKit tree
//! (row labels, buttons, selected state) and the status bar text.

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

/// The tree panel's `String` report oracle key (must match `tree.rs`).
const TREE_REPORT_KEY: &str = "ui_c1_tree_report";
/// The shared selection id key (must match `tree.rs`).
const TREE_SELECTION_KEY: &str = "isolith.ui.tree.selection";
/// The canonical `.oepdoc` seed key (must match `tree.rs`).
const TREE_SEED_KEY: &str = "ui_c1_tree_seed";

/// The committed demo STEP's three live rows (id / kind locked by the import).
const DEMO_PART: &str = "edpart (Part)";
const DEMO_BODY: &str = "step-shell-1 (Body)";
const DEMO_FEATURE: &str = "step-import (ImportStep)";
const DEMO_FEATURE_ID: &str = "oep-feature-step-import";

/// Build the harness around the real `app::App` at the product window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// Build the harness with `json` placed under the tree seed key BEFORE the
/// first frame, so the tree's very first seed uses it as its live document
/// (exactly as if that canonical `.oepdoc` had been opened).
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

/// The tree panel's `String` report, read back through the same context.
fn tree_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(TREE_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of the tree report, e.g. `("hash", "123")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The shared selection id currently published by the tree.
fn shared_selection(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(TREE_SELECTION_KEY)))
        .unwrap_or_default()
}

/// A label node exactly matches `label` anywhere in the AccessKit tree.
/// Uses the non-panicking `query_all` variant so an absent row is a plain
/// `false`, never a query panic.
fn has_label(harness: &Harness<'_, app::App>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// All rendered text inside the region named `region` (like the A11 helper).
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

/// All rendered text in the model-tree region (rows + header + buttons).
fn tree_text(harness: &Harness<'_, app::App>) -> Vec<String> {
    region_values(harness, "model tree")
}

/// Click the node at `pos` over a few frames (the proven B-wave pattern:
/// hover -> press frame -> release frame -> settle), then let the app settle.
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

/// Click the accessible node with exactly this label.
fn click_label(harness: &mut Harness<'_, app::App>, label: &str) {
    let node = harness.query_by_label(label).expect("node present");
    let pos = node.rect().center();
    click_pos(harness, pos);
}

/// Set the open rename text field to `text` through the accessibility SetValue
/// action (the same deterministic entry the B7 dimension test uses).
fn set_rename_text(harness: &mut Harness<'_, app::App>, text: &str) {
    let input = harness
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .next()
        .expect("rename TextInput present");
    input.set_text_value_accesskit(text);
    harness.step();
    harness.run();
}

/// The committed demo STEP imports to the exact row set the tree derives.
#[test]
fn tree_derives_the_live_demo_document_rows() {
    let mut harness = real_app_harness();
    harness.run();

    let r = tree_report(&harness);
    assert_eq!(field(&r, "open").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "parts").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "bodies").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "features").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "rows").as_deref(), Some("3"), "{r}");

    for label in [DEMO_PART, DEMO_BODY, DEMO_FEATURE] {
        assert!(has_label(&harness, label), "row {label} must render");
    }
    let text = tree_text(&harness).join("\n");
    assert!(
        text.contains("edpart.step") && text.contains("1 body, 1 feature"),
        "the A11 header is kept and extended, not duplicated:\n{text}"
    );
}

/// Selecting a row highlights it: the shared selection id is published for the
/// viewport/properties/status to read, the row renders selected, and the
/// status bar reads the shared id ("1 selected").
#[test]
fn selecting_a_feature_publishes_the_shared_selection_and_highlights() {
    let mut harness = real_app_harness();
    harness.run();

    assert_eq!(shared_selection(&harness), "", "nothing selected yet");
    assert!(region_values(&harness, "status bar")
        .join("\n")
        .contains("0 selected"));

    click_label(&mut harness, DEMO_FEATURE);

    assert_eq!(
        shared_selection(&harness),
        DEMO_FEATURE_ID,
        "the shared selection id is the feature's document id"
    );
    assert_eq!(
        field(&tree_report(&harness), "sel").as_deref(),
        Some(DEMO_FEATURE_ID),
        "the report reflects the shared selection"
    );
    // The rendered row carries the selectable "toggled"/selected state.
    let node = harness.get_by_label(DEMO_FEATURE);
    assert!(
        node.accesskit_node().toggled().is_some(),
        "the selected row is painted selected (toggled state present)"
    );
    // The status bar reads the same shared id.
    assert!(
        region_values(&harness, "status bar")
            .join("\n")
            .contains("1 selected"),
        "status bar shows the shared selection"
    );
}

/// Rename persists in the document through the journal: the row re-derives
/// with the new name and the document hash moves (undoable edit recorded).
#[test]
fn renaming_a_feature_persists_in_the_live_document() {
    let mut harness = real_app_harness();
    harness.run();

    click_label(&mut harness, DEMO_FEATURE);
    let hash_before = field(&tree_report(&harness), "hash").unwrap();

    click_label(&mut harness, "Rename");
    assert!(
        has_label(&harness, "Apply"),
        "the inline rename field appears with an Apply button"
    );
    set_rename_text(&mut harness, "mount-hole");
    click_label(&mut harness, "Apply");

    let r = tree_report(&harness);
    assert!(
        has_label(&harness, "mount-hole (ImportStep)"),
        "the renamed row re-derives from the live document"
    );
    assert!(
        !has_label(&harness, DEMO_FEATURE),
        "the old row label is gone"
    );
    assert_ne!(
        field(&r, "hash").unwrap(),
        hash_before,
        "the rename wrote the entity name into the document (hash moved)"
    );

    // The edit is journaled and undoable (the same exact-hash contract).
    click_label(&mut harness, "Undo");
    let r_undo = tree_report(&harness);
    assert!(
        has_label(&harness, DEMO_FEATURE),
        "undo restores the original name"
    );
    assert_eq!(
        field(&r_undo, "hash").unwrap(),
        hash_before,
        "undo restores the exact pre-rename document hash"
    );
}

/// Delete asks for confirmation, touches nothing until confirmed, then Undo
/// restores the document hash exactly.
#[test]
fn delete_confirms_then_undo_restores_the_exact_document_hash() {
    let mut harness = real_app_harness();
    harness.run();

    click_label(&mut harness, DEMO_FEATURE);
    let hash_before = field(&tree_report(&harness), "hash").unwrap();

    // Delete opens a confirmation; nothing is removed yet.
    click_label(&mut harness, "Delete");
    assert!(
        has_label(&harness, "Confirm"),
        "delete asks for an explicit confirmation"
    );
    assert_eq!(
        field(&tree_report(&harness), "hash").unwrap(),
        hash_before,
        "no delete before confirmation"
    );
    assert!(
        has_label(&harness, DEMO_FEATURE),
        "row still present pre-confirm"
    );

    // Confirming really deletes: the row is gone and the document hash moved.
    click_label(&mut harness, "Confirm");
    let r_deleted = tree_report(&harness);
    assert!(
        !has_label(&harness, DEMO_FEATURE),
        "row removed after confirm"
    );
    assert_eq!(
        field(&r_deleted, "rows").as_deref(),
        Some("2"),
        "{r_deleted}"
    );
    assert_ne!(
        field(&r_deleted, "hash").unwrap(),
        hash_before,
        "confirmed delete changes the document"
    );
    assert_eq!(
        field(&r_deleted, "sel").as_deref(),
        Some("-"),
        "deletion clears the selection"
    );

    // Undo restores the document hash EXACTLY (the UI-B2 journal contract).
    click_label(&mut harness, "Undo");
    let r_undo = tree_report(&harness);
    assert!(
        has_label(&harness, DEMO_FEATURE),
        "undo brings the row back"
    );
    assert_eq!(
        field(&r_undo, "hash").unwrap(),
        hash_before,
        "undo restores the exact pre-delete document hash"
    );
}

/// Cancelling a pending delete leaves the document untouched.
#[test]
fn cancelling_a_delete_leaves_the_document_untouched() {
    let mut harness = real_app_harness();
    harness.run();

    click_label(&mut harness, DEMO_FEATURE);
    let hash_before = field(&tree_report(&harness), "hash").unwrap();

    click_label(&mut harness, "Delete");
    click_label(&mut harness, "Cancel");

    let r = tree_report(&harness);
    assert_eq!(
        field(&r, "hash").unwrap(),
        hash_before,
        "cancel deletes nothing"
    );
    assert!(
        has_label(&harness, DEMO_FEATURE),
        "row still present after cancel"
    );
    assert_eq!(field(&r, "confirm").as_deref(), Some("-"), "confirm closed");
}

/// A canonical document containing Sketch and VariableSet features renders all
/// five row kinds the card names, each selectable, renameable and deletable
/// (with confirmation and undo) like any other row.
#[test]
fn sketch_and_variable_set_rows_render_select_rename_and_delete() {
    // Build the live document the tree will show: one part owning one body,
    // a Sketch feature, a VariableSet feature and an Extrude feature.
    let mut doc = oep_import::Document::new("mm");
    doc.push_part(
        oep_import::Part::new("p1", "part-1")
            .with_body("body-1")
            .with_feature("sketch-1")
            .with_feature("varset-1")
            .with_feature("feat-1"),
    );
    doc.push_body(oep_import::Body::new(
        "body-1",
        "solid-1",
        oep_import::BodyKind::Solid,
    ));
    doc.push_feature(oep_import::Feature::new("sketch-1", "profile", "Sketch"));
    doc.push_feature(oep_import::Feature::new(
        "varset-1",
        "params",
        "VariableSet",
    ));
    doc.push_feature(oep_import::Feature::new("feat-1", "pad", "Extrude"));
    let json = doc.to_json_pretty().expect("canonical seed JSON");

    let mut harness = seeded_harness(&json);
    harness.run();

    // All five row kinds render, sourced from the live document.
    for label in [
        "part-1 (Part)",
        "solid-1 (Body)",
        "profile (Sketch)",
        "params (Variable Set)",
        "pad (Extrude)",
    ] {
        assert!(has_label(&harness, label), "row kind {label} must render");
    }
    let r = tree_report(&harness);
    assert_eq!(field(&r, "rows").as_deref(), Some("5"), "{r}");
    assert_eq!(field(&r, "sketch").as_deref(), Some("1"), "{r}");
    assert_eq!(field(&r, "varset").as_deref(), Some("1"), "{r}");

    // A Sketch row is selectable, renameable and re-derives with the new name.
    click_label(&mut harness, "profile (Sketch)");
    assert_eq!(shared_selection(&harness), "sketch-1");
    click_label(&mut harness, "Rename");
    set_rename_text(&mut harness, "profile-2");
    click_label(&mut harness, "Apply");
    assert!(
        has_label(&harness, "profile-2 (Sketch)"),
        "the Sketch row renames and persists"
    );

    // A Variable Set row deletes with confirmation and undoes exactly.
    let hash_before = field(&tree_report(&harness), "hash").unwrap();
    click_label(&mut harness, "params (Variable Set)");
    click_label(&mut harness, "Delete");
    assert!(
        has_label(&harness, "Confirm"),
        "varset delete asks to confirm"
    );
    click_label(&mut harness, "Confirm");
    assert!(
        !has_label(&harness, "params (Variable Set)"),
        "varset row deleted after confirmation"
    );
    click_label(&mut harness, "Undo");
    assert!(
        has_label(&harness, "params (Variable Set)"),
        "varset undo brings the row back"
    );
    assert_eq!(
        field(&tree_report(&harness), "hash").unwrap(),
        hash_before,
        "varset delete+undo restores the exact document hash"
    );
}

/// Renaming and deleting go through the UI-B2 journal: after a rename and a
/// delete, a single Undo steps BACK over both edits in order (the newest
/// first), restoring the earlier hash each time.
#[test]
fn journal_records_edits_in_order_for_multistep_undo() {
    let mut harness = real_app_harness();
    harness.run();

    click_label(&mut harness, DEMO_FEATURE);
    let original = field(&tree_report(&harness), "hash").unwrap();

    // Edit 1: rename.
    click_label(&mut harness, "Rename");
    set_rename_text(&mut harness, "step-import-renamed");
    click_label(&mut harness, "Apply");
    let after_rename = field(&tree_report(&harness), "hash").unwrap();
    assert_ne!(after_rename, original);

    // Edit 2: delete.
    click_label(&mut harness, "Delete");
    click_label(&mut harness, "Confirm");
    let after_delete = field(&tree_report(&harness), "hash").unwrap();
    assert_ne!(after_delete, after_rename);

    // Undo 1: back to the renamed state.
    click_label(&mut harness, "Undo");
    assert_eq!(
        field(&tree_report(&harness), "hash").unwrap(),
        after_rename,
        "first undo returns to the post-rename hash"
    );
    assert!(has_label(&harness, "step-import-renamed (ImportStep)"));

    // Undo 2: back to the original document.
    click_label(&mut harness, "Undo");
    assert_eq!(
        field(&tree_report(&harness), "hash").unwrap(),
        original,
        "second undo returns to the original hash exactly"
    );
    assert!(has_label(&harness, DEMO_FEATURE));
}
