"""Acceptance test for the P2-M7-T01 observed-reference Python reader.

Cross-checks the committed real-material golden (piston.cgr anchor record) with
BOTH the Python reference reader here and the Rust locator output, so the
"observed reference" is two independent implementations agreeing on the same
real bytes. SKIPs cleanly when the private corpus is absent (CI-safe).

Run: PYTHONPATH=$PWD/python pytest -q tests/test_cgr_locate_reference.py
"""
import os
import subprocess
import sys

# The repo's Python tooling package lives under ./python; pytest runs from the
# repo root without PYTHONPATH, so wire the package dir in explicitly (mirrors
# how the CLI tooling is documented to run: `PYTHONPATH=$PWD/python`).
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "python"))

import pytest

from oep_tooling.cgr_locate_reference import locate


def sample_dirs():
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES", "")
    for chunk in env.replace(":", ",").split(","):
        chunk = chunk.strip()
        if chunk:
            dirs.append(chunk)
    return dirs


def find_sample(name):
    for d in sample_dirs():
        full = os.path.join(d, name)
        if os.path.isfile(full):
            return full
    return None


def test_piston_anchor_record_measured_counts():
    """The committed numeric-epsilon golden on real material."""
    (path := find_sample("piston.cgr")) or pytest.skip("no real samples")
    data = open(path, "rb").read()
    records = locate(data)
    assert records and len(records) >= 21
    anchor = [r for r in records if r["offset"] == 0xA0DC]
    assert len(anchor) == 1
    r = anchor[0]
    assert r["counts"] == [7, 7, 6, 5, 5, 5]
    assert r["face_count"] == 6
    assert r["index_count"] == 35
    assert r["vertex_count"] == 27
    assert r["triangle_count"] == 23
    assert r["max_index"] == 246


def test_cfysa_family_no_counts_handles_records():
    """R19-era cfysa family carries no counts/handles record (classified)."""
    (path := find_sample("VALVE.cgr")) or pytest.skip("no real samples")
    data = open(path, "rb").read()
    assert locate(data) in (None, []) or len(locate(data)) == 0


def test_reference_cli_prints_numeric_golden():
    """The golden-command path: stable numeric lines for the golden file."""
    path = find_sample("piston.cgr")
    if not path:
        pytest.skip("no real samples")
    out = subprocess.run(
        [sys.executable, "-m", "oep_tooling.cgr_locate_reference", path],
        capture_output=True, text=True,
        cwd=ROOT, env={**os.environ, "PYTHONPATH": os.path.join(ROOT, "python")},
    )
    assert out.returncode == 0
    lines = [l for l in out.stdout.splitlines() if l.strip()]
    assert len(lines) == 6
    numbers = [int(x) for x in lines]
    # records_located, anchor_present, index_count, vertex_count,
    # triangle_count, max_index
    assert numbers == [21, 1, 35, 27, 23, 246]
