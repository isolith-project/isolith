"""FEATURE-BASELINE acceptance: the CATIA/SolidWorks editor parity baseline
scope register is committed with honest, evidence-backed claims.

RED first (this file before the content). The register must validate against a
closed schema (status / cost-class / vendor tokens), carry all 14 required
capabilities exactly once, and no row may claim a status higher than its
evidence proves (honest-claims discipline). `building` and `shippable` rows
must cite a test that actually exists in the tree; `planned` rows must carry a
recorded decision; `not-scoped` rows need no proof. The QA script must run to
a classified report (never a crash) and the human register doc must stay in
sync with the machine-checkable JSON.

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
    assert reg["schema_version"] == 1
    policy = reg["policy"]
    for status in (r["status"] for r in reg["rows"]):
        assert status in {"not-scoped", "planned", "building", "shippable"}, \
            f"uncommitted status token {status!r}"
    for cost in (r["cost_class"] for r in reg["rows"]):
        assert cost in {"class-1-additive", "class-2-model-extend",
                        "class-3-kernel-gated", "class-4-different-foundation"}, \
            f"uncommitted cost class {cost!r}"
    for vendor in ("catia", "solidworks"):
        for tier in (r[vendor] for r in reg["rows"]):
            assert tier in {"base", "module", "limited", "no"}, \
                f"uncommitted vendor tier {tier!r} in {vendor}"


def test_register_covers_all_required_capabilities_exactly_once():
    reg = _load_register()
    ids = [r["id"] for r in reg["rows"]]
    for cap in REQUIRED_CAPABILITIES:
        assert ids.count(cap) == 1, \
            f"required capability {cap!r} present {ids.count(cap)}x"
    assert len(ids) == len(set(ids)), f"duplicate ids: {ids}"


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


def test_checker_runs_to_classified_report():
    proc = _run_checker()
    assert proc.returncode == 0, proc.stderr
    report = json.loads(proc.stdout)
    assert report["schema_version"] == 1
    assert report["exit"] == 0
    assert report["summary"]["fail"] == 0, report["summary"]["failures"]
    assert report["summary"]["rows"] == len(REQUIRED_CAPABILITIES)


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
