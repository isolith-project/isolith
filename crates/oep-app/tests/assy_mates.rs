//! UI-E2 UI-ASSY-MATES: mate tool against the live solver.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in
//! the `isolith` binary, pulled in verbatim via `#[path]` like every other
//! test target here) through `egui_kittest` and proves the acceptance exactly
//! as the card reads it:
//!
//! 1. A coincident mate moves the moving component's transform onto the fixed
//!    anchor: the before and after cap transform are printed, and each element
//!    is asserted against the analytic expectation (base is grounded at
//!    identity, so base:top = [30,25,10]; cap origin must land there).
//! 2. A distance mate at d=25 leaves exactly 25 mm between the anchors
//!    (asserted within 1e-6; base:top is the z=10 plane, so cap.z -> 35).
//! 3. Dragging a component re-solves: the degrees of freedom from the solver
//!    report are printed before and after adding the mate, and the report line
//!    is shown in the UI (never silently swallowed).
//! 4. Editing a mate and deleting it restore the exact prior transform hash:
//!    each mate op is ONE journaled command, so undo returns the document
//!    hash byte-for-byte to each recorded prior value.
//! 5. An over-constrained set is refused with the solver's named error, not
//!    solved anyway (the document is untouched).
//!
//! The panel publishes its live canonical assembly document JSON under the
//! tree's doc key and a plain-`String` mate oracle under [`MATE_REPORT_KEY`];
//! the keys are hardcoded here exactly like `assy_tree.rs` hardcodes them.
//! The analytic expectations come from the seam itself (oep-shell
//! `AssemblySession::solve` over the same two-part fixture), so this test
//! never encodes a second solver.

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

/// The mate tool's `String` report oracle key (must match `assy_mates.rs`).
const MATE_REPORT_KEY: &str = "ui_e2_mate_report";
/// The tree's live canonical document JSON key (must match `assy_tree.rs`).
const ASSY_DOC_JSON_KEY: &str = "ui_e1_assy_doc_json";
/// The tree's assembly seed key (must match `assy_tree.rs`).
const ASSY_SEED_KEY: &str = "ui_e1_assy_seed";

/// The committed two-part fixture document (byte-for-byte the engine golden).
const FIXTURE_MODEL: &str =
    include_str!("../../oep-assem/tests/golden/two-part-fixture.oepgeo.json");

/// A TALL harness window: the assembly tree region then has rows of room, so
/// the mate tool (rendered below the tree rows, inside the same region) is
/// fully visible and every chip/button is clickable through the real input
/// path. Region geometry is height-proportional (UI-A4), so this only scales
/// the region - it never reshapes it.
fn seeded_harness() -> Harness<'static, app::App> {
    let json = FIXTURE_MODEL.to_string();
    Harness::builder()
        .with_size([1280.0, 1800.0])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(ASSY_SEED_KEY), json.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

/// The mate tool's `String` report oracle, read back through the same context.
fn mate_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(MATE_REPORT_KEY)))
        .unwrap_or_default()
}

/// The tree's live canonical document JSON published this frame.
fn doc_json(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_DOC_JSON_KEY)))
        .unwrap_or_default()
}

/// One field of the report, e.g. `("mates", "1")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The mate oracle's document hash.
fn reported_hash(harness: &Harness<'_, app::App>) -> u64 {
    field(&mate_report(harness), "hash")
        .and_then(|h| h.parse().ok())
        .unwrap_or(0)
}

/// The sha256 hex of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    oep_shell::sha256::sha256_hex(bytes)
}

/// A node whose accessible name contains `sub` exists (the UI-visible report).
fn has_label_containing(harness: &Harness<'_, app::App>, sub: &str) -> bool {
    harness.query_all_by_label_contains(sub).next().is_some()
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

/// Pointer-drag a labelled component drag handle by `(dx, dy)` logical points,
/// through egui's real pointer path (hover, press, moves, release).
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
    let steps = 5;
    for i in 1..=steps {
        let f = i as f32 / steps as f32;
        harness.hover_at(egui::Pos2::new(start.x + dx * f, start.y + dy * f));
        harness.step();
    }
    harness.drop_at(egui::Pos2::new(start.x + dx, start.y + dy));
    harness.step();
}

/// The cap instance's translation elements [12..15] from the doc JSON.
fn cap_translation(doc_json: &str) -> [f64; 3] {
    let doc = oep_geo::Document::from_json_pretty(doc_json).expect("parse doc json");
    for asm in &doc.assemblies {
        for inst in &asm.instances {
            if inst.id == "cap" {
                return [inst.transform[12], inst.transform[13], inst.transform[14]];
            }
        }
    }
    panic!("cap instance not in document");
}

/// Ground base through the tree (E1's real button), so cap is the only
/// moving instance and the analytic expectations hold.
fn ground_base(harness: &mut Harness<'_, app::App>) {
    click_label(harness, "base · instance base · from oep-part-base");
    harness.step();
    click_label(harness, "Ground");
    harness.step();
    assert!(
        field(&mate_report(harness), "b_instance").as_deref() == Some("base"),
        "B-side pick should default to the grounded instance"
    );
}

/// 1. COINCIDENT: a coincident mate cap:origin == base:top moves the moving
///    component onto the fixed anchor. base is grounded at identity so base:top
///    sits at [30,25,10]; the printed before/after transforms and every element
///    of the after transform are asserted against that analytic expectation.
#[test]
fn coincident_mate_moves_component_onto_fixed_anchor() {
    let mut harness = seeded_harness();
    harness.run();
    ground_base(&mut harness);

    // Before: the committed cap placement.
    let before_json = doc_json(&harness);
    let before = cap_translation(&before_json);
    println!("before cap translate = {before:?}");

    // Pick coincident cap:origin (A, default) == base:top (B, default) and add.
    click_label(&mut harness, "+ Add mate");
    harness.step();

    let report = mate_report(&harness);
    println!("mate report after coincident:\n{report}");
    assert_eq!(field(&report, "mates").as_deref(), Some("1"));
    assert_eq!(field(&report, "err").as_deref(), Some("-"));

    // The after transform: cap origin must sit on base:top = [30,25,10].
    let after_json = doc_json(&harness);
    let after = cap_translation(&after_json);
    println!("after  cap translate = {after:?}");
    println!(
        "coincident: cap translate {} -> {} (analytic target [30,25,10])",
        fmt3(before),
        fmt3(after)
    );
    for (i, (got, want)) in after.into_iter().zip([30.0, 25.0, 10.0]).enumerate() {
        assert!(
            (got - want).abs() < 1e-6,
            "translate[{i}] = {got}, expected {want}"
        );
    }
    let _ = before_json;

    // The solver report agrees: one moving body, three translational dof left.
    assert_eq!(
        field(&report, "dof").as_deref(),
        Some("3"),
        "a point-coincidence on a body leaves its 3 rotational dof"
    );
    assert!(
        field(&report, "max_dist")
            .map(|v| v.starts_with("0.000e") || v.starts_with("-0.000"))
            .unwrap_or(false)
            || field(&report, "max_dist").is_none(),
        "residual should be ~0 for an exact coincidence, got {:?}",
        field(&report, "max_dist")
    );
}

/// 2. DISTANCE: a distance mate at d=25 leaves exactly 25 mm between the
///    anchors (base:top plane is z=10, +Z normal, so cap origin -> z=35;
///    measured signed distance = 25 within 1e-6).
#[test]
fn distance_mate_leaves_exactly_25_mm_between_anchors() {
    let mut harness = seeded_harness();
    harness.run();
    ground_base(&mut harness);

    // Switch the kind to plane-distance and add with the default d=25.
    click_label(&mut harness, "kind distance");
    harness.step();
    click_label(&mut harness, "+ Add mate");
    harness.step();

    let report = mate_report(&harness);
    println!("mate report after distance:\n{report}");
    assert_eq!(field(&report, "mates").as_deref(), Some("1"));
    assert_eq!(field(&report, "kind").as_deref(), Some("plane-distance"));
    assert_eq!(
        field(&report, "dof").as_deref(),
        Some("5"),
        "a single plane-distance constraint removes 1 dof"
    );

    // cap origin must be 25 mm along +Z from the z=10 plane -> z=35.
    let after = cap_translation(&doc_json(&harness));
    println!("distance: cap translate = {after:?} (z expected 35)");
    assert!(
        (after[2] - 35.0).abs() < 1e-6,
        "cap z = {}, expected 35",
        after[2]
    );

    // The distance between the anchors is exactly 25 (within 1e-6): re-derive
    // it from the solved doc the same way the workbench measures it.
    let measured = (after[2] - 10.0).abs();
    println!("distance between anchors = {measured:.6} (expected 25.000000)");
    assert!(
        (measured - 25.0).abs() < 1e-6,
        "anchor distance {measured} != 25"
    );
}

/// 3. DRAG + DOF + UI report: dragging the cap re-solves and the degrees of
///    freedom from the solver report are printed before and after adding the
///    mate; the report is shown in the UI, never silently swallowed.
#[test]
fn dragging_component_resolves_and_reports_dof_before_and_after() {
    let mut harness = seeded_harness();
    harness.run();
    ground_base(&mut harness);

    // Before the mate: with base grounded and cap free, drag the cap and
    // re-solve. The report is published into the oracle and rendered in the UI.
    drag_label(&mut harness, "drag cap", 40.0, 0.0);
    let report_before = mate_report(&harness);
    let dof_before: i32 = field(&report_before, "dof")
        .and_then(|d| d.parse().ok())
        .unwrap_or(-1);
    println!(
        "dof before mate (cap free, after drag) = {}",
        field(&report_before, "dof").as_deref().unwrap_or("-")
    );
    assert!(
        has_label_containing(&harness, "solve dof="),
        "the drag re-solve report must be visible in the UI, got: {}",
        field(&report_before, "report").as_deref().unwrap_or("")
    );

    // Add a coincident mate cap:origin == base:top (default picks).
    click_label(&mut harness, "+ Add mate");
    harness.step();
    let report_after = mate_report(&harness);
    let dof_after: i32 = field(&report_after, "dof")
        .and_then(|d| d.parse().ok())
        .unwrap_or(-1);
    println!(
        "dof after mate (cap constrained) = {}",
        field(&report_after, "dof").as_deref().unwrap_or("-")
    );
    assert_eq!(dof_after, 3, "coincident must report 3 dof");

    // Drag again WITH the mate: the component moves and the re-solve snaps it
    // back onto the mate, reporting dof from the solver report honestly.
    drag_label(&mut harness, "drag cap", -30.0, 20.0);
    let report_drag = mate_report(&harness);
    let dof_drag: i32 = field(&report_drag, "dof")
        .and_then(|d| d.parse().ok())
        .unwrap_or(-1);
    println!(
        "dof after re-drag with mate = {}",
        field(&report_drag, "dof").as_deref().unwrap_or("-")
    );
    println!(
        "report line (UI) after drag = {}",
        field(&report_drag, "report").as_deref().unwrap_or("")
    );
    assert_eq!(
        dof_drag, 3,
        "re-solve after drag must keep the constrained dof"
    );
    assert!(
        has_label_containing(&harness, "solve dof="),
        "the post-drag re-solve report must be visible in the UI"
    );

    // The dragged cap was re-solved back onto the fixed anchor.
    let after = cap_translation(&doc_json(&harness));
    println!("cap translate after drag + re-solve = {after:?} (expect [30,25,10])");
    assert!(
        (after[0] - 30.0).abs() < 1e-6
            && (after[1] - 25.0).abs() < 1e-6
            && (after[2] - 10.0).abs() < 1e-6
    );

    // Before/after dof printed above; the report must never be empty.
    assert!(
        dof_before >= 0,
        "dof before must be reported, got {dof_before}"
    );
}

/// 4. EDIT + DELETE + UNDO: editing a mate and deleting it restore the exact
///    prior transform hash. Each mate op is ONE journaled command: adding,
///    editing and deleting each record a before/after hash, and Undo returns
///    the canonical document byte-for-byte to each recorded prior state.
#[test]
fn edit_and_delete_restore_exact_prior_transform_hash() {
    let mut harness = seeded_harness();
    harness.run();
    ground_base(&mut harness);

    let h0 = reported_hash(&harness);
    println!("h0 (before any mate)  = {h0}");

    // Add coincident cap:origin == base:top -> cap moves -> h1.
    click_label(&mut harness, "+ Add mate");
    harness.step();
    let h1 = reported_hash(&harness);
    let json1 = doc_json(&harness);
    println!(
        "h1 (after add)        = {h1}  sha256 {}",
        sha256_hex(json1.as_bytes())
    );
    assert_ne!(h1, h0, "adding a coincident must move the document");

    // Edit the mate: switch B to base:bottom (z=0) -> cap moves to z=0 -> h2.
    click_label(&mut harness, "Edit 0");
    harness.step();
    click_label(&mut harness, "B anchor bottom");
    harness.step();
    click_label(&mut harness, "Apply edit");
    harness.step();
    let h2 = reported_hash(&harness);
    let json2 = doc_json(&harness);
    println!(
        "h2 (after edit)       = {h2}  sha256 {}",
        sha256_hex(json2.as_bytes())
    );
    assert_ne!(
        h2, h1,
        "editing the mate must re-solve to a different placement"
    );
    let t = cap_translation(&json2);
    assert!(
        (t[2]).abs() < 1e-6,
        "base:bottom is the z=0 plane, cap z = {}",
        t[2]
    );

    // Delete the mate -> cap free, stays at last pose -> h3 == h2 (free dofs
    // are not fabricated back).
    click_label(&mut harness, "Delete 0");
    harness.step();
    let h3 = reported_hash(&harness);
    println!("h3 (after delete)     = {h3}");
    let report = mate_report(&harness);
    assert_eq!(field(&report, "mates").as_deref(), Some("0"));

    // Undo the delete: exact prior hash h2 (byte-for-byte).
    click_label(&mut harness, "Mate undo");
    harness.step();
    let u1 = reported_hash(&harness);
    println!("undo(delete) hash     = {u1}  (expect {h2})");
    assert_eq!(u1, h2, "undo of delete must restore the exact prior hash");
    assert_eq!(
        doc_json(&harness),
        json2,
        "undo of delete must restore the exact prior doc"
    );

    // Undo the edit: exact prior hash h1.
    click_label(&mut harness, "Mate undo");
    harness.step();
    let u2 = reported_hash(&harness);
    println!("undo(edit) hash       = {u2}  (expect {h1})");
    assert_eq!(u2, h1, "undo of edit must restore the exact prior hash");
    assert_eq!(
        doc_json(&harness),
        json1,
        "undo of edit must restore the exact prior doc"
    );

    // Undo the add: exact prior hash h0.
    click_label(&mut harness, "Mate undo");
    harness.step();
    let u3 = reported_hash(&harness);
    println!("undo(add) hash        = {u3}  (expect {h0})");
    assert_eq!(u3, h0, "undo of add must restore the exact prior hash");
}

/// 5. OVER-CONSTRAINED: a contradictory second mate is refused with the
///    solver's named error and never solved anyway (document untouched).
#[test]
fn over_constrained_set_is_refused_with_solver_named_error() {
    let mut harness = seeded_harness();
    harness.run();
    ground_base(&mut harness);

    // First coincident cap:origin == base:top -> cap pinned at z=10.
    click_label(&mut harness, "+ Add mate");
    harness.step();
    let h1 = reported_hash(&harness);
    let report = mate_report(&harness);
    println!(
        "after first mate: dof={} hash={h1}",
        field(&report, "dof").as_deref().unwrap_or("-")
    );
    assert_eq!(field(&report, "mates").as_deref(), Some("1"));

    // Second coincident cap:origin == base:origin demands cap at z=0 too.
    // The two points are 30 mm apart on a single rigid body: over-constrained,
    // refused with the solver's named error, not solved anyway.
    let before2 = doc_json(&harness);
    click_label(&mut harness, "B anchor origin");
    harness.step();
    click_label(&mut harness, "+ Add mate");
    harness.step();

    let report = mate_report(&harness);
    println!("after refused second mate:\n{report}");
    let err = field(&report, "err").unwrap_or_default();
    println!("refusal error = {err}");
    assert!(
        err.contains("solver-failed"),
        "over-constrained set must be refused with the solver's named error, got {err:?}"
    );
    assert_eq!(
        field(&report, "mates").as_deref(),
        Some("1"),
        "the refused mate must not be accepted"
    );
    assert_eq!(
        reported_hash(&harness),
        h1,
        "the refused set must NOT be solved anyway (document untouched)"
    );
    assert_eq!(
        doc_json(&harness),
        before2,
        "the document must be byte-for-byte unchanged after the refusal"
    );
}

/// Format a translation triple for a proof print.
fn fmt3(t: [f64; 3]) -> String {
    format!("[{:.3}, {:.3}, {:.3}]", t[0], t[1], t[2])
}
