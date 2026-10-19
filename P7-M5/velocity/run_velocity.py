#!/usr/bin/env python3
"""P7-M5-T01 power-user velocity harness.

Builds the OEP interchange workbench CLI (the oep-shell `workbench` example
binary, P7-M2-T01) and runs the 30-common-operation catalog in
`P7-M5/velocity/ops.json`, timing each op wall-clock on the host and writing a
measured report to `P7-M5/velocity/report-velocity.json`.

Design:

  - Corpus and committed fixtures. Legal, license-clean real material is
    resolved through the same env vars the crates use (`OEP_*_SAMPLES`, a
    colon/comma-separated list of directories), plus the known sample roots on
    the reference host. Binaries are never committed (clean-room invariant);
    the committed goldens (box-reference.step) are resolved from the repo
    itself. A token like `@box-reference` in the catalog `cli` field is
    replaced with the resolved absolute path; if a required token cannot be
    resolved the op is recorded as `skip` so the harness still runs everywhere.
  - Measurement. Wall-clock per invocation of the built binary, one process per
    op run (the cold-start a power user actually feels). Each op: one warmup,
    then REPEATS measured runs. Report records min / mean / max / p95.
  - Honesty. Each op also asserts its catalog `expected` outcome (success,
    success-with-pending, failure) against the real exit code / output in a
    non-timing verification pass, printed for the operator. A mismatch is
    recorded but does not fabricate a number.

Usage:
  python3 P7-M5/velocity/run_velocity.py [--ops ops.json] [--repeats N]
         [--out report-velocity.json] [--build|--no-build]
Run from the repo root. Stdlib only.
"""
import argparse
import json
import os
import platform
import shutil
import socket
import statistics
import subprocess
import sys
import tempfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DEFAULT_OPS = os.path.join(REPO, "P7-M5", "velocity", "ops.json")
DEFAULT_OUT = os.path.join(REPO, "P7-M5", "velocity", "report-velocity.json")

BOX_REFERENCE = os.path.join(
    REPO, "crates", "oep-step", "tests", "golden", "box-reference.step")

# Well-known real-material sample roots on the reference host, probed in order
# after the OEP_*_SAMPLES env dirs. Same spirit as the crate tests' probe dirs.
SAMPLE_ROOTS = [
    os.path.join(REPO, "..", "3dxml-samples-1015a", "corpus", "data", "3dxml"),
    os.path.join(REPO, "..", "catpart-samples-1015a", "corpus", "data"),
    os.path.join(REPO, "..", "cgr-real-latch-1016a", "corpus"),
    os.path.join(REPO, "..", "p7m2t01-interchange-workbench-1016a", "corpus"),
    os.path.expanduser(os.path.join("~", ".hermes", "cache", "scratch", "nist")),
    os.path.expanduser(os.path.join("~", ".hermes", "cache", "scratch", "surface-acq")),
    os.path.expanduser(os.path.join("~", "Downloads")),
]

ENV_DIRS = {
    "OEP_3DXML_SAMPLES": ["cube1.3dxml", "quad.3dxml", "blocks.3dxml",
                          "machinist.3DXML", "cycloidal.3DXML", "FM1_satellite.3DXML",
                          "cube1.3DXML", "quad.3DXML", "blocks.3DXML",
                          "machinist.3dxml", "cycloidal.3dxml", "FM1_satellite.3dxml"],
    "OEP_CGR_SAMPLES": ["nist_stc_06_asme1_V56R2023.CATPart",
                        "nist_stc_08_asme1_V56R2023.CATPart", "VALVE.cgr",
                        "nist_stc_06_asme1_V5-6R2023.CATPart",
                        "nist_stc_08_asme1_V5-6R2023.CATPart"],
    "OEP_STEP_SAMPLES": ["assembly.step", "nist_stc_06_asme1_ap242-e3.stp",
                         "nist_stc_06_asme1_ap242_e3.stp", "example-bracket-ap242.stp"],
}

# Named tokens <-> the sample filename(s) each can resolve to (first hit wins).
TOKENS = {
    "@box-reference": [BOX_REFERENCE],
    "@cube1-3dxml": [["OEP_3DXML_SAMPLES", "cube1.3dxml"],
                     ["OEP_3DXML_SAMPLES", "cube1.3DXML"]],
    "@quad-3dxml": [["OEP_3DXML_SAMPLES", "quad.3dxml"],
                    ["OEP_3DXML_SAMPLES", "quad.3DXML"]],
    "@blocks-3dxml": [["OEP_3DXML_SAMPLES", "blocks.3dxml"],
                      ["OEP_3DXML_SAMPLES", "blocks.3DXML"]],
    "@machinist-3dxml": [["OEP_3DXML_SAMPLES", "machinist.3DXML"],
                         ["OEP_3DXML_SAMPLES", "machinist.3dxml"]],
    "@cycloidal-3dxml": [["OEP_3DXML_SAMPLES", "cycloidal.3DXML"],
                         ["OEP_3DXML_SAMPLES", "cycloidal.3dxml"]],
    "@fm1-3dxml": [["OEP_3DXML_SAMPLES", "FM1_satellite.3DXML"],
                   ["OEP_3DXML_SAMPLES", "FM1_satellite.3dxml"]],
    "@assembly-step": [["OEP_STEP_SAMPLES", "assembly.step"]],
    "@nist-ap242-step": [["OEP_STEP_SAMPLES", "nist_stc_06_asme1_ap242-e3.stp"],
                         ["OEP_STEP_SAMPLES", "nist_stc_06_asme1_ap242_e3.stp"]],
    "@modern-stc06": [["OEP_CGR_SAMPLES", "nist_stc_06_asme1_V56R2023.CATPart"]],
    "@modern-stc08": [["OEP_CGR_SAMPLES", "nist_stc_08_asme1_V56R2023.CATPart"]],
    "@valve-cgr": [["OEP_CGR_SAMPLES", "VALVE.cgr"]],
}

JUNK = b"junk junk junk, decidedly not a CAD container: 0123456789abcdef \xab\xcd\xef"


def env_sample_dirs():
    dirs = []
    for env in ("OEP_3DXML_SAMPLES", "OEP_CGR_SAMPLES", "OEP_STEP_SAMPLES"):
        val = os.environ.get(env, "")
        for chunk in val.replace(",", ":").split(":"):
            chunk = chunk.strip()
            if chunk and os.path.isdir(chunk):
                dirs.append(chunk)
    return dirs


def search_tree(root, filename):
    """Depth-limited recursive search for a sample name below a root."""
    matches = []
    try:
        for dirpath, dirnames, filenames in os.walk(root):
            for fn in filenames:
                if fn.lower() == filename.lower():
                    matches.append(os.path.join(dirpath, fn))
            if len(matches) >= 5:
                break
    except OSError:
        return []
    return matches


def resolve_token(token):
    """Return the absolute path for a @token, or None if unresolvable."""
    specs = TOKENS.get(token)
    if not specs:
        return None
    for spec in specs:
        if isinstance(spec, str):
            if os.path.isfile(spec):
                return spec
            continue
        env_name, fname = spec
        for d in env_sample_dirs():
            cand = os.path.join(d, fname)
            if os.path.isfile(cand):
                return cand
    # Well-known root probes (first file found, case-insensitive).
    for root in SAMPLE_ROOTS:
        if not os.path.isdir(root):
            continue
        for spec in specs:
            if not isinstance(spec, list):
                continue
            fname = spec[1]
            found = search_tree(root, fname)
            if found:
                return found[0]
    return None


def resolve_cli(cli_tokens, junk_path):
    """Replace @tokens in argv tokens; fail with the missing token name."""
    resolved = []
    missing = []
    for tok in cli_tokens:
        if "@" in tok:
            name = tok[tok.index("@"):]
            if name.startswith("@junk-"):
                # Junk is generated per-run, never a committed sample.
                resolved.append(tok[:tok.index("@")] + junk_path)
                continue
            p = resolve_token(name)
            if not p:
                missing.append(name)
                continue
            tok = tok[:tok.index("@")] + p
        resolved.append(tok)
    return resolved, missing


def build_binary(with_build):
    bin_path = os.path.join(REPO, "target", "debug", "examples", "workbench")
    if with_build:
        print("[build] cargo build --example workbench ...")
        t0 = time.time()
        subprocess.run(["cargo", "build", "--example", "workbench"],
                       cwd=REPO, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("[build] done in %.1fs" % (time.time() - t0))
    if not os.path.isfile(bin_path):
        print("ERROR: workbench example binary not found at %s" % bin_path)
        sys.exit(1)
    return bin_path


def expected_matches(op, exit_code, out):
    exp = op.get("expected")
    if exp == "failure":
        return exit_code != 0
    if exp == "pending":
        return exit_code == 0 and "save pending" in out
    return exit_code == 0


def run_op(binary, op, junk_path, repeats):
    argv, missing = resolve_cli(op["cli"].split(), junk_path)
    if missing:
        return {"id": op["id"], "status": "skip",
                "reason": "missing sample: %s" % ", ".join(missing)}
    # Non-timing verification pass: the op must behave as the catalog says.
    first = subprocess.run([binary] + argv, capture_output=True, text=True)
    behaved = expected_matches(op, first.returncode, first.stdout)
    # Timing pass: warmup + repeats, mean/min/max, p95 from the repeats.
    if repeats > 1:
        subprocess.run([binary] + argv, capture_output=True)
    samples = []
    for _ in range(repeats):
        t0 = time.perf_counter()
        subprocess.run([binary] + argv, capture_output=True)
        samples.append(time.perf_counter() - t0)
    mean = statistics.mean(samples)
    p95 = sorted(samples)[int(len(samples) * 0.95) - 1] if repeats > 1 else samples[0]
    return {
        "id": op["id"],
        "status": "measured",
        "behavior": "ok" if behaved else "MISMATCH(%s)" % op["expected"],
        "mean_seconds": round(mean, 6),
        "min_seconds": round(min(samples), 6),
        "max_seconds": round(max(samples), 6),
        "p95_seconds": round(p95, 6),
        "runs": repeats,
    }


def git_head():
    try:
        out = subprocess.run(["git", "rev-parse", "--short", "HEAD"],
                             cwd=REPO, capture_output=True, text=True)
        return out.stdout.strip()
    except Exception:
        return "unknown"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ops", default=DEFAULT_OPS)
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--no-build", action="store_true",
                    help="skip cargo build (binary already present)")
    args = ap.parse_args()

    with open(args.ops, encoding="utf-8") as fh:
        ops = json.load(fh)
    binary = build_binary(with_build=not args.no_build)

    junk_fd, junk_path = tempfile.mkstemp(suffix=".bin")
    with os.fdopen(junk_fd, "wb") as jf:
        jf.write(JUNK)

    results = [run_op(binary, op, junk_path, args.repeats) for op in ops]
    os.unlink(junk_path)

    measured = [r for r in results if r["status"] == "measured"]
    skipped = [r for r in results if r["status"] == "skip"]
    mismatched = [r for r in measured if r.get("behavior") != "ok"]

    report = {
        "purpose": "P7-M5-T01 power-user velocity: 30 common ops under budget",
        "host": socket.gethostname(),
        "platform": platform.platform(),
        "built_from": git_head(),
        "measured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "binary": os.path.basename(binary),
        "ops": results,
    }
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2, sort_keys=True)
        fh.write("\n")

    print("Wrote %s" % args.out)
    print("  ops: %d total, %d measured, %d skipped, %d behavior MISMATCH"
          % (len(results), len(measured), len(skipped), len(mismatched)))
    for op in sorted(measured, key=lambda r: r["mean_seconds"], reverse=True)[:8]:
        print("    %-8s mean=%8.4fs  p95=%8.4fs  %s"
              % (op["id"], op["mean_seconds"], op["p95_seconds"], op.get("behavior", "")))
    if mismatched:
        print("  WARNING: ops whose runtime behavior disagreed with the catalog:")
        for op in mismatched:
            print("    %-8s %s" % (op["id"], op.get("behavior")))


if __name__ == "__main__":
    main()
