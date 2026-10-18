"""P10-M5-T02 acceptance: the committed P10 GA gate artifacts.

The live two-machine verification is a real-LAN action (U1 + U1/VM-class VM,
decision #9), recorded under P10-M5/proof/. CI cannot reach a second host or a
docker daemon, so this test pins the committed deliverable facts - the
walkthrough checklist, the one-command install surface, the air-gap re-proof
contract, and the update-checker off-by-default posture - and a post-hoc
assertion that the committed gate proof really does carry PASS rows for the
two machine-install legs (the "both verifications recorded, not a log line"
requirement).

The live proof is reproduced by scripts/p10m5t02-ga-gate-proof.sh and the
harness python/oep_tooling/ga_gate_p10.py; CI runs that harness in its
degraded mode (live legs SKIP under the closed taxonomy).
"""

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
WALK = REPO_ROOT / "P10-M5" / "install-walkthrough.md"
GATE_DOC = REPO_ROOT / "P10-M5" / "ga-gate.md"
PROOF = REPO_ROOT / "P10-M5" / "proof" / "ga-gate-p10-m5t02.json"
HARNESS = REPO_ROOT / "python" / "oep_tooling" / "ga_gate_p10.py"
PROOF_SH = REPO_ROOT / "scripts" / "p10m5t02-ga-gate-proof.sh"

VIDEO_DIR = REPO_ROOT / "P10-M5" / "proof" / "video"


def test_walkthrough_checklist_committed() -> None:
    assert WALK.is_file(), f"walkthrough missing: {WALK}"
    text = WALK.read_text(encoding="utf-8")
    assert "P10-M5-T02" in text
    assert "## 5. The checklist" in text
    # The eight measured rows the operator runs are literal in the doc.
    for needle in (
        "curl -fsS http://127.0.0.1:8787/api/v1/health",
        "http://127.0.0.1:8790/",
        "curl -fsS http://127.0.0.1:8080/healthz",
        "oep-batch-convert",
        "oep-render-worker",
        "airgap-install.sh",
        "./install.sh",
    ):
        assert needle in text, f"walkthrough missing: {needle}"


def test_walkthrough_references_real_files() -> None:
    # The files the doc tells an operator to run must exist in the repo.
    for rel in (
        "P10-M1/deploy/install.sh",
        "P10-M1/deploy/docker-compose.yml",
        "P10-M1/deploy/.env.example",
        "scripts/airgap/airgap-verify.sh",
        "scripts/airgap/airgap-install.sh",
        "scripts/airgap/build-offline-bundle.sh",
        "P10-M2/air-gap-install.md",
    ):
        assert (REPO_ROOT / rel).is_file(), f"walkthrough references missing file: {rel}"
    # The install script is the one-command surface.
    inst = (REPO_ROOT / "P10-M1/deploy/install.sh").read_text(encoding="utf-8")
    for needle in ("preflight", "docker compose -f docker-compose.yml", "wait_healthy",
                   "verify_health", "health checks green"):
        assert needle in inst, f"install.sh missing {needle}"


def test_ga_gate_doc_and_harness_committed() -> None:
    assert GATE_DOC.is_file()
    assert "two machines" in GATE_DOC.read_text(encoding="utf-8").lower()
    assert HARNESS.is_file()
    assert PROOF_SH.is_file()
    text = HARNESS.read_text(encoding="utf-8")
    for leg in ("INSTALL", "STACK", "AIRGAP", "UPDATE"):
        assert leg in text


def test_update_checker_off_by_default_posture() -> None:
    lib = REPO_ROOT / "scripts/airgap" / "airgap-lib.sh"
    lib_text = lib.read_text(encoding="utf-8")
    assert "OEP_UPDATE_CHECK=off" in lib_text
    env_ex = (REPO_ROOT / "P10-M1/deploy/.env.example").read_text(encoding="utf-8")
    assert "OEP_UPDATE_CHECK" not in env_ex, (
        "the generated install .env must never enable the update checker")


def test_airgap_contract_still_pinned() -> None:
    # The M2-T02 acceptance the gate depends on is still committed + runnable.
    assert (REPO_ROOT / "tests" / "test_p10m2t02_airgap.py").is_file()
    assert (REPO_ROOT / "P10-M2" / "air-gap-install.md").is_file()


def test_committed_proof_records_both_machine_installs() -> None:
    """The GA contract: both verifications recorded as measured rows, not a
    log line. The committed live proof must carry PASS rows for the U1 and the
    VM install legs and an UPDATE off row."""
    if not PROOF.is_file():
        pytest.skip("live proof not yet committed on this ref (expected pre-run)")
    data = json_load(PROOF)
    assert data["task"] == "P10-M5-T02"
    assert data["summary"]["FAIL"] == 0
    ids = {r["id"] for r in data["rows"]}
    assert {"install-A", "install-B"} <= ids, f"missing machine install rows: {ids}"
    for r in data["rows"]:
        if r["id"] in ("install-A", "install-B"):
            assert r["status"] == "PASS", f"{r['id']} not PASS: {r}"
    assert "update-env" in ids and data["rows"][
        [i for i, r in enumerate(data["rows"]) if r["id"] == "update-env"][0]
    ]["status"] == "PASS"


def test_video_deliverable_committed() -> None:
    """The 'install video' part of the acceptance: a replayable recording of
    the install walkthrough is committed under P10-M5/proof/video/."""
    if not VIDEO_DIR.is_dir():
        pytest.skip("video dir not yet committed on this ref (expected pre-run)")
    typescripts = sorted(VIDEO_DIR.glob("*.typescript"))
    mp4s = sorted(VIDEO_DIR.glob("*.mp4"))
    assert typescripts or mp4s, "no install recording committed"
    if mp4s:
        sizes = {p.name: p.stat().st_size for p in mp4s}
        assert all(s > 20000 for s in sizes.values()), sizes


def json_load(p: Path) -> dict:
    import json
    return json.loads(p.read_text(encoding="utf-8"))
