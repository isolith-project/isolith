//! UI-C3 UI-VARIABLE-TABLE: units, expressions, usage count (wave C).
//!
//! A variable table drives the model. The acceptance harness drives BOTH the
//! pure table engine and the real `show` panel (pulled in verbatim via
//! `#[path]` exactly like the B7 dimension engine and the A2 tree tests), and
//! asserts the card's three named behaviours as written:
//!
//! 1. **A reference resolves.** An expression that names another variable
//!    resolves through the EXISTING UI-B7 expression path
//!    ([`variables::dimensions::resolve_expression`], the same surface and
//!    the same [`oep_sketch::varset::VariableSet`] store B7 drove) - `width =
//!    base*2`, `height = width/2`, and the driven volume moves when a driver
//!    changes (cross references work AND the table drives the model).
//! 2. **A circular reference is detected and refused with a named error.**
//!    Editing a variable so the table can no longer be ordered topologically
//!    is refused; the error names the cycle path (`circular reference: base
//!    -> width -> base`) and the document is untouched (exact hash).
//! 3. **Each row reports how many places use it, computed from the
//!    document.** [`variables::usage_of`] re-scans the live document every
//!    call (other variables' expressions plus every string-form feature
//!    parameter); nothing is bookkept by hand, so an edit that changes who
//!    references whom moves the counts the next time they are read.
//!
//! The real panel is rendered through `Harness::build_ui_state` with a real
//! `app::App` as state (the panel is delivered `#[path]`-includable and not
//! yet wired into the shell regions; this card owns exactly the two files
//! under test), seeded with a canonical `.oepdoc` before its first frame
//! exactly as the tree panel seeds its document.

#[path = "../src/app.rs"]
mod app;

#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;

#[path = "../src/theme.rs"]
#[allow(dead_code)]
mod theme;

// The exact variable-table panel + engine source the app will compile once a
// wiring card hosts it, included verbatim (the same `#[path]` copy pattern
// every wave-B/C card uses for its panel).
#[path = "../src/panels/variables.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod variables;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use egui::PointerButton;

use oep_import::Document;

/// The panel's `String` report oracle key (must match `variables.rs`).
const REPORT_KEY: &str = "ui_c3_variables_report";
/// The shared resolved-set key (must match `variables.rs`).
const RESOLVED_KEY: &str = "isolith.ui.variables.resolved";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build a canonical document holding a VariableSet feature with `rows`
/// (`(name, unit, expr)`) plus, when `extra` is non-empty, a separate
/// non-variable feature whose parameters hold the string referrers (dimension
/// places that reference variable names).
fn doc_with(rows: &[(&str, &str, &str)], extra: &[(&str, &str)]) -> Document {
    use oep_import::Feature;
    let mut doc = Document::new("mm");
    let list = rows.iter().map(|r| r.0).collect::<Vec<_>>().join("\n");
    let mut feature = Feature::new(
        variables::VARIABLES_FEATURE_ID,
        "Variables",
        variables::VARIABLES_FEATURE_KIND,
    );
    feature = feature.with_param(variables::VARIABLES_LIST_PARAM_KEY.to_string(), list);
    for &(name, unit, expr) in rows {
        feature = feature.with_param(variables::expr_param_key(name), expr.to_string());
        feature = feature.with_param(variables::unit_param_key(name), unit.to_string());
    }
    doc.push_feature(feature);
    if !extra.is_empty() {
        let mut mount = Feature::new("oep-feature-mount", "mount", "Extrude");
        for (k, v) in extra {
            mount = mount.with_param(k.to_string(), v.to_string());
        }
        doc.push_feature(mount);
    }
    doc
}

/// The canonical `.oepdoc` JSON of [`doc_with`] - what a seed file looks like.
fn seed_json(rows: &[(&str, &str, &str)], extra: &[(&str, &str)]) -> String {
    doc_with(rows, extra)
        .to_json_pretty()
        .expect("fixture serializes")
}

/// The classic driven-worksheet rows: `base` (plain 40), `width = base*2` (80)
/// and `height = width/2` (40), with a `mount` feature whose `span` parameter
/// references `width` (one place that uses it outside the table).
fn worksheet_rows() -> [(&'static str, &'static str, &'static str); 3] {
    [
        ("base", "mm", "40"),
        ("width", "mm", "base*2"),
        ("height", "mm", "width/2"),
    ]
}

fn worksheet_extra() -> [(&'static str, &'static str); 1] {
    [("span", "width")]
}

/// A fresh UI-B2 journal for the pure engine tests.
fn journal() -> variables::doc_io::EditHistory {
    variables::doc_io::EditHistory::new()
}

/// One field of an oracle, e.g. `("v1_name", "width")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

fn rows_of(report: &str) -> usize {
    field(report, "rows")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Section 1: the pure engine - cross references through the B7 path
// ---------------------------------------------------------------------------

/// Dependency extraction answers "which variables does an expression
/// mention?" using the same identifier rule as the B7 surface (a full
/// identifier is one token, so `widthx` never counts as `width`).
#[test]
fn referenced_names_extract_exact_identifiers_deduplicated() {
    assert_eq!(variables::referenced_names("width"), ["width"]);
    assert_eq!(variables::referenced_names("base*2"), ["base"]);
    assert_eq!(variables::referenced_names("40"), Vec::<String>::new());
    assert_eq!(variables::referenced_names("width*1.5"), ["width"]);
    assert_eq!(
        variables::referenced_names("a + b - a*b"),
        ["a", "b"],
        "first-appearance order, deduplicated"
    );
    assert_eq!(variables::referenced_names("height/2"), ["height"]);
    assert_eq!(
        variables::referenced_names("widthx"),
        ["widthx"],
        "an identifier is consumed whole - widthx is not width"
    );
}

/// A table of literals and bare references settles through the B7 surface.
#[test]
fn resolve_table_resolves_literals_and_cross_references() {
    let doc = doc_with(&worksheet_rows(), &[]);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let resolved = variables::resolve_table(&entries, None).unwrap();
    assert!((resolved["base"] - 40.0).abs() < 1e-9);
    // width = base*2 resolves 80 through the existing expression path.
    assert!((resolved["width"] - 80.0).abs() < 1e-9);
    // height = width/2 resolves 40 through a deeper cross reference.
    assert!((resolved["height"] - 40.0).abs() < 1e-9);
}

/// A deeper chain resolves topologically: each variable waits for its
/// dependency, so order never matters to the answer.
#[test]
fn resolve_table_settles_a_deeper_chain_in_dependency_order() {
    let doc = doc_with(
        &[("c", "mm", "3"), ("a", "mm", "b+1"), ("b", "mm", "c*2")],
        &[],
    );
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let resolved = variables::resolve_table(&entries, None).unwrap();
    assert!((resolved["c"] - 3.0).abs() < 1e-9);
    assert!((resolved["b"] - 6.0).abs() < 1e-9);
    assert!((resolved["a"] - 7.0).abs() < 1e-9);
}

/// A reference to a name that is not in the table is refused, named.
#[test]
fn resolve_table_rejects_unknown_variables_with_their_name() {
    let doc = doc_with(&[("a", "mm", "bogus*2")], &[]);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    assert_eq!(
        variables::resolve_table(&entries, None),
        Err(variables::VariableTableError::UnknownVariable(
            "bogus".into()
        ))
    );
}

/// Malformed arithmetic from the B7 surface is refused with its named error.
#[test]
fn resolve_table_rejects_invalid_expressions() {
    let doc = doc_with(&[("a", "mm", "2**"), ("b", "mm", "3")], &[]);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let err = variables::resolve_table(&entries, None).unwrap_err();
    assert!(
        matches!(err, variables::VariableTableError::InvalidExpression(_)),
        "kinds: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Section 2: circular references are refused with a named cycle
// ---------------------------------------------------------------------------

/// A variable cannot reference itself (`width = width` can never resolve).
#[test]
fn resolve_table_names_a_self_cycle() {
    let doc = doc_with(&[("width", "mm", "width+1")], &[]);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let err = variables::resolve_table(&entries, None).unwrap_err();
    assert_eq!(
        err.to_string(),
        "circular reference: width -> width",
        "the cycle is named, not just refused: {err:?}"
    );
}

/// Two variables referencing each other cannot be ordered; the cycle is named.
#[test]
fn resolve_table_names_a_mutual_cycle() {
    let doc = doc_with(&[("a", "mm", "b+1"), ("b", "mm", "a+1")], &[]);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let err = variables::resolve_table(&entries, Some("a")).unwrap_err();
    assert_eq!(
        err.to_string(),
        "circular reference: a -> b -> a",
        "cycle named through the edited variable: {err:?}"
    );
}

/// Editing `base` to reference `width` (which depends on `base`) introduces
/// the cycle base -> width -> base: refused with the named path, and the
/// document hash is untouched (no partial edit).
#[test]
fn edit_introducing_a_cycle_is_refused_named_and_document_untouched() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();
    let before = variables::doc_io::document_hash(&doc);
    // `width = base*2` and `height = width/2` are healthy.
    let err = variables::edit_variable(&mut doc, &mut history, "base", "width*0.5").unwrap_err();
    assert_eq!(
        err.to_string(),
        "circular reference: base -> width -> base",
        "the refusal names the cycle: {err:?}"
    );
    assert_eq!(
        variables::doc_io::document_hash(&doc),
        before,
        "a refused edit leaves the document byte-identical"
    );
    assert_eq!(history.len(), 0, "no journal entry for a refused edit");
}

/// Editing a variable to reference a name that does not exist is refused with
/// the name shown, document untouched.
#[test]
fn edit_to_an_unknown_reference_is_refused_and_named() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();
    let before = variables::doc_io::document_hash(&doc);
    let err = variables::edit_variable(&mut doc, &mut history, "width", "missing*2").unwrap_err();
    assert_eq!(
        err,
        variables::VariableTableError::UnknownVariable("missing".into())
    );
    assert_eq!(variables::doc_io::document_hash(&doc), before);
}

// ---------------------------------------------------------------------------
// Section 3: the table drives the model; usage counts come from the document
// ---------------------------------------------------------------------------

/// Cross references drive the model: `width = base*2` makes the driven solid
/// 32000 at base 40; editing base to 60 re-resolves width to 120 and the
/// document-driven volume moves to 72000 - the acceptance numbers derived
/// from the document, never a bare literal in the test.
#[test]
fn cross_references_drive_the_document_volume() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();

    // width resolves 80 -> the driven document weighs 5*80^2 = 32000.
    let feature = variables::variable_feature(&doc).unwrap();
    let resolved =
        variables::resolve_table(&variables::parse_variables(feature).unwrap(), None).unwrap();
    let v0 = variables::driven_volume(&resolved)
        .unwrap()
        .expect("width defined drives");
    let mut vars = oep_sketch::varset::VariableSet::new();
    vars.set(variables::dimensions::WIDTH_NAME, resolved["width"]);
    let expected0 = variables::dimensions::driven_volume(&vars).unwrap();
    assert!((v0 - expected0).abs() < 1e-6, "v0 {v0} vs {expected0}");
    assert!((v0 - 32000.0).abs() < 1e-6, "fixture 80 -> {v0}");

    // base 40 -> 60 re-resolves width 80 -> 120 through the same path.
    variables::edit_variable(&mut doc, &mut history, "base", "60").unwrap();
    let feature = variables::variable_feature(&doc).unwrap();
    let resolved =
        variables::resolve_table(&variables::parse_variables(feature).unwrap(), None).unwrap();
    assert!((resolved["width"] - 120.0).abs() < 1e-9);
    let v1 = variables::driven_volume(&resolved)
        .unwrap()
        .expect("width still defined");
    let mut vars = oep_sketch::varset::VariableSet::new();
    vars.set(variables::dimensions::WIDTH_NAME, resolved["width"]);
    let expected1 = variables::dimensions::driven_volume(&vars).unwrap();
    assert!((v1 - expected1).abs() < 1e-6);
    assert!((v1 - 72000.0).abs() < 1e-6, "fixture 120 -> {v1}");
}

/// Usage counts are computed from the document: base is used by width's
/// expression (1), width by height's expression AND the mount feature's `span`
/// parameter (2), height by nothing (0).
#[test]
fn usage_counts_are_computed_from_the_document() {
    let doc = doc_with(&worksheet_rows(), &worksheet_extra());
    assert_eq!(variables::usage_of(&doc, "base"), 1);
    assert_eq!(variables::usage_of(&doc, "width"), 2);
    assert_eq!(variables::usage_of(&doc, "height"), 0);
}

/// An edit that changes who references whom moves the counts on the next
/// read - the count is a scan of the live document, never bookkeeping.
#[test]
fn usage_counts_follow_edits_not_bookkeeping() {
    let mut doc = doc_with(&worksheet_rows(), &worksheet_extra());
    assert_eq!(variables::usage_of(&doc, "base"), 1);
    assert_eq!(variables::usage_of(&doc, "width"), 2);
    // height now references base instead of width.
    variables::edit_variable(&mut doc, &mut journal(), "height", "base*2").unwrap();
    assert_eq!(
        variables::usage_of(&doc, "base"),
        2,
        "width and height both reference base now"
    );
    assert_eq!(
        variables::usage_of(&doc, "width"),
        1,
        "only the mount feature still references width"
    );
}

// ---------------------------------------------------------------------------
// Section 4: routed edits through the journal
// ---------------------------------------------------------------------------

/// Add through the pure seam: a new variable resolves with the table and is a
/// journaled command.
#[test]
fn add_variable_resolves_with_units_and_journals() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();
    let msg = variables::add_variable(&mut doc, &mut history, "thick", "mm", "5").unwrap();
    assert_eq!(msg, "added thick (mm) = 5.000000");
    assert_eq!(history.len(), 1);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    let thick = entries.iter().find(|e| e.name == "thick").unwrap();
    assert_eq!(thick.unit, "mm");
    assert_eq!(thick.expr, "5");
    let resolved = variables::resolve_table(&entries, None).unwrap();
    assert!((resolved["thick"] - 5.0).abs() < 1e-9);
}

/// Adding a duplicate name is refused with the name shown.
#[test]
fn add_variable_refuses_duplicates_with_the_name() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let err = variables::add_variable(&mut doc, &mut journal(), "width", "mm", "9").unwrap_err();
    assert_eq!(
        err,
        variables::VariableTableError::DuplicateVariable("width".into())
    );
}

/// A name that is not a usable expression identifier is refused.
#[test]
fn add_variable_refuses_invalid_names() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    for bad in ["", "1abc", "a b", "a-b"] {
        let err = variables::add_variable(&mut doc, &mut journal(), bad, "mm", "1").unwrap_err();
        assert!(
            matches!(err, variables::VariableTableError::InvalidName(_)),
            "{bad:?}: {err:?}"
        );
    }
}

/// Removing a variable that another expression still references is refused
/// (the rest of the table would dangle); removing an unused one succeeds.
#[test]
fn remove_variable_refuses_dangling_removals_and_allows_unused() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();
    let before = variables::doc_io::document_hash(&doc);
    // base is referenced by width (`base*2`): removing it would dangle.
    let err = variables::remove_variable(&mut doc, &mut history, "base").unwrap_err();
    assert_eq!(
        err,
        variables::VariableTableError::UnknownVariable("base".into())
    );
    assert_eq!(variables::doc_io::document_hash(&doc), before, "untouched");
    // height references nothing and is referenced by nothing: remove succeeds.
    variables::remove_variable(&mut doc, &mut history, "height").unwrap();
    assert_eq!(history.len(), 1);
    let feature = variables::variable_feature(&doc).unwrap();
    let entries = variables::parse_variables(feature).unwrap();
    assert!(entries.iter().all(|e| e.name != "height"));
}

/// Undo restores the EXACT prior document hash and redo returns to the edited
/// one (the UI-B2 canonical-snapshot contract, through the same journal seam).
#[test]
fn undo_redo_restore_exact_hashes() {
    let mut doc = doc_with(&worksheet_rows(), &[]);
    let mut history = journal();
    let before = variables::doc_io::document_hash(&doc);
    variables::edit_variable(&mut doc, &mut history, "width", "base*4").unwrap();
    let after = variables::doc_io::document_hash(&doc);
    assert_ne!(after, before, "the edit really moved the document");
    let undone = history.undo(&mut doc).unwrap();
    assert_eq!(undone, before);
    assert_eq!(variables::doc_io::document_hash(&doc), before, "hash exact");
    let redone = history.redo(&mut doc).unwrap();
    assert_eq!(redone, after);
    assert_eq!(variables::doc_io::document_hash(&doc), after, "hash exact");
}

// ---------------------------------------------------------------------------
// Section 5: the real panel, driven through kittest
// ---------------------------------------------------------------------------

/// The content rect the standalone panel lays out in. kittest's [`UiState`]
/// host wraps the ui closure in `Frame::central_panel(...).outer_margin(8.0)`
/// (see egui_kittest 0.37 `app_kind.rs`), so `ui.max_rect()` is the window
/// inset by 8 points on each side - the panel's own `show` reads that inset
/// rect, and the test must compute the same widget rects against it.
fn window_region() -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(8.0, 8.0),
        egui::pos2(config::WINDOW_SIZE[0] - 8.0, config::WINDOW_SIZE[1] - 8.0),
    )
}

/// Build a harness around the REAL `variables::show` with a real `app::App`
/// as state, seeding `json` under the seed key BEFORE the first frame (the
/// same seeding the tree panel test uses - exact as opening that document).
fn panel_harness(json: &str) -> Harness<'static, app::App> {
    let json = json.to_string();
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(
                    egui::Id::unique(variables::VARIABLES_SEED_KEY),
                    json.clone(),
                );
            });
        })
        .build_ui_state(variables::show, app::App::new(false))
}

/// The panel's report oracle, read back through the same context.
fn report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(REPORT_KEY)))
        .unwrap_or_default()
}

/// The shared resolved set, read back through the same context.
fn resolved(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(RESOLVED_KEY)))
        .unwrap_or_default()
}

fn click(harness: &mut Harness<'_, app::App>, pos: egui::Pos2) {
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

/// Set a row's expression field to `text` (via the accesskit SetValue action,
/// the proven B7 pattern) and click that row's Apply.
fn edit_row(harness: &mut Harness<'_, app::App>, index: usize, text: &str) {
    let rects = variables::row_rects(window_region(), index);
    let input = harness
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| {
            let r = n.rect();
            r.max.x >= rects.expr.min.x
                && r.min.x <= rects.expr.max.x
                && r.max.y >= rects.expr.min.y
                && r.min.y <= rects.expr.max.y
        })
        .expect("row expression TextInput inside its widget rect");
    input.set_text_value_accesskit(text);
    harness.step();
    harness.step();
    click(harness, rects.apply.center());
}

/// Section 5a: seeded panel renders the resolved table - values, usage counts
/// and the driven volume come from the document, and the shared resolved set
/// is published for whoever drives the model.
#[test]
fn seeded_panel_publishes_resolved_values_usage_and_volume() {
    let mut harness = panel_harness(&seed_json(&worksheet_rows(), &worksheet_extra()));
    harness.run();

    let r = report(&harness);
    assert_eq!(field(&r, "open").as_deref(), Some("1"));
    assert_eq!(rows_of(&r), 3, "base, width, height: {r}");
    // Cross references resolved through the B7 path, live in the oracle.
    assert_eq!(field(&r, "v0_name").as_deref(), Some("base"));
    assert_eq!(field(&r, "v0_value").as_deref(), Some("40.000000"));
    assert_eq!(field(&r, "v0_uses").as_deref(), Some("1"));
    assert_eq!(field(&r, "v1_name").as_deref(), Some("width"));
    assert_eq!(field(&r, "v1_value").as_deref(), Some("80.000000"));
    assert_eq!(field(&r, "v1_expr").as_deref(), Some("base*2"));
    assert_eq!(
        field(&r, "v1_uses").as_deref(),
        Some("2"),
        "height expr + span param"
    );
    assert_eq!(field(&r, "v2_name").as_deref(), Some("height"));
    assert_eq!(field(&r, "v2_value").as_deref(), Some("40.000000"));
    assert_eq!(field(&r, "v2_uses").as_deref(), Some("0"));
    // The driven volume is the document's own mass property (80 -> 32000).
    assert_eq!(field(&r, "volume_ok").as_deref(), Some("1"));
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("volume");
    assert!((volume - 32000.0).abs() < 1e-6, "volume {volume}");
    assert_eq!(field(&r, "error").as_deref(), Some("-"));

    // The shared resolved set is published (the driving seam).
    let res = resolved(&harness);
    assert!(
        res.lines().any(|l| l == "width=80.000000"),
        "resolved: {res}"
    );
    assert!(res.lines().any(|l| l == "base=40.000000"));
}

/// Section 5b: editing an expression through the real panel re-resolves the
/// cross reference and moves the driven volume (base 40 -> 60 => width 120 =>
/// 72000) - one journaled command, hash moves.
#[test]
fn panel_editing_a_driver_expression_moves_the_driven_volume() {
    let mut harness = panel_harness(&seed_json(&worksheet_rows(), &[]));
    harness.run();
    let before_hash = field(&report(&harness), "hash");

    edit_row(&mut harness, 0, "60"); // base

    let r = report(&harness);
    assert_eq!(field(&r, "v0_value").as_deref(), Some("60.000000"));
    assert_eq!(field(&r, "v0_expr").as_deref(), Some("60"));
    // width stays `base*2`: 120, and the driven volume moves to 72000.
    assert_eq!(field(&r, "v1_value").as_deref(), Some("120.000000"));
    let volume: f64 = field(&r, "volume")
        .and_then(|v| v.parse().ok())
        .expect("volume");
    assert!((volume - 72000.0).abs() < 1e-6, "volume {volume}");
    let hash = field(&r, "hash");
    assert_ne!(hash, before_hash, "the edit moved the document");
    assert_eq!(field(&r, "error").as_deref(), Some("-"));
    let res = resolved(&harness);
    assert!(
        res.lines().any(|l| l == "width=120.000000"),
        "resolved: {res}"
    );
}

/// Section 5c: a circular edit through the real panel is refused, the error
/// names the cycle, and the document hash is untouched.
#[test]
fn panel_refuses_a_circular_edit_with_the_named_cycle() {
    let mut harness = panel_harness(&seed_json(&worksheet_rows(), &[]));
    harness.run();
    let before_hash = field(&report(&harness), "hash").expect("hash present");

    // width = base*2 and height = width/2; making width reference height
    // closes the cycle width -> height -> width.
    edit_row(&mut harness, 1, "height*2");

    let r = report(&harness);
    let err = field(&r, "error").expect("named error present");
    assert!(
        err.contains("circular reference") && err.contains("width -> height -> width"),
        "named cycle expected, got: {err}"
    );
    assert_eq!(
        field(&r, "hash"),
        Some(before_hash),
        "a refused edit must not move the document"
    );
    // The refused edit never reached the document: the row still carries its
    // last committed expression and the previously resolved value.
    assert_eq!(field(&r, "v1_expr").as_deref(), Some("base*2"));
    assert_eq!(field(&r, "v1_value").as_deref(), Some("80.000000"));
}
