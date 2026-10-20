#!/usr/bin/env python3
"""Acceptance test for Isolith P9-M4-T02: checkpoint/resume for long jobs.

Written FIRST (TDD). The farm crate (crates/oep-farm) + its committed
real-material proof must make this test pass. It verifies the real material,
not mocks:

  crate       => a self-contained std-only crate crates/oep-farm exists as a
                 workspace member, exposing hash / items / record / store /
                 artifact / farm modules and the contract types
  checkpoints => a durable checkpoint record + store exist (the record is a
                 self-contained deterministic wire format with integrity; the
                 store has an in-memory and a directory backend that survive
                 a fresh store handle)
  resume      => the runner resumes from the last checkpoint after an eviction
                 (a bounded run stops before completion; a brand-new worker
                 on the same store rewinds to the checkpoint boundary and
                 finishes)
  output      => the resumed artifact is byte-identical to a clean run: the
                 committed proof JSON reports byte_equivalent=true, both proof
                 artifacts (clean.bin / resumed.bin) are SHA-256 identical,
                 and a live drill run regenerated them from real execution
  live        => cargo test -p oep-farm passes on this host

Plus the program surface-language rule (no em dashes, no curly quotes) on the
files this card adds.

Run:  python3 tests/test_p9_m4_checkpoint_resume.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FARM = os.path.join(ROOT, "crates", "oep-farm")
SRC = os.path.join(FARM, "src")
PROOF_DIR = os.path.join(ROOT, "workstreams/compute/gpu", "proof")
PROOF_JSON = os.path.join(PROOF_DIR, "checkpoint-resume-proof.json")
PROOF_CLEAN = os.path.join(PROOF_DIR, "clean.bin")
PROOF_RESUMED = os.path.join(PROOF_DIR, "resumed.bin")
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


def sha256_of(path):
    with open(path, "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


def run_checks():
    print("P9-M4-T02 checkpoint/resume acceptance")
    print("farm: %s" % FARM)
    print("=" * 66)

    # 1. Crate exists and is a workspace member; std-only (no external deps).
    check(os.path.isfile(os.path.join(FARM, "Cargo.toml")),
          "missing: crates/oep-farm/Cargo.toml")
    root_cargo = read_text(os.path.join(ROOT, "Cargo.toml"))
    check("crates/oep-farm" in root_cargo,
          "crates/oep-farm must be a workspace member (root Cargo.toml)")
    farm_cargo = read_text(os.path.join(FARM, "Cargo.toml"))
    deps = re.search(r"\[dependencies\](.*?)(\n\[|\Z)", farm_cargo, re.S)
    if deps:
        dep_block = deps.group(1)
        declared = [ln for ln in dep_block.splitlines()
                    if ln.strip() and not ln.lstrip().startswith("#")]
        # allow only none: a std-only crate
        check(not declared, "oep-farm must be std-only, found deps: %r" % declared)

    # 2. Library exposes the contract modules + types.
    lib = read_text(os.path.join(SRC, "lib.rs")) if os.path.isfile(os.path.join(SRC, "lib.rs")) else ""
    for token in ("pub mod hash", "pub mod items", "pub mod record",
                  "pub mod store", "pub mod artifact", "pub mod farm"):
        check(token in lib, "lib.rs must expose %s" % token)
    for bind in ("Checkpoint", "CheckpointStore", "FarmRunner", "FarmConfig",
                 "RunReport", "ItemSource", "SeededItemSource", "DirStore"):
        check(bind in lib, "lib.rs must re-export %s" % bind)

    # 3. The record is a deterministic wire format with integrity (magic
    #    OEPF, version, bounds, trailing tag).
    rec = read_text(os.path.join(SRC, "record.rs"))
    for key in ('MAGIC', '"OEPF"', "VERSION", "encode", "decode",
                "BadIntegrity", "BadMagic"):
        check(key in rec, "record.rs must carry the integrity wire format (missing %r)" % key)

    # 4. The store is durable (a directory backend that a fresh handle reads).
    st = read_text(os.path.join(SRC, "store.rs"))
    for key in ("trait CheckpointStore", "MemoryStore", "DirStore", "load_latest",
                "rename", "fn save"):
        check(key in st, "store.rs must carry in-memory + durable stores (missing %r)" % key)

    # 5. The runner resumes from the last checkpoint: run_bounded (eviction)
    #    + corruption/lost-artifact refusal + checkpoints written.
    fm = read_text(os.path.join(SRC, "farm.rs"))
    for key in ("run_bounded", "checkpoint_every", "resumed_from",
                "FarmError::Corruption", "ArtifactShorterThanCheckpoint",
                "CheckpointAheadOfJob", "save_checkpoint"):
        check(key in fm, "farm.rs must implement the resume contract (missing %r)" % key)

    # 6. Acceptance tests exist and exercise eviction + byte-equivalence.
    acc = os.path.join(FARM, "tests", "checkpoint_resume_acceptance.rs")
    check(os.path.isfile(acc), "missing acceptance test: tests/checkpoint_resume_acceptance.rs")
    if os.path.isfile(acc):
        at = read_text(acc)
        for key in ("survives_eviction", "byte_equivalent", "resume_refuses",
                    "DirStore", "FileArtifact", "run_bounded"):
            check(key in at,
                  "acceptance test must drive the eviction drill (missing %r)" % key)

    # 7. Committed proof: byte_equivalent=true and SHA-256-identical artifacts.
    check(os.path.isfile(PROOF_JSON), "missing committed proof JSON")
    proof = {}
    if os.path.isfile(PROOF_JSON):
        proof = json.loads(read_text(PROOF_JSON))
        check(proof.get("byte_equivalent") is True,
              "proof must report byte_equivalent=true, got %r"
              % proof.get("byte_equivalent"))
    check(os.path.isfile(PROOF_CLEAN), "missing proof artifact: clean.bin")
    check(os.path.isfile(PROOF_RESUMED), "missing proof artifact: resumed.bin")
    if os.path.isfile(PROOF_CLEAN) and os.path.isfile(PROOF_RESUMED):
        hc = sha256_of(PROOF_CLEAN)
        hr = sha256_of(PROOF_RESUMED)
        check(hc == hr, "clean.bin and resumed.bin must be byte-identical "
                        "(sha256 differ: %s vs %s)" % (hc, hr))
        stored_clean = (proof.get("clean") or {}).get("sha256")
        if stored_clean:
            check(hc == stored_clean,
                  "committed clean.bin sha256 %s must match proof JSON %s"
                  % (hc, stored_clean))

    # 8. Live drill regenerates a byte-equivalent pair on this host (real
    #    material executed now, not just the committed snapshot).
    try:
        import tempfile
        with tempfile.TemporaryDirectory(prefix="oep-farm-drill-") as td:
            rr = subprocess.run(["cargo", "run", "-q", "-p", "oep-farm",
                                 "--bin", "oep-farm-drill", "--", "--out-dir", td],
                                cwd=ROOT, capture_output=True, text=True, timeout=600)
            check(rr.returncode == 0,
                  "live drill failed (rc=%d): %s"
                  % (rr.returncode, (rr.stdout + rr.stderr)[-800:]))
            if rr.returncode == 0:
                try:
                    live = json.loads(rr.stdout.strip())
                except ValueError:
                    live = None
                check(live is not None and live.get("byte_equivalent") is True,
                      "live drill must report byte_equivalent=true")
                live_resumed = os.path.join(td, "resumed.bin")
                live_clean = os.path.join(td, "clean.bin")
                if os.path.isfile(live_clean) and os.path.isfile(live_resumed):
                    check(sha256_of(live_clean) == sha256_of(live_resumed),
                          "live drill clean/resumed artifacts must be byte-identical")
    except FileNotFoundError:
        warn("cargo not found; skipping live drill")

    # 9. Live gate: cargo test for the crate passes (real execution).
    try:
        r = subprocess.run(["cargo", "test", "-p", "oep-farm"],
                           cwd=ROOT, capture_output=True, text=True, timeout=900)
        check(r.returncode == 0,
              "cargo test -p oep-farm failed (rc=%d): %s"
              % (r.returncode, (r.stdout + r.stderr)[-1200:]))
    except FileNotFoundError:
        warn("cargo not found; skipping live build test")
    except subprocess.TimeoutExpired:
        check(False, "cargo test -p oep-farm timed out")

    # 10. Surface-language rule on THIS card's files (no em dashes, no curly
    #     quotes) across the crate source and the committed docs.
    for base in (SRC, os.path.join(FARM, "tests"), PROOF_DIR,
                 os.path.join(ROOT, "workstreams/compute/gpu")):
        if not os.path.isdir(base):
            continue
        for dirpath, _dirs, names in os.walk(base):
            for name in names:
                if not (name.endswith(".rs") or name.endswith(".md") or name.endswith(".json")):
                    continue
                p = os.path.join(dirpath, name)
                try:
                    text = read_text(p)
                except (UnicodeDecodeError, OSError):
                    continue
                nd = text.count(EM_DASH)
                nq = (text.count("\u201c") + text.count("\u201d")
                      + text.count("\u2018") + text.count("\u2019"))
                if nd or nq:
                    check(False, "%s: em dash x%d, curly quote x%d"
                          % (os.path.relpath(p, ROOT), nd, nq))

    return failures


def main():
    failures = run_checks()
    if failures:
        print("CHECKPOINT/RESUME ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    if warnings:
        print("warnings:")
        for w in warnings:
            print("  -", w)
    print("CHECKPOINT/RESUME ACCEPTANCE PASSED: a long job survives worker "
          "eviction via durable checkpoints, resumes from the last "
          "checkpoint, and the resumed output is byte-identical (SHA-256 "
          "verified) to a clean run.")
    return 0


def test_p9_m4_checkpoint_resume_acceptance():
    """pytest entry point: same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "checkpoint/resume acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
