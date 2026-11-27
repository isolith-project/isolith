//! UI-E4 UI-ASSY-EXPLODE: exploded view control.
//!
//! Drives the REAL `app::App` (the same `crates/oep-app` source shipped in the
//! `isolith` binary, pulled in verbatim via `#[path]` like every other test
//! target here) through `egui_kittest` and proves the acceptance exactly as
//! the card reads it, with real numbers read back from the running panel:
//!
//! 1. **Factor 0 leaves every transform byte-identical to the unexploded
//!    document.** Exploding to factor 1 and back to factor 0 returns the
//!    document to EXACTLY the unexploded bytes (document hash printed, JSON
//!    compared byte-for-byte, per-component transform hashes printed).
//! 2. **Factor 1 and factor 2 move each component along its own direction by
//!    exactly factor times its committed offset.** The slider is driven
//!    through the real input path (AccessKit SetValue), and each component's
//!    before/after position AND the offset used are printed; the actual
//!    document placement equals `before + factor * offset` for every
//!    component, at factor 1 and at factor 2.
//! 3. **A per-component override moves only that component.** Overriding one
//!    component's offset through its DragValue changes only that component;
//!    the other components' transform hashes are byte-identical before and
//!    after (printed).
//! 4. **The exploded state round-trips: save, reopen, and the exploded
//!    positions and factors are byte-identical.** The full saved payload (the
//!    canonical document JSON plus the embedded `explode` state) is written to
//!    disk and reopened in a fresh harness seeded with it; the reopened saved
//!    payload and the reopened positions/factors are byte-identical to the
//!    saved side (both JSON hashes printed), and the saved payload is proven
//!    to still open as a canonical oep-geo document.
//!
//! The explode panel publishes a plain-`String` oracle (`factor`, per
//! component `pos_`/`exp_`/`off_`/`before_`/`thash_`, the document `hash`, the
//! saved-payload `saved_hash`, `err`), the full saved payload under
//! `EXPLODE_SAVED_JSON_KEY` and the encoded explode state under
//! `EXPLODE_STATE_JSON_KEY`, so every clause below is proven with numbers read
//! back from the running panel (positions come from the live document, the
//! source of truth, never recomputed in the harness).

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

/// The explode panel's `String` report oracle key (must match
/// `assy_explode.rs`).
const EXPLODE_REPORT_KEY: &str = "ui_e4_explode_report";
/// The explode panel's published full saved `.oepdoc` payload key.
const EXPLODE_SAVED_JSON_KEY: &str = "ui_e4_explode_saved_json";
/// The explode panel's published encoded explode state key.
const EXPLODE_STATE_JSON_KEY: &str = "ui_e4_explode_state_json";
/// The tree's assembly seed key (must match `assy_tree.rs`).
const ASSY_SEED_KEY: &str = "ui_e1_assy_seed";
/// The tree's live canonical document JSON key (must match `assy_tree.rs`).
const ASSY_DOC_JSON_KEY: &str = "ui_e1_assy_doc_json";

/// The committed two-part fixture (base + cap): base at identity, cap placed
/// at [28,14,20] with a rotation - a real unexploded assembly.
const TWO_PART_FIXTURE: &str =
    include_str!("../../oep-assem/tests/golden/two-part-fixture.oepgeo.json");

/// A TALL harness window (same as UI-E2/UI-E3): the assembly region then has
/// rows of room, so the tree, diagnostics, explode panel AND mate tool are all
/// fully visible and every widget is clickable / driveable through the real
/// input path.
fn seeded_doc_harness(json: &str) -> Harness<'static, app::App> {
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

/// The explode panel's `String` report oracle.
fn explode_report(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(EXPLODE_REPORT_KEY)))
        .unwrap_or_default()
}

/// The full saved `.oepdoc` payload the panel publishes this frame.
fn saved_payload(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(EXPLODE_SAVED_JSON_KEY)))
        .unwrap_or_default()
}

/// The encoded explode state the panel publishes this frame.
fn explode_state(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(EXPLODE_STATE_JSON_KEY)))
        .unwrap_or_default()
}

/// The live canonical document JSON the tree publishes this frame.
fn doc_json(harness: &Harness<'_, app::App>) -> String {
    harness
        .ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_DOC_JSON_KEY)))
        .unwrap_or_default()
}

/// One `key=value` field of a report oracle line.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The (x,y,z) translation triplet from a `x,y,z` report field.
fn triplet(field: &str) -> [f64; 3] {
    let mut it = field.split(',');
    let x = it.next().unwrap().parse().unwrap();
    let y = it.next().unwrap().parse().unwrap();
    let z = it.next().unwrap().parse().unwrap();
    [x, y, z]
}

/// Drive the explode slider to an exact factor through AccessKit SetValue
/// (the "reachable without a mouse" input path), then run two frames so the
/// journal write lands and the panel republishes.
fn set_factor(harness: &mut Harness<'_, app::App>, factor: f64) {
    harness
        .get_by_label("explode factor")
        .set_value_accesskit(factor);
    harness.step();
    harness.step();
}

/// FNV-1a-64 over a string - the same one-letter hashing language the panel
/// and the whole wave use for the canonical bytes, so the "both JSON hashes"
/// printed here are comparable to the panel's `saved_hash`.
fn text_hash(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &byte in text.as_bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// 1. FACTOR 0 IS BYTE-IDENTICAL: explode to factor 1 (document moves) and
///    back to factor 0 (document returns to the unexploded bytes). The
///    document hash, the full document JSON and every per-component transform
///    hash are printed and asserted identical to the unexploded side.
#[test]
fn factor_zero_returns_every_transform_byte_identical() {
    let mut harness = seeded_doc_harness(TWO_PART_FIXTURE);
    harness.run();

    // The unexploded baseline (the document exactly as the fixture seeds it).
    let baseline_json = doc_json(&harness);
    let baseline_hash = field(&explode_report(&harness), "hash")
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let base_th0 = field(&explode_report(&harness), "thash_base").unwrap();
    let cap_th0 = field(&explode_report(&harness), "thash_cap").unwrap();
    println!("unexploded doc hash = {baseline_hash}");
    println!("unexploded base thash = {base_th0}  cap thash = {cap_th0}");

    // Explode to factor 1: the document must move (hash differs).
    set_factor(&mut harness, 1.0);
    let exploded_hash = field(&explode_report(&harness), "hash")
        .unwrap()
        .parse::<u64>()
        .unwrap();
    println!("factor 1 doc hash     = {exploded_hash}");
    assert_ne!(
        baseline_hash, exploded_hash,
        "explode factor 1 must move the document"
    );

    // Back to factor 0: byte-identical to the unexploded side.
    set_factor(&mut harness, 0.0);
    let restored_hash = field(&explode_report(&harness), "hash")
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let restored_json = doc_json(&harness);
    let base_th1 = field(&explode_report(&harness), "thash_base").unwrap();
    let cap_th1 = field(&explode_report(&harness), "thash_cap").unwrap();
    println!("factor 0 doc hash     = {restored_hash}");
    println!("factor 0 base thash = {base_th1}  cap thash = {cap_th1}");
    println!(
        "unexploded JSON == factor-0 JSON : {}",
        restored_json == baseline_json
    );

    assert_eq!(
        restored_hash, baseline_hash,
        "factor 0 must hash to the unexploded document hash"
    );
    assert_eq!(
        restored_json, baseline_json,
        "factor 0 must return the document bytes byte-identically"
    );
    assert_eq!(
        (base_th1.as_str(), cap_th1.as_str()),
        (base_th0.as_str(), cap_th0.as_str()),
        "every per-component transform hash must return byte-identical"
    );
}

/// 2. FACTOR 1 AND FACTOR 2: each component moves along its own direction by
///    exactly factor times its committed offset. The actual document placement
///    is printed and asserted to equal `before + factor * offset` for every
///    component, at both factors, with the offset used printed alongside.
#[test]
fn factor_one_and_two_move_each_component_by_factor_times_its_offset() {
    let mut harness = seeded_doc_harness(TWO_PART_FIXTURE);
    harness.run();

    let report = explode_report(&harness);
    let before_base = triplet(&field(&report, "before_base").unwrap());
    let before_cap = triplet(&field(&report, "before_cap").unwrap());
    let off_base = triplet(&field(&report, "off_base").unwrap());
    let off_cap = triplet(&field(&report, "off_cap").unwrap());
    let ids = ["base", "cap"];

    for (factor, label) in [(1.0_f64, "factor 1"), (2.0_f64, "factor 2")] {
        set_factor(&mut harness, factor);
        let report = explode_report(&harness);
        println!("{label}:");
        for id in ids {
            let pos = triplet(&field(&report, &format!("pos_{id}")).unwrap());
            let exp = triplet(&field(&report, &format!("exp_{id}")).unwrap());
            let before = if id == "base" {
                before_base
            } else {
                before_cap
            };
            let off = if id == "base" { off_base } else { off_cap };
            let expect = [
                before[0] + factor * off[0],
                before[1] + factor * off[1],
                before[2] + factor * off[2],
            ];
            let moved = [pos[0] - before[0], pos[1] - before[1], pos[2] - before[2]];
            println!(
                "  {id}: before ({:.6},{:.6},{:.6}) after ({:.6},{:.6},{:.6}) moved ({:.6},{:.6},{:.6}) offset ({:.3},{:.3},{:.3})",
                before[0], before[1], before[2],
                pos[0], pos[1], pos[2],
                moved[0], moved[1], moved[2],
                off[0], off[1], off[2],
            );
            for i in 0..3 {
                assert!(
                    (exp[i] - pos[i]).abs() < 1e-9,
                    "{label} {id}[{i}]: published pos must equal before + factor*offset"
                );
                assert!(
                    (expect[i] - pos[i]).abs() < 1e-9,
                    "{label} {id}[{i}]: pos {pos:?} must equal before + factor*offset {expect:?}"
                );
            }
        }
        let factor_line = field(&report, "factor").unwrap();
        assert_eq!(factor_line.parse::<f64>().unwrap(), factor);
    }
}

/// 3. PER-COMPONENT OVERRIDE MOVES ONLY THAT COMPONENT: a DragValue override of
///    one component's offset changes exactly that component's placement; the
///    other component's document placement and transform hash stay
///    byte-identical (printed).
#[test]
fn per_component_override_moves_only_that_component() {
    let mut harness = seeded_doc_harness(TWO_PART_FIXTURE);
    harness.run();
    set_factor(&mut harness, 1.0);

    let before = explode_report(&harness);
    let base_before = triplet(&field(&before, "pos_base").unwrap());
    let cap_before = triplet(&field(&before, "pos_cap").unwrap());
    let base_th_before = field(&before, "thash_base").unwrap();
    let base_th_cap_before = field(&before, "thash_cap").unwrap();
    let base_hash_before = field(&before, "hash").unwrap();

    // Override cap's x-offset through its DragValue (real input path): cap's
    // own direction gains 40 mm in x, so ONLY cap moves.
    harness
        .get_by_label("explode offset cap 0")
        .set_value_accesskit(40.0);
    harness.step();
    harness.step();

    let after = explode_report(&harness);
    let base_after = triplet(&field(&after, "pos_base").unwrap());
    let cap_after = triplet(&field(&after, "pos_cap").unwrap());
    let base_th_after = field(&after, "thash_base").unwrap();
    let cap_th_after = field(&after, "thash_cap").unwrap();
    let base_hash_after = field(&after, "hash").unwrap();

    println!("override cap offset.x = 40 (factor 1):");
    println!(
        "  base before ({:.3},{:.3},{:.3}) after ({:.3},{:.3},{:.3})  moved exactly? {}",
        base_before[0],
        base_before[1],
        base_before[2],
        base_after[0],
        base_after[1],
        base_after[2],
        base_before == base_after
    );
    println!(
        "  cap  before ({:.3},{:.3},{:.3}) after ({:.3},{:.3},{:.3})  x moved by {:.3}",
        cap_before[0],
        cap_before[1],
        cap_before[2],
        cap_after[0],
        cap_after[1],
        cap_after[2],
        cap_after[0] - cap_before[0]
    );
    println!("  base transform hash unchanged?  {base_th_before} -> {base_th_after}");
    println!("  cap  transform hash changed?    {base_th_cap_before} -> {cap_th_after}");
    println!("  doc  hash                       {base_hash_before} -> {base_hash_after}");

    assert_eq!(base_before, base_after, "the OTHER component must not move");
    assert_eq!(
        base_th_before, base_th_after,
        "the other component's transform hash must be byte-identical"
    );
    assert_ne!(
        base_th_cap_before, cap_th_after,
        "the overridden component moves"
    );
    // cap's committed base x is 28 (fixture); the override sets its own x
    // offset to 40; at factor 1 the placement is exactly base + 1 * offset.
    let base_cap_x = triplet(&field(&before, "before_cap").unwrap())[0];
    assert!(
        (cap_after[0] - (base_cap_x + 40.0)).abs() < 1e-9,
        "cap must sit exactly at base + 1 * overridden offset x=40, got {}",
        cap_after[0]
    );
    // Both components moved by factor 1 times their own offsets before the
    // override; cap's override adds exactly the delta, base untouched.
    assert_ne!(
        base_hash_before, base_hash_after,
        "the document changed (cap moved)"
    );
}

/// 4. ROUND-TRIP: save the exploded document (canonical JSON + embedded
///    explode state) to disk, reopen it in a fresh harness seeded with the
///    saved payload, and confirm the exploded positions and factors are
///    byte-identical - both JSON hashes printed, and the save payload proven
///    to still open as a canonical oep-geo document.
#[test]
fn exploded_state_round_trips_save_and_reopen() {
    use std::io::Write;

    // Build an exploded document: factor 2 plus a per-component override, so
    // the saved state carries both the global factor and a committed offset.
    let mut harness_a = seeded_doc_harness(TWO_PART_FIXTURE);
    harness_a.run();
    set_factor(&mut harness_a, 1.0);
    harness_a
        .get_by_label("explode offset cap 0")
        .set_value_accesskit(30.0);
    harness_a.step();
    harness_a.step();
    set_factor(&mut harness_a, 2.0);

    let saved_a = saved_payload(&harness_a);
    let state_a = explode_state(&harness_a);
    let report_a = explode_report(&harness_a);
    let hash_a = text_hash(&saved_a);
    println!("saved payload hash (factor 2 + cap override) = {hash_a}");
    println!("saved encode       = {state_a}");
    println!(
        "factor = {}  base pos = {}  cap pos = {}",
        field(&report_a, "factor").unwrap(),
        field(&report_a, "pos_base").unwrap(),
        field(&report_a, "pos_cap").unwrap(),
    );

    // Prove the saved payload is still a canonical oep-geo document (the
    // embedded explode member is ignored, never corrupting the document).
    let parsed = oep_geo::Document::from_json_pretty(&saved_a)
        .expect("the saved payload must still open as a canonical oep-geo document");
    let canonical = parsed.to_json_pretty().expect("serialize");
    assert!(
        canonical.contains("\"format\": \"oep-geo\""),
        "reopened payload is still an oep-geo document"
    );

    // Write to disk and read back - the literal save/reopen you would do.
    let dir = std::env::temp_dir().join(format!("isolith-e4-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("exploded.oepdoc");
    let mut f = std::fs::File::create(&path).expect("create saved file");
    f.write_all(saved_a.as_bytes()).expect("write saved file");
    drop(f);
    let saved_from_disk = std::fs::read_to_string(&path).expect("read saved file");

    // Reopen in a completely fresh app seeded with the saved file exactly as
    // the app opens a saved document.
    let mut harness_b = seeded_doc_harness(&saved_from_disk);
    harness_b.run();

    let saved_b = saved_payload(&harness_b);
    let state_b = explode_state(&harness_b);
    let report_b = explode_report(&harness_b);
    let hash_b = text_hash(&saved_b);
    println!("reopened payload hash = {hash_b}");
    println!("reopened encode      = {state_b}");
    println!(
        "saved payload == reopened payload (byte-identical): {}",
        saved_a == saved_b
    );
    println!(
        "factor reopened = {}  base pos = {}  cap pos = {}",
        field(&report_b, "factor").unwrap(),
        field(&report_b, "pos_base").unwrap(),
        field(&report_b, "pos_cap").unwrap(),
    );

    // Both JSON hashes byte-identical, positions byte-identical, factor intact.
    assert_eq!(
        hash_a, hash_b,
        "saved and reopened saved-payload hashes must match"
    );
    assert_eq!(
        saved_a, saved_b,
        "the saved payload must round-trip byte-identically"
    );
    assert_eq!(
        state_a, state_b,
        "the encoded explode state must round-trip byte-identically"
    );
    assert_eq!(
        doc_json(&harness_a),
        doc_json(&harness_b),
        "the reopened document must be byte-identical to the saved document"
    );
    assert_eq!(
        field(&report_a, "factor"),
        field(&report_b, "factor"),
        "the reopened factor must equal the saved factor"
    );
    assert_eq!(
        field(&report_a, "pos_base"),
        field(&report_b, "pos_base"),
        "base exploded position must round-trip byte-identically"
    );
    assert_eq!(
        field(&report_a, "pos_cap"),
        field(&report_b, "pos_cap"),
        "cap exploded position must round-trip byte-identically"
    );

    let _ = parsed;
}
