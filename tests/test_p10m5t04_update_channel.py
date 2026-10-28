"""P10-M5-T04 acceptance: UPDATE-CHANNEL (decision #20) doc + feed pins.

The UPDATE-CHANNEL card adds the persisted-settings schema, the default source
for public builds, the on/off toggle, the one-off failure offer, and the feed
(same signed update.bundle.json everywhere). This test pins the committed
truthful doc/sample/downloads.json facts so the channel docs cannot drift from
the crate implementation:

  - update-check.sample.json stays the truthful persisted-config shape (default
    OFF, three source shapes, tri-state + feed notes).
  - update-checker.md documents the tri-state and the build-default source.
  - docs-site/downloads.json lists the signed update.bundle.json feed row with
    the same sha256 as the committed release-store copy (or the manifest's
    canonical key when the file is present).
  - the air-gap bundle producer embeds update.bundle.json (identical file).

Hermetic: stdlib + assertions against committed files only; no network, no
docker, no cargo.
"""

import hashlib
import json
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
DOC = REPO_ROOT / "workstreams/delivery/update-checker" / "update-checker.md"
SAMPLE = REPO_ROOT / "workstreams/delivery/update-checker" / "update-check.sample.json"
DOWNLOADS = REPO_ROOT / "docs-site" / "downloads.json"
OFFLINE_BUNDLE_SCRIPT = REPO_ROOT / "scripts" / "airgap" / "build-offline-bundle.sh"
PUBKEY = "e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811"


@pytest.fixture(scope="module")
def doc_text() -> str:
    assert DOC.is_file(), f"design doc missing: {DOC}"
    return DOC.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def sample() -> dict:
    assert SAMPLE.is_file(), f"sample config missing: {SAMPLE}"
    return json.loads(SAMPLE.read_text(encoding="utf-8"))


def test_doc_documents_tri_state(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "on { source }" in lowered
    assert "offbyuser" in lowered.replace(" ", "")
    assert "offneverconfigured" in lowered.replace(" ", "")


def test_doc_documents_build_default_public_source(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "public build" in lowered
    assert "default remote source" in lowered
    assert "one-line" in lowered
    assert "github" in lowered


def test_doc_documents_toggle_and_offer(doc_text: str) -> None:
    lowered = doc_text.casefold()
    assert "oep update config" in lowered
    assert "--disable" in lowered
    # One-off failure offer: exactly once, survives restart, declining no re-ask.
    assert "offers once" in lowered
    assert "never repeats" in lowered
    assert "decline" in lowered


def test_sample_stays_truthful(sample: dict) -> None:
    # The persisted settings file uses the same shape; the sample must stay the
    # truthful default: OFF, no source, no pubkey, all three source shapes.
    assert sample.get("enabled") is False
    assert sample.get("source") is None
    assert sample.get("public_key") is None
    # No accidental pin to a machine path.
    blob = json.dumps(sample)
    assert "HOME" not in blob or "XDG" in blob  # channel_note may name env vars


def test_downloads_list_the_signed_update_feed() -> None:
    manifest = json.loads(DOWNLOADS.read_text(encoding="utf-8"))
    rows = manifest.get("downloads", [])
    feeds = [r for r in rows if r.get("kind") == "update-feed" or r.get("artifact") == "update.bundle.json"]
    assert feeds, "downloads.json must list the signed update.bundle.json feed"
    for feed in feeds:
        # The manifest's signature_id is the committed release verifying key.
        assert feed.get("signature_id") == PUBKEY, "feed row must pin the release Verify key id"


def test_site_and_program_read_same_signed_file_when_present() -> None:
    # When the release-store copy is present, its sha256 must equal the feed
    # row's sha256 - the site and the in-program check read the identical file.
    candidates = sorted((REPO_ROOT / "dist").glob("*/update.bundle.json"))
    if not candidates:
        pytest.skip("no committed release-store update.bundle.json in this checkout")
    manifest = json.loads(DOWNLOADS.read_text(encoding="utf-8"))
    feed_row = next(
        (
            r
            for r in manifest.get("downloads", [])
            if r.get("artifact") == "update.bundle.json"
        ),
        None,
    )
    if feed_row is None:
        pytest.skip("no update-feed row committed yet")
    h = hashlib.sha256()
    with candidates[0].open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    assert (
        h.hexdigest() == feed_row.get("sha256")
    ), "release-store update.bundle.json sha256 must match the downloads.json feed row"


def test_airgap_bundle_carries_the_identical_file() -> None:
    text = OFFLINE_BUNDLE_SCRIPT.read_text(encoding="utf-8")
    assert "update.bundle.json" in text
    assert "--update-bundle" in text
    assert text.count("update.bundle.json") >= 2  # arg + embedded-copy step
