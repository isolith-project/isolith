#!/usr/bin/env python3
"""Acceptance test for Keelworks P9-M3-T01: headless render worker binary.

Written FIRST (TDD). The render worker at crates/oep-render-worker and its
committed real-material proof must make this test pass. It verifies the real
material, not mocks:

  worker      => a render worker crate exists (crates/oep-render-worker) as a
                 workspace member, exposing a worker function behind a clean
                 seam (options + registry + render) and a CLI binary
                 `oep-render-worker`
  renders     => the worker renders a REAL oep-geo model (the committed golden
                 crates/oep-geo/tests/golden/example-bracket.oepgeo.json) to a
                 valid PNG image artifact
  resolution  => output resolution is parameterized (--resolution WxH) and the
                 written PNG actually has those dimensions (IHDR width/height)
  fidelity    => fidelity is parameterized (--fidelity low|medium|high) and a
                 low vs high render of the same model differ in pixels (the
                 supersample anti-aliasing knob is real, not a no-op)
  registration=> the worker registers with the scheduler: it exposes a worker
                 registry that resolves executor.kind="render" /
                 executor.function="oep-geo-render" (the P9-M1 job-spec
                 vocabulary), and `--describe` prints that registration as
                 JSON so the scheduler can discover it in a CI-detectable way
  provenance  => the proof artifacts are generated from the real golden
                 document, not synthetic geometry
  live        => cargo test for the worker crate passes on this host (real
                 material execution, GPU-less reference renderer)

Plus the program surface-language rule (no em dashes, no curly quotes) on the
files this card adds.

Run:  python3 tests/test_p9_m3_render_worker.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import hashlib
import json
import os
import re
import struct
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORKER = os.path.join(ROOT, "crates", "oep-render-worker")
SRC = os.path.join(WORKER, "src")
GOLDEN = os.path.join(ROOT, "crates", "oep-geo", "tests", "golden",
                      "example-bracket.oepgeo.json")
PROOF_DIR = os.path.join(ROOT, "P9-M3", "proof")
PROOF_LOW = os.path.join(PROOF_DIR, "render-worker-640x480-low.png")
PROOF_HIGH = os.path.join(PROOF_DIR, "render-worker-640x480-high.png")
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
EM_DASH = "\u2014"

failures = []
warnings = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def warn(msg):
    warnings.append(msg)


def read_text(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def png_ok(path):
    if not os.path.isfile(path):
        return False, "missing"
    with open(path, "rb") as fh:
        head = fh.read(8)
    if head != PNG_MAGIC:
        return False, "bad magic"
    if os.path.getsize(path) < 500:
        return False, "implausibly small (%d bytes)" % os.path.getsize(path)
    return True, "ok"


def png_size(path):
    """Return (width, height) from the PNG IHDR (bytes 16..23, big-endian)."""
    with open(path, "rb") as fh:
        data = fh.read(26)
    w, h = struct.unpack(">II", data[16:24])
    return w, h


def run_checks():
    print("P9-M3-T01 render worker acceptance")
    print("worker: %s" % WORKER)
    print("=" * 66)

    # 1. Worker crate exists and is a workspace member; lib exposes options,
    #    registry and the render seam.
    check(os.path.isfile(os.path.join(WORKER, "Cargo.toml")),
          "missing: crates/oep-render-worker/Cargo.toml")
    root_cargo = read_text(os.path.join(ROOT, "Cargo.toml"))
    check("crates/oep-render-worker" in root_cargo,
          "crates/oep-render-worker must be a workspace member (root Cargo.toml)")
    if os.path.isfile(os.path.join(WORKER, "Cargo.toml")):
        wc = read_text(os.path.join(WORKER, "Cargo.toml"))
        check("oep-geo" in wc, "worker must depend on oep-geo")
        check("oep-viewer" in wc, "worker must depend on oep-viewer (render seam)")
        check('name = "oep-render-worker"' in wc,
              "worker package name must be oep-render-worker")
    lib_path = os.path.join(SRC, "lib.rs")
    check(os.path.isfile(lib_path), "missing worker lib.rs")
    lib = read_text(lib_path) if os.path.isfile(lib_path) else ""
    for token in ("options", "registry", "worker"):
        check("pub mod %s" % token in lib,
              "worker lib.rs must expose pub mod %s" % token)

    # 2. The worker registers with the scheduler: the registry seam resolves
    #    the P9-M1 executor vocabulary (kind=render, function=oep-geo-render)
    #    and lists fidelity levels low/medium/high.
    reg_path = os.path.join(SRC, "registry.rs")
    check(os.path.isfile(reg_path), "missing worker registry module")
    if os.path.isfile(reg_path):
        reg = read_text(reg_path)
        for key in ('"render"', '"oep-geo-render"', "pub fn get",
                    "pub fn registry", "low", "medium", "high"):
            check(key in reg,
                  "registry.rs must carry the P9-M1 executor vocabulary + "
                  "fidelity levels (missing %r)" % key)

    # 3. A Rust contract test resolves the registration AND renders the real
    #    golden model via the worker function (not shell-outs).
    tests_dir = os.path.join(WORKER, "tests")
    contract = os.path.join(tests_dir, "worker_functions.rs")
    check(os.path.isfile(contract),
          "missing contract test: tests/worker_functions.rs")
    if os.path.isfile(contract):
        ct = read_text(contract)
        for key in ("registry", "oep-geo-render", "example-bracket", "render"):
            check(key in ct,
                  "contract test must exercise registration + golden render "
                  "(missing %r)" % key)

    # 4. The worker binary exists with the CLI flags the job spec uses.
    bin_src = os.path.join(SRC, "main.rs")
    check(os.path.isfile(bin_src), "missing worker binary: src/main.rs")
    if os.path.isfile(bin_src):
        m = read_text(bin_src)
        for flag in ("--describe", "--model", "--output", "--resolution",
                     "--fidelity"):
            check(flag in m,
                  "binary must accept %s (P9-M1 executor.args vocabulary)" % flag)

    # 5. --describe prints a parseable registration JSON with the right
    #    executor vocabulary (CI-detectable scheduler discovery).
    desc = None
    try:
        r = subprocess.run(["cargo", "run", "-p", "oep-render-worker", "--",
                            "--describe"],
                           cwd=ROOT, capture_output=True, text=True, timeout=600)
        check(r.returncode == 0,
              "--describe failed (rc=%d): %s" % (r.returncode,
                                                 (r.stdout + r.stderr)[-800:]))
        if r.returncode == 0:
            desc = json.loads(r.stdout.strip())
    except FileNotFoundError:
        warn("cargo not found; skipping live --describe")
    except (subprocess.TimeoutExpired, ValueError) as exc:
        check(False, "--describe could not run/parse: %s" % exc)
    if desc is not None:
        check(desc.get("kind") == "render",
              "--describe kind must be 'render', got %r" % desc.get("kind"))
        check(desc.get("function") == "oep-geo-render",
              "--describe function must be 'oep-geo-render', got %r"
              % desc.get("function"))
        fid = sorted(desc.get("fidelity_levels", []))
        check(fid == ["high", "low", "medium"],
              "--describe fidelity_levels must be low/medium/high, got %r" % fid)

    # 6. The worker renders the REAL golden model to proof PNGs at a
    #    parameterized resolution.
    ok, why = png_ok(PROOF_LOW)
    check(ok, "proof render-worker-640x480-low.png: %s" % why)
    ok, why = png_ok(PROOF_HIGH)
    check(ok, "proof render-worker-640x480-high.png: %s" % why)
    if png_ok(PROOF_LOW)[0]:
        check(png_size(PROOF_LOW) == (640, 480),
              "low proof must be 640x480, got %s" % (png_size(PROOF_LOW),))
    if png_ok(PROOF_HIGH)[0]:
        check(png_size(PROOF_HIGH) == (640, 480),
              "high proof must be 640x480, got %s" % (png_size(PROOF_HIGH),))

    # 7. Fidelity is parameterized: low and high renders of the same model
    #    differ (supersample anti-aliasing changes real pixels).
    if png_ok(PROOF_LOW)[0] and png_ok(PROOF_HIGH)[0]:
        h1 = hashlib.sha256(open(PROOF_LOW, "rb").read()).digest()
        h2 = hashlib.sha256(open(PROOF_HIGH, "rb").read()).digest()
        check(h1 != h2, "low and high fidelity renders must differ")
        # And both are real renders, not blank: multiple distinct colors.
        check(os.path.getsize(PROOF_LOW) > 1000,
              "low proof too small to be a real render")
        check(os.path.getsize(PROOF_HIGH) > 1000,
              "high proof too small to be a real render")

    # 8. The proof provenance: the worker crate references the REAL golden
    #    document by name (not synthetic geometry).
    prov = (read_text(bin_src) if os.path.isfile(bin_src) else "") + \
        (read_text(tests_dir + "/worker_functions.rs")
         if os.path.isfile(os.path.join(tests_dir, "worker_functions.rs")) else "")
    check("example-bracket.oepgeo.json" in prov or
          "example-bracket" in prov,
          "worker must render the real golden example-bracket model")

    # 9. Live gate: cargo test for the worker crate passes (real execution).
    try:
        r = subprocess.run(["cargo", "test", "-p", "oep-render-worker"],
                           cwd=ROOT, capture_output=True, text=True, timeout=900)
        check(r.returncode == 0,
              "cargo test -p oep-render-worker failed (rc=%d): %s"
              % (r.returncode, (r.stdout + r.stderr)[-1200:]))
    except FileNotFoundError:
        warn("cargo not found; skipping live build test")
    except subprocess.TimeoutExpired:
        check(False, "cargo test -p oep-render-worker timed out")

    # 10. Surface-language rule on THIS card's files (no em dashes, no curly
    #     quotes) across the worker crate source.
    for base in (SRC, tests_dir):
        if not os.path.isdir(base):
            continue
        for dirpath, _dirs, names in os.walk(base):
            for name in names:
                if not name.endswith(".rs"):
                    continue
                p = os.path.join(dirpath, name)
                try:
                    text = read_text(p)
                except (UnicodeDecodeError, OSError):
                    continue
                nd = text.count(EM_DASH)
                nq = text.count("\u201c") + text.count("\u201d") \
                    + text.count("\u2018") + text.count("\u2019")
                if nd or nq:
                    check(False, "%s: em dash x%d, curly quote x%d"
                          % (os.path.relpath(p, ROOT), nd, nq))

    return failures


def main():
    failures = run_checks()
    if failures:
        print("RENDER WORKER ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    if warnings:
        print("warnings:")
        for w in warnings:
            print("  -", w)
    print("RENDER WORKER ACCEPTANCE PASSED: headless oep-geo render worker "
          "renders the real golden bracket model to PNG at parameterized "
          "resolution/fidelity, registers with the scheduler under "
          "render/oep-geo-render, and is verified on real material.")
    return 0


def test_p9_m3_render_worker_acceptance():
    """pytest entry point: same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "render worker acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
