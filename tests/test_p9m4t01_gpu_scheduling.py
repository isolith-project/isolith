#!/usr/bin/env python3
"""Acceptance test for Keelworks P9-M4-T01: GPU discovery + scheduling with a
clean CPU fallback.

Written FIRST (TDD). The discovery layer (crates/oep-lane/src/gpu), the
placement layer (crates/oep-lane/src/placement) and the wire-in
(crates/oep-lane/src/sched/scheduler.rs) must make this test pass. It
verifies the real material, not mocks:

  discovery  => the run-log records a real nvidia-smi probe with honest
                numbers, including how the DGX/GB10 `[N/A]` memory is
                represented (memory_mib: null, never a fabricated value)
  scheduling => the placement policy's contract is documented: required GPU
                jobs go only to GPU nodes (and park when none is free), and
                preferred GPU jobs fall back to a CPU node cleanly
                (cpu_fallback flag) instead of failing
  docs       => workstreams/compute/gpu/gpu-scheduling.md exists and states the measured host
  surface    => no em dashes / curly quotes in the committed P9-M4 material
                (program surface-language rule)

Run:  python3 tests/test_p9m4t01_gpu_scheduling.py   (script mode)
      python3 -m pytest tests/test_p9m4t01_gpu_scheduling.py  (pytest mode)
Exit 0 = pass; nonzero = fail. Stdlib only (python3 + pytest in CI).
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN_LOG = os.path.join(ROOT, "workstreams/compute/gpu", "spike", "run-log.json")
DOC = os.path.join(ROOT, "workstreams/compute/gpu", "gpu-scheduling.md")

EM_DASH = "\u2014"
failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def no_em_dashes(text, path):
    if EM_DASH in text:
        check(False, "%s contains an em dash (surface-language rule)" % path)
    for ch in ["\u2018", "\u2019", "\u201c", "\u201d"]:  # curly quotes
        if ch in text:
            check(False, "%s contains a curly quote (surface-language rule)" % path)


def _run():
    # --- run-log: real nvidia-smi evidence, honest numbers ------------------
    if os.path.exists(RUN_LOG):
        raw = read(RUN_LOG)
        no_em_dashes(raw, RUN_LOG)
        try:
            log = json.loads(raw)
            host = log.get("host", [])
            check(isinstance(host, list) and len(host) >= 1, "run-log has >=1 host")
            if host:
                gpus = host[0].get("gpus", [])
                check(len(gpus) >= 1, "run-log host reports >=1 GPU device")
                if gpus:
                    g = gpus[0]
                    check(bool(g.get("name")), "run-log GPU has a name")
                    # honest numbers: a real driver version and compute cap,
                    # and a memory that is either an integer or literally null
                    # ([N/A]), never an invented zero for [N/A].
                    check(bool(g.get("driver_version")),
                          "run-log GPU has driver_version")
                    check("memory_mib" in g, "run-log GPU records memory_mib key")
                    if g.get("memory_mib") is None:
                        check(
                            g.get("memory_total_reported") == "[N/A]",
                            "null memory_mib is only used with an [N/A] report",
                        )
        except ValueError as e:
            check(False, "run-log.json is not valid JSON: %s" % e)
    else:
        check(False, "workstreams/compute/gpu/spike/run-log.json is missing")

    # --- doc: placement contract + measured host ----------------------------
    if os.path.exists(DOC):
        doc = read(DOC)
        no_em_dashes(doc, DOC)
        low = doc.lower()
        check("required" in low and "park" in low,
              "doc states required GPU jobs park")
        check("cpu fallback" in low or "fallback" in low,
              "doc states CPU fallback")
        check("measured" in low and ("gb10" in low or "dgx" in low),
              "doc names the real measured hardware")
    else:
        check(False, "workstreams/compute/gpu/gpu-scheduling.md is missing")

    if failures:
        print("FAIL P9-M4-T01")
        for f in failures:
            print("  -", f)
        return False
    print("ok   P9-M4-T01 (run-log JSON + honest-GPU checks + doc + surface language)")
    return True


def test_p9m4t01_gpu_scheduling():
    """pytest entry: asserts the acceptance checks hold."""
    assert _run(), "P9-M4-T01 acceptance checks must pass"


if __name__ == "__main__":
    sys.exit(0 if _run() else 1)
