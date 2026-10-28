//! P7-M4-T02 acceptance contract (Rust): the `oep-rules` rules/checks sandbox.
//!
//! TDD RED: written before the implementation exists. It locks the public
//! surface the card must deliver:
//!   1. a knowledge set (parameters + formulas + checks + single-pass rules)
//!      serialized in our own `oep-knowledge` v1 JSON format,
//!   2. evaluation of that set against a REAL part (`PartContext` extracted
//!      from the canonical oep-geo document, the committed bracket golden),
//!   3. a deterministic report whose bytes are locked to the committed golden
//!      `bracket-report.golden.json` (regen with OEP_RULES_REGEN=1),
//!   4. the expression engine (our own documented grammar: arithmetic,
//!      comparisons, boolean logic, functions, string constants, part member
//!      paths with feature/parameter indexes).
//!
//! Every expectation here is derived from the real bracket part data
//! (80x40x6 mm, fillet radius 2, hole diameter 6, hole depth 6) plus the
//! committed knowledge set in `tests/golden/iso-bracket.json`. No mocks.
//!
//! Run: cargo test -p oep-rules

use std::fs;
use std::path::PathBuf;

use oep_geo::Document;
use oep_rules::expr::{parse, Index, PathSeg, Value};
use oep_rules::knowledge::KnowledgeSet;
use oep_rules::part::PartContext;

fn regen() -> bool {
    std::env::var("OEP_RULES_REGEN").as_deref() == Ok("1")
}

fn repo_path(rel: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/oep-rules
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.parent().unwrap().parent().unwrap().join(rel)
}

fn load_bracket_document() -> Document {
    let text = fs::read_to_string(repo_path(
        "crates/oep-geo/tests/golden/example-bracket.oepgeo.json",
    ))
    .expect("real bracket part golden must exist");
    Document::from_json_pretty(&text).expect("bracket golden parses as oep-geo document")
}

fn load_bracket_context() -> PartContext {
    let doc = load_bracket_document();
    PartContext::from_document(&doc, Some("oep-part-bracket"))
        .expect("part context extraction for the real bracket")
}

fn load_knowledge() -> KnowledgeSet {
    let text = fs::read_to_string(repo_path("crates/oep-rules/tests/golden/iso-bracket.json"))
        .expect("committed knowledge set must exist");
    KnowledgeSet::from_json(&text).expect("knowledge set must parse and validate")
}

// ---------------------------------------------------------------------------
// 1. Expression engine contract (our own documented grammar)
// ---------------------------------------------------------------------------

#[test]
fn expr_arithmetic_precedence() {
    let lookup = |_: &[PathSeg]| -> Result<Value, oep_rules::error::RulesError> {
        unreachable!("no paths in these expressions")
    };
    let v = parse("2 + 3 * 4").unwrap().eval(&lookup).unwrap();
    assert_eq!(
        v,
        Value::from(14.0),
        "multiplication binds tighter than addition"
    );
    let v = parse("(2 + 3) * 4").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(20.0), "parens override precedence");
    let v = parse("10 - 2 - 3").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(5.0), "left associativity");
    let v = parse("-2 * 3").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(-6.0), "unary minus");
    let v = parse("7 / 2").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(3.5));
}

#[test]
fn expr_logical_and_comparisons() {
    let lookup = |_: &[PathSeg]| -> Result<Value, oep_rules::error::RulesError> {
        unreachable!("no paths in these expressions")
    };
    let v = parse("1 < 2 and 2 <= 2 and not false")
        .unwrap()
        .eval(&lookup)
        .unwrap();
    assert_eq!(v, Value::from(true), "and/not composition");
    let v = parse("3 == 3 and 4 != 5").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(true));
    let v = parse("false or true").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(true), "or");
    let v = parse("3 >= 4 or 4 >= 3").unwrap().eval(&lookup).unwrap();
    assert_eq!(v, Value::from(true));
}

#[test]
fn expr_functions_and_strings() {
    let lookup = |_: &[PathSeg]| -> Result<Value, oep_rules::error::RulesError> {
        unreachable!("no paths in these expressions")
    };
    assert_eq!(
        parse("abs(-3.5)").unwrap().eval(&lookup).unwrap(),
        Value::from(3.5)
    );
    assert_eq!(
        parse("round(2.6)").unwrap().eval(&lookup).unwrap(),
        Value::from(3.0)
    );
    assert_eq!(
        parse("floor(2.9)").unwrap().eval(&lookup).unwrap(),
        Value::from(2.0)
    );
    assert_eq!(
        parse("ceil(2.1)").unwrap().eval(&lookup).unwrap(),
        Value::from(3.0)
    );
    assert_eq!(
        parse("min(3, 1.5, 2)").unwrap().eval(&lookup).unwrap(),
        Value::from(1.5)
    );
    assert_eq!(
        parse("max(3, 1.5, 2)").unwrap().eval(&lookup).unwrap(),
        Value::from(3.0)
    );
    assert_eq!(
        parse("sqrt(16)").unwrap().eval(&lookup).unwrap(),
        Value::from(4.0)
    );
    assert_eq!(
        parse("pow(2, 10)").unwrap().eval(&lookup).unwrap(),
        Value::from(1024.0)
    );
    assert_eq!(
        parse("\"a\" + \"b\"").unwrap().eval(&lookup).unwrap(),
        Value::from("ab")
    );
}

#[test]
fn expr_part_path_indexing() {
    // part.features["oep-feature-bracket-hole"].parameters["diameter"] = 6.0
    let path = r#"part.features["oep-feature-bracket-hole"].parameters["diameter"]"#;
    let e = parse(path).unwrap();
    let got = e
        .eval(&|segs: &[PathSeg]| {
            assert_eq!(segs.len(), 5, "part / features / [id] / parameters / [key]");
            match &segs[0] {
                PathSeg::Field(s) => assert_eq!(s, "part"),
                other => panic!("unexpected leading seg: {other:?}"),
            }
            assert!(
                matches!(&segs[2], PathSeg::Index(Index::Str(id)) if id == "oep-feature-bracket-hole")
            );
            assert!(
                matches!(&segs[3], PathSeg::Field(f) if f == "parameters")
            );
            assert!(matches!(&segs[4], PathSeg::Index(Index::Str(k)) if k == "diameter"));
            Ok(Value::from(6.0))
        })
        .unwrap();
    assert_eq!(got, Value::from(6.0));
}

#[test]
fn expr_errors() {
    let lookup = |_: &[PathSeg]| -> Result<Value, oep_rules::error::RulesError> {
        unreachable!("never reached")
    };
    assert!(parse("1 / 0").is_ok(), "parse is structural only");
    assert!(parse("2 *").is_err(), "trailing operator is a parse error");
    assert!(
        parse("(1 + 2").is_err(),
        "unbalanced paren is a parse error"
    );
    let e = parse("1 / 0").unwrap();
    assert!(
        e.eval(&lookup).is_err(),
        "division by zero is a typed eval error"
    );
    let e = parse("sqrt(-1)").unwrap();
    assert!(
        e.eval(&lookup).is_err(),
        "sqrt of negative is a typed error"
    );
    let e = parse("abs(\"x\")").unwrap();
    assert!(e.eval(&lookup).is_err(), "abs of a string is a typed error");
    let e = parse("true - 1").unwrap();
    assert!(
        e.eval(&lookup).is_err(),
        "bool minus number is a typed error"
    );
}

// ---------------------------------------------------------------------------
// 2. Knowledge set contract on the REAL bracket part
// ---------------------------------------------------------------------------

#[test]
fn knowledge_set_parses() {
    let _ks = load_knowledge();
}

#[test]
fn part_context_reads_real_part() {
    let ctx = load_bracket_context();
    assert_eq!(ctx.id, "oep-part-bracket");
    assert_eq!(ctx.name, "bracket");
    assert_eq!(ctx.part_number.as_deref(), Some("ISO-BKT-001"));
    assert_eq!(ctx.bbox_min, [0.0, 0.0, 0.0]);
    assert_eq!(ctx.bbox_max, [80.0, 40.0, 6.0]);
    let hole = ctx
        .features
        .get("oep-feature-bracket-hole")
        .expect("hole feature present on the real part");
    assert_eq!(
        hole.get("diameter"),
        Some(&Value::from(6.0)),
        "real mount hole diameter"
    );
    assert_eq!(
        hole.get("depth"),
        Some(&Value::from(6.0)),
        "real mount hole depth"
    );
}

#[test]
fn rules_demo_on_real_part() {
    let ctx = load_bracket_context();
    let ks = load_knowledge();
    let report = ks
        .evaluate(&ctx)
        .expect("evaluation succeeds on the real part");

    // --- part identity (the "real part" half of the acceptance) ---
    assert_eq!(report.part.id, "oep-part-bracket");
    assert_eq!(report.part.name, "bracket");

    // --- formulas resolved from real part data ---
    let p = |name: &str| report.value_of(name).expect("parameter present");
    assert_eq!(p("sheet_thickness"), &Value::from(6.0));
    assert_eq!(p("sheet_area"), &Value::from(3200.0), "80 x 40 envelope");
    assert_eq!(p("edge_fillet"), &Value::from(2.0));
    assert_eq!(p("hole_diameter"), &Value::from(6.0));
    assert_eq!(p("hole_depth"), &Value::from(6.0));

    // --- rules applied (single-pass, deterministic) ---
    assert_eq!(
        p("safe_factor"),
        &Value::from(1.5),
        "thick-sheet rule tightens the factor"
    );
    assert_eq!(p("max_safe_hole"), &Value::from(9.0), "1.5 x 6 mm");
    assert_eq!(
        p("deburr_status"),
        &Value::from("ACT"),
        "deburr escalation rule fires"
    );

    // --- sources recorded for provenance ---
    assert_eq!(report.source_of("safe_factor"), Some("rule"));
    assert_eq!(report.source_of("sheet_thickness"), Some("formula"));
    assert_eq!(report.source_of("material"), Some("literal"));

    // --- checks: 4 pass, 1 fail, deterministic messages ---
    let outcome = |name: &str| report.check(name).expect("check present");
    assert!(outcome("sheet_min_thickness").passed);
    assert!(outcome("sheet_max_thickness").passed);
    assert!(
        outcome("mount_hole_safe").passed,
        "6 <= 9 after the tightened rule"
    );
    assert!(outcome("through_hole").passed);
    let deburr = outcome("edge_fillet_deburr");
    assert!(
        !deburr.passed,
        "real part fillet 2.0 < required 3.0 fails this check"
    );
    assert!(
        deburr.message.contains("2"),
        "message carries the real fillet radius"
    );

    // --- rule trace ---
    let rule = report.rule("auto_deburr_escalate").expect("rule traced");
    assert_eq!(rule.branch, "then");
    let set = rule
        .actions
        .iter()
        .find(|a| a.parameter.as_deref() == Some("deburr_status"));
    assert_eq!(set.map(|a| a.value.clone()), Some(Value::from("ACT")));

    let tight = report
        .rule("thick_sheet_uses_tight_factor")
        .expect("rule traced");
    assert_eq!(tight.branch, "then");
    let f = tight
        .actions
        .iter()
        .find(|a| a.parameter.as_deref() == Some("safe_factor"));
    assert_eq!(f.map(|a| a.value.clone()), Some(Value::from(1.5)));

    // --- deterministic report locked to the committed golden ---
    let golden = repo_path("crates/oep-rules/tests/golden/bracket-report.golden.json");
    let report_text = report.to_json_pretty().expect("report serializes");
    if regen() {
        // Keep the golden authoritative: regen explicitly with OEP_RULES_REGEN=1.
        println!("OEP_RULES_REGEN=1: rewriting {golden:?}");
        fs::write(&golden, report_text.as_bytes()).expect("write golden");
    } else {
        let expected = fs::read_to_string(&golden)
            .expect("committed report golden must exist (regen with OEP_RULES_REGEN=1)");
        assert_eq!(
            report_text, expected,
            "report bytes drifted from the committed golden: run with OEP_RULES_REGEN=1 "
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Error discipline: cycles and unknown references are typed, not panics
// ---------------------------------------------------------------------------

#[test]
fn knowledge_set_detects_cycles() {
    let ks = KnowledgeSet::from_json(
        r#"{
            "format": "oep-knowledge",
            "version": 1,
            "units": "mm",
            "parameters": [
                {"name": "a", "type": "formula", "expr": "b + 1"},
                {"name": "b", "type": "formula", "expr": "a + 1"}
            ],
            "checks": [],
            "rules": []
        }"#,
    )
    .expect("parses");
    let ctx = load_bracket_context();
    let err = ks
        .evaluate(&ctx)
        .expect_err("cyclic formulas are a typed error");
    assert!(
        err.to_string().contains("cyclic"),
        "message names the cycle: {err}"
    );
}

#[test]
fn knowledge_set_rejects_duplicate_names() {
    let err = KnowledgeSet::from_json(
        r#"{
            "format": "oep-knowledge",
            "version": 1,
            "units": "mm",
            "parameters": [
                {"name": "a", "type": "number", "value": 1},
                {"name": "a", "type": "number", "value": 2}
            ],
            "checks": [],
            "rules": []
        }"#,
    )
    .expect_err("duplicate parameter names are rejected at load");
    assert!(
        err.to_string().contains("duplicate"),
        "message names the problem: {err}"
    );
}
