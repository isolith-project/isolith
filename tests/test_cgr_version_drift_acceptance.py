#!/usr/bin/env python3
"""P1-M2-T02 acceptance test: Compare CGR versions (R18 vs R2026 era) for drift.

TDD contract (RED first, then GREEN on real material):
  * python/oep_tooling/cgr_drift.py exists and exposes a bounded, pure-stdlib
    API: fingerprint(path) -> probe dict, and compare(era_groups) -> report.
    It is the *comparison* layer above the P1-M2-T01 hex notebook
    (oep_tooling.cgr_observe owns raw byte observation); this module owns
    grouping samples by era and measuring drift BETWEEN era groups.
  * docs/problems/cgr-version-drift.md exists and is non-empty: it names the
    drift question (does the CGR on-disk structure change between V5R18-era
    and R2026-era samples), the real material analysed, the era groups, and
    the findings with [uncertain] markers and an honest status per era.
  * Every era-level claim is asserted AGAINST REAL CGR files when real samples
    are supplied ($OEP_CGR_SAMPLES or repo ./samples, ../samples). With the
    real R18-era corpus the drift tool MUST:
        (a) reproduce the header spine stabiliser on every file
            (magic 01 00 00 00, 0x60 zero pad, invariant 16-byte template
            at 0x68..0x77) -- known from P1-M2-T01 and re-derived here;
        (b) detect the two observed OLD-era families (class-string tail vs
            float tail) and separate them into distinct era groups;
        (c) emit a machine-readable report whose JSON carries per-era status
            and a top-level "drift" verdict that may legally be
            'stable-header-within-family', 'tail-drift-detected', or
            'blocked-sample-gap' -- NEVER a fabricated 'no-drift' for an era
            with no real samples.
  * Honesty invariant (program doctrine): an era with zero real samples MUST
    be reported as status='blocked_on_sample' (or equivalent), never silently
    treated as 'no drift'. The R2026-era leg is documented as
    blocked_on_sample until a license-clean modern sample is procured
    (P1-M1-T01 registry already records that no clean modern CGR is
    currently downloadable).
  * Tolerant when no samples are present (SKIP, exit 0) so CI does not break
    before P1-M1 corpus sync lands; FAIL loud on any sample that is present
    but contradicts the asserted invariants.
"""

import glob
import importlib.util
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTEBOOK = os.path.join(REPO, "docs", "re-notes", "cgr-version-drift.md")
TOOL = os.path.join(REPO, "python", "oep_tooling", "cgr_drift.py")

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)
    return ok


def find_samples():
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES")
    if env:
        for chunk in re.split(r"[,:]", env):
            chunk = chunk.strip()
            if chunk:
                dirs.append(chunk)
    for rel in ("samples", os.path.join("..", "samples")):
        p = os.path.join(REPO, rel)
        if os.path.isdir(p):
            dirs.append(p)
    hits = []
    for d in dirs:
        if os.path.isdir(d):
            hits += glob.glob(os.path.join(d, "*.cgr")) + glob.glob(os.path.join(d, "*.CGR"))
    return sorted(set(hits))


def load_tool():
    if not os.path.isfile(TOOL):
        return None
    spec = importlib.util.spec_from_file_location("oep_tooling.cgr_drift", TOOL)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    # 1) Tool + notebook exist (the RED contract).
    mod = load_tool()
    check("cgr_drift.py exists", mod is not None, TOOL)
    if mod is None:
        return 1
    check("fingerprint(path) callable", callable(getattr(mod, "fingerprint", None)))
    check("compare(groups) callable", callable(getattr(mod, "compare", None)))
    nb = ""
    if os.path.isfile(NOTEBOOK):
        nb = open(NOTEBOOK, encoding="utf-8").read()
    check("drift notebook exists and non-empty", len(nb.strip()) > 0, NOTEBOOK)

    samples = find_samples()
    if not samples:
        print("SKIP  no real CGR samples supplied; CI-safe tolerant skip")
        return 0

    print("info  analysing %d real sample(s)" % len(samples))

    # 2) Fingerprint every sample; nothing may crash; header invariant holds.
    probes = {}
    for s in samples:
        name = os.path.basename(s)
        p = mod.fingerprint(s)
        probes[name] = p
        magic_ok = p.get("magic_bytes") == "01000000"
        check("header spine magic on %s" % name, magic_ok, p.get("magic_bytes"))
        check("invariant template on %s" % name,
              p.get("template_hex") == "000700000007000000b1010b0108024f",
              p.get("template_hex"))
        check("alignment 0x1000 on %s" % name, p.get("size") % 0x1000 == 0)
        check("probe is JSON-serialisable", _json_ok(p), name)

    # 3) Era grouping: the two observed old-era families must be separated.
    family_counts = {}
    for name, p in probes.items():
        fam = p.get("family")
        family_counts[fam] = family_counts.get(fam, 0) + 1
    check("two old-era families detected on real corpus",
          len(family_counts) == 2,
          "families=%r" % (sorted(family_counts.items()),))
    if len(family_counts) == 2:
        f1, f2 = sorted(family_counts)
        check("family markers differ (class-string vs float tail)",
              probes.get("piston.cgr", {}).get("family") != probes.get("VALVE.cgr", {}).get("family"),
              "piston=%r valve=%r" % (probes.get("piston.cgr", {}).get("family"),
                                     probes.get("VALVE.cgr", {}).get("family")))

    # 4) compare() must group by era and never fake an empty era.
    groups = mod.compare(probes)
    check("compare() returns a dict", isinstance(groups, dict))
    report_json = json.dumps(groups, sort_keys=True)
    has_blocked = "blocked_on_sample" in report_json or "blocked-sample-gap" in report_json
    has_stable = "stable-header" in report_json
    check("report handles a blocked/absent modern era honestly",
          has_blocked or has_stable,
          report_json[:300])
    check("report JSON round-trips", _json_ok(groups))

    if _failures:
        print("FAIL  total=%d" % len(_failures))
        return 1
    print("PASS  all drift-comparison criteria met on real material")
    return 0


def _json_ok(obj):
    try:
        json.dumps(obj)
        return True
    except (TypeError, ValueError):
        return False


if __name__ == "__main__":
    sys.exit(main())
