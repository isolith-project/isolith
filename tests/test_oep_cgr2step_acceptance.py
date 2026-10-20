# -*- coding: utf-8 -*-
"""P2-M5-T02 acceptance (independent-reader leg) for the `oep-cgr2step` CLI.

TDD: written BEFORE the CLI existed. Verifies the produced STEP file with an
INDEPENDENT ISO 10303-21 parser (`steputils`) in addition to the Rust-side
structural asserts in crates/oep-cgr/tests/step_writer.rs.

Legs:
  1. Deterministic synthetic CGR payload (observed header spine + a simple
     indexed triangle stream) -> CLI -> STEP. `steputils` must parse it and
     the parsed DATA section must contain the faceted-shell entity graph
     (SHELL_BASED_SURFACE_MODEL / CLOSED_SHELL / OPEN_SHELL / FACE_SURFACE /
     POLY_LOOP / PLANE) and the honest tessellated-not-exact markers.
  2. Real-material: every sample in OEP_CGR_SAMPLES reaches a *classified*
     CLI outcome (STEP produced, or a documented non-zero exit code) and is
     never killed by a signal. When a STEP is produced it must parse with
     `steputils`. SKIPs cleanly when the corpus is absent so CI stays green
     before corpus sync (P1-M1-T05).

`steputils` is an optional test dependency: when it is not installed the
independent-parse asserts are skipped (the repo gate and CI run python3
without it), so this file must never fail on import.
"""
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path

import pytest

try:
    from steputils import p21

    STEPUTILS_AVAILABLE = True
except Exception:  # pragma: no cover - CI python3 has no steputils
    STEPUTILS_AVAILABLE = False
    p21 = None

REPO_ROOT = Path(__file__).resolve().parent.parent
CLI_NAME = "oep-cgr2step"

# Documented CLI exit-code contract (see docs/cgr-to-step.md).
EXIT_OK, EXIT_IO, EXIT_USAGE, EXIT_NOT_CGR, EXIT_SKIP, EXIT_INTERNAL = range(6)
DOCUMENTED_CODES = {EXIT_OK, EXIT_IO, EXIT_USAGE, EXIT_NOT_CGR, EXIT_SKIP, EXIT_INTERNAL}

SYNTHETIC_HEADER = bytes([0x01, 0x00, 0x00, 0x00]) + bytes(0x60) + (0x10CD3600).to_bytes(4, "little")
SYNTHETIC_TEMPLATE = bytes(
    [0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F]
)


def sample_dirs():
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES")
    if env:
        for chunk in env.replace(",", ":").split(":"):
            if chunk.strip():
                dirs.append(Path(chunk.strip()))
    dirs.append(Path("samples"))
    dirs.append(REPO_ROOT / "samples")
    return dirs


def all_cgr_samples():
    out = []
    for d in sample_dirs():
        if not d.is_dir():
            continue
        for p in sorted(d.iterdir()):
            if p.suffix.lower() == ".cgr":
                out.append(p)
    return sorted(set(out))


def synthetic_grid_payload(cols=6, rows=5):
    """CGR-shaped payload with a mesh the extractor latches (5x6 grid).

    Mirrors the observed header facts in P2-M2-T01's golden synthetic payload;
    30 vertices, 40 triangles.
    """
    b = bytearray(SYNTHETIC_HEADER)
    b += SYNTHETIC_TEMPLATE
    for y in range(rows):
        for x in range(cols):
            b += struct.pack("<fff", float(x), float(y), 0.0)
    b += bytes([0xFF]) * 64  # separator: NaN as f32, opaque as run of u16
    idx = []
    for y in range(rows - 1):
        for x in range(cols - 1):
            a = y * cols + x
            c = y * cols + x + 1
            b2 = (y + 1) * cols + x
            c2 = (y + 1) * cols + x + 1
            idx += [a, b2, c, b2, c2, c]
    for i in idx:
        b += struct.pack("<H", i)
    return bytes(b)


def build_cli(tmp_path):
    """Build (once per session) and return the CLI binary path."""
    cache = getattr(build_cli, "_bin", None)
    if cache is not None and cache.exists():
        return cache
    print("building oep-cgr2step ...", flush=True)
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(tmp_path)
    subprocess.run(
        ["cargo", "build", "-p", "oep-cgr", "--bin", CLI_NAME],
        cwd=REPO_ROOT,
        env=env,
        check=True,
        capture_output=True,
    )
    bin_path = tmp_path / "debug" / CLI_NAME
    assert bin_path.exists(), "CLI binary built"
    build_cli._bin = bin_path
    return bin_path


def run_cli(bin_path, cgr_path, out_path):
    proc = subprocess.run(
        [str(bin_path), str(cgr_path), str(out_path)],
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout, proc.stderr


def parse_step_entities(text):
    """Independent-parse `text` with steputils.

    Returns (counts_by_entity_name, raw_lowercase_text). Raises on malformed
    PART 21 syntax (the independent-reader acceptance).
    """
    d = p21.loads(text)
    counts = {}
    for inst in d.data[0].instances.values():
        if hasattr(inst, "entity"):
            name = inst.entity.name
            counts[name] = counts.get(name, 0) + 1
        else:  # ComplexEntityInstance (e.g. units context)
            for e in inst.entities:
                name = e.name
                counts[name] = counts.get(name, 0) + 1
    return counts, text.lower()


@pytest.fixture(scope="session")
def cli_bin(tmp_path_factory):
    return build_cli(tmp_path_factory.getbasetemp())


def test_cli_usage_error(cli_bin):
    proc = subprocess.run([str(cli_bin)], capture_output=True, text=True)
    assert proc.returncode == EXIT_USAGE
    assert "usage" in proc.stderr.lower()


def test_cli_converts_mesh_cgr_to_step_parsed_by_independent_reader(cli_bin, tmp_path):
    payload = synthetic_grid_payload()
    inp = tmp_path / "grid.cgr"
    out = tmp_path / "grid.step"
    inp.write_bytes(payload)
    rc, stdout, stderr = run_cli(cli_bin, inp, out)
    assert rc == EXIT_OK, f"exit 0 expected, got {rc}: {stderr}"
    assert out.exists()
    text = out.read_text()
    assert text.startswith("ISO-10303-21;")
    assert text.rstrip().endswith("END-ISO-10303-21;")

    # Independent-reader leg: steputils must parse the file (syntax) and the
    # data section must show the faceted-shell B-Rep + honest markers.
    if not STEPUTILS_AVAILABLE:
        pytest.skip("steputils not installed (optional test dependency)")
    counts, lowered = parse_step_entities(text)
    assert counts.get("SHELL_BASED_SURFACE_MODEL", 0) == 1
    assert counts.get("OPEN_SHELL", 0) == 1, "open grid mesh -> OPEN_SHELL"
    assert "CLOSED_SHELL" not in counts, "open mesh must not claim CLOSED_SHELL"
    assert counts.get("FACE_SURFACE", 0) == 40, "40 triangles -> 40 planar faces"
    assert counts.get("POLY_LOOP", 0) == 40
    assert counts.get("PLANE", 0) == 40
    assert counts.get("CARTESIAN_POINT", 0) > 0
    assert "tessellat" in lowered, "marked tessellated"
    assert "not " in lowered and "exact" in lowered, "says NOT exact"


def test_real_material_every_sample_classified_cli_outcome(cli_bin, tmp_path):
    samples = all_cgr_samples()
    if not samples:
        print("SKIP: no real CGR samples supplied (OEP_CGR_SAMPLES unset)")
        return
    n_success = 0
    n_notcgr = 0
    n_skip = 0
    for p in samples:
        out = tmp_path / f"{p.stem}.step"
        if out.exists():
            out.unlink()
        rc, stdout, stderr = run_cli(cli_bin, p, out)
        assert rc in DOCUMENTED_CODES, (
            f"CLI returned undocumented code {rc} on {p.name}: {stderr}"
        )
        if rc == EXIT_OK:
            n_success += 1
            assert out.exists()
            text = out.read_text()
            assert text.startswith("ISO-10303-21;")
            if STEPUTILS_AVAILABLE:
                counts, lowered = parse_step_entities(text)
                assert counts.get("SHELL_BASED_SURFACE_MODEL", 0) >= 1
        elif rc == EXIT_NOT_CGR:
            n_notcgr += 1
        elif rc == EXIT_SKIP:
            n_skip += 1
        else:  # I/O or internal on a readable sample is a real failure
            assert False, f"unexpected code {rc} on readable sample {p.name}"
    print(
        f"summary: {n_success} step, {n_notcgr} not-cgr, {n_skip} skip of {len(samples)}",
    )
    assert n_success + n_notcgr + n_skip == len(samples)
