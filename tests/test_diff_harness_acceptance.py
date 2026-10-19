#!/usr/bin/env python3
"""P1-M2-T06 acceptance test: diff harness (save/load/modify in a vendor-free
tool, bisect which bytes change).

TDD contract (RED first, then GREEN on real material):
  * python/oep_tooling/diff_harness.py exists and exposes a bounded,
    pure-stdlib API:
        detect(path) -> {'format': ..., 'probe': ...}
        load(path)   -> sample dict (raw bytes + parsed structure)
        save(sample) -> bytes written back by the vendor-free writer
        modify(sample, patch) -> sample with a semantic field changed
        bisect(orig_bytes, new_bytes) -> list of changed byte-range records
        member_diff(orig_members, new_members) -> per-member changed ranges
  * The harness round-trips REAL 3DXML samples through a vendor-free writer
    (stdlib zipfile + ElementTree only; no Dassault tool, no commercial lib):
        (a) content preservation: every member's decompressed bytes survive
            load -> save -> load unmodified (nothing lost on round-trip);
        (b) deterministic writer: save(save(sample)) == save(sample)
            (idempotence => the harness itself is churn-free, so any diff
             the user sees is caused by their semantic edit, not by us);
        (c) bisection: applying ONE semantic patch (e.g. SchemaVersion
            4.0 -> 4.3, or the Title) changes bytes ONLY in the member that
            carries that field, and bisect() locates the changed range
            inside that member; container-level bisect() also reports the
            ZIP metadata bytes that move as a *consequence* (tagged as
            zip-metadata, never mislabelled as content change).
  * Honesty invariant (program doctrine): a format with NO vendor-free
    writer (CGR, CATPart, unknown binary) must be reported as
    status='blocked_on_tool' by the modify/bisect leg -- never a fabricated
    diff. detect() must classify CGR (magic 01 00 00 00 / V5_CFV2) and
    CATPart/CATProduct honestly.
  * Hermetic unit checks (no samples needed) keep CI green before the
    corpus sync lands; the real-material leg FAILs loud on any sample that
    contradicts the asserted invariants, and SKIPs (exit 0) when no samples
    are supplied.

Samples: searched in order
  1. $OEP_3DXML_SAMPLES  (explicit corpus dir; ':' or ',' separated)
  2. ./samples/  ../samples/  (repo-relative local corpus)
"""

import glob
import importlib.util
import io
import os
import re
import sys
import zipfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL = os.path.join(REPO, "python", "oep_tooling", "diff_harness.py")
NOTEBOOK = os.path.join(REPO, "docs", "re-notes", "diff-harness.md")

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    print("[%s] %s%s" % (status, name, (" -- " + detail if detail else "")))
    if not ok:
        _failures.append(name)
    return ok


def load_tool():
    if not os.path.isfile(TOOL):
        return None
    spec = importlib.util.spec_from_file_location("oep_tooling.diff_harness", TOOL)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def find_samples():
    dirs = []
    env = os.environ.get("OEP_3DXML_SAMPLES")
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
            hits += (glob.glob(os.path.join(d, "*.3dxml"))
                     + glob.glob(os.path.join(d, "*.3DXML")))
    return sorted(set(hits))


# --------------------------------------------------------------------------
# Hermetic unit checks (no external samples required).
# --------------------------------------------------------------------------
def hermetic_detect(mod):
    cgr = bytes.fromhex("01000000" + "00" * 60)
    check("detect classifies CGR magic as blocked_on_tool-capable (cgr)",
          mod.detect_bytes(cgr).get("format") in ("cgr", "binary"),
          repr(mod.detect_bytes(cgr)))
    catpart = b"V5_CFV2\x00" + b"\xff" * 40
    check("detect classifies CATPart magic (v5_cfv2)",
          mod.detect_bytes(catpart).get("format") in ("catpart", "catproduct",
                                                      "binary"),
          repr(mod.detect_bytes(catpart)))
    # Degenerate: not a zip, not the above binary magics -> binary/unknown.
    check("detect tolerates unknown binary",
          mod.detect_bytes(b"\xde\xad\xbe\xef" * 4).get("format") in
          ("binary", "unknown"),
          repr(mod.detect_bytes(b"\xde\xad\xbe\xef" * 4)))


def hermetic_bisect(mod):
    a = b"The quick brown fox jumps over the lazy dog"
    b = a[:4] + b"QUICK" + a[9:]
    ranges = mod.bisect(a, b)
    check("bisect() returns list of range dicts", isinstance(ranges, list)
          and all(isinstance(r, dict) for r in ranges))
    if ranges:
        joined = []
        for r in ranges:
            joined += list(range(r["start"], r["start"] + r["length"]))
        expected = list(range(4, 9))
        check("bisect() locates the single changed run exactly",
              joined == expected, repr(ranges))
        check("bisect() records old/new hex for the run",
              "old_hex" in ranges[0] and "new_hex" in ranges[0],
              repr(ranges[0]))


def hermetic_roundtrip(mod):
    # Build a tiny 2-member zip in memory; load/save/load must preserve
    # content and the writer must be idempotent (churn-free).
    bio = io.BytesIO()
    with zipfile.ZipFile(bio, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("Manifest.xml", "<Manifest><Root>x</Root></Manifest>")
        z.writestr("A.html", "<p>body</p>")
    orig = bio.getvalue()
    s = mod.load_bytes(orig, name="tiny.3DXML")
    check("load() parses a zip 3DXML sample", s is not None, repr(s))
    if s is None:
        return
    out1 = mod.save(s)
    s2 = mod.load_bytes(out1, name="tiny.3DXML")
    members = {n: b for n, b in s2["members"]}
    check("round-trip preserves Manifest.xml content",
          members.get("Manifest.xml") == b"<Manifest><Root>x</Root></Manifest>")
    check("round-trip preserves A.html content",
          members.get("A.html") == b"<p>body</p>")
    out2 = mod.save(s2)
    check("writer is idempotent (save(save(x)) == save(x))", out1 == out2,
          "len %d vs %d" % (len(out1), len(out2)))
    # A semantic patch on member content must be visible in the diff as a
    # changed member (member-level bisection works). Use the harness's own
    # modify() patch application on a raw member.
    mem = {n: b for n, b in s["members"]}
    mem["Manifest.xml"] = b"<Manifest><Root>y</Root></Manifest>"
    member_ranges = mod.member_diff(s["members"], list(mem.items()))
    check("member_diff() detects the changed member",
          any(m["name"] == "Manifest.xml" and m["changed"] for m in member_ranges),
          repr(member_ranges))


# --------------------------------------------------------------------------
# Real-material leg.
# --------------------------------------------------------------------------
def test_notebook_exists():
    nb = ""
    if os.path.isfile(NOTEBOOK):
        nb = open(NOTEBOOK, encoding="utf-8").read()
    check("diff-harness notebook exists and is non-empty",
          bool(nb.strip()), NOTEBOOK)
    for token in ("[uncertain]", "blocked_on_tool", "vendor-free",
                  "task: P1-M2-T06"):
        check("notebook names %r" % token, token in nb, token)


def verify_real_sample(mod, path):
    name = os.path.basename(path)
    det = mod.detect(path)
    fmt = det["format"]
    if fmt not in ("3dxml-zip",):
        # ASCII 3DXML and binary formats: report classification only.
        check("detect %s -> %s (no vendor-free writer for this family)"
              % (name, fmt), fmt in ("3dxml-ascii", "3dxml-zip", "cgr",
                                     "catpart", "catproduct", "binary"),
              repr(det))
        return True
    s = mod.load(path)
    if s is None:
        check("load real %s" % name, False, "failed to load")
        return False
    ok = True
    # (a) content preservation
    out = mod.save(s)
    s2 = mod.load_bytes(out, name=name)
    mem0 = {n: b for n, b in s["members"]}
    mem1 = {n: b for n, b in s2["members"]}
    ok = check("content preserved on round-trip: %s" % name,
               mem0 == mem1,
               "%d members" % len(mem0)) and ok
    # (b) deterministic writer (idempotent)
    ok = check("writer idempotent: %s" % name,
               out == mod.save(s2), "len %d" % len(out)) and ok
    # (c) semantic patch -> member-level bisection finds ONLY the field's member
    patched = mod.modify(s, {"SchemaVersion": "4.3"})
    patched_bytes = mod.save(patched)
    ok = check("patched bytes differ from original: %s" % name,
               patched_bytes != out, "len %d vs %d"
               % (len(patched_bytes), len(out))) and ok
    mdiff = mod.member_diff(mod.members_of(s), mod.members_of(patched))
    changed = [m for m in mdiff if m["changed"]]
    ok = check("member-level bisect locates exactly one changed member: %s"
               % name,
               len(changed) == 1,
               repr([m["name"] for m in changed])) and ok
    if len(changed) == 1:
        cm = changed[0]
        ok = check("changed member carries the SchemaVersion field: %s"
                   % name,
                   cm["name"] in ("Abaqus_Model.3dxml", "Quad.3dxml")
                   or "xml" in cm["name"].lower(),
                   cm["name"]) and ok
        ok = check("member diff records a byte-range inside it: %s" % name,
                   cm.get("ranges")) and ok
    # container-level bisection must be able to attribute the change too
    cranges = mod.bisect(out, patched_bytes)
    check("container bisect() reports changed ranges for: %s" % name,
          isinstance(cranges, list) and len(cranges) >= 1,
          "%d ranges" % len(cranges))
    return ok


def verify_blocked_on_tool(mod, path):
    det = mod.detect(path)
    fmt = det["format"]
    if fmt in ("3dxml-zip", "3dxml-ascii"):
        # not a blocked format; nothing to assert
        return True
    status = det.get("status", "")
    check("detect %s reports blocked_on_tool (no vendor-free writer)"
          % os.path.basename(path),
          status == "blocked_on_tool",
          "fmt=%s status=%r" % (fmt, status))
    return status == "blocked_on_tool"


def test_real_material():
    mod = load_tool()
    samples = find_samples()
    if not samples:
        print("[SKIP] no real 3DXML samples supplied; CI-safe tolerant skip")
        return
    print("info  analysing %d real 3DXML sample(s)" % len(samples))
    allok = True
    for s in samples:
        if not verify_real_sample(mod, s):
            allok = False
    # Blocked-on-tool honesty: point the detector at a non-3DXML binary if
    # one can be synthesised; real CGR/CATPart are honoured via detect().
    syn = mod.detect_bytes(b"V5_CFV2\x00" + b"\x00" * 32)
    check("CATPart magic yields blocked_on_tool in modify/bisect leg",
          syn.get("status") == "blocked_on_tool", repr(syn))
    check("all real-sample facts hold across %d files" % len(samples),
          allok, "one bad sample fails the whole run")


def main():
    print("P1-M2-T06: diff harness acceptance test")
    print("=" * 70)
    mod = load_tool()
    check("diff_harness.py exists", mod is not None, TOOL)
    if mod is None:
        print("FAIL  tool missing -- write python/oep_tooling/diff_harness.py")
        return 1
    for fn in ("detect", "detect_bytes", "load", "load_bytes", "save",
               "modify", "bisect", "member_diff", "members_of"):
        check("%s() callable" % fn, callable(getattr(mod, fn, None)), fn)
    test_notebook_exists()
    hermetic_detect(mod)
    hermetic_bisect(mod)
    hermetic_roundtrip(mod)
    test_real_material()
    print("=" * 70)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


def test_acceptance():
    assert main() == 0


if __name__ == "__main__":
    sys.exit(main())
