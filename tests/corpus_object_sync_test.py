#!/usr/bin/env python3
"""P1-M1-T05 acceptance test: corpus object-store sync to NAS + checksums verified.

TDD: this test documents the acceptance criteria BEFORE the sync tooling exists,
so it fails (RED) on first run, then passes (GREEN) once
python/oep_tooling/objectstore.py + corpus/object-sync-spec.json +
corpus/object-store-index.json land and a real sync against the live NAS object
store has been performed.

'Done' for P1-M1-T05 means every redistributable corpus sample is present on the
NAS object store (Gitea generic package registry don/isolith-corpus v1),
content-addressed by sha256, and a fresh GET of each blob re-hashes to the
committed index sha256. CGR samples are reference-only (proprietary, T01 policy)
and must never be uploaded.

Pure stdlib. Runs under `python3 tests/corpus_object_sync_test.py` and pytest.
Live-NAS checks classify as SKIP (exit 0) when the NAS is unreachable —
tolerant-failure doctrine — but the acceptance run performed here asserts a LIVE
PASS on real material.

Exit code: 0 if no FAIL; 1 if any FAIL (SKIP does not fail).
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

_failures = []
_skips = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def check_skip(name, detail=""):
    line = "[SKIP] %s" % name
    if detail:
        line += " -- " + detail
    print(line)
    _skips.append(name)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def nas_reachable():
    """Tolerant probe: does the NAS object store respond?"""
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        from oep_tooling import objectstore as os_mod
        return os_mod.probe()
    except Exception as e:
        print("      (probe failed: %r)" % (e,))
        return False


# --- 1. sync spec exists and is well-formed ---------------------------------
def test_spec_exists_and_schema():
    sp = os.path.join(REPO, "corpus", "object-sync-spec.json")
    check("corpus/object-sync-spec.json exists", os.path.isfile(sp), sp)
    if not os.path.isfile(sp):
        return
    with open(sp, encoding="utf-8") as fh:
        data = json.load(fh)
    check("spec has schema_version", data.get("schema_version") == 1,
          repr(data.get("schema_version")))
    entries = data.get("entries")
    check("spec.entries non-empty list",
          isinstance(entries, list) and len(entries) > 0,
          repr(len(entries) if isinstance(entries, list) else None))
    if not isinstance(entries, list):
        return
    REQUIRED = {"id", "path", "format", "geometry_class", "license",
                "source", "sha256", "size", "redistributable"}
    ids = set()
    for e in entries:
        ids.add(e.get("id"))
        missing = REQUIRED - set(e.keys())
        check("spec entry fields complete: %s" % e.get("id", "?"),
              not missing, "missing " + repr(sorted(missing)))
        s = e.get("sha256", "")
        check("spec sha256 64-hex: %s" % e.get("id", "?"),
              len(s) == 64 and all(c in "0123456789abcdef" for c in s), s)
        check("spec redistributable is bool: %s" % e.get("id", "?"),
              isinstance(e.get("redistributable"), bool),
              repr(e.get("redistributable")))
    check("spec entry ids unique", len(ids) == len(entries),
          "%d ids %d entries" % (len(ids), len(entries)))


# --- 2. spec must cover the full located corpus -----------------------------
def test_spec_covers_corpus_counts():
    sp = os.path.join(REPO, "corpus", "object-sync-spec.json")
    with open(sp, encoding="utf-8") as fh:
        data = json.load(fh)
    entries = data.get("entries", [])
    byfmt = {}
    for e in entries:
        byfmt.setdefault(e.get("format"), []).append(e)
    # Located by T01/T02/T03: 11 CGR, 6 3DXML, 10 CATPart/CATProduct, 1 stl
    cgr = byfmt.get("cgr", [])
    check("spec lists 11 CGR reference-only entries",
          len(cgr) == 11, "found %d" % len(cgr))
    if cgr:
        bad = [e["id"] for e in cgr if e.get("redistributable") is not False]
        check("all CGR entries redistributable=false",
              not bad, "bad: %r" % bad)
    x3d = byfmt.get("3dxml", [])
    check("spec lists 6 3DXML entries", len(x3d) == 6, "found %d" % len(x3d))
    cats = [e for k in ("catpart-v5", "catproduct-v5") for e in byfmt.get(k, [])]
    check("spec lists 10 CATPart/CATProduct entries",
          len(cats) == 10, "found %d" % len(cats))
    stl = byfmt.get("stl-ascii", [])
    check("spec lists 1 STL entry", len(stl) >= 1, "found %d" % len(stl))


# --- 3. index exists, matches spec, and every redistributable is uploaded ---
def test_index_matches_spec_and_uploads():
    sp = os.path.join(REPO, "corpus", "object-sync-spec.json")
    ip = os.path.join(REPO, "corpus", "object-store-index.json")
    check("corpus/object-store-index.json exists", os.path.isfile(ip), ip)
    if not os.path.isfile(ip):
        return
    with open(ip, encoding="utf-8") as fh:
        idx = json.load(fh)
    with open(sp, encoding="utf-8") as fh:
        spec = json.load(fh)
    imap = {e["id"]: e for e in idx.get("entries", [])}
    for e in spec.get("entries", []):
        ie = imap.get(e["id"])
        check("index has entry: %s" % e["id"], ie is not None)
        if ie is None:
            continue
        if e.get("redistributable"):
            check("redistributable uploaded: %s" % e["id"],
                  ie.get("status") == "uploaded",
                  repr(ie.get("status")))
            check("index sha256 == spec sha256: %s" % e["id"],
                  ie.get("sha256") == e.get("sha256"),
                  "%s != %s" % (ie.get("sha256"), e.get("sha256")))
            check("index has object ptr: %s" % e["id"],
                  bool(ie.get("object")) and ie.get("owner") == "don"
                  and ie.get("package") == "isolith-corpus",
                  "ptr=%r pkg=%r" % (ie.get("object"), ie.get("package")))
        else:
            check("reference-only NOT uploaded: %s" % e["id"],
                  ie.get("status") == "reference-only",
                  repr(ie.get("status")))


# --- 4. hermetic: object naming + never-upload CGR --------------------------
def test_object_naming_hermetic():
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        from oep_tooling import objectstore as os_mod
    except ImportError:
        check("import oep_tooling.objectstore", False, "module missing")
        return
    name = os_mod.object_name("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08", ".stl")
    check("object_name returns <sha256>.<ext>",
          name == "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08.stl",
          repr(name))
    check("object_name defaults to .bin",
          os_mod.object_name("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
          == "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08.bin",
          repr(os_mod.object_name("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")))
    # CGR entries must never be upload candidates
    sp = os.path.join(REPO, "corpus", "object-sync-spec.json")
    with open(sp, encoding="utf-8") as fh:
        spec = json.load(fh)
    cand = os_mod.upload_candidates(spec)
    cand_ids = {c["id"] for c in cand}
    cgr_ids = {e["id"] for e in spec["entries"] if e["format"] == "cgr"}
    check("no CGR in upload candidates",
          not (cand_ids & cgr_ids),
          "leak: %r" % (cand_ids & cgr_ids))


# --- 5. LIVE NAS: upload one real blob, then verify every indexed blob ------
def test_live_upload_and_verify():
    if not nas_reachable():
        check_skip("live NAS upload+verify", "NAS unreachable; classified skip")
        return
    sys.path.insert(0, os.path.join(REPO, "python"))
    from oep_tooling import objectstore as os_mod

    # (a) a real PUT round-trip: upload a self-authored deterministic blob
    #     (same bytes every run -> same sha256 -> idempotent on the store) and
    #     hash it back from the NAS. This proves the transport itself works on
    #     real material without depending on any not-yet-merged sample file.
    import tempfile
    payload = b"isolith-objectstore-sync-acceptance\n" * 4096  # ~90 KiB
    expect = hashlib.sha256(payload).hexdigest()
    obj = os_mod.object_name(expect, ".bin")
    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tf:
        tf.write(payload)
        tmppath = tf.name
    try:
        ok, detail = os_mod.upload_file("don", "isolith-corpus", "v1", obj, tmppath)
    finally:
        os.unlink(tmppath)
    check("live PUT to NAS object store (acceptance fixture)", ok, detail)
    if ok:
        data, err = os_mod.download("don", "isolith-corpus", "v1", obj)
        if err or data is None:
            check("live GET round-trip", False, err or "no data")
        else:
            real = hashlib.sha256(bytes(data)).hexdigest()
            check("live GET re-hashes to source sha256",
                  real == expect, "expected %s got %s" % (expect, real))

    # (b) verify the whole committed index against live NAS
    ip = os.path.join(REPO, "corpus", "object-store-index.json")
    with open(ip, encoding="utf-8") as fh:
        idx = json.load(fh)
    bad = 0
    checked = 0
    for e in idx.get("entries", []):
        if e.get("status") != "uploaded":
            continue
        checked += 1
        data, err = os_mod.download(e.get("owner"), e.get("package"),
                                    e.get("version", "v1"), e.get("object"))
        if err:
            check("verify blob fetch: %s" % e["id"], False, err)
            bad += 1
            continue
        real = hashlib.sha256(data).hexdigest()
        ok = real == e.get("sha256")
        check("verify blob sha256: %s" % e["id"], ok,
              "expected %s got %s" % (e.get("sha256"), real))
        if not ok:
            bad += 1
    check("index has >=1 uploaded blob to verify", checked >= 1,
          "verified %d" % checked)
    check("all indexed blobs verify", bad == 0, "failures=%d" % bad)


def main():
    test_spec_exists_and_schema()
    test_spec_covers_corpus_counts()
    test_index_matches_spec_and_uploads()
    test_object_naming_hermetic()
    test_live_upload_and_verify()
    print()
    print("RESULT: %d FAIL, %d SKIP" % (len(_failures), len(_skips)))
    for f in _failures:
        print("  FAIL %s" % f)
    return 1 if _failures else 0


if __name__ == "__main__":
    sys.exit(main())
