"""P4-M5-T01: MBD validator service - acceptance + per-rule unit tests.

TDD direction: this file is written FIRST (RED). It names the public surface
of python/oep_tooling/mbd_validate.py (rule engine, rule sets, loaders, report,
thin runner) and pins every acceptance criterion in the task card:

  - rule engine plus service checks corpus fixtures
  - each rule unit-tested
  - report output honest (UNVERIFIED stays UNVERIFIED, never PASS; no
    fabricated FAILs; rollup arithmetic exact)
  - designed to be demoable to defense primes (committed report + one command)
"""

import json
import os
import sys

import pytest

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
# oep_tooling is a package under python/ (decision 6 tooling location).
sys.path.insert(0, os.path.join(REPO, "python"))

import oep_tooling.mbd_validate as mbd  # noqa: E402  (import-time TDD gate)

FIXTURES = os.path.join(REPO, "corpus", "mbd-fixtures")
MANIFEST = os.path.join(REPO, "corpus", "mbd-fixture-manifest.json")

# --------------------------------------------------------------------------
# Helpers - build valid oep-pmi documents / input bundles quickly.
# --------------------------------------------------------------------------
GOLDEN_BRACKET = {
    "format": "oep-pmi",
    "version": 1,
    "units": "mm",
    "datums": [
        {"id": "datum_A", "label": "A", "feature_ids": ["feat-extrude"], "body_ids": []},
        {"id": "datum_B", "label": "B", "feature_ids": ["feat-hole"], "body_ids": []},
        {"id": "datum_C", "label": "C", "feature_ids": ["feat-hole"], "body_ids": []},
    ],
    "frames": [
        {
            "id": "drf_abc",
            "name": "A-B-C",
            "refs": [
                {"datum_id": "datum_A", "precedence": 1, "modifier": "rfs"},
                {"datum_id": "datum_B", "precedence": 2, "modifier": "rfs"},
                {"datum_id": "datum_C", "precedence": 3},
            ],
        }
    ],
    "tolerances": [
        {
            "id": "gt_1",
            "characteristic": "position",
            "value": 0.2,
            "diameter_zone": True,
            "material_condition": "mmc",
            "frame_id": "drf_abc",
            "feature_ids": ["feat-hole"],
            "body_ids": [],
        },
        {
            "id": "gt_flat",
            "characteristic": "flatness",
            "value": 0.05,
            "diameter_zone": False,
            "feature_ids": ["feat-extrude"],
            "body_ids": [],
        },
    ],
    "notes": [],
}


def pmi(**over):
    """A valid oep-pmi document dict (golden bracket base), with overrides."""
    doc = json.loads(json.dumps(GOLDEN_BRACKET))
    doc.update(over)
    return doc


def model(units="mm", feature_ids=("feat-extrude", "feat-hole"), body_ids=()):
    return {"units": units, "feature_ids": list(feature_ids), "body_ids": list(body_ids)}


def bundle(pmi_doc=None, pmi_extracted=None, model_facts=None, model_present=None, **src):
    b = {
        "schema_version": 1,
        "source": {"origin": "test", "file": "inline", **src},
    }
    if pmi_doc is not None:
        b["pmi"] = pmi_doc
        b["pmi_extracted"] = pmi_extracted
    if model_facts is not None:
        b["model"] = model_facts
        b["model_present"] = model_present
    return b


def one(findings, rule_id):
    """The single finding for a rule (one finding per rule per input)."""
    matches = [f for f in findings if f["rule"] == rule_id]
    assert len(matches) == 1, f"rule {rule_id}: expected 1 finding, got {len(matches)}"
    return matches[0]


STAT = {"pass", "fail", "warn", "unverified"}


# --------------------------------------------------------------------------
# 1. Module surface + rule catalog integrity
# --------------------------------------------------------------------------
def test_module_exposes_public_surface():
    assert callable(mbd.check_input)
    assert callable(mbd.load_input)
    assert callable(mbd.run_manifest)
    assert callable(mbd.build_report)
    assert callable(mbd.render_markdown)
    assert callable(mbd.RULE_SETS.__contains__)
    assert "y4141" in mbd.RULE_SETS and "prime" in mbd.RULE_SETS


def test_rule_catalog_integrity():
    rule_ids = set()
    for r in mbd.RULE_CATALOG:
        for key in ("id", "name", "category", "sets", "severity", "requirement"):
            assert key in r, f"rule {r.get('id')} missing {key}"
        assert r["severity"] in ("error", "warning", "info"), r["id"]
        assert set(r.get("sets", [])) <= set(mbd.RULE_SETS), r["id"]
        rule_ids.add(r["id"])
    # ids unique
    assert len(rule_ids) == len(mbd.RULE_CATALOG)
    # every rule id has a working pure check function
    for r in mbd.RULE_CATALOG:
        fn = mbd.RULES.get(r["id"])
        assert callable(fn), f"no check function for rule {r['id']}"
    # every rule lives in at least y4141
    for r in mbd.RULE_CATALOG:
        assert "y4141" in r["sets"], f"rule {r['id']} not in y4141 set"


def test_rule_set_shape():
    for sid, s in mbd.RULE_SETS.items():
        assert s["name"] and s["description"] and s["basis"]
        for rid in s.get("rules", []):
            assert rid in mbd.RULES, f"set {sid} names unknown rule {rid}"


# --------------------------------------------------------------------------
# 2. Per-rule unit tests (one test per rule id)
# --------------------------------------------------------------------------
def test_r_s01_pmi_format():
    good = bundle(pmi_doc=pmi())
    assert one(mbd.check_input(good, "y4141")["findings"], "s01-pmi-format")["status"] == "pass"
    bad = bundle(pmi_doc=pmi(format="not-oep-pmi"))
    f = one(mbd.check_input(bad, "y4141")["findings"], "s01-pmi-format")
    assert f["status"] == "fail" and "not-oep-pmi" in f["evidence"]


def test_r_s02_pmi_version():
    good = bundle(pmi_doc=pmi())
    assert one(mbd.check_input(good, "y4141")["findings"], "s02-pmi-version")["status"] == "pass"
    bad = bundle(pmi_doc=pmi(version=99))
    assert one(mbd.check_input(bad, "y4141")["findings"], "s02-pmi-version")["status"] == "fail"


def test_r_s03_unique_ids():
    good = bundle(pmi_doc=pmi())
    assert one(mbd.check_input(good, "y4141")["findings"], "s03-unique-ids")["status"] == "pass"
    dup = pmi(notes=[{"id": "datum_A", "text": "dup id across classes"}])
    f = one(mbd.check_input(bundle(pmi_doc=dup), "y4141")["findings"], "s03-unique-ids")
    assert f["status"] == "fail" and "datum_A" in f["evidence"]


def test_r_s04_unique_labels():
    # duplicate label between two datums
    two = pmi(datums=[
        {"id": "datum_X", "label": "A", "feature_ids": [], "body_ids": []},
        {"id": "datum_Y", "label": "A", "feature_ids": [], "body_ids": []},
    ], frames=[], tolerances=[])
    f = one(mbd.check_input(bundle(pmi_doc=two), "y4141")["findings"], "s04-unique-labels")
    assert f["status"] == "fail" and "A" in f["evidence"]
    # blank label
    blank = pmi(datums=[{"id": "datum_X", "label": "  ", "feature_ids": [], "body_ids": []}],
                frames=[], tolerances=[])
    assert one(mbd.check_input(bundle(pmi_doc=blank), "y4141")["findings"],
               "s04-unique-labels")["status"] == "fail"


def test_r_s05_refs_unique():
    dup_ref = pmi(tolerances=[{
        "id": "gt_a", "characteristic": "flatness", "value": 0.1,
        "feature_ids": ["feat-a", "feat-a"], "body_ids": [],
    }])
    f = one(mbd.check_input(bundle(pmi_doc=dup_ref), "y4141")["findings"], "s05-refs-unique")
    assert f["status"] == "fail" and "feat-a" in f["evidence"]


def test_r_s06_datum_frame_resolution():
    bad_frame = pmi(frames=[{
        "id": "drf_z", "name": "Z",
        "refs": [
            {"datum_id": "datum_NOPE", "precedence": 1},
            {"datum_id": "datum_A", "precedence": 0},
            {"datum_id": "datum_B", "precedence": 2},
            {"datum_id": "datum_C", "precedence": 2},
        ],
    }])
    f = one(mbd.check_input(bundle(pmi_doc=bad_frame), "y4141")["findings"],
            "s06-datum-frame-resolution")
    assert f["status"] == "fail"
    # evidence carries each class of violation
    assert "NOPE" in f["evidence"] and "precedence" in f["evidence"]


def test_r_s07_tolerance_frame_resolution():
    bad_tol = pmi(tolerances=[{
        "id": "gt_x", "characteristic": "position", "value": 0.1,
        "frame_id": "drf_missing", "feature_ids": ["feat-hole"], "body_ids": [],
    }])
    f = one(mbd.check_input(bundle(pmi_doc=bad_tol), "y4141")["findings"],
            "s07-tolerance-frame-resolution")
    assert f["status"] == "fail" and "drf_missing" in f["evidence"]


def test_r_s08_tolerance_values():
    bad_vals = pmi(tolerances=[
        {"id": "gt_p", "characteristic": "position", "value": -0.1,
         "feature_ids": ["feat-a"], "body_ids": []},
        {"id": "gt_q", "characteristic": "flatness", "value": float("nan"),
         "feature_ids": ["feat-a"], "body_ids": []},
        {"id": "gt_r", "characteristic": "profile-of-surface", "value": 0.1,
         "second_value": -2.0, "feature_ids": ["feat-a"], "body_ids": []},
    ])
    f = one(mbd.check_input(bundle(pmi_doc=bad_vals), "y4141")["findings"],
            "s08-tolerance-values")
    assert f["status"] == "fail"


def test_r_s09_note_position():
    bad_pos = pmi(notes=[{"id": "n1", "text": "TEXT", "feature_ids": [],
                          "body_ids": [], "position": [1.0, 2.0]}])  # len 2
    f = one(mbd.check_input(bundle(pmi_doc=bad_pos), "y4141")["findings"],
            "s09-note-position")
    assert f["status"] == "fail"


def test_r_y01_pmi_present():
    # declared-complete extraction with zero annotations => FAIL (missing PMI)
    empty = pmi(datums=[], frames=[], tolerances=[], notes=[])
    f = one(mbd.check_input(bundle(pmi_doc=empty, pmi_extracted=True), "y4141")["findings"],
            "y01-pmi-present")
    assert f["status"] == "fail" and "missing" in f["evidence"].lower()
    # unknown extraction status => honest UNVERIFIED, never FAIL
    f2 = one(mbd.check_input(bundle(pmi_doc=empty, pmi_extracted=None), "y4141")["findings"],
             "y01-pmi-present")
    assert f2["status"] == "unverified"
    # non-empty => PASS
    f3 = one(mbd.check_input(bundle(pmi_doc=pmi(), pmi_extracted=True), "y4141")["findings"],
             "y01-pmi-present")
    assert f3["status"] == "pass"


def test_r_y02_datum_anchored():
    floating = pmi(datums=[{"id": "datum_A", "label": "A", "feature_ids": [], "body_ids": []}],
                   frames=[], tolerances=[])
    f = one(mbd.check_input(bundle(pmi_doc=floating), "y4141")["findings"],
            "y02-datum-anchored")
    assert f["status"] == "fail" and "datum_A" in f["evidence"]
    assert one(mbd.check_input(bundle(pmi_doc=pmi()), "y4141")["findings"],
               "y02-datum-anchored")["status"] == "pass"


def test_r_y03_tolerance_anchored():
    floating = pmi(tolerances=[{"id": "gt_t", "characteristic": "flatness", "value": 0.1,
                                "feature_ids": [], "body_ids": []}])
    f = one(mbd.check_input(bundle(pmi_doc=floating), "y4141")["findings"],
            "y03-tolerance-anchored")
    assert f["status"] == "fail" and "gt_t" in f["evidence"]


def test_r_y04_datum_used():
    dangling = pmi(datums=[{"id": "datum_A", "label": "A", "feature_ids": ["feat-x"],
                            "body_ids": []}, {"id": "datum_B", "label": "B",
                            "feature_ids": ["feat-x"], "body_ids": []}],
                   frames=[{"id": "drf_a", "name": "A",
                            "refs": [{"datum_id": "datum_A", "precedence": 1}]}],
                   tolerances=[{"id": "gt_u", "characteristic": "parallelism", "value": 0.1,
                                "frame_id": "drf_a", "feature_ids": ["feat-x"],
                                "body_ids": []}])
    f = one(mbd.check_input(bundle(pmi_doc=dangling), "y4141")["findings"],
            "y04-datum-used")
    assert f["status"] == "fail" and "datum_B" in f["evidence"]
    assert one(mbd.check_input(bundle(pmi_doc=pmi()), "y4141")["findings"],
               "y04-datum-used")["status"] == "pass"


def test_r_y05_frame_used():
    orphan = pmi(frames=[{"id": "drf_orphan", "name": "A-B-C", "refs": [
        {"datum_id": "datum_A", "precedence": 1},
        {"datum_id": "datum_B", "precedence": 2},
        {"datum_id": "datum_C", "precedence": 3}]}])
    # tolerances in base pmi reference drf_abc, not the orphan frame
    f = one(mbd.check_input(bundle(pmi_doc=orphan), "y4141")["findings"],
            "y05-frame-used")
    assert f["status"] == "fail" and "drf_orphan" in f["evidence"]
    assert one(mbd.check_input(bundle(pmi_doc=pmi()), "y4141")["findings"],
               "y05-frame-used")["status"] == "pass"


def test_r_u01_units_declared():
    assert one(mbd.check_input(bundle(pmi_doc=pmi(units="mm")), "y4141")["findings"],
               "u01-units-declared")["status"] == "pass"
    f = one(mbd.check_input(bundle(pmi_doc=pmi(units="parsecs")), "y4141")["findings"],
            "u01-units-declared")
    assert f["status"] == "warn" and "parsecs" in f["evidence"]
    f2 = one(mbd.check_input(bundle(pmi_doc=pmi(units="  ")), "y4141")["findings"],
             "u01-units-declared")
    assert f2["status"] == "fail"


def test_r_u02_units_consistent():
    # mismatch mm vs in => FAIL (both present)
    mm_doc = pmi(units="mm")
    f = one(mbd.check_input(bundle(pmi_doc=mm_doc, model_facts=model("in")), "y4141")["findings"],
            "u02-units-consistent")
    assert f["status"] == "fail"
    # same physical unit via aliases => PASS (in vs inch)
    f2 = one(mbd.check_input(
        bundle(pmi_doc=pmi(units="in"), model_facts=model("inch")), "y4141")["findings"],
        "u02-units-consistent")
    assert f2["status"] == "pass"
    # only PMI side present => UNVERIFIED (nothing to compare against), never fail
    f3 = one(mbd.check_input(bundle(pmi_doc=mm_doc), "y4141")["findings"],
             "u02-units-consistent")
    assert f3["status"] == "unverified"


def test_r_c01_model_ref_resolution():
    # model present, feature ref resolves => PASS
    f = one(mbd.check_input(
        bundle(pmi_doc=pmi(), model_facts=model()), "y4141")["findings"],
        "c01-model-ref-resolution")
    assert f["status"] == "pass"
    # ref into nothing => FAIL
    bogus = pmi(tolerances=[{"id": "gt_b", "characteristic": "flatness", "value": 0.1,
                             "feature_ids": ["feat-ghost"], "body_ids": []}])
    f2 = one(mbd.check_input(
        bundle(pmi_doc=bogus, model_facts=model()), "y4141")["findings"],
        "c01-model-ref-resolution")
    assert f2["status"] == "fail" and "feat-ghost" in f2["evidence"]
    # no model supplied => UNVERIFIED
    f3 = one(mbd.check_input(bundle(pmi_doc=pmi()), "y4141")["findings"],
             "c01-model-ref-resolution")
    assert f3["status"] == "unverified"


# --------------------------------------------------------------------------
# 3. check_input integration: one finding per rule, rollup exact
# --------------------------------------------------------------------------
def test_check_input_one_finding_per_rule_per_set():
    r = mbd.check_input(bundle(pmi_doc=pmi(), pmi_extracted=True,
                               model_facts=model()), "y4141")
    assert r["rule_set"] == "y4141"
    expected = len(mbd.RULE_SETS["y4141"]["rules"])
    assert len(r["findings"]) == expected
    # rollup equals arithmetic
    s = r["summary"]
    assert s["total"] == expected
    assert s["pass"] + s["fail"] + s["warn"] + s["unverified"] == expected
    statuses = {f["status"] for f in r["findings"]}
    assert statuses <= STAT

    rp = mbd.check_input(bundle(pmi_doc=pmi(), pmi_extracted=True,
                                model_facts=model()), "prime")
    assert len(rp["findings"]) == len(mbd.RULE_SETS["prime"]["rules"])


def test_effective_severity_per_set():
    # tolerance-anchored is warning under y4141 but upgraded to error under prime
    floating = pmi(tolerances=[{"id": "gt_t", "characteristic": "flatness", "value": 0.1,
                                "feature_ids": [], "body_ids": []}])
    fy = one(mbd.check_input(bundle(pmi_doc=floating), "y4141")["findings"],
             "y03-tolerance-anchored")
    fp = one(mbd.check_input(bundle(pmi_doc=floating), "prime")["findings"],
             "y03-tolerance-anchored")
    assert mbd.RULE_SETS["y4141"]["overrides"].get("y03-tolerance-anchored", "warning") == "warning"
    assert fy["severity"] == "warning"
    assert fp["severity"] == "error"


# --------------------------------------------------------------------------
# 4. Honesty: the report never lies
# --------------------------------------------------------------------------
def test_unverified_never_passes_or_rolls_into_pass():
    # EMPTY pmi document whose extraction status is unknown: y01 cannot
    # distinguish "no PMI on part" from "PMI not extractable" -> UNVERIFIED.
    empty = pmi(datums=[], frames=[], tolerances=[], notes=[])
    no_model = mbd.check_input(bundle(pmi_doc=empty, pmi_extracted=None), "y4141")
    # the specific rule: y01 with unknown extraction must be UNVERIFIED
    assert one(no_model["findings"], "y01-pmi-present")["status"] == "unverified"
    # a non-empty document with unknown extraction still PASSES: the data
    # itself demonstrates PMI is present (no extraction flag needed for that).
    has_pmi = mbd.check_input(bundle(pmi_doc=pmi(), pmi_extracted=None), "y4141")
    assert one(has_pmi["findings"], "y01-pmi-present")["status"] == "pass"
    # and the summary counts unverified separately from pass
    s = no_model["summary"]
    assert s["unverified"] >= 1 and s["pass"] + s["unverified"] == s["total"] - s["fail"] - s["warn"]


def test_unknown_units_is_warn_not_fail_and_not_silently_pass():
    r = mbd.check_input(bundle(pmi_doc=pmi(units="foot-stones")), "y4141")
    # units-consistent becomes UNVERIFIED, units-declared WARN
    assert one(r["findings"], "u01-units-declared")["status"] == "warn"
    assert one(r["findings"], "u02-units-consistent")["status"] == "unverified"


def test_report_rollup_matches_input_rows():
    reports = mbd.run_manifest(MANIFEST)
    report = mbd.build_report(reports, {"task": "P4-M5-T01", "rule_sets": ["y4141", "prime"]})
    assert report["report_format"] == mbd.REPORT_FORMAT
    assert report["report_version"] == mbd.REPORT_VERSION
    # inputs rows = fixtures * rule sets (+ possibly error rows)
    manifest = json.load(open(MANIFEST))
    n_fixtures = len(manifest["fixtures"])
    assert report["summary"]["inputs"] == len(report["inputs"])
    assert report["summary"]["inputs"] == n_fixtures * 2
    # findings rollup exact
    expect_p = expect_f = expect_w = expect_u = 0
    for row in report["inputs"]:
        s = row["summary"]
        expect_p += s["pass"]; expect_f += s["fail"]
        expect_w += s["warn"]; expect_u += s["unverified"]
    assert report["summary"]["pass"] == expect_p
    assert report["summary"]["fail"] == expect_f
    assert report["summary"]["warn"] == expect_w
    assert report["summary"]["unverified"] == expect_u
    assert report["summary"]["findings"] == expect_p + expect_f + expect_w + expect_u


def test_report_honesty_block_present():
    reports = mbd.run_manifest(MANIFEST)
    report = mbd.build_report(reports, {"task": "P4-M5-T01", "rule_sets": ["y4141", "prime"]})
    assert report.get("honesty") is not None
    assert any("NOT" in line for line in report["honesty"]["scope"])


def test_report_deterministic():
    r1 = mbd.build_report(mbd.run_manifest(MANIFEST), {"task": "P4-M5-T01"})
    r2 = mbd.build_report(mbd.run_manifest(MANIFEST), {"task": "P4-M5-T01"})
    assert json.dumps(r1, sort_keys=True) == json.dumps(r2, sort_keys=True)


# --------------------------------------------------------------------------
# 5. Corpus fixture expectations (service checks the corpus fixtures)
# --------------------------------------------------------------------------
def _run_fixture(fixture_id):
    manifest = json.load(open(MANIFEST))
    entry = next(f for f in manifest["fixtures"] if f["id"] == fixture_id)
    reports = mbd.run_manifest(MANIFEST, only=[fixture_id])
    assert len(reports) == 2  # both rule sets
    return reports, entry


def test_fixture_bracket_complete_passes():
    reports, _ = _run_fixture("bracket-complete")
    for r in reports:
        fails = [f["rule"] for f in r["findings"] if f["status"] == "fail"]
        assert not fails, f"bracket-complete fails: {fails}"
        assert r["summary"]["unverified"] == 0, "fully-known input must not be UNVERIFIED"


def test_fixture_bracket_no_pmi_fails_missing_pmi():
    reports, _ = _run_fixture("bracket-no-pmi")
    for r in reports:
        assert one(r["findings"], "y01-pmi-present")["status"] == "fail"


def test_fixture_bracket_dangling_datum_fails_y04():
    reports, _ = _run_fixture("bracket-dangling-datum")
    for r in reports:
        assert one(r["findings"], "y04-datum-used")["status"] == "fail"


def test_fixture_bracket_floating_fails_anchoring():
    reports, _ = _run_fixture("bracket-floating-frame")
    for r in reports:
        assert one(r["findings"], "y02-datum-anchored")["status"] == "fail"
        assert one(r["findings"], "y03-tolerance-anchored")["status"] == "fail"


def test_fixture_bracket_unit_mismatch_fails_u02():
    reports, _ = _run_fixture("bracket-unit-mismatch")
    for r in reports:
        assert one(r["findings"], "u02-units-consistent")["status"] == "fail"


def test_fixture_bracket_unknown_units_warns():
    reports, _ = _run_fixture("bracket-unknown-units")
    for r in reports:
        assert one(r["findings"], "u01-units-declared")["status"] == "warn"


def test_fixture_bracket_unresolved_refs_fails_structural():
    reports, _ = _run_fixture("bracket-unresolved-refs")
    for r in reports:
        assert one(r["findings"], "s06-datum-frame-resolution")["status"] == "fail"
        assert one(r["findings"], "s07-tolerance-frame-resolution")["status"] == "fail"


def test_fixture_bracket_no_pmi_unknown_is_unverified():
    reports, _ = _run_fixture("bracket-no-pmi-unknown")
    for r in reports:
        assert one(r["findings"], "y01-pmi-present")["status"] == "unverified"


# --------------------------------------------------------------------------
# 6. Real-material leg: bracket-complete is grounded on the committed
#    oep-pmi golden (example-bracket.oeppmi.json, P6-M1-T02 real-material gate)
# --------------------------------------------------------------------------
def test_real_material_bracket_complete_matches_golden_schema():
    with open(os.path.join(FIXTURES, "bracket-complete.oeppmi.json")) as fh:
        doc = json.load(fh)
    assert doc["format"] == "oep-pmi" and doc["version"] == 1 and doc["units"] == "mm"
    assert {"datums", "frames", "tolerances", "notes"} <= set(doc)
    # model facts must carry the golden's actual feature ids so cross-model
    # resolution is exercised against the real referenced ids.
    refs = set()
    for group in (doc.get("datums", []), doc.get("tolerances", []), doc.get("notes", [])):
        for obj in group:
            refs.update(obj.get("feature_ids", [])); refs.update(obj.get("body_ids", []))
    r = mbd.check_input(bundle(pmi_doc=doc, pmi_extracted=True,
                               model_facts=model(feature_ids=sorted(refs))), "y4141")
    fails = [f["rule"] for f in r["findings"] if f["status"] == "fail"]
    assert not fails, f"real-material bracket fails rules it should pass: {fails}"


# --------------------------------------------------------------------------
# 7. Tolerant failure / typed errors (never a crash)
# --------------------------------------------------------------------------
def test_load_input_missing_file_raises_typed_error():
    with pytest.raises(mbd.MbdInputError):
        mbd.load_input(os.path.join(FIXTURES, "does-not-exist.oeppmi.json"))


def test_load_input_malformed_json_raises_typed_error(tmp_path):
    bad = tmp_path / "bad.json"
    bad.write_text("{ not json")
    with pytest.raises(mbd.MbdInputError):
        mbd.load_input(str(bad))


def test_run_manifest_survives_missing_manifest_file(tmp_path):
    reports = mbd.run_manifest(str(tmp_path / "nope.json"))
    assert isinstance(reports, list) and len(reports) == 0


# --------------------------------------------------------------------------
# 8. Report rendering (demo surface): JSON + Markdown both produced
# --------------------------------------------------------------------------
def test_render_markdown_contains_rules_and_honesty(tmp_path):
    reports = mbd.run_manifest(MANIFEST, only=["bracket-complete"])
    md = mbd.render_markdown(mbd.build_report(reports, {"task": "P4-M5-T01"}))
    assert md.startswith("#") and "bracket-complete" in md
    assert "PASS" in md and "UNVERIFIED" in md


def test_write_report_emits_json_and_md(tmp_path):
    reports = mbd.run_manifest(MANIFEST, only=["bracket-complete"])
    report = mbd.build_report(reports, {"task": "P4-M5-T01"})
    jp, mdp = mbd.write_report(report, str(tmp_path))
    assert os.path.exists(jp) and os.path.exists(mdp)
    assert json.load(open(jp))["report_format"] == mbd.REPORT_FORMAT
