"""P10-M2-T02 acceptance: the signed offline/air-gap install path.

The card delivers HARDWARE-CLASS OFFLINE INSTALL TOOLING, not a document:
an air-gapped ITAR/defense machine with NO network access installs the full
Keelworks stack from a signed offline bundle and passes the same health checks
as online install (decision #10), with no phone-home and the update checker
off by default (decision #11), and signed artifacts (P11 tie).

This test pins the committed deliverable on real material (the files in the
repo): the four scripts, the public verification key, the update-checker
defaults, and the fact that the installer is pull-free (never touches a
registry). The live docker run is recorded as the committed proof in
workstreams/P10-M2/proof/ (U1, network disabled); CI holds these contract facts.
"""

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
AIRGAP = REPO_ROOT / "scripts" / "airgap"
DOC = REPO_ROOT / "workstreams/P10-M2" / "air-gap-install.md"

SCRIPTS = [
    "build-offline-bundle.sh",
    "airgap-install.sh",
    "airgap-verify.sh",
    "airgap-rollback.sh",
    "airgap-lib.sh",
]


@pytest.fixture(scope="module")
def airgap_lib_text() -> str:
    lib = AIRGAP / "airgap-lib.sh"
    assert lib.is_file(), f"airgap-lib.sh missing: {lib}"
    return lib.read_text(encoding="utf-8")


def test_doc_exists() -> None:
    assert DOC.is_file(), f"air-gap doc missing: {DOC}"
    text = DOC.read_text(encoding="utf-8")
    assert "Version: 1.0.0" in text
    assert "decision #10" in text
    assert "decision #11" in text


def test_all_scripts_committed_and_executable() -> None:
    missing = [s for s in SCRIPTS if not (AIRGAP / s).is_file()]
    assert not missing, f"missing air-gap scripts: {missing}"
    for s in SCRIPTS:
        assert os_access_x(AIRGAP / s), f"{s} must be executable"


def test_public_verification_key_committed() -> None:
    key = AIRGAP / "keys" / "oep-airgap-signing.pub.asc"
    assert key.is_file(), f"public key missing: {key}"
    text = key.read_text(encoding="utf-8")
    assert text.startswith("-----BEGIN PGP PUBLIC KEY BLOCK-----")


def test_update_checker_off_by_default(airgap_lib_text: str) -> None:
    # Decision #11: disable-able + non-phoning by default.
    assert "OEP_UPDATE_CHECK=off" in airgap_lib_text
    assert "OEP_UPDATE_MIRROR_URL=" in airgap_lib_text
    assert "OEP_UPDATE_OFFLINE_BUNDLE=" in airgap_lib_text


def test_installer_never_pulls(airgap_lib_text: str) -> None:
    install = (AIRGAP / "airgap-install.sh").read_text(encoding="utf-8")
    # The whole point: no registry, no internet. docker load only.
    assert "docker load" in install
    assert "docker pull" not in install
    assert "docker compose -f" in install
    assert "up -d" in install
    # Decision #11: the offline installer hard-refuses any non-off update-check
    # posture unless it is a LOCAL offline bundle that actually exists — no
    # phone-home, no reach-back.
    assert "OEP_UPDATE_CHECK" in install
    assert "no phone-home" in install.lower() or "no phone-home" in install
    # And the baked-in template default is off (from airgap-lib.sh).
    assert "OEP_UPDATE_CHECK=off" in airgap_lib_text


def test_bundle_builder_signs_manifest_and_bundle() -> None:
    build = (AIRGAP / "build-offline-bundle.sh").read_text(encoding="utf-8")
    assert "gpg_detach_sign" in build
    assert "MANIFEST.json" in build
    assert "SHA256SUMS" in build


def test_doc_covers_the_seven_step_airgap_flow() -> None:
    text = DOC.read_text(encoding="utf-8")
    for word in ["build", "transfer", "load", "configure", "start", "verify", "rollback"]:
        assert word in text, f"air-gap doc must cover step: {word}"
    assert "airgap-install-record.json" in text


def os_access_x(path: Path) -> bool:
    return bool(path.stat().st_mode & 0o111)
