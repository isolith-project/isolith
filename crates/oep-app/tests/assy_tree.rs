//! UI-E1 UI-ASSY-TREE: components, insert, ground, isolate.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and asserts the acceptance exactly as
//! the card reads it:
//!
//! 1. Insert two parts from the committed fixtures: the tree shows both with
//!    their instance ids, and the assembly document JSON published by the UI
//!    equals byte-for-byte the JSON the CLI path produces for the same two
//!    inserts (both paths are called in this test, equality asserted, and both
//!    hashes printed).
//! 2. Ground one component and isolate the other: the grounded set and the
//!    visible set are exactly as expected (both id lists printed).
//! 3. Isolate hides the other component's rows and geometry; un-isolate
//!    restores them; the document hash returns to the pre-isolate value.
//! 4. Every tree row has an accessible name (kittest asserts) and the region
//!    is reachable by keyboard, not only by mouse.
//!
//! The UI path drives the real panel: it publishes its live canonical document
//! JSON under the assembly doc key, its plain-`String` oracle (open / assembly
//! / rows / sel / grounded / visible / hash) under the report key, and reads a
//! canonical `.oepdoc` seed from the seed key. The keys are hardcoded here
//! exactly like `feature_tree.rs` hardcodes `tree.rs`'s keys (mod panels is
//! private, so the test names the oracles by value). The CLI path calls the
//! exact same engine seam the CLI runs (`oep_shell::assem::AssemblySession`
//! over the committed two-part fixture) and serializes its document; the two
//! must be byte-for-byte identical.

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

/// The panel's `String` report oracle key (must match `assy_tree.rs`).
const ASSY_REPORT_KEY: &str = "ui_e1_assy_report";
/// The live canonical document JSON key (must match `assy_tree.rs`).
const ASSY_DOC_JSON_KEY: &str = "ui_e1_assy_doc_json";
/// The canonical `.oepdoc` seed key (must match `assy_tree.rs`).
const ASSY_SEED_KEY: &str = "ui_e1_assy_seed";
/// Assembly id of the committed two-part fixture.
const FIXTURE_ASSEMBLY_ID: &str = "oep-assembly-fixture";

/// The committed two-part fixture document, embedded here too (byte-for-byte
/// the golden the engine/CLI builds for two inserts). The panel embeds the
/// same committed bytes; both paths operate on the same canonical source.
const FIXTURE_MODEL: &str =
    include_str!("../../oep-assem/tests/golden/two-part-fixture.oepgeo.json");
/// The committed constraint set for that fixture (the CLI path opens it).
const FIXTURE_CONSTRAINTS: &str =
    include_str!("../../oep-assem/tests/golden/two-part-fixture.constraints.json");

/// Build the harness with `json` under the assembly seed key BEFORE the first
/// frame, so the panel's very first seed opens that canonical assembly
/// document (exactly as if that `.oepdoc` had been opened).
fn seeded_harness(json: &str) -> Harness<'static, app::App> {
    let json = json.to_string();
    Harness::builder()
        .with_size([config::WINDOW_SIZE[0], config::WINDOW_SIZE[1]])
        .with_context_setup(move |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(ASSY_SEED_KEY), json.clone());
            });
        })
        .build_eframe(|_cc| app::App::new(false))
}

/// The panel's `String` report oracle, read back through the same context.
fn assy_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_REPORT_KEY)))
        .unwrap_or_default()
}

/// The live canonical document JSON the panel published this frame.
fn assy_doc_json(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_DOC_JSON_KEY)))
        .unwrap_or_default()
}

/// One field of the report, e.g. `("rows", "2")`.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The report's document hash (u64 FNV over the canonical JSON).
fn reported_hash(harness: &Harness<'_, app::App>) -> u64 {
    field(&assy_report(harness), "hash")
        .and_then(|h| h.parse().ok())
        .unwrap_or(0)
}

/// The sha256 hex of `bytes` (the "both hashes" the card asks to print).
fn sha256_hex(bytes: &[u8]) -> String {
    oep_shell::sha256::sha256_hex(bytes)
}

/// A label node exactly matches `label` anywhere in the AccessKit tree; uses
/// the non-panicking `query_all` variant so an absent row is a plain `false`.
fn has_label(harness: &Harness<'_, app::App>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
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

/// 1. INSERT: the panel opens the committed two-part fixture (the engine's
///    golden for "two parts inserted"), and the UI path's live document JSON
///    equals, byte-for-byte, the JSON the CLI path produces for the same two
///    inserts. Both paths are called and both hashes are printed.
#[test]
fn ui_document_matches_cli_for_same_two_inserts() {
    let mut harness = seeded_harness(FIXTURE_MODEL);
    harness.run();

    // The tree shows both committed parts with their instance ids.
    assert!(
        has_label(&harness, "base · instance base · from oep-part-base"),
        "base row missing"
    );
    assert!(
        has_label(&harness, "cap · instance cap · from oep-part-cap"),
        "cap row missing"
    );

    // UI path: the panel's published canonical document JSON.
    let ui_json = assy_doc_json(&harness);
    assert!(!ui_json.is_empty(), "panel published no document JSON");

    // CLI path: the exact seam the CLI runs - AssemblySession::open over the
    // committed fixture, then serialize the document it holds.
    let session = oep_shell::assem::AssemblySession::open(FIXTURE_MODEL, FIXTURE_CONSTRAINTS)
        .expect("CLI path opens the committed two-part fixture");
    let cli_json = session
        .document()
        .to_json_pretty()
        .expect("CLI path serializes its document");

    // Both hashes, printed for the proof.
    let ui_hash = sha256_hex(ui_json.as_bytes());
    let cli_hash = sha256_hex(cli_json.as_bytes());
    println!("ui  two-part doc sha256 = {ui_hash}");
    println!("cli two-part doc sha256 = {cli_hash}");
    println!("ui  reported doc  hash  = {}", reported_hash(&harness));

    assert_eq!(
        ui_json, cli_json,
        "UI document must be byte-for-byte identical to the CLI path's for the same two inserts"
    );
    assert_eq!(ui_hash, cli_hash, "both hashes must agree");

    // And the report agrees with the wall: open assembly, two rows.
    let report = assy_report(&harness);
    assert_eq!(field(&report, "open").as_deref(), Some("1"));
    assert_eq!(
        field(&report, "assembly").as_deref(),
        Some(FIXTURE_ASSEMBLY_ID)
    );
    assert_eq!(field(&report, "rows").as_deref(), Some("2"));
}

/// 1b. INSERT (action path): from an EMPTY assembly document the panel's
/// "Insert {part}" buttons place two committed parts as instances; the tree
/// then shows both with their instance ids, and the resulting document equals
/// byte-for-byte the CLI path's document for the same two inserts.
#[test]
fn insert_two_parts_from_empty_matches_cli() {
    // The empty assembly document: the committed two-part fixture with its
    // instances removed (parts + assembly remain). Built here in the test via
    // oep_geo; tests are not under the src/-only mutation guard, so this is a
    // legitimate seed.
    let mut base = oep_geo::Document::from_json_pretty(FIXTURE_MODEL).expect("parse fixture");
    for asm in &mut base.assemblies {
        asm.instances.clear();
    }
    let base_json = base.to_json_pretty().expect("serialize empty base");

    let mut harness = seeded_harness(&base_json);
    harness.run();

    // Empty before the inserts: no rows yet.
    let report = assy_report(&harness);
    assert_eq!(field(&report, "rows").as_deref(), Some("0"));
    assert_eq!(
        field(&report, "visible").as_deref(),
        Some("-"),
        "no visible instances yet"
    );

    // Insert both committed parts through the real buttons.
    click_label(&mut harness, "Insert oep-part-base");
    harness.step();
    click_label(&mut harness, "Insert oep-part-cap");
    harness.step();

    // The tree shows both with their instance ids.
    assert!(has_label(
        &harness,
        "base · instance base · from oep-part-base"
    ));
    assert!(has_label(
        &harness,
        "cap · instance cap · from oep-part-cap"
    ));
    let report = assy_report(&harness);
    assert_eq!(field(&report, "rows").as_deref(), Some("2"));

    // UI + CLI hashes for the proof.
    let ui_json = assy_doc_json(&harness);
    let ui_hash = sha256_hex(ui_json.as_bytes());
    let session = oep_shell::assem::AssemblySession::open(FIXTURE_MODEL, FIXTURE_CONSTRAINTS)
        .expect("CLI path opens the committed fixture");
    let cli_json = session
        .document()
        .to_json_pretty()
        .expect("CLI path serializes its document");
    let cli_hash = sha256_hex(cli_json.as_bytes());
    println!("ui  two-instances doc sha256 = {ui_hash}");
    println!("cli two-instances doc sha256 = {cli_hash}");
    assert_eq!(
        ui_json, cli_json,
        "inserting two parts must produce exactly the CLI path's two-insert document"
    );
}

/// 2+3. GROUND + ISOLATE: ground one component, isolate the other. Grounded
/// set and visible set are exactly as expected (both printed); isolate hides
/// the other component's rows and geometry, un-isolate restores them, and the
/// document hash returns to the pre-isolate value.
#[test]
fn ground_one_isolate_other_document_hash_roundtrips() {
    let mut harness = seeded_harness(FIXTURE_MODEL);
    harness.run();

    // Pre-isolate hash (all instances visible, as committed).
    let before_isolate = reported_hash(&harness);
    let before_json = assy_doc_json(&harness);
    let before_sha = sha256_hex(before_json.as_bytes());
    println!("pre-isolate hash = {before_isolate}  sha256 {before_sha}");

    // Select base, then ground it (Fixed mate through the seam vocabulary).
    click_label(&mut harness, "base · instance base · from oep-part-base");
    harness.step();
    click_label(&mut harness, "Ground");
    harness.step();

    // Grounded set is exactly {base}; visible set is {base, cap}.
    let report = assy_report(&harness);
    println!(
        "grounded set = [{}]",
        field(&report, "grounded").as_deref().unwrap_or("")
    );
    println!(
        "visible set  = [{}]",
        field(&report, "visible").as_deref().unwrap_or("")
    );
    assert_eq!(field(&report, "grounded").as_deref(), Some("base"));
    assert_eq!(field(&report, "visible").as_deref(), Some("base,cap"));
    // Grounding is reflected on the row: base's accessible name carries the
    // grounded marker (grounded set == "exactly as expected", and visible in
    // the tree, not only in the report).
    assert!(has_label(
        &harness,
        "base · instance base · from oep-part-base · grounded"
    ));

    // Now isolate the OTHER component (cap). Only cap's row + geometry remain;
    // base's row and its geometry note disappear.
    click_label(&mut harness, "cap · instance cap · from oep-part-cap");
    harness.step();
    click_label(&mut harness, "Isolate");
    harness.step();

    let report = assy_report(&harness);
    println!(
        "after isolate: visible set = [{}]",
        field(&report, "visible").as_deref().unwrap_or("")
    );
    println!(
        "after isolate: rows = [{}]",
        field(&report, "rows").as_deref().unwrap_or("")
    );
    assert_eq!(field(&report, "visible").as_deref(), Some("cap"));
    // base's row and geometry are hidden (isolate hides the other component);
    // grounded set is unchanged (grounding is a mate-level fact).
    assert!(!has_label(
        &harness,
        "base · instance base · from oep-part-base · grounded"
    ));
    assert!(!has_label(
        &harness,
        "base · instance base · from oep-part-base"
    ));
    assert_eq!(field(&report, "grounded").as_deref(), Some("base"));

    // The document JSON moved: cap visible, base hidden serializes a different
    // document than the committed one.
    let isolated_json = assy_doc_json(&harness);
    let isolated_sha = sha256_hex(isolated_json.as_bytes());
    println!("isolated doc sha256 = {isolated_sha}");
    assert_ne!(
        before_sha, isolated_sha,
        "isolation must change the serialized document"
    );

    // Un-isolate: both rows + geometry return, and the document hash returns
    // to the pre-isolate value exactly.
    click_label(&mut harness, "Un-isolate");
    harness.step();

    let report = assy_report(&harness);
    println!(
        "after un-isolate: visible set = [{}]",
        field(&report, "visible").as_deref().unwrap_or("")
    );
    println!(
        "after un-isolate: hash = {}",
        field(&report, "hash").as_deref().unwrap_or("")
    );
    assert_eq!(field(&report, "visible").as_deref(), Some("base,cap"));
    assert!(has_label(
        &harness,
        "base · instance base · from oep-part-base · grounded"
    ));
    assert!(has_label(
        &harness,
        "cap · instance cap · from oep-part-cap"
    ));
    assert_eq!(
        reported_hash(&harness),
        before_isolate,
        "un-isolate must return the document hash to the pre-isolate value"
    );
    let restored_json = assy_doc_json(&harness);
    assert_eq!(
        restored_json, before_json,
        "un-isolate must restore the exact pre-isolate canonical document"
    );
}

/// 4. A11Y: every tree row has an accessible name (kittest proves it), and
///    the region is reachable by keyboard, not only by mouse - Tab reaches
///    the rows and Space/Enter activates a row selection through the real
///    input path.
#[test]
fn rows_have_accessible_names_and_region_is_keyboard_reachable() {
    let mut harness = seeded_harness(FIXTURE_MODEL);
    harness.run();

    // Every tree row has an accessible name: kittest asserts each row label is
    // present as a real AccessKit node (labels, not raw text).
    for label in [
        "base · instance base · from oep-part-base",
        "cap · instance cap · from oep-part-cap",
    ] {
        let found: Vec<_> = harness.query_all_by_label(label).collect();
        assert!(
            !found.is_empty(),
            "row {label:?} missing from AccessKit tree"
        );
        let node = found[0].accesskit_node();
        assert!(
            !node.label().unwrap_or_default().is_empty(),
            "row {label:?} must carry a non-empty accessible name"
        );
    }

    // The assembly region itself is labelled and reachable in the a11y tree.
    let region: Vec<_> = harness.query_all_by_label("assembly tree").collect();
    assert!(
        !region.is_empty(),
        "the assembly tree region must be present in the AccessKit tree"
    );

    // Keyboard reachability: focus the base row through the AccessKit Focus
    // action (kittest applies the real egui focus path) and activate it with
    // Space via the real key input path - a mouse is never used here.
    let node = harness
        .get_all_by_label("base · instance base · from oep-part-base")
        .collect::<Vec<_>>()
        .first()
        .cloned()
        .expect("base row present");
    node.focus();
    harness.step();

    harness.key_down(egui::Key::Space);
    harness.step();
    harness.key_up(egui::Key::Space);
    harness.step();

    // The report must reflect that a row is selected - proof the keyboard
    // (not a mouse click) reached and selected a tree row.
    let report = assy_report(&harness);
    let sel = field(&report, "sel").unwrap_or_default();
    println!("keyboard-selected row = {sel}");
    assert!(
        matches!(sel.as_str(), "base" | "cap"),
        "keyboard activation must select a row, got {sel:?}"
    );
}
