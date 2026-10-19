"""P10-M5-T01 acceptance: the update checker feature (decision #11).

The card builds a REAL component (the `oep-update` crate + two CLI binaries),
so the design note (workstreams/P10-M5/update-checker.md) and the sample config
(workstreams/P10-M5/update-check.sample.json) are the doc layer this test pins, and the
Rust integration suite (crates/oep-update/tests/update_checker_acceptance.rs)
is the executable layer. This Python test makes sure the doc and the shipped
config cannot drift away from the acceptance items: advisory + non-blocking,
configurable source, air-gap safe, signed payloads, disable-able and
non-phoning by default, no analytics reach-back.
"""

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
DOC = REPO_ROOT / "workstreams/P10-M5" / "update-checker.md"
SAMPLE = REPO_ROOT / "workstreams/P10-M5" / "update-check.sample.json"
ACCEPTANCE_RS = REPO_ROOT / "crates" / "oep-update" / "tests" / "update_checker_acceptance.rs"


@pytest.fixture(scope="module")
def doc_text() -> str:
    assert DOC.is_file(), f"design doc missing: {DOC}"
    return DOC.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def sample() -> dict:
    assert SAMPLE.is_file(), f"sample config missing: {SAMPLE}"
    import json

    return json.loads(SAMPLE.read_text(encoding="utf-8"))


def test_doc_exists_and_names_decision(doc_text: str) -> None:
    assert "decision #11" in doc_text
    assert "advisory" in doc_text.casefold()
    assert "Version: 1" in doc_text


def test_doc_says_non_blocking_and_never_fails_closed(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "non-blocking" in lowered
    assert "never fails closed" in lowered
    assert "no update gate" in lowered


def test_doc_says_sources_are_configurable_and_airgap(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "configurable source" in lowered
    assert "remote url" in lowered
    assert "on-site mirror" in lowered
    assert "offline signed bundle" in lowered
    assert "air-gap" in lowered


def test_doc_says_payloads_signed_before_download(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "signed" in lowered
    assert "before any download" in lowered


def test_doc_says_no_analytics_reach_back(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "no analytics reach-back" in lowered
    assert "telemetry" in lowered
    assert "non-phoning" in lowered


def test_doc_behavior_matrix_has_no_block(doc_text: str) -> None:
    # Every row in the behavior matrix must map to Proceed - never Block.
    lowered = doc_text.casefold()
    assert "proceed" in lowered
    # The only "Block" mention is the explicit negation: no Block variant.
    assert "no `block` variant" in lowered or "no block variant" in lowered


def test_sample_config_is_off_by_default(sample: dict) -> None:
    # Default deployment: non-phoning, no source. Acceptance +
    # decision #11 "disable-able and non-phoning by default".
    assert sample.get("enabled") is False
    assert sample.get("source") is None
    assert sample.get("public_key") is None


def test_sample_names_all_three_source_shape(sample: dict) -> None:
    # The configurable source must document all three accepted shapes:
    # remote URL, on-site mirror, offline signed bundle.
    blob = SAMPLE.read_text(encoding="utf-8")
    assert "mirror" in blob
    assert "url" in blob
    assert "bundle" in blob


def test_acceptance_suite_committed() -> None:
    assert ACCEPTANCE_RS.is_file(), f"acceptance tests missing: {ACCEPTANCE_RS}"
    text = ACCEPTANCE_RS.read_text(encoding="utf-8")
    assert "launch_without_source" in text
    assert "launch_with_old_bundle" in text
    assert "tampered_bundle_signature" in text
    assert "no public key" in text or "public_key" in text
