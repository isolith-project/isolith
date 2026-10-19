"""P4-M5-T02: MBD validator CLI + report output - acceptance tests.

TDD direction: this file is written FIRST (RED). It names the public surface
of python/oep_tooling/mbd_validate_cli.py (the product wedge surface: CLI +
report output + documented exit codes) and pins every acceptance criterion in
the task card:

  - CLI runs on a real corpus fixture and emits the report artifact
  - exit codes documented (tested here: 0/1/2/3/4 semantics)
  - demo-able end to end (module + `python -m`, bundle and directory input)
  - results verified against real data, not a log line: the emitted report is
    re-read and its rollup is recomputed from the granular findings, and the
    per-fixture verdicts are checked against the fixture manifest expect
    blocks, not against what the tool printed.

The CLI reuses the P4-M5-T01 service (mbd.build_report / render_markdown /
write_report) - it must never re-implement the rule engine.
"""

import json
import os
import subprocess
import sys

import pytest

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
PYTHON_DIR = os.path.join(REPO, "python")
MANIFEST = os.path.join(REPO, "corpus", "mbd-fixture-manifest.json")
FIXTURES = os.path.join(REPO, "corpus", "mbd-fixtures")
COMMITTED_REPORT = os.path.join(REPO, "corpus", "mbd-validate-report.json")
CLI_MOD = "oep_tooling.mbd_validate_cli"

STATUSES = ("pass", "fail", "warn", "unverified")


# ---------------------------------------------------------------------------
# subprocess runner (the real CLI is exercised as an external process, the way
# a defense-prime customer would drive it).
# ---------------------------------------------------------------------------
def run_cli(*args, cwd=None):
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join(
        [p for p in (env.get("PYTHONPATH", ""), PYTHON_DIR) if p]
    )
    return subprocess.run(
        [sys.executable, "-m", CLI_MOD, *args],
        cwd=cwd or REPO,
        env=env,
        capture_output=True,
        text=True,
    )


def read_manifest():
    with open(MANIFEST, "r", encoding="utf-8") as fh:
        return json.load(fh)


# ---------------------------------------------------------------------------
# Surface (module-level) acceptance
# ---------------------------------------------------------------------------
def test_cli_module_defines_documented_exit_codes_and_main():
    sys.path.insert(0, PYTHON_DIR)
    import oep_tooling.mbd_validate_cli as cli  # noqa: E402  (import gate)

    # The exit-code contract from docs/re-notes/p4-m5-t02-mbd-validator-cli.md.
    assert cli.EXIT_OK == 0
    assert cli.EXIT_FINDINGS == 1
    assert cli.EXIT_USAGE == 2
    assert cli.EXIT_INPUT == 3
    assert cli.EXIT_INTERNAL == 4
    assert callable(cli.main)


def test_cli_reuses_service_not_a_rewrite():
    """The product surface must reuse the P4-M5-T01 report pipeline."""
    sys.path.insert(0, PYTHON_DIR)
    import oep_tooling.mbd_validate_cli as cli  # noqa: E402
    import oep_tooling.mbd_validate as mbd  # noqa: E402

    assert cli.mbd is mbd or cli.mbd_validate is mbd
    # The CLI builds the artifact through the service API, not its own engine.
    assert callable(cli.mbd.build_report)
    assert callable(cli.mbd.write_report)


# ---------------------------------------------------------------------------
# End-to-end on the real corpus fixture
# ---------------------------------------------------------------------------
def test_cli_runs_manifest_emits_report_artifact_and_verifies_real_data(tmp_path):
    """Run the CLI over the whole manifest; re-read the artifact and verify the
    numbers against the actual findings (never trust the printed log line)."""
    out = tmp_path / "out"
    res = run_cli("--manifest", MANIFEST, "--out", str(out))
    assert res.returncode == cli_expected_findings_rc(), res.stderr[-2000:]

    json_path = out / "mbd-validate-report.json"
    md_path = out / "mbd-validate-report.md"
    assert json_path.exists() and md_path.exists()

    report = json.loads(json_path.read_text(encoding="utf-8"))
    assert report["report_format"] == "oep-mbd-validate"
    assert report["report_version"] == 1
    assert report["generated"]
    assert report["tool"]["name"] == "mbd_validate"

    # Honest rollup: recompute from the granular findings and compare.
    assert report["summary"]["inputs"] == len(report["inputs"])
    recomputed = {s: 0 for s in STATUSES}
    findings_total = 0
    for row in report["inputs"]:
        for st in STATUSES:
            recomputed[st] += row["summary"].get(st, 0)
        findings_total += row["summary"].get("total", 0)
        # row summary must equal its own granular findings
        per_rule = {}
        for f in row["findings"]:
            per_rule[f["status"]] = per_rule.get(f["status"], 0) + 1
        for st in STATUSES:
            assert row["summary"].get(st, 0) == per_rule.get(st, 0), row["id"]
    assert report["summary"].get("pass") == recomputed["pass"]
    assert report["summary"].get("fail") == recomputed["fail"]
    assert report["summary"].get("warn") == recomputed["warn"]
    assert report["summary"].get("unverified") == recomputed["unverified"]
    assert report["summary"]["findings"] == findings_total

    # Every fixture x rule set is present: 8 fixtures x 2 rule sets = 16 rows.
    assert report["summary"]["inputs"] == 16
    assert report["summary"]["findings"] == 272

    # Matches the reference committed report (same honest verdicts, not a log).
    committed = json.loads(open(COMMITTED_REPORT, "r", encoding="utf-8").read())
    def key(row):
        return (row["id"], row["rule_set"])
    assert set(key(r) for r in report["inputs"]) == set(key(r) for r in committed["inputs"])
    for row in report["inputs"]:
        c_row = next(r for r in committed["inputs"] if key(r) == key(row))
        assert {f["rule"]: f["status"] for f in row["findings"]} == \
               {f["rule"]: f["status"] for f in c_row["findings"]}, key(row)


def test_cli_per_fixture_verdicts_match_manifest_expect(tmp_path):
    """Per-fixture outcomes verified against the fixture manifest expect blocks
    (the ground truth the fixtures were authored to), re-derived from the
    emitted report artifact."""
    out = tmp_path / "out"
    result = run_cli("--manifest", MANIFEST, "--out", str(out))
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    manifest = read_manifest()
    rows = {(r["id"], r["rule_set"]): r for r in report["inputs"]}
    for entry in manifest["fixtures"]:
        fid = entry["id"]
        for rule_set, expect in entry.get("expect", {}).items():
            row = rows[(fid, rule_set)]
            by_rule = {f["rule"]: f["status"] for f in row["findings"]}
            for rid in expect.get("fail_rules", []):
                assert by_rule[rid] == "fail", (fid, rule_set, rid)
            for rid in expect.get("warn_rules", []):
                assert by_rule[rid] == "warn", (fid, rule_set, rid)
            for rid in expect.get("unverified_rules", []):
                assert by_rule[rid] == "unverified", (fid, rule_set, rid)
            # Designated pass rules must actually pass.
            for rid in expect.get("pass_rules", []):
                assert by_rule[rid] == "pass", (fid, rule_set, rid)


def cli_expected_findings_rc():
    # Default --fail-on fail: the corpus contains deliberate FAIL fixtures.
    return 1


# ---------------------------------------------------------------------------
# Exit code contract
# ---------------------------------------------------------------------------
def test_exit_0_clean_scope(tmp_path):
    out = tmp_path / "out"
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-complete", "--out", str(out))
    assert res.returncode == 0, res.stderr
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    assert report["summary"]["fail"] == 0
    assert report["summary"]["warn"] == 0
    assert report["summary"]["unverified"] == 0
    assert report["summary"]["pass"] == report["summary"]["findings"]
    # two rule sets for one fixture
    assert report["summary"]["inputs"] == 2


def test_default_threshold_any_fail_gives_exit_1(tmp_path):
    for only in ("bracket-no-pmi", "bracket-dangling-datum",
                 "bracket-unit-mismatch", "bracket-unresolved-refs"):
        res = run_cli("--manifest", MANIFEST, "--only", only, "--out", str(tmp_path / only))
        assert res.returncode == 1, (only, res.stdout, res.stderr)


def test_fail_on_warn_threshold(tmp_path):
    out = tmp_path / "out"
    # bracket-unknown-units only produces a WARN (u01) under y4141 and prime.
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-unknown-units",
                  "--fail-on", "warn", "--out", str(out))
    assert res.returncode == 1, res.stderr
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-unknown-units",
                  "--fail-on", "fail", "--out", str(out))
    assert res.returncode == 0, res.stderr
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-unknown-units",
                  "--fail-on", "none", "--out", str(out))
    assert res.returncode == 0, res.stderr


def test_fail_on_unverified_strictest(tmp_path):
    out = tmp_path / "out"
    # honest UNVERIFIED (extraction unknown) counts as non-passing under
    # --fail-on unverified but not under the default --fail-on fail.
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-no-pmi-unknown",
                  "--fail-on", "unverified", "--out", str(out))
    assert res.returncode == 1, res.stderr
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-no-pmi-unknown",
                  "--fail-on", "fail", "--out", str(out))
    assert res.returncode == 0, res.stderr


def test_fail_on_none_always_ok_on_successful_run(tmp_path):
    out = tmp_path / "out"
    res = run_cli("--manifest", MANIFEST, "--fail-on", "none", "--out", str(out))
    assert res.returncode == 0
    # the artifact was still emitted (exit code and report are independent)
    assert (out / "mbd-validate-report.json").exists()


def test_usage_errors_exit_2(tmp_path):
    # no input mode
    res = run_cli("--out", str(tmp_path))
    assert res.returncode == 2
    # unknown rule set
    res = run_cli("--manifest", MANIFEST, "--rule-sets", "bogus")
    assert res.returncode == 2
    # --only without a manifest
    res = run_cli("--only", "bracket-complete")
    assert res.returncode == 2
    # unknown flag
    res = run_cli("--bogus-flag")
    assert res.returncode == 2
    # both --manifest and a positional input are mutually exclusive
    res = run_cli("--manifest", MANIFEST, os.path.join(FIXTURES, "bracket-complete.oeppmi.json"))
    assert res.returncode == 2


def test_input_errors_exit_3(tmp_path):
    # missing manifest
    res = run_cli("--manifest", str(tmp_path / "nope.json"))
    assert res.returncode == 3
    # malformed manifest
    bad = tmp_path / "bad.json"
    bad.write_text("{not json", encoding="utf-8")
    res = run_cli("--manifest", str(bad))
    assert res.returncode == 3
    # explicit input file missing
    res = run_cli(str(tmp_path / "missing.oeppmi.json"))
    assert res.returncode == 3
    # directory with no loadable inputs
    empty = tmp_path / "empty"
    empty.mkdir()
    res = run_cli(str(empty))
    assert res.returncode == 3


# ---------------------------------------------------------------------------
# Input modes
# ---------------------------------------------------------------------------
def test_single_bundle_input(tmp_path):
    # A bare oep-pmi file (no model facts, extraction unknown) is an honest
    # UNVERIFIED for u02/c01 (and y01), never a fabricated PASS or FAIL.
    bundle = os.path.join(FIXTURES, "bracket-complete.oeppmi.json")
    out = tmp_path / "out"
    res = run_cli(bundle, "--out", str(out))
    assert res.returncode == 0, res.stderr  # no FAIL under default threshold
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    assert report["summary"]["inputs"] == 2
    assert all(r["status"] == "checked" for r in report["inputs"])
    assert report["summary"]["fail"] == 0
    # honest: model-less, extraction-unknown inputs cannot claim full PASS
    assert report["summary"]["unverified"] > 0
    assert report["summary"]["pass"] == report["summary"]["findings"] - \
        report["summary"]["unverified"]


def test_directory_input_tolerant_error_rows(tmp_path):
    d = tmp_path / "batch"
    d.mkdir()
    (d / "ok.oeppmi.json").write_text(
        open(os.path.join(FIXTURES, "bracket-complete.oeppmi.json"), encoding="utf-8").read(),
        encoding="utf-8")
    (d / "broken.json").write_text("{nope", encoding="utf-8")
    (d / "notabundle.json").write_text('{"hello": 1}', encoding="utf-8")
    out = tmp_path / "out"
    res = run_cli(str(d), "--out", str(out))
    # one good input (2 rows) + two tolerant error rows -> breaches default --fail-on
    # (an unvalidated input is never a PASS), so exit 1 while the report is emitted.
    assert res.returncode == 1, (res.stdout, res.stderr)
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    statuses = [r["status"] for r in report["inputs"]]
    # 2 unreadable files -> an error row per rule set (2) each; 1 good file -> 2 checked rows
    assert statuses.count("error") == 4
    assert statuses.count("checked") == 2


def test_rule_sets_filter(tmp_path):
    out = tmp_path / "out"
    res = run_cli("--manifest", MANIFEST, "--rule-sets", "y4141",
                  "--only", "bracket-complete", "--out", str(out))
    assert res.returncode == 0, res.stderr
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    assert [r["rule_set"] for r in report["inputs"]] == ["y4141"]
    res = run_cli("--manifest", MANIFEST, "--rule-sets", "prime",
                  "--only", "bracket-complete", "--out", str(out))
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    assert [r["rule_set"] for r in report["inputs"]] == ["prime"]


def test_only_filter_reduces_rows(tmp_path):
    out = tmp_path / "out"
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-complete",
                  "--only", "bracket-no-pmi", "--out", str(out))
    assert res.returncode == 1  # bracket-no-pmi has a FAIL
    report = json.loads((out / "mbd-validate-report.json").read_text(encoding="utf-8"))
    assert sorted(r["id"] for r in report["inputs"]) == \
           ["bracket-complete", "bracket-complete", "bracket-no-pmi", "bracket-no-pmi"]


def test_format_flag(tmp_path):
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-complete",
                  "--format", "json", "--out", str(tmp_path / "a"))
    assert (tmp_path / "a" / "mbd-validate-report.json").exists()
    assert not (tmp_path / "a" / "mbd-validate-report.md").exists()
    res = run_cli("--manifest", MANIFEST, "--only", "bracket-complete",
                  "--format", "md", "--out", str(tmp_path / "b"))
    assert (tmp_path / "b" / "mbd-validate-report.md").exists()
    assert not (tmp_path / "b" / "mbd-validate-report.json").exists()


def test_version_flag(tmp_path):
    res = run_cli("--version")
    assert res.returncode == 0
    assert "mbd_validate" in res.stdout


# ---------------------------------------------------------------------------
# Report rendering surface
# ---------------------------------------------------------------------------
def test_markdown_artifact_self_explanatory(tmp_path):
    out = tmp_path / "out"
    run_cli("--manifest", MANIFEST, "--only", "bracket-complete", "--out", str(out))
    md = (out / "mbd-validate-report.md").read_text(encoding="utf-8")
    for token in ("PASS", "FAIL", "WARN", "UNVERIFIED", "Honesty scope",
                  "bracket-complete", "oep-mbd-validate"):
        assert token in md, token
    assert "certificate of conformance" in md


def test_markdown_title_tracks_task(tmp_path):
    sys.path.insert(0, PYTHON_DIR)
    import oep_tooling.mbd_validate as mbd  # noqa: E402

    reports = mbd.run_manifest(MANIFEST, only=["bracket-complete"])
    md = mbd.render_markdown(mbd.build_report(reports, {"task": "P4-M5-T02",
                                                        "rule_sets": ["y4141"]}))
    assert md.startswith("# P4-M5-T02"), md.splitlines()[0]
    # backward-compatible: no task present still renders the format title
    md2 = mbd.render_markdown(mbd.build_report(reports, {}))
    assert md2.startswith("#")
