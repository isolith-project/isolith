#!/usr/bin/env python3
"""P1-M4-T01 acceptance test: object-store corpus mirror materialized + verified.

TDD: documents the acceptance criteria BEFORE the mirror exists, so it fails
(RED) against the wave-1 state (where 122 object-store entries resolve as
SKIP object-store-unresolved because the blobs were never materialized), then
passes (GREEN) once the full mirror is on the NAS object store and the
golden/regression runners resolve entries from it end to end.

'Done' for P1-M4-T01 means:
  - every backend=object-store entry of corpus/manifest.json has a wellformed
    content-addressed object_store pointer (owner/package/version/object);
  - corpus/object-store-index.json covers all 122 mirror entries with
    status=uploaded (committed proof of materialization), object names exactly
    matching the manifest pointers;
  - live NAS: every uploaded mirror blob GETs back and its sha256 equals the
    manifest sha256 (checksums match P1-M1-T04 manifest) — 0 failures, 0
    pending;
  - live NAS: the P2/P3 regression runner resolves the shared corpus from the
    object store end to end (`--fetch`), 139/139 entries are LOAD (OPEN) or a
    classified SKIP with a real reason, and zero entries are left at
    object-store-unresolved;
  - no NEW skips vs wave 1: every entry that was OPEN in the wave-1 P2/P3
    regression matrices is still OPEN (box.stl, the 3DXML repo set).

Live-NAS checks classify as SKIP (exit 0) when the store is unreachable or the
token is absent — tolerant-failure doctrine — but the acceptance run performs a
LIVE PASS on real material.

Pure stdlib. Runs under `python3 tests/corpus_object_mirror_test.py` and pytest.
Exit code: 0 if no FAIL; 1 if any FAIL (SKIP does not fail).
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

_failures = []
_skips = []
_results = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    _results.append((name, status))
    if not ok:
        _failures.append(name)


def check_skip(name, detail=""):
    print("[SKIP] %s -- %s" % (name, detail))
    _skips.append(name)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def nas_reachable():
    try:
        from oep_tooling import objectstore as os_mod
        return os_mod.probe()
    except Exception as e:
        print("      (probe failed: %r)" % (e,))
        return False


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _object_store_entries(manifest):
    return [e for e in manifest.get("entries", [])
            if e.get("backend") == "object-store"]


# --- 1. manifest object-store pointers wellformed + content-addressed -------
def test_manifest_pointers_wellformed():
    m = load(os.path.join(REPO, "corpus", "manifest.json"))
    es = _object_store_entries(m)
    check("manifest declares 122 object-store entries",
          len(es) == 122, "found %d" % len(es))
    for e in es:
        ptr = e.get("object_store") or {}
        ok = ({"owner", "package", "version", "object"} <= set(ptr)
              and ptr.get("object") == "%s.%s" % (e["sha256"], ptr["object"].split(".")[-1]))
        check("object-store pointer wellformed: %s" % e["id"][:42], ok, repr(ptr))


# --- 2. committed mirror index covers all 122, names match manifest ---------
def test_mirror_index_covers_manifest():
    m = load(os.path.join(REPO, "corpus", "manifest.json"))
    ip = os.path.join(REPO, "corpus", "object-store-index.json")
    check("corpus/object-store-index.json exists", os.path.isfile(ip), ip)
    if not os.path.isfile(ip):
        return
    idx = load(ip)
    entries = idx.get("entries", [])
    uploaded = {e["id"]: e for e in entries if e.get("status") == "uploaded"}
    want = {e["id"] for e in _object_store_entries(m)}
    missing = want - set(uploaded)
    check("mirror index covers all 122 manifest object-store entries",
          not missing, "missing: %r" % sorted(missing)[:5])
    # object names exactly match the manifest pointers (no translation at fetch)
    # -- assessed on the manifest-set only; carried-over wave-1 spec-id entries
    # are preserved for the P1-M1-T05 acceptance and are out of this invariant.
    nmis = [e["id"] for e in _object_store_entries(m)
            if uploaded.get(e["id"])
            and uploaded[e["id"]].get("object") != e["object_store"]["object"]]
    check("mirror object names == manifest pointers (manifest set)",
          not nmis, "mismatch: %r" % nmis[:5])


# --- 3. LIVE: every mirror blob fetches back and matches manifest sha256 ------
def test_live_mirror_checksums():
    if not nas_reachable():
        check_skip("live mirror checksum verify", "NAS unreachable")
        return
    from oep_tooling import objectstore as os_mod
    m = load(os.path.join(REPO, "corpus", "manifest.json"))
    ok_count, (uploaded, pending), out = os_mod.verify_manifest_mirror(
        os.path.join(REPO, "corpus", "manifest.json"),
        os.path.join(REPO, "corpus", "object-store-index.json"))
    check("live: no pending mirror blobs",
          pending == 0, "pending=%d" % pending)
    bad = [o for o in out if not o[1]]
    check("live: every uploaded blob re-hashes (0 BAD)",
          ok_count == uploaded and not bad,
          "ok=%d/%d bad=%r" % (ok_count, uploaded, bad[:3]))
    # every manifest object-store entry is uploaded AND matches the manifest sha
    manifest_ids = {e["id"] for e in _object_store_entries(m)}
    by_id = {o[0]: o for o in out}
    missing = sorted(manifest_ids - set(by_id))
    mism = [eid for eid in manifest_ids
            if eid in by_id and by_id[eid][1] is False]
    check("live: all 122 manifest mirror blobs verified (checksum == manifest)",
          not missing and not mism,
          "not-uploaded=%r bad=%r" % (missing, mism[:3]))


# --- 4. LIVE: regression runner resolves the shared corpus end to end --------
def test_live_regression_fetch_full_corpus():
    if not nas_reachable():
        check_skip("live regression --fetch sweep", "NAS unreachable")
        return
    import oep_tooling.regression as R
    mpath = os.path.join(REPO, "corpus", "manifest.json")
    m = load(mpath)
    with tempfile.TemporaryDirectory(prefix="kw-mirror-cache-") as td:
        rows = R.sweep(entries=m["entries"], corpus_root=os.path.join(REPO, "corpus"),
                       cache_dir=td, fetch=True)
    by_id = {r.id: r for r in rows}
    check("sweep covers all 139 manifest entries",
          len(rows) == 139, "rows=%d" % len(rows))
    un = [r.id for r in rows if r.cls == "object-store-unresolved"]
    check("zero object-store-unresolved (store end-to-end)",
          not un, "still unresolved: %r" % un[:5])
    for r in rows:
        check("row classified OPEN or real SKIP: %s" % r.id[:50],
              r.status in ("OPEN", "SKIP")
              and (r.status == "OPEN" or r.cls in R.CLOSED_TAXONOMY),
              "status=%r cls=%r" % (r.status, r.cls))
    # no NEW skips vs wave 1: wave-1 OPEN set must still be OPEN
    wave1_open = {"box.stl", "FM1_satellite.3DXML", "blocks.3dxml",
                  "cube1.3dxml", "cycloidal.3DXML", "machinist.3DXML",
                  "quad.3dxml"}
    regressed = [eid for eid in sorted(wave1_open)
                 if by_id.get(eid) and by_id[eid].status != "OPEN"]
    check("no NEW skips vs wave 1 (wave-1 OPEN set stays OPEN)",
          not regressed, "regressed: %r" % regressed)
    obj_open = [r.id for r in rows if r.backend == "object-store" and r.status == "OPEN"]
    check("object-store backend resolves real opens (>=1)",
          len(obj_open) >= 1, "object-store OPEN: %r" % obj_open)


def main():
    print("Keelworks P1-M4-T01 object-store mirror acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_manifest_pointers_wellformed()
    test_mirror_index_covers_manifest()
    test_live_mirror_checksums()
    test_live_regression_fetch_full_corpus()
    print("=" * 60)
    print("RESULT: %d FAIL, %d SKIP" % (len(_failures), len(_skips)))
    for f in _failures:
        print("  FAIL %s" % f)
    return 1 if _failures else 0


if __name__ == "__main__":
    sys.exit(main())
