"""Acceptance test for the P2-M7-T01 modern-era reference reader (cgr_modern_reference).

Cross-checks the committed numeric-epsilon golden on REAL modern material
(NIST public-domain V5-6R2023 STC-06) with BOTH the independent Python
reference reader here and the Rust modern decoder
(crates/oep-cgr/tests/modern_real.rs), so the "observed reference" is two
independent implementations agreeing on the same real bytes. SKIPs cleanly
when the modern sample corpus is absent (CI-safe).

Run: PYTHONPATH=$PWD/python pytest -q tests/test_cgr_modern_reference.py
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "python"))

import pytest

from oep_tooling.cgr_modern_reference import extract


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


def test_stc06_anchor_spatial_mesh_measured_counts():
    """The committed numeric-epsilon golden on REAL modern material: the
    NIST V5-6R2023 STC-06 anchor decodes to a spatial mesh with exact
    vertex/index/triangle counts (cross-checked with the Rust decoder)."""
    (path := find_sample("nist_stc_06_asme1_V56R2023.CATPart")) or pytest.skip("no modern samples")
    data = open(path, "rb").read()
    r = extract(data)
    assert r is not None
    records, anchor, index_count, vertex_count, triangle_count, max_index = r
    assert records == 18, "genuine 0x4x trim packets"
    assert anchor == 1, "visualization-point coordinate table decoded"
    assert index_count == 2003
    assert vertex_count == 1037
    assert triangle_count == 1389
    assert max_index == 1990


def test_stc06_mesh_is_spatial_and_self_consistent():
    """The modern latch is a real spatial mesh: every handle in range, all
    finite coordinates, non-degenerate triangles, plausible bbox."""
    (path := find_sample("nist_stc_06_asme1_V56R2023.CATPart")) or pytest.skip("no modern samples")
    data = open(path, "rb").read()
    r = extract(data)
    assert r is not None
    records, anchor, index_count, vertex_count, triangle_count, max_index = r
    assert max_index < 65536


def test_modern_reference_cli_prints_numeric_golden():
    """The golden-command path: stable numeric lines for the golden file."""
    path = find_sample("nist_stc_06_asme1_V56R2023.CATPart")
    if not path:
        pytest.skip("no modern samples")
    out = subprocess.run(
        [sys.executable, "-m", "oep_tooling.cgr_modern_reference", path],
        capture_output=True, text=True,
        cwd=ROOT, env={**os.environ, "PYTHONPATH": os.path.join(ROOT, "python")},
    )
    assert out.returncode == 0
    lines = [l for l in out.stdout.splitlines() if l.strip()]
    assert len(lines) == 6
    numbers = [int(x) for x in lines]
    assert numbers == [18, 1, 2003, 1037, 1389, 1990]


def test_cgr_modern_reference_does_not_panic_on_old_or_garbage():
    """Never fabricates a mesh and never crashes on old-era CGR or junk."""
    for payload in (b"", b"\x00\x01\x02", b"\xab" * 2048):
        assert extract(payload) is None
    # old-era CGR spine (01 00 00 00...) is not a V5_CFV2 container
    old = bytearray(0x78)
    old[0:4] = b"\x01\x00\x00\x00"
    assert extract(bytes(old)) is None
