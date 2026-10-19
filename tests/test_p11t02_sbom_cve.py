#!/usr/bin/env python3
"""P11-T02 acceptance (TDD) - dependency SBOM + CVE gate.

Written BEFORE `oep_tooling.sbom_cve` landed (RED), these checks pin what the
gate must do, then go GREEN against the shippable tool:

  1. SBOM leg  - the release emits CycloneDX + SPDX BOMs for the Rust crate
     tree (from the committed Cargo.lock: the exact thing the release build
     resolves) and a CycloneDX BOM for the container images the release
     embeds (base images declared in the repo Dockerfiles). Emitted output is
     well-formed JSON with pkg:cargo purls and reproducible content.
  2. Scan/gate - an UNFIXED critical vulnerability in a dependency that is in
     the lockfile (a test/dev dependency is resolved into Cargo.lock exactly
     like a normal one, which is the seeded-critical-vuln-in-a-test-
     dependency acceptance) BLOCKS the gate (exit 1). Moderate/low/
     informational findings do NOT block.
  3. Waivers   - a committed waiver (workstreams/P11/waivers.toml semantics) that matches
     the finding lifts the block (exit 0) and is surfaced in the report as
     waived; a revoked/absent waiver blocks again. Waivers live in a committed
     file, so the release carries an auditable record.
  4. Green baseline - scanning the REAL release-workspace Cargo.lock against
     the REAL rustsec advisory-db must be green (exit 0, zero blockers) when
     the advisory-db is locally available. The CI job clones the DB before
     running these tests; locally the test degrades to skip when the DB is
     absent (hermetic CI-safe rule, same as the other lane acceptance tests).

Hermetic fixtures live under tests/fixtures/p11t02/: a mini repo (Cargo.lock
+ Dockerfile) and a hand-authored advisory-db in the exact rustsec layout
(fenced-toml .md files) - no network, no copied third-party content.
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(ROOT / "python" / "oep_tooling"))
import oep_tooling.sbom_cve as S  # noqa: E402

FIX = ROOT / "tests" / "fixtures" / "p11t02"
FIX_REPO = FIX / "fixturerepo"
FIX_DB = FIX / "advisory-db"
REPO_ROOT_REAL = ROOT

CRIT_ADV = "RUSTSEC-0000-2026-CRIT"
INFO_ADV = "RUSTSEC-0000-2026-INFO"


def _run(argv, expect_report=True):
    """Run the CLI in-process; returns (exit_code, outdir)."""
    outdir = tempfile.mkdtemp(prefix="p11t02-")
    full = [a.format(outdir=outdir) for a in argv]
    try:
        rc = S.main(full)
    except SystemExit as e:
        rc = e.code if e.code is not None else 0
    boms = ("rust.cdx.json", "rust.spdx.json", "images.cdx.json")
    for name in boms:
        assert (Path(outdir) / name).exists(), f"expected BOM artifact {name} in {outdir}"
    if expect_report:
        assert (Path(outdir) / "scan-report.json").exists(), f"expected scan-report.json in {outdir}"
    return rc, Path(outdir)


def _read_report(outdir: Path) -> dict:
    return json.loads((outdir / "scan-report.json").read_text())


# --------------------------------------------------------------------------
# matcher unit checks (rust semver ranges used by advisories)
# --------------------------------------------------------------------------

@pytest.mark.parametrize("req,ver,expected", [
    (">= 0.2.23", "0.2.23", True),
    (">= 0.2.23", "0.1.25", False),
    ("< 0.8.0", "0.7.0", True),
    ("< 0.8.0", "0.8.0", False),
    ("= 0.4.3", "0.4.3", True),
    ("= 0.4.3", "0.4.4", False),
    (">= 0.6.14, < 1.0.0", "0.6.16", True),
    (">= 0.6.14, < 1.0.0", "1.0.2", False),
    ("^0.3.4", "0.3.9", True),
    ("^0.3.4", "0.4.0", False),
    ("^1.2.3", "1.9.0", True),
    ("^1.2.3", "2.0.0", False),
    ("~1.2.3", "1.2.9", True),
    ("~1.2.3", "1.3.0", False),
    ("1.5.0", "1.5.0", True),      # bare version => caret semantics
    ("1.5.0", "1.9.9", True),
    ("1.5.0", "2.0.0", False),
    ("^1.0.2-alpha", "1.0.2-alpha", True),  # pre-release lower bound matches itself
    ("^1.0.2-alpha", "1.0.2", True),        # stable above the pre-release still matches
    ("=1.0.2", "1.0.2-alpha", False),       # exact needs a stable match
    ("*", "0.0.0", True),
])
def test_version_req_matches(req, ver, expected):
    assert S.version_req_matches(req, ver) is expected


# --------------------------------------------------------------------------
# SBOM emission
# --------------------------------------------------------------------------

def test_emit_sbom_fixture():
    rc, out = _run([
        "sbom", "--repo", str(FIX_REPO), "--ref", "fixture-1.0.0", "--out", "{outdir}",
    ], expect_report=False)
    assert rc == 0
    cdx = json.loads((out / "rust.cdx.json").read_text())
    spdx = json.loads((out / "rust.spdx.json").read_text())
    imgs = json.loads((out / "images.cdx.json").read_text())
    # 4 lockfile packages -> 4 components; images -> 2 base images
    assert cdx["bomFormat"] == "CycloneDX" and cdx["specVersion"] == "1.5"
    assert len(cdx["components"]) == 4
    assert len(spdx["packages"]) == 4
    purls = {c["purl"] for c in cdx["components"]}
    assert "pkg:cargo/keelworks-fixture-unsafe@1.5.0" in purls
    assert all(p.startswith("pkg:cargo/") for p in purls)
    assert [i["name"] for i in imgs["components"]] == ["debian:bookworm-slim", "rust:1.98-bookworm"]
    assert len(spdx["relationships"]) == 4


def test_sbom_head_stays_over_100_lockfile_components():
    # real lockfile should produce a non-trivial component set; guards against
    # silently regressing to an empty BOM
    lock_txt, pkgs = S._parse_lockfile_and_pkgs(REPO_ROOT_REAL)
    cdx = S.emit_rust_cyclonedx(lock_txt, pkgs, "test")
    assert len(cdx["components"]) >= 100


def test_sbom_deterministic_content():
    lock_txt, pkgs = S._parse_lockfile_and_pkgs(FIX_REPO)
    a = S.emit_rust_cyclonedx(lock_txt, pkgs, "fixture-1.0.0")
    b = S.emit_rust_cyclonedx(lock_txt, pkgs, "fixture-1.0.0")
    assert a["serialNumber"] == b["serialNumber"]
    assert json.dumps(a["components"], sort_keys=True) == json.dumps(b["components"], sort_keys=True)


# --------------------------------------------------------------------------
# CVE gate: seeded critical vuln blocks, waivers lift/revoke
# --------------------------------------------------------------------------

def test_gate_blocks_seeded_critical():
    rc, out = _run([
        "gate", "--repo", str(FIX_REPO), "--ref", "fixture-1.0.0", "--out", "{outdir}",
        "--advisory-db", str(FIX_DB), "--offline",
    ])
    assert rc == 1, "seeded critical vuln MUST fail the release gate"
    rep = _read_report(out)
    assert rep["gate"] == "FAIL"
    ids = [b["id"] for b in rep["blocking"]]
    assert CRIT_ADV in ids
    assert rep["counts"]["blockers"] == 1
    # the informational finding is present but must NOT block
    row = next(r for r in rep["index"] if r["id"] == INFO_ADV)
    assert row["severity"] == "informational"
    assert row["blocking"] is False


def test_gate_waiver_lifts_block_and_is_surfaced():
    with tempfile.TemporaryDirectory() as td:
        wf = Path(td) / "waivers.toml"
        wf.write_text(
            f'[[waiver]]\nid = "{CRIT_ADV}"\npackage = "keelworks-fixture-unsafe"\n'
            f'document = "deliberate-failure repro"\nreason = "fixture: waiver mechanism honours committed waivers"\n'
        )
        rc, out = _run([
            "gate", "--repo", str(FIX_REPO), "--ref", "fixture-1.0.0", "--out", "{outdir}",
            "--advisory-db", str(FIX_DB), "--offline", "--waivers", str(wf),
        ])
    assert rc == 0, "matching waiver must lift the block"
    rep = _read_report(out)
    assert rep["gate"] == "PASS"
    assert rep["counts"]["blockers"] == 0
    row = next(r for r in rep["index"] if r["id"] == CRIT_ADV)
    assert row["waived"] is True


def test_gate_revoked_waiver_still_blocks():
    with tempfile.TemporaryDirectory() as td:
        wf = Path(td) / "waivers.toml"
        wf.write_text(
            f'[[waiver]]\nid = "{CRIT_ADV}"\npackage = "keelworks-fixture-unsafe"\n'
            'revoked = true\nreason = "revocation must re-block"\n'
        )
        rc, out = _run([
            "gate", "--repo", str(FIX_REPO), "--ref", "fixture-1.0.0", "--out", "{outdir}",
            "--advisory-db", str(FIX_DB), "--offline", "--waivers", str(wf),
        ])
    assert rc == 1, "a revoked waiver must not lift the block"
    rep = _read_report(out)
    assert rep["counts"]["blockers"] == 1


# --------------------------------------------------------------------------
# green baseline: real workspace lockfile vs the real advisory-db
# --------------------------------------------------------------------------

def test_real_workspace_scan_green_or_skip():
    """The release-branch gate must be green. Runs only when the real
    advisory-db is present (the CI job clones it first); otherwise hermetic
    SKIP - pytest must pass offline."""
    db = ROOT / "workstreams/P11" / ".advisory-db"
    if not (db / "crates").exists():
        pytest.skip("real rustsec advisory-db not present (workstreams/P11/.advisory-db); CI clones it first")
    rc, out = _run([
        "gate", "--repo", str(ROOT), "--ref", "scan-test", "--out", "{outdir}",
        "--advisory-db", str(db), "--offline",
    ])
    rep = _read_report(out)
    assert rc == 0, f"real release workspace must scan green; blockers={rep['counts']['blockers']}"
    assert rep["counts"]["blockers"] == 0
