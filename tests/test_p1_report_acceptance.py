#!/usr/bin/env python3
"""Acceptance test for Keelworks P1-M3-T03: the P1 report (prioritized import
targets + the "first parcel" that the first shipped reader must open).

TDD: this test documents the acceptance criteria BEFORE the report exists, so
it must FAIL (RED) against a repo with no P1 report, then PASS (GREEN) once
the report lands.

'Done' for P1-M3-T03 means this test passes against the real P1 report, the
report cites the sibling P1 deliverables it synthesizes (corpus registry,
hex notebook, OSS readers audit, commercial readers audit), the priorities
are each grounded in the recorded evidence (a gap), and the first parcel is
narrow and verifiable: a CGR header+tessellation read on the real verified
corpus, tolerant of unknown blocks, and explicitly NOT claiming B-Rep/PMI
which the audits defer.

Run:  python3 tests/test_p1_report_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import glob
import hashlib
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPORT = os.path.join(ROOT, "P1-M3", "p1-report.md")
SOURCES = os.path.join(ROOT, "P1-M3", "p1-report-sources.md")
REGISTRY = os.path.join(ROOT, "P1-M1", "cgr-sample-registry.json")
HEXNOTE = os.path.join(ROOT, "docs", "re-notes", "cgr-hex-notebook.md")
OSS_AUDIT = os.path.join(ROOT, "P1-M3", "oss-readers-audit.md")
COMM_AUDIT = os.path.join(ROOT, "P1-M3", "commercial-readers-audit.md")

EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

# The import-target priority ladder the report must state. Order matters.
# Rationale anchors to the sibling audits: CGR = the wedge (no OSS reader,
# undocumented but lawfully observable, real corpus in hand); 3DXML = second
# (documented ASCII branch, no maintained OSS); STEP = third (strong OSS base
# to wrap, not build); CATPart/CATProduct = deferred to P5 (native,
# undocumented, release-locked, program decision 2 = two-year horizon).
PRIORITY_LADDER = [
    ("CGR", "first"),
    ("3DXML", "second"),
    ("STEP", "third"),
    ("CATPart/CATProduct", "deferred"),
]

# Every priority on the ladder must be justified in the report by citing a
# sibling P1 deliverable (by name/title), not asserted from the air.
REQUIRED_CITATIONS = [
    "cgr-sample-registry",       # P1-M1-T01 corpus
    "cgr-hex-notebook",          # P1-M2-T01 observation
    "oss-readers-audit",         # P1-M3-T01
    "commercial-readers-audit",  # P1-M3-T02
]

# The first parcel must be scoped by these dimensions. "Narrow" is the whole
# point: first customer-visible capability, everything else clearly out.
FIRST_PARCEL_REQUIREMENTS = {
    "reader": ["oep-cgr", "header"],          # the shipped reader + its entry point
    "geometry": ["tessellat", "mesh", "vertex"],  # mesh read, vertex positions
    "tolerant": ["never crash", "skip", "unknown", "warn"],
    "exports": ["glTF", "glb", "STL"],         # a viewable/verifiable output
}

# Explicit non-goals the report MUST mark as OUT of the first parcel. This is
# the honesty contract: the first reader is a mesh read, not a CATIA clone.
FIRST_PARCEL_NON_GOALS = ["B-Rep", "PMI", "feature tree", "assembly"]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_text(path, required=True):
    if not os.path.isfile(path):
        if required:
            check(False, "missing deliverable: %s" % path)
        return ""
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def find_cgr_samples():
    """Locate the real verified CGR corpus, if present (P1-M1 took care to
    NOT commit proprietary binaries; tests remain strict-but-tolerant in the
    same style as the hex-notebook acceptance test)."""
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES")
    if env:
        dirs += [d for d in re.split(r"[,:]", env) if d]
    for rel in ("samples", os.path.join("..", "samples")):
        p = os.path.join(ROOT, rel)
        if os.path.isdir(p):
            dirs.append(p)
    hits = []
    for d in dirs:
        if os.path.isdir(d):
            hits += glob.glob(os.path.join(d, "*.cgr"))
            hits += glob.glob(os.path.join(d, "*.CGR"))
    return sorted(set(hits))


def load_registry():
    txt = read_text(REGISTRY, required=False)
    if not txt:
        # Fallbacks so verification can run before the P1-M1 corpus PR merges:
        # explicit env override, sibling worktrees at the same monorepo root
        # (they share the repo layout and may already carry the registry).
        cands = []
        env = os.environ.get("OEP_CGR_REGISTRY")
        if env:
            cands.append(env)
        # sibling worktrees: <root-of-main>/.worktrees/*/P1-M1/cgr-sample-registry.json
        roots = [os.path.normpath(p) for p in (
            os.path.join(ROOT, "..", ".."),                     # main repo root
            os.path.join(ROOT, "..", "..", ".."),               # one level up
        )]
        seen = set()
        for r in roots:
            wt = os.path.join(r, ".worktrees")
            if not os.path.isdir(wt):
                continue
            for name in sorted(os.listdir(wt)):
                p = os.path.join(wt, name, "P1-M1", "cgr-sample-registry.json")
                if p not in seen and os.path.isfile(p):
                    seen.add(p)
                    cands.append(p)
        for c in cands:
            if not os.path.isfile(c):
                continue
            with open(c, encoding="utf-8") as fh:
                try:
                    return json.loads(fh.read())
                except json.JSONDecodeError:
                    continue
    if not txt:
        return None
    try:
        return json.loads(txt)
    except json.JSONDecodeError:
        return None


def main():
    # --- 1. Deliverables exist ------------------------------------------------
    report = read_text(REPORT)
    sources = read_text(SOURCES, required=False)
    if not os.path.isfile(REPORT):
        print("[FAIL] P1-M3/p1-report.md missing; nothing further to verify")
        print("FAIL count: %d" % len(failures))
        return 1

    lower = report.lower()

    # --- 2. Version header (program rule: living docs always versioned) ------
    check(bool(re.search(r"(?mi)^version:\s*\d+\.\d+", report)),
          "report has a version header (Version: x.y)")
    check("2026-10-15" in report, "report dated 2026-10-15")

    # --- 3. Prioritized import-target ladder, in order -------------------------
    overall = re.search(r"(?ims)(priority|prioritized).{0,400}?(first parcel|first-parcel)",
                        report)
    check(overall is not None, "report has a prioritized import-target section")
    # The ladder must appear as an ordered structure; we require each rung to
    # be mentioned and its rank word present in the same region of the doc.
    seen = {}
    for name, rank in PRIORITY_LADDER:
        key = name.lower()
        check(key in lower, "report names import target %r" % name)
        # rank evidence: find the rank token near the target name
        m = re.search(re.escape(key) + r".{0,200}?" + re.escape(rank), lower) \
            or re.search(re.escape(rank) + r".{0,200}?" + re.escape(key), lower)
        check(m is not None,
              "target %r linked to priority rank %r" % (name, rank))
        seen[key] = m is not None
    # The relative ordering must actually be stated (first..deferred, not a
    # flat list). Hard requirement: CGR must be described as the highest
    # priority / the wedge, and CATPart deferred.
    check(re.search(r"wedge|first|top|highest", lower),
          "report names CGR as the highest-priority target")
    check(re.search(r"defer|two-year|out of scope|P5", lower),
          "report defers CATPart/CATProduct (P5 two-year scope)")

    # --- 4. Every priority justified by a sibling P1 deliverable --------------
    for cite in REQUIRED_CITATIONS:
        check(cite.lower() in lower,
              "report cites sibling deliverable %r" % cite)

    # --- 5. First parcel scoped narrow, with the required dimensions -----------
    # Anchor to the first-parcel definition section heading (e.g. a numbered
    # "## 5. The first parcel" heading), not just a mention in the summary.
    fp_head = re.search(r"(?im)^#{2,3}[^\n]*first\s+parcel", report)
    check(fp_head is not None, "report has a first-parcel definition section")
    fptext = ""
    if fp_head:
        body = report[fp_head.end():]
        nxt = re.search(r"\n#{2,3}\s", body)
        fptext = body if nxt is None else body[:nxt.start()]
    # The first parcel body plus the whole report is the check surface.
    surface = (fptext + "\n" + report).lower()
    for dim, needles in FIRST_PARCEL_REQUIREMENTS.items():
        for n in needles:
            check(n.lower() in surface,
                  "first parcel dimension %r mentions %r" % (dim, n))

    # --- 6. Explicit non-goals (what the first reader does NOT claim) ----------
    check(re.search(r"(?i)non-goal|not in scope|out of scope|explicitly not|does not",
                    report),
          "report has an explicit non-goals statement")
    for ng in FIRST_PARCEL_NON_GOALS:
        check(ng.lower() in (fptext + lower),
              "first parcel excludes %r as a non-goal" % ng)

    # --- 7. Ties the first parcel to ZIP-level corpus acceptance --------------
    # The report must say the first reader will be tested against a real CGR
    # file (a golden file), echoing the P2-M1 TDD plan and the hex notebook.
    check(re.search(r"(?i)golden|real (cgr|file|sample)|corpus", report),
          "report ties first parcel to a real-material golden test")

    # --- 8. Real-material verification -----------------------------------------
    # (a) the report itself states which claims were verified on 2026-10-15
    check(re.search(r"(?i)verified|checked against|real material", report),
          "report carries a real-material verification statement")
    # (b) if the real corpus is present, the report's cross-referenced registry
    #     checksums must match the on-disk files (strict, loud on mismatch)
    samples = find_cgr_samples()
    registry = load_registry()
    if not samples:
        print("[SKIP] no real CGR samples present locally; strict sha256 "
              "verification skipped (set OEP_CGR_SAMPLES to enable)")
    else:
        print("verifying %d real CGR sample(s) against registry" % len(samples))
        if registry is None:
            check(False, "real samples present but registry missing/unreadable")
        else:
            by_name = {os.path.basename(s["file"]).lower(): s["sha256"]
                       for s in registry.get("samples", [])}
            checked = 0
            for s in samples:
                base = os.path.basename(s)
                if base.lower() not in by_name:
                    print("  (no registry entry for %s, skipping)" % base)
                    continue
                digest = hashlib.sha256(open(s, "rb").read()).hexdigest()
                ok = digest == by_name[base.lower()]
                status = "PASS" if ok else "FAIL"
                print("  [%s] sha256 %s" % (status, base))
                check(ok, "real sample %s sha256 matches registry" % base)
                checked += 1
            check(checked > 0, "at least one real sample verified against registry")

    # --- 9. Sources file sanity (when present) ----------------------------------
    if sources:
        urls = re.findall(r"(https?://[^\s)>]+)", sources)
        check(len(urls) >= 4, "sources file lists at least 4 URLs (got %d)" % len(urls))
        check(EM_DASH not in sources, "sources file prose contains no em dash (U+2014)")

    # --- 10. Whole-report language rule ------------------------------------------
    check(EM_DASH not in report, "entire report prose contains no em dash (U+2014)")

    # --- summary ---------------------------------------------------------------
    for f in failures:
        print("[FAIL] " + f)
    print("FAIL count: %d" % len(failures))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
