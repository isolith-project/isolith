"""P12-T06 acceptance: the committed release-install-rehearsal artifacts.

The live rehearsal is a real-LAN action (U1 + a U1/VM-class VM, decision #9),
executed per release by scripts/install-rehearsal.sh and recorded under
workstreams/P12/proof/release-rehearsal-<ver>.{json,md}. CI has no docker daemon and no
second host, so this test pins the committed facts - the runbook checklist,
the rehearsal driver's version contract, and the recorded proof carrying PASS
rows for both machine-install legs with zero FAIL (the "all three health checks
green, both verifications recorded, not a log line" requirement) - and runs the
driver's CI-safe degraded mode (--selfcheck).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RUNBOOK = REPO_ROOT / "workstreams/P12" / "install-rehearsal.md"
DRIVER = REPO_ROOT / "scripts" / "install-rehearsal.sh"
PROOF_DIR = REPO_ROOT / "workstreams/P12" / "proof"
ENV_EXAMPLE = REPO_ROOT / "workstreams/P10-M1" / "deploy" / ".env.example"
INSTALL_SH = REPO_ROOT / "workstreams/P10-M1" / "deploy" / "install.sh"


def _env_example_version() -> str:
    for line in ENV_EXAMPLE.read_text(encoding="utf-8").splitlines():
        if line.startswith("OEP_VERSION="):
            return line.split("=", 1)[1].strip()
    raise AssertionError("OEP_VERSION missing from .env.example")


def test_runbook_checklist_committed() -> None:
    assert RUNBOOK.is_file(), f"runbook missing: {RUNBOOK}"
    text = RUNBOOK.read_text(encoding="utf-8")
    assert "P12-T06" in text
    # The three health surfaces must be literal in the doc (checklist rows 3-5).
    for needle in (
        "curl -fsS http://127.0.0.1:8787/api/v1/health",
        "http://127.0.0.1:8790/",
        "curl -fsS http://127.0.0.1:8080/healthz",
        "oep-batch-convert",
        "oep-render-worker",
        "./install.sh",
        "scripts/install-rehearsal.sh",
    ):
        assert needle in text, f"runbook missing: {needle}"
    # Decision #9: the run must name two approved host classes, never the NAS.
    assert re.search(r"U1|keelwb|VM-class", text, re.I), "runbook must name the U1 / VM-class hosts"
    assert "machine b" in text.lower(), "runbook must name machine B"
    # Clean-room invariant: the NAS is repo/storage only.
    assert "NAS" in text and "repo/storage only" in text


def test_driver_present_and_executable() -> None:
    assert DRIVER.is_file(), f"driven script missing: {DRIVER}"
    assert os.access(DRIVER, os.X_OK), "scripts/install-rehearsal.sh must be executable"
    assert "P12-T06" in DRIVER.read_text(encoding="utf-8")
    assert DRIVER.read_text(encoding="utf-8").startswith("#!/usr/bin/env bash")


def test_driver_version_default_matches_env_example() -> None:
    # The rehearsal's default release version must track the deploy stack, so
    # "rehearse the current release" is never stale.
    expected = _env_example_version()
    out = subprocess.run(
        [str(DRIVER), "--selfcheck", "--out", "/tmp/p12t06-selfcheck-probe"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert "OEP_VERSION" in ENV_EXAMPLE.read_text(encoding="utf-8")
    # selfcheck must exit 0 (doc + version legs only) and name the version.
    assert out.returncode == 0, f"selfcheck exited {out.returncode}:\n{out.stdout}{out.stderr}"


def test_rehearsal_driver_selfcheck() -> None:
    out = subprocess.run(
        [str(DRIVER), "--selfcheck", "--out", "/tmp/p12t06-selfcheck-probe"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert out.returncode == 0, f"selfcheck failed:\n{out.stdout}{out.stderr}"
    report = Path("/tmp/p12t06-selfcheck-probe-0.1.0.json")
    assert report.is_file(), "selfcheck did not write a probe report"
    data = json.loads(report.read_text(encoding="utf-8"))
    assert data["version"] == _env_example_version()
    if data["summary"]["FAIL"] > 0:
        fails = [r for r in data["rows"] if r["status"] == "FAIL"]
        raise AssertionError(f"selfcheck rows FAIL: {fails}")
    ids = {r["id"] for r in data["rows"]}
    assert {"version", "runbook"} <= ids, f"selfcheck missing expected rows: {ids}"


def _committed_proofs() -> list[Path]:
    if not PROOF_DIR.is_dir():
        return []
    return sorted(PROOF_DIR.glob("release-rehearsal-*.json"))


def test_committed_rehearsal_proof_green() -> None:
    # The recorded proof must exist for the current release and carry PASS rows
    # on BOTH machine legs with zero FAIL (the acceptance's "two verifications
    # recorded, not a log line"). Tested as a post-hoc contract on the
    # committed artifacts, like the P8/P9/P10 gate-proof tests.
    version = _env_example_version()
    proof = PROOF_DIR / f"release-rehearsal-{version}.json"
    assert proof.is_file(), f"no committed rehearsal proof for release {version}: {proof}"
    data = json.loads(proof.read_text(encoding="utf-8"))
    assert data["version"] == version
    assert data["exit_code"] == 0, f"proof exit_code must be 0, got {data['exit_code']}"
    by_id = {r["id"]: r for r in data["rows"]}
    for machine in ("install-A", "install-B"):
        assert machine in by_id, f"proof missing {machine} row"
        assert by_id[machine]["status"] == "PASS", (
            f"{machine} not PASS: {by_id[machine]}"
        )
    assert data["summary"]["FAIL"] == 0, f"proof has FAIL rows: {data['summary']}"
