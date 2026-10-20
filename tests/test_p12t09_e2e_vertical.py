"""P12-T09 acceptance: the committed E2E vertical-suite artifacts.

The full vertical (import CGR/STEP -> model -> release via PDM ->
render/convert via P2P -> share) runs headlessly against the real stack via
scripts/e2e-vertical.sh (python/oep_tooling/e2e_vertical.py), per release, and
is recorded under workstreams/release-engineering/proof/e2e-vertical-<ver>.{json,md}. Its pass is a
release-blocking CI check (ga-gate-p12e2e.yaml) alongside the GA gates.

CI has no docker daemon and no second host (decision #9), and cannot stage real
.cgr material (clean-room invariant), so this test pins the committed facts and
runs the driver's CI-safe modes:
  * the runbook (stage checklist) is committed and names every stage + the
    closed taxonomy;
  * --selfcheck exits 0 (fixtures present, break-flag contract);
  * the committed green proof for the current release carries PASS rows for
    every vertical stage (import/model/release/render/share) with zero FAIL;
  * the committed deliberate-failure repro proves a broken stage BLOCKS the
    gate: it has a FAIL row in exactly the broken stage and exit_code != 0.
That last point is what makes the gate honest: a stage regression cannot go
green, and the CI job runs the same --break-stage repro to assert non-zero.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RUNBOOK = REPO_ROOT / "workstreams/release-engineering" / "e2e-vertical.md"
DRIVER = REPO_ROOT / "scripts" / "e2e-vertical.sh"
MODULE = REPO_ROOT / "python" / "oep_tooling" / "e2e_vertical.py"
PROOF_DIR = REPO_ROOT / "workstreams/release-engineering" / "proof"
ENV_EXAMPLE = REPO_ROOT / "workstreams/delivery/deploy" / "deploy" / ".env.example"

# Primary row id per stage of the reference vertical (each must be PASS in the
# committed green proof). Sub-rows (audit under RELEASE, cgr under IMPORT) may
# SKIP under the closed taxonomy.
STAGES = ["import-step", "model-geo", "release", "render", "share"]


def _env_example_version() -> str:
    for line in ENV_EXAMPLE.read_text(encoding="utf-8").splitlines():
        if line.startswith("OEP_VERSION="):
            return line.split("=", 1)[1].strip()
    raise AssertionError("OEP_VERSION missing from .env.example")


def _run_driver(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(DRIVER), *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=300,
    )


def test_runbook_stage_checklist_committed() -> None:
    assert RUNBOOK.is_file(), f"runbook missing: {RUNBOOK}"
    text = RUNBOOK.read_text(encoding="utf-8")
    assert "P12-T09" in text
    # Every stage of the reference vertical must be literally named.
    for needle in ("import", "model", "release via PDM", "render", "share"):
        assert needle.lower() in text.lower(), f"runbook missing stage: {needle}"
    for needle in ("oep-batch-convert", "oep-render-worker", "oep-web"):
        assert needle in text, f"runbook missing tool: {needle}"
    # The closed SKIP taxonomy names the honest classifications (same contract
    # as the P8/P9/P10/P12 gates).
    for code in ("corpus-missing", "bin-missing", "audit-unavailable"):
        assert code in text, f"runbook missing taxonomy code: {code}"
    # Decision #10 on-prem posture + clean-room invariant.
    assert re.search(r"127\.0\.0\.1|localhost", text, re.I), "runbook must show the on-prem bind"
    assert "clean-room" in text.lower(), "runbook must state the clean-room invariant"


def test_driver_present_and_executable() -> None:
    assert DRIVER.is_file(), f"driver missing: {DRIVER}"
    assert os.access(DRIVER, os.X_OK), "scripts/e2e-vertical.sh must be executable"
    assert "P12-T09" in DRIVER.read_text(encoding="utf-8")
    assert DRIVER.read_text(encoding="utf-8").startswith("#!/usr/bin/env bash")
    assert MODULE.is_file(), "python/oep_tooling/e2e_vertical.py missing"


def test_driver_selfcheck_green() -> None:
    out = _run_driver("--selfcheck", "--out", "/tmp/p12t09-selfcheck-probe")
    assert out.returncode == 0, f"selfcheck failed:\n{out.stdout}{out.stderr}"
    report = Path("/tmp/p12t09-selfcheck-probe.json")
    assert report.is_file(), "selfcheck did not write a probe report"
    data = json.loads(report.read_text(encoding="utf-8"))
    assert data["exit_code"] == 0
    sums = data["summary"]
    assert sums["FAIL"] == 0, f"selfcheck FAIL rows: {data['rows']}"
    assert {r["id"] for r in data["rows"]} >= {"fixtures"}, data["rows"]


def test_committed_e2e_proof_green() -> None:
    """The recorded green run for the current release: every vertical stage
    PASS, zero FAIL, exit_code 0 — the \"passes on every release\" record."""
    version = _env_example_version()
    proof = PROOF_DIR / f"e2e-vertical-{version}.json"
    assert proof.is_file(), f"no committed E2E proof for release {version}: {proof}"
    data = json.loads(proof.read_text(encoding="utf-8"))
    assert data["version"] == version
    assert data["exit_code"] == 0, f"proof exit_code must be 0, got {data['exit_code']}"
    assert data["summary"]["FAIL"] == 0, f"proof has FAIL rows: {data['summary']}"
    by_id = {r["id"]: r for r in data["rows"]}
    # The primary stage row of every stage must be PASS. Additional sub-rows
    # (e.g. audit under RELEASE) may classify SKIP under the closed taxonomy
    # (audit-unavailable) exactly like every other gate's live/honest legs.
    for stage in STAGES:
        assert stage in by_id, f"proof missing primary {stage} row: {list(by_id)}"
        assert by_id[stage]["status"] == "PASS", f"{stage} not PASS: {by_id[stage]}"
    # The rendered + shared surface must be real (drawn pixels > 0 implies the
    # geometry actually rendered; the CGR leg may be SKIP under clean-room).
    share = [r for r in data["rows"] if r["id"] == "share"][0]
    assert "drawn" in share["detail"] or "shareable" in share["detail"], share


def test_committed_broken_stage_proof_blocks() -> None:
    """A deliberately broken stage must BLOCK the gate: the committed repro has
    a FAIL row in exactly the broken stage and exit_code != 0. This proves a
    regression cannot pass, which is the acceptance's \"proving it blocks\"."""
    version = _env_example_version()
    broken = PROOF_DIR / f"e2e-vertical-break-{version}.json"
    assert broken.is_file(), f"no committed broken-stage repro for {version}: {broken}"
    data = json.loads(broken.read_text(encoding="utf-8"))
    assert data["exit_code"] != 0, "broken-stage repro must be non-zero (it proves blocking)"
    assert data["summary"]["FAIL"] >= 1, f"no FAIL row in broken repro: {data['summary']}"
    fails = [r for r in data["rows"] if r["status"] == "FAIL"]
    assert any("deliberate" in (r.get("detail", "") + r.get("class", "")) for r in fails), (
        f"FAIL row must be a deliberate break: {fails}"
    )
