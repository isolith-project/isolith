//! UI-E3 UI-ASSY-DIAGNOSTICS: dof readout and interference panel.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in
//! the `isolith` binary, pulled in verbatim via `#[path]` like every other
//! test target here) through `egui_kittest` and proves the acceptance exactly
//! as the card reads it:
//!
//! 1. The committed interference fixture reports EXACTLY the known overlapping
//!    pair and nothing else (the fixture's expected pair and the reported
//!    list are both printed; the panel's oracle `pairs=1` and its single
//!    `pair_0` are asserted, and independently re-derived through the same
//!    analysis check).
//! 2. A non-overlapping pair reports zero interference (the committed
//!    two-part base + cap: no overlap - printed, no false positive).
//! 3. Jump-to selects both components of the reported pair (the selection id
//!    list is asserted to contain exactly the two instance ids).
//! 4. The dof readout equals the solver's report for the same assembly (both
//!    numbers printed, asserted equal via an independent
//!    `AssemblySession::solve` over the same constraint set).
//! 5. The dof readout updates when a mate is added (coincident) or removed -
//!    both transitions printed and independently verified against the same
//!    solver.
//!
//! The interference numbers come from the EXISTING analysis check
//! (`oep_analysis::clash::detect_in_assembly`) - both the panel and this
//! harness call it; no second collision implementation. The dof numbers come
//! from the EXISTING solver seam (`oep_shell::assem::AssemblySession::solve`)
//! (the panel reads it, and this harness re-derives it independently with the
//! same seam over the same constraint set).

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

/// The diagnostics panel's `String` report oracle key (must match
/// `assy_diagnostics.rs`).
const DIAG_REPORT_KEY: &str = "ui_e3_diag_report";
/// The diagnostics panel's published jump-to selection id list key.
const DIAG_SELECTION_KEY: &str = "ui_e3_diag_selection";
/// The tree's live canonical document JSON key (must match `assy_tree.rs`).
const ASSY_DOC_JSON_KEY: &str = "ui_e1_assy_doc_json";
/// The tree's assembly seed key (must match `assy_tree.rs`).
const ASSY_SEED_KEY: &str = "ui_e1_assy_seed";
/// The tree's `String` report oracle key (its `sel=` is the jump focus).
const ASSY_REPORT_KEY: &str = "ui_e1_assy_report";

/// The committed interference fixture: the analysis crate's golden - a base
/// plate 40x30x10 at the origin and an interfering block 20x10x10 placed at
/// (25,20,5), which PENETRATES the base. Exactly one known overlapping pair:
/// `i-base` x `i-block`, overlap 750 mm3 at (32.5, 25, 7.5).
const CLASH_FIXTURE: &str =
    include_str!("../../oep-analysis/tests/golden/clash-fixture.oepgeo.json");

/// The committed two-part fixture (base + cap): base at identity, cap placed
/// well above it - NO overlap (the no-false-positive fixture).
const TWO_PART_FIXTURE: &str =
    include_str!("../../oep-assem/tests/golden/two-part-fixture.oepgeo.json");

/// A TALL harness window (same as UI-E2): the assembly region then has rows
/// of room, so the tree, the mate tool AND the diagnostics panel are fully
/// visible and every chip/button is clickable through the real input path.
fn seeded_harness(json: &str) -> Harness<'static, app::App> {
    let json = json.to_string();
    Harness::builder()
        .with_size([1280.0, 1800.0])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(ASSY_SEED_KEY), json.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

fn harness_with(json: &str) -> Harness<'static, app::App> {
    seeded_harness(json)
}

/// The diagnostics panel's `String` report oracle.
fn diag_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(DIAG_REPORT_KEY)))
        .unwrap_or_default()
}

/// The diagnostics panel's jump-to selection id list.
fn diag_selection(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(DIAG_SELECTION_KEY)))
        .unwrap_or_default()
}

/// The tree's report oracle (its `sel=` is the jump focus).
fn assy_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_REPORT_KEY)))
        .unwrap_or_default()
}

/// One field of a report, e.g. `("pairs", "1")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// Click a labelled node (row or button) exactly, through the real input path.
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

/// Ground base through the tree (E1's real button) so cap is the only moving
/// instance and the analytic dof expectations hold.
fn ground_base(harness: &mut Harness<'_, app::App>) {
    click_label(harness, "base · instance base · from oep-part-base");
    harness.step();
    click_label(harness, "Ground");
    harness.step();
}

/// Independent dof for the same assembly: solve the current document JSON +
/// a constraint-set JSON through the EXISTING seam (the same
/// `AssemblySession::solve` the panels use) and return the report's dof.
fn solver_dof(doc_json: &str, constraints_json: &str) -> i32 {
    let mut session =
        oep_shell::assem::AssemblySession::open(doc_json, constraints_json).expect("seam opens");
    match session.solve() {
        oep_shell::assem::SolveOutcome::Solved(report) => report.dof,
        oep_shell::assem::SolveOutcome::Diverged(e) => {
            panic!("independent solve diverged: {e:?}")
        }
    }
}

/// The constraint JSON for a grounded base fixed mate (exactly the shape the
/// tree's `mate_set_json` builds for grounding).
fn grounded_base_cs() -> String {
    "{\"format\":\"oep-assem-constraints\",\"version\":1,\"units\":\"mm\",\
     \"assembly\":\"oep-assembly-fixture\",\"mates\":[{\"kind\":\"fixed\",\"instance\":\"base\"}]}"
        .to_string()
}

/// The constraint JSON for a grounded base + the coincident
/// `cap:origin == base:top` (exactly the shape the mate tool builds).
fn grounded_base_coincident_cs() -> String {
    "{\"format\":\"oep-assem-constraints\",\"version\":1,\"units\":\"mm\",\
     \"assembly\":\"oep-assembly-fixture\",\
     \"mates\":[{\"kind\":\"fixed\",\"instance\":\"base\"},\
     {\"kind\":\"coincident\",\"a\":{\"instance\":\"cap\",\"anchor\":\"origin\"},\
     \"b\":{\"instance\":\"base\",\"anchor\":\"top\"}}]}"
        .to_string()
}

/// The live canonical document JSON the tree publishes this frame.
fn doc_json(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_DOC_JSON_KEY)))
        .unwrap_or_default()
}

/// The ids (innermost instance of each chain) of every clash the analysis
/// check reports for a document - the independent re-derivation.
fn direct_clash_ids(doc_json: &str) -> Vec<(String, String)> {
    let doc = oep_geo::Document::from_json_pretty(doc_json).expect("parse doc json");
    let clashes = oep_analysis::clash::detect_in_assembly(&doc, "oep-assembly-fixture")
        .expect("interference check runs");
    clashes
        .into_iter()
        .map(|c| {
            (
                c.chain_a.last().cloned().unwrap_or_default(),
                c.chain_b.last().cloned().unwrap_or_default(),
            )
        })
        .collect()
}

/// 1. INTERFERENCE FIXTURE: the committed interference fixture reports EXACTLY
///    the known overlapping pair (`i-base` x `i-block`) and nothing else. Both
///    the fixture's expected pair and the reported list are printed.
#[test]
fn interference_fixture_reports_exactly_the_known_overlapping_pair() {
    let mut harness = harness_with(CLASH_FIXTURE);
    harness.run();

    let report = diag_report(&harness);
    let pairs: usize = field(&report, "pairs")
        .and_then(|v| v.parse().ok())
        .expect("pairs field");
    let pair_0 = field(&report, "pair_0").unwrap_or_default();
    println!("fixture expected pair = i-base x i-block");
    println!("reported list:");

    let direct = direct_clash_ids(&doc_json(&harness));
    for (a, b) in &direct {
        println!("  {a} x {b}");
    }
    println!("panel oracle = {report}");

    assert_eq!(
        pairs, 1,
        "the committed interference fixture must report exactly ONE pair"
    );
    assert!(
        pair_0.starts_with("i-base x i-block"),
        "the single reported pair must be the known pair, got {pair_0:?}"
    );
    assert_eq!(
        direct,
        vec![("i-base".to_string(), "i-block".to_string())],
        "the independent re-derivation must agree: exactly the known pair, nothing else"
    );
    assert_eq!(field(&report, "err").as_deref(), Some("-"));
}

/// 2. NON-OVERLAP: a non-overlapping pair reports zero interference (the
///    committed two-part base + cap) - no false positive.
#[test]
fn non_overlapping_pair_reports_zero_interference() {
    let mut harness = harness_with(TWO_PART_FIXTURE);
    harness.run();

    let report = diag_report(&harness);
    let pairs: usize = field(&report, "pairs")
        .and_then(|v| v.parse().ok())
        .expect("pairs field");
    println!("non-overlap fixture: base + cap (cap well above base)");
    println!("reported pairs = {pairs}");
    println!("panel oracle = {report}");

    assert_eq!(
        pairs, 0,
        "a non-overlapping pair must report ZERO interference (no false positive)"
    );
    let direct = direct_clash_ids(&doc_json(&harness));
    assert!(
        direct.is_empty(),
        "independent re-derivation must also find nothing: {direct:?}"
    );
}

/// 3. JUMP-TO: jump-to selects BOTH components of the reported pair. The
///    selection id list published by the panel is asserted to be exactly the
///    pair; the tree's single highlight (the jump focus) is the first id.
#[test]
fn jump_to_selects_both_components_of_the_reported_pair() {
    let mut harness = harness_with(CLASH_FIXTURE);
    harness.run();

    // The interference table shows the pair row with its Jump button.
    assert!(
        harness
            .query_all_by_label("Jump i-base x i-block")
            .next()
            .is_some(),
        "the reported pair row must carry its Jump-to button"
    );

    click_label(&mut harness, "Jump i-base x i-block");
    harness.step();

    let sel = diag_selection(&harness);
    let from_report = field(&diag_report(&harness), "selection").unwrap_or_default();
    let focus = field(&assy_report(&harness), "sel").unwrap_or_default();
    println!("jump-to selection id list = {sel}");
    println!("selection in report = {from_report}");
    println!("tree highlight (jump focus) = {focus}");

    assert_eq!(
        sel, "i-base,i-block",
        "jump-to must select BOTH components of the reported pair"
    );
    assert_eq!(from_report, sel, "report selection must match the id list");
    assert_eq!(
        focus, "i-base",
        "the jump focus is the pair's first component"
    );
    assert!(
        harness
            .query_all_by_label("interference: 1 pair")
            .next()
            .is_some(),
        "the interference summary must be visible in the UI"
    );
}

/// 4. DOF EQUALS SOLVER REPORT: the dof readout equals the solver's report
///    for the same assembly. Both numbers printed and asserted equal: the
///    panel's readout and an independent `AssemblySession::solve` over the
///    same document + the same constraint set at the same moment.
///
///    Note the seam's honest semantics: dof = 7 per moving body minus the
///    rank of the Jacobian (which includes one unit-quaternion boundedness
///    row per moving body), and the solver reports 0 when the starting state
///    already satisfies every residual (the same 0 UI-E2 printed for a free
///    cap after a drag). The panel shows exactly that solver number - the
///    clause is equality with the report, not a particular value.
#[test]
fn dof_readout_equals_the_solver_report_for_the_same_assembly() {
    let mut harness = harness_with(TWO_PART_FIXTURE);
    harness.run();
    ground_base(&mut harness);

    // The doc and the constraint set at this instant (base grounded fixed,
    // cap the only moving body, no user mate yet).
    let doc = doc_json(&harness);
    let cs = grounded_base_cs();
    let independent = solver_dof(&doc, &cs);

    let report = diag_report(&harness);
    let panel_dof: i32 = field(&report, "dof")
        .and_then(|v| v.parse().ok())
        .expect("dof field");
    let source = field(&report, "dof_source").unwrap_or_default();
    println!("panel dof readout = {panel_dof}  (from {source} solver report)");
    println!("solver report dof  = {independent}  (AssemblySession::solve, same assembly)");

    assert_eq!(
        panel_dof, independent,
        "the dof readout must equal the solver's report for the same assembly"
    );
    assert!(
        panel_dof >= 0,
        "the readout must be the solver's report, never fabricated"
    );
    assert_eq!(
        field(&report, "tree_dof")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(-2),
        independent,
        "the panel's tree-level solve must agree with the independent re-derivation"
    );
}

/// 5. DOF UPDATES ON MATE ADD/REMOVE: the dof readout updates when a mate is
///    added (a coincident, through the real mate tool) and when it is removed.
///    Each transition is printed and independently verified through the same
///    solver over the same document+constraint state the tool solved: 0 (free
///    cap, mate tool quiet) -> 3 (a point-coincidence pins the three
///    translations) -> 0 (mate removed, cap free again).
#[test]
fn dof_updates_when_a_mate_is_added_or_removed() {
    let mut harness = harness_with(TWO_PART_FIXTURE);
    harness.run();
    ground_base(&mut harness);

    let panel_dof = |h: &Harness<'_, app::App>| -> i32 {
        let report = diag_report(h);
        field(&report, "dof")
            .and_then(|v| v.parse().ok())
            .expect("dof field")
    };

    // Before any mate: the mate tool is quiet, so the readout is the panel's
    // own tree-level solve over the grounded set -> 0 (all residuals already
    // satisfied).
    let doc_before = doc_json(&harness);
    let before: i32 = panel_dof(&harness);
    let independent_before = solver_dof(&doc_before, &grounded_base_cs());
    println!("dof before mate (cap free, base grounded) = {before}");
    println!("solver report dof (same state)            = {independent_before}");
    assert_eq!(before, independent_before);
    assert_eq!(
        before, 0,
        "a fully-satisfied free state reports 0, like every panel in this wave"
    );

    // Add a coincident cap:origin == base:top through the real mate tool.
    // The tool solves the document AS IT WAS (cap at [28,14,20], NOT yet on
    // base:top), so the solver iterates and reports 3.
    click_label(&mut harness, "+ Add mate");
    harness.step();
    // The diagnostics section renders BEFORE the mate tool in the frame, so
    // its dof source (the mate tool's report) lands one repaint after the op;
    // one more step makes the readout reflect the just-published report.
    harness.step();
    let after_add: i32 = panel_dof(&harness);
    let independent_add = solver_dof(&doc_before, &grounded_base_coincident_cs());
    println!("dof after mate added = {after_add}");
    println!("solver report dof (same assembly, same start state) = {independent_add}");
    println!("dof readout updated on mate add: {before} -> {after_add}");
    assert_eq!(
        after_add, 3,
        "adding the coincident must drop the dof readout to 3 (3 rotational dof left)"
    );
    assert_eq!(
        after_add, independent_add,
        "the updated readout must equal the solver's report for the same assembly"
    );

    // Remove the mate through the real mate tool -> cap free again, and the
    // solver reports 0 for the fully-satisfied start state (free dof is not
    // fabricated). The readout updates 3 -> 0.
    click_label(&mut harness, "Delete 0");
    harness.step();
    // Same frame-ordering note as above: the diagnostics readout reflects the
    // mate tool's report one repaint after the op.
    harness.step();
    let doc_after = doc_json(&harness);
    let after_remove: i32 = panel_dof(&harness);
    let independent_remove = solver_dof(&doc_after, &grounded_base_cs());
    println!("dof after mate removed = {after_remove}");
    println!("solver report dof (same state)   = {independent_remove}");
    println!("dof readout updated on mate remove: {after_add} -> {after_remove}");
    assert_eq!(
        after_remove, independent_remove,
        "the restored readout must equal the solver's report for the same assembly"
    );
    assert_ne!(
        after_remove, after_add,
        "the readout must have UPDATED when the mate was removed"
    );
}
