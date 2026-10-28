"""FEATURE-BASELINE + PRACTITIONER-INPUT-CONTRACT acceptance: the CATIA/
SolidWorks editor parity baseline scope register is committed with honest,
evidence-backed claims.

RED first (this file before the content). The register must validate against a
closed schema (status / cost-class / vendor tokens), carry all 14 required
capabilities exactly once, and no row may claim a status higher than its
evidence proves (honest-claims discipline). `building` and `shippable` rows
must cite a test that actually exists in the tree; `planned` rows must carry a
recorded decision; `not-scoped` rows need no proof.

The practitioner_input_contract section (schema version 2 + tolerant migration
from 1) tracks the editor INPUT AND BEHAVIOUR contract the LTT practitioner
test of SolidCraft and Purrametric specifies (decision #19 bar: same-or-greater
editor functionality). Its rows obey the SAME status ceiling as the capability
rows - a behaviour row may not claim `building`/`shippable` without a green
accepted test that exercises that behaviour - and each row must name an
explicit test hook, so nothing is tracked as real until a test can be pointed
at. The QA script must run to a classified report (never a crash) and the
human register doc must stay in sync with the machine-checkable JSON.

Hermetic: no docker, no cargo build, no network. Pure stdlib. The "green"
half of the honest-claims rule is proven by the full pytest/cargo suite this
file rides in; here we prove the claim has a citable test at all.
"""

import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REGISTER = os.path.join(ROOT, "docs", "scope-register.json")
MARKDOWN = os.path.join(ROOT, "docs", "scope-register.md")
CHECKER = os.path.join(ROOT, "python", "oep_tooling", "scope_register_check.py")

REQUIRED_CAPABILITIES = {
    "2d-drafting",
    "configurations-design-tables",
    "mass-properties",
    "interference-clash",
    "exploded-views",
    "sheet-metal",
    "weldments",
    "mold-tooling",
    "routing",
    "electrical-harness",
    "composites",
    "cam",
    "fea",
    "class-a-surfacing",
}

# The minimum practitioner behaviour set, from the verified episode findings
# of the LTT practitioner test (SolidCraft / Purrametric), Don directive
# 2026-10-27; the drift guard requires these exact ids in the register.
PRACTITIONER_IDS = {
    "input-command-search",
    "input-right-click-wheel",
    "input-spacebar-accept",
    "input-snap-inference",
    "input-escape-semantics",
    "input-cut-region",
    "input-datum-plane-legibility",
    "input-toolbar-customisation",
    "input-undo-redo-parity",
}

STATUS_RANK = {"not-scoped": 0, "planned": 1, "building": 2, "shippable": 3}


def _load_register():
    with open(REGISTER) as fh:
        return json.load(fh)


def _run_checker(register_path=None, markdown_path=None):
    cmd = [sys.executable, CHECKER, "--out", "-"]
    if register_path:
        cmd += ["--register", register_path]
    if markdown_path:
        cmd += ["--markdown", markdown_path]
    env = dict(os.environ)
    env["PYTHONPATH"] = ROOT + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(cmd, capture_output=True, text=True, env=env,
                          timeout=120)


def test_register_json_and_doc_exist():
    assert os.path.isfile(REGISTER), "missing docs/scope-register.json"
    assert os.path.isfile(MARKDOWN), "missing docs/scope-register.md"


def test_register_schema_and_closed_tokens():
    reg = _load_register()
    assert reg["schema_version"] == 2
    policy = reg["policy"]
    all_rows = list(reg["rows"]) + list(reg["practitioner_input_contract"]["rows"])
    for status in (r["status"] for r in all_rows):
        assert status in {"not-scoped", "planned", "building", "shippable"}, \
            f"uncommitted status token {status!r}"
    for cost in (r["cost_class"] for r in all_rows):
        assert cost in {"class-1-additive", "class-2-model-extend",
                        "class-3-kernel-gated", "class-4-different-foundation"}, \
            f"uncommitted cost class {cost!r}"
    for vendor in ("catia", "solidworks"):
        for tier in (r[vendor] for r in all_rows):
            assert tier in {"base", "module", "limited", "no"}, \
                f"uncommitted vendor tier {tier!r} in {vendor}"


def test_register_covers_all_required_capabilities_exactly_once():
    reg = _load_register()
    ids = [r["id"] for r in reg["rows"]]
    for cap in REQUIRED_CAPABILITIES:
        assert ids.count(cap) == 1, \
            f"required capability {cap!r} present {ids.count(cap)}x"
    assert len(ids) == len(set(ids)), f"duplicate ids: {ids}"


def test_practitioner_section_complete_with_test_hooks():
    reg = _load_register()
    section = reg["practitioner_input_contract"]
    rows = section["rows"]
    ids = [r["id"] for r in rows]
    for behaviour in PRACTITIONER_IDS:
        assert ids.count(behaviour) == 1, \
            f"practitioner behaviour {behaviour!r} present {ids.count(behaviour)}x"
    assert len(ids) == len(set(ids)), f"duplicate practitioner ids: {ids}"
    for row in rows:
        assert row.get("test_hook"), f"{row['id']} lacks an explicit test_hook"
        assert row.get("capability"), f"{row['id']} lacks a capability label"
        assert row.get("workstream"), f"{row['id']} lacks an owning workstream"
    # the policy carries the authoritative required set the drift guard checks
    assert set(reg["policy"].get("practitioner_required_ids") or []) == PRACTITIONER_IDS


def test_status_evidence_ladder_enforced():
    """No row may claim a status higher than its evidence proves."""
    reg = _load_register()
    for row in reg["rows"]:
        status = row["status"]
        kind = (row.get("evidence") or {}).get("kind")
        ref = (row.get("evidence") or {}).get("ref") or ""
        if status in ("building", "shippable"):
            msg = (f"{row['id']} claims {status} with evidence kind {kind!r}; "
                   "building/shippable require a cited green test")
            assert kind == "test", msg
            assert ref, f"{row['id']} claims {status} with no test ref"
            # the cited test must actually exist in the tree
            resolved = _resolve_ref(ref)
            assert resolved, f"{row['id']} cites non-existent test {ref!r}"
        elif status == "planned":
            assert kind in ("decision", "test"), \
                f"{row['id']} is planned with unproven evidence kind {kind!r}"
            assert ref, f"{row['id']} is planned with no evidence ref"
        # not-scoped needs no evidence by design


def test_practitioner_status_evidence_ladder_enforced():
    """The practitioner input-contract rows obey the SAME status ceiling: a
    behaviour row cannot claim building/shippable without a green accepted
    test that exercises it (the LTT-specified behaviours, not feature lists,
    are what prove the parity bar)."""
    reg = _load_register()
    for row in reg["practitioner_input_contract"]["rows"]:
        status = row["status"]
        kind = (row.get("evidence") or {}).get("kind")
        ref = (row.get("evidence") or {}).get("ref") or ""
        if status in ("building", "shippable"):
            msg = (f"{row['id']} claims {status} with evidence kind {kind!r}; "
                   "a behaviour row needs a cited green test that exercises it")
            assert kind == "test", msg
            assert ref, f"{row['id']} claims {status} with no test ref"
            resolved = _resolve_ref(ref)
            assert resolved, f"{row['id']} cites non-existent test {ref!r}"
        elif status == "planned":
            assert kind in ("decision", "test"), \
                f"{row['id']} is planned with unproven evidence kind {kind!r}"
            assert ref, f"{row['id']} is planned with no evidence ref"
        # not-scoped needs no evidence by design


def _resolve_ref(ref):
    cands = []
    if "/" in ref:
        cands.append(os.path.normpath(os.path.join(ROOT, ref)))
    cands.append(os.path.join(ROOT, "tests", os.path.basename(ref)))
    cands.append(os.path.join(ROOT, ref))
    return next((c for c in cands if os.path.isfile(c)), None)


def test_commitment_order_references_real_capabilities():
    reg = _load_register()
    ids = {r["id"] for r in reg["rows"]}
    order = reg["commitment_order"]
    for group in ("near_term", "after_kernel_seam", "secondary",
                  "class_a_program"):
        for item in order.get(group, []):
            assert item in ids, \
                f"commitment order references unknown capability {item!r}"
    assert order["near_term"][0] == "2d-drafting"
    assert "mass-properties" in order["near_term"]
    assert "class-a-surfacing" in order["class_a_program"]


def test_markdown_register_stays_in_sync():
    reg = _load_register()
    md = open(MARKDOWN, encoding="utf-8").read()
    for row in reg["rows"]:
        line = next((l for l in md.splitlines()
                     if l.strip().startswith("| %s " % row["id"])), None)
        assert line, f"{row['id']} missing from docs/scope-register.md table"
        assert "| %s |" % row["status"] in line, \
            f"{row['id']} status {row['status']} missing from its md row"
    for row in reg["practitioner_input_contract"]["rows"]:
        line = next((l for l in md.splitlines()
                     if l.strip().startswith("| %s " % row["id"])), None)
        assert line, f"{row['id']} missing from docs/scope-register.md table"
        assert "| %s |" % row["status"] in line, \
            f"{row['id']} status {row['status']} missing from its md row"


def test_checker_runs_to_classified_report():
    proc = _run_checker()
    assert proc.returncode == 0, proc.stderr
    report = json.loads(proc.stdout)
    assert report["schema_version"] == 2
    assert report["exit"] == 0
    assert report["summary"]["fail"] == 0, report["summary"]["failures"]
    assert report["summary"]["rows"] == len(REQUIRED_CAPABILITIES)
    assert report["summary"]["practitioner_rows"] == len(PRACTITIONER_IDS)


def test_checker_fails_on_claim_without_evidence():
    """Deliberate-failure repro: a 'building' row with decision evidence (no
    test) must FAIL the drift guard - the honest-claims rule actually bites."""
    reg = _load_register()
    reg["schema_version"] = 1
    bad = dict(reg["rows"][0])
    bad["status"] = "shippable"
    bad["evidence"] = {"kind": "decision", "ref": "decision #99",
                       "note": "overclaim: no green test"}
    reg["rows"] = [bad]
    reg.pop("practitioner_input_contract", None)
    with tempfile.TemporaryDirectory() as tmp:
        bad_json = os.path.join(tmp, "bad-register.json")
        bad_md = os.path.join(tmp, "bad-register.md")
        with open(bad_json, "w") as fh:
            json.dump(reg, fh)
        with open(bad_md, "w") as fh:
            # a minimal md so a missing-row sync failure is not the tested cause
            fh.write("| %s | base | base | class-1-additive | ws | shippable | d |\n"
                     % bad["id"])
        proc = _run_checker(register_path=bad_json, markdown_path=bad_md)
        assert proc.returncode != 0, \
            "drift guard must reject a claim without a green accepted test"
        report = json.loads(proc.stdout)
        assert report["summary"]["fail"] >= 1
        assert any("shippable" in f for f in report["summary"]["failures"])


def test_checker_fails_on_practitioner_row_overclaim():
    """Deliberate-failure repro over the practitioner section: a behaviour row
    bumped to 'building' with decision-only evidence must FAIL - the parity
    bar is proven by a green test of the behaviour, not by a feature list."""
    reg = _load_register()
    section = reg["practitioner_input_contract"]
    bad = dict(section["rows"][0])
    bad["status"] = "building"
    bad["evidence"] = {"kind": "decision", "ref": "decision #99",
                       "note": "overclaim: no green test of the behaviour"}
    section["rows"] = [bad]
    with tempfile.TemporaryDirectory() as tmp:
        bad_json = os.path.join(tmp, "bad-register.json")
        bad_md = os.path.join(tmp, "bad-register.md")
        with open(bad_json, "w") as fh:
            json.dump(reg, fh)
        with open(bad_md, "w") as fh:
            # a minimal md so a missing-row sync failure is not the tested cause
            fh.write("| %s | base | base | class-1-additive | ws | building | d |\n"
                     % bad["id"])
        proc = _run_checker(register_path=bad_json, markdown_path=bad_md)
        assert proc.returncode != 0, \
            "drift guard must reject a practitioner overclaim"
        report = json.loads(proc.stdout)
        assert report["summary"]["fail"] >= 1
        assert any("practitioner" in f and "building" in f
                   for f in report["summary"]["failures"]), \
            report["summary"]["failures"]


def test_checker_tolerates_schema_1_register_without_practitioner():
    """Tolerant migration: a register still at schema 1 (no practitioner
    section) validates cleanly - the checker accepts old registers rather than
    breaking them the moment the new section exists on main."""
    reg = _load_register()
    reg["schema_version"] = 1
    reg.pop("practitioner_input_contract", None)
    policy = reg["policy"]
    policy["practitioner_required_ids"] = []
    with tempfile.TemporaryDirectory() as tmp:
        old_json = os.path.join(tmp, "old-schema.json")
        with open(old_json, "w") as fh:
            json.dump(reg, fh)
        # real markdown: the schema-1 register keeps the 14 capability rows,
        # whose statuses are all present in the committed table.
        proc = _run_checker(register_path=old_json,
                            markdown_path=os.path.join(ROOT, "docs",
                                                       "scope-register.md"))
        assert proc.returncode == 0, proc.stderr
        report = json.loads(proc.stdout)
        assert report["summary"]["rows"] == len(REQUIRED_CAPABILITIES)
        assert report["summary"]["practitioner_rows"] == 0


def test_checker_rejects_unknown_capability_and_unknown_status():
    """The closed schema must reject a register that invents capabilities or
    status tokens, so the baseline cannot drift into marketing copy."""
    reg = _load_register()
    reg["rows"].append(dict(reg["rows"][0], id="made-up-capability",
                            status="basically-done"))
    with tempfile.TemporaryDirectory() as tmp:
        bad_json = os.path.join(tmp, "bad-register.json")
        bad_md = os.path.join(tmp, "bad-register.md")
        with open(bad_json, "w") as fh:
            json.dump(reg, fh)
        with open(bad_md, "w") as fh:
            fh.write("| made-up-capability | base | base | c1 | ws | basically-done | d |\n")
        proc = _run_checker(register_path=bad_json, markdown_path=bad_md)
        assert proc.returncode != 0, "closed schema must reject invented tokens"
