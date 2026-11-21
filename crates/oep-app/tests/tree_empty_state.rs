//! UI-A11 UI-TREE-EMPTY-STATE: the model tree tells the truth about the
//! document instead of the hardcoded "No model open" stub.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts what the left panel
//! actually renders:
//!
//! 1. No-document case: when the doc panel's oracle reports an empty session
//!    (its own no-document format, `file=Untitled` with no contents), the tree
//!    shows the honest "No model open" line. UI-A7 auto-opens the committed
//!    demo STEP on the first frame, so this oracle is what the app would
//!    publish for a genuinely empty session; seeding it is the only way to
//!    observe the empty branch deterministically.
//! 2. Document-open case (the gate contradiction): once a document is open the
//!    same panel names it (its live file name), reports what the document
//!    actually contains (the body/feature counts taken from the document
//!    itself), and labels the placeholder plainly as "Feature tree arrives in
//!    wave C (UI-C1)" instead of pretending to be a tree. It never fabricates
//!    tree rows.
//! 3. Update-on-open case: the transition from "No model open" to the named
//!    document happens in the same running app, no restart, purely because the
//!    tree re-reads the doc panel's per-frame oracle every frame.
//!
//! Only `app.rs` (and `config.rs`) are included: `app.rs` pulls in the panels
//! (tree, doc, ...) through `panels/mod.rs`, so this test drives the exact
//! production tree panel. The tree renders ordinary egui labels inside the
//! "model tree" region, read back here through the AccessKit tree.

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

/// Build the harness around the real `app::App` at the product's window size.
fn real_app_harness() -> Harness<'static, app::App> {
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .build_eframe(|_cc| app::App::new(false))
}

/// All rendered text inside the "model tree" region (the shell's left panel),
/// exactly as the real app painted it into the AccessKit tree. egui renders a
/// `ui.label` as a Label node whose text lives in its (and its TextRun
/// child's) accessible `value`, so both are collected.
fn tree_labels(harness: &Harness<'_, app::App>) -> Vec<String> {
    let region = &harness.get_all_by_label("model tree").collect::<Vec<_>>()[0];
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

/// The doc panel's plain-`String` oracle key under `DOC_REPORT_KEY` in `doc.rs`
/// (a std type, so its identity is shared across `#[path]` copies; a private
/// struct would not be). The tree panel reads it through the same handle.
const REPORT_KEY: &str = "ui_a7_doc_report";

/// The exact report the doc panel's own `DocState::report()` emits for an
/// empty session (no document): `file=Untitled`, no contents. Feeding this to
/// the shared oracle is how a genuinely-empty session reaches the tree panel.
fn empty_session_oracle() -> String {
    "file=Untitled\ndirty=false\nhash=0\nsaved=\nbodies=0\nfeatures=0\n".to_string()
}

/// Seed the doc-panel oracle with `report` and step one frame so the tree
/// renders against it (the doc panel republishes the real report at the end of
/// every frame, so the seeded value is only visible on the frame it is set).
fn seed_oracle_and_step(harness: &mut Harness<'static, app::App>, report: String) {
    harness
        .ctx
        .data_mut(|d| d.insert_temp::<String>(egui::Id::unique(REPORT_KEY), report));
    harness.step();
}

/// The committed demo STEP imports to exactly one body and one feature
/// (verified independently of the app via the import hub), which is the truth
/// the doc panel publishes while that document is open.
#[test]
fn committed_demo_step_imports_to_one_body_and_one_feature() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("demos/editor-sketch-extrude/edpart.step");
    let bytes = std::fs::read(&path).expect("demo STEP present");
    let outcome =
        oep_import::import_bytes(oep_import::SourceKind::Step, &bytes).expect("demo STEP imports");
    let document = match outcome {
        oep_import::ImportOutcome::Imported { document, .. } => document,
        other => panic!("demo STEP must import, got {other:?}"),
    };
    assert_eq!(document.bodies.len(), 1, "demo doc has one body");
    assert_eq!(document.features.len(), 1, "demo doc has one feature");
}

/// No-document case: with the doc panel's empty-session oracle the tree keeps
/// the honest "No model open" line and fabricates nothing.
#[test]
fn tree_says_no_model_open_with_empty_session_oracle() {
    let mut harness = real_app_harness(); // settles with the demo doc already open
    seed_oracle_and_step(&mut harness, empty_session_oracle());

    let labels = tree_labels(&harness);
    let joined = labels.join("\n");
    assert!(
        joined.contains("No model open"),
        "no-doc tree must keep the honest placeholder, got:\n{joined}"
    );
    assert!(
        !labels
            .iter()
            .any(|l| l.contains("edpart.step") || l.contains("body")),
        "no document open, tree must not invent contents, got:\n{joined}"
    );
    assert!(
        !labels
            .iter()
            .any(|l| l.contains("Feature tree arrives in wave C")),
        "placeholder note is for the open-document case, got:\n{joined}"
    );
}

/// Document-open case: once a document is open, the tree names it, reports
/// what it actually contains, and labels the wave-C placeholder.
#[test]
fn tree_names_open_document_with_truthful_counts_and_wave_c_note() {
    let mut harness = real_app_harness(); // demo doc auto-opened by UI-A7
    harness.run();

    let labels = tree_labels(&harness);
    let joined = labels.join("\n");

    assert!(
        joined.contains("edpart.step"),
        "tree must name the open document, got:\n{joined}"
    );
    assert!(
        joined.contains("1 body, 1 feature"),
        "tree must report the doc's truthful body/feature count, got:\n{joined}"
    );
    assert!(
        joined.contains("Feature tree arrives in wave C (UI-C1)"),
        "tree must label the wave-C placeholder, got:\n{joined}"
    );
    assert!(
        !joined.contains("No model open"),
        "with a document open the tree must not claim otherwise, got:\n{joined}"
    );
}

/// Update-on-open case: the panel flips from "No model open" to naming the
/// document in the same running app, no restart — purely because it re-reads
/// the doc panel's per-frame oracle every frame.
#[test]
fn tree_updates_to_named_document_on_open_without_restart() {
    let mut harness = real_app_harness();

    // Frame A: feed the empty-session oracle; the tree honestly says so.
    seed_oracle_and_step(&mut harness, empty_session_oracle());
    let before = tree_labels(&harness).join("\n");
    assert!(before.contains("No model open"), "no-doc before: {before}");

    // Frame B: the doc panel republished the real (demo) report next frame;
    // the same harness, same app, no restart now names the document.
    harness.step();
    let after = tree_labels(&harness).join("\n");
    assert!(
        after.contains("edpart.step") && after.contains("1 body, 1 feature"),
        "tree must reflect the open document next frame, got:\n{after}"
    );
    assert!(
        !after.contains("No model open"),
        "placeholder must go once the document is named, got:\n{after}"
    );
}
