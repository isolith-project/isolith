#!/usr/bin/env python3
"""P1-M1-T03 acceptance test: public CATPart/CATProduct sample corpus located
and file identity verified (magic V5_CFV2) per file.

TDD: this test documents the acceptance criteria BEFORE the samples are located,
so it must FAIL (RED) when run against a tree with no samples, then PASS (GREEN)
once real public samples land and every file's identity is verified.

'Done' for P1-M1-T03 means: a non-empty set of real public CATPart/CATProduct
files are present in this repo under corpus/data/catpart/ and each one starts
with the Dassault V5 container magic "V5_CFV2" (hex 56 35 5f 43 46 56 32), and
every file is registered in the sample registry with provenance + license.

Pure stdlib. Runs under `python3 tests/catpart_samples_test.py` and pytest.
Exit code 1 on any failure.
"""
import json
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Canonical Dassault V5 container magic, per PRONOM x-fmt/439 and
# cadmpeg/docs/formats/catia.md: files start with ASCII "V5_CFV2" then a NUL.
V5_MAGIC = b"V5_CFV2\x00"
SAMPLE_DIR = os.path.join(REPO, "corpus", "data", "catpart")
REGISTRY = os.path.join(REPO, "corpus", "catpart-samples.json")

_failures = []
_files = []
_entries = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def sha256_of(path):
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def test_sample_corpus_exists():
    """A real, non-empty sample corpus exists."""
    check("sample dir exists: corpus/data/catpart/", os.path.isdir(SAMPLE_DIR))
    if os.path.isdir(SAMPLE_DIR):
        _files[:] = [f for f in os.listdir(SAMPLE_DIR)
                     if f.lower().endswith((".catpart", ".catproduct"))
                     and not f.startswith(".")]
        check("at least one .CATPart/.CATProduct present", len(_files) >= 1,
              "found %d" % len(_files))
    else:
        _files[:] = []


def test_magic_identity():
    """File identity: magic V5_CFV2 per file. No vendor tool, no extraction:
    read the first 8 bytes and assert the magic."""
    for f in _files:
        p = os.path.join(SAMPLE_DIR, f)
        with open(p, "rb") as fh:
            head = fh.read(len(V5_MAGIC))
        check("magic V5_CFV2: %s" % f, head == V5_MAGIC,
              "head=%r" % head if head != V5_MAGIC else "")


def test_registry():
    """Every sample is registered with provenance + license; registered
    identity (size + sha256) matches the on-disk file."""
    check("registry exists: corpus/catpart-samples.json", os.path.isfile(REGISTRY))
    if os.path.isfile(REGISTRY):
        with open(REGISTRY, encoding="utf-8") as fh:
            reg = json.load(fh)
        _entries[:] = reg.get("entries", [])
        check("registry has schema_version 1", reg.get("schema_version") == 1,
              "schema_version=%r" % reg.get("schema_version"))
        check("registry is non-empty", len(_entries) >= 1,
              "found %d" % len(_entries))
        reg_ids = {e["id"] for e in _entries}
        for f in _files:
            check("registered: %s" % f, f in reg_ids)
        for e in _entries:
            p = os.path.join(SAMPLE_DIR, e["id"])
            if not os.path.isfile(p):
                check("entry file exists: %s" % e["id"], False)
                continue
            size = os.path.getsize(p)
            check("entry size matches: %s" % e["id"], e["size"] == size,
                  "registered=%s actual=%s" % (e["size"], size))
            check("entry sha256 matches: %s" % e["id"],
                  e["sha256"] == sha256_of(p))
            for req in ("source", "license"):
                check("entry has %s: %s" % (req, e["id"]), bool(e.get(req)),
                      repr(e.get(req)))
    else:
        _entries[:] = []


def main():
    print("Isolith P1-M1-T03 CATPart/CATProduct corpus acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_sample_corpus_exists()
    test_magic_identity()
    test_registry()
    print("=" * 60)
    if _failures:
        print("ACCEPTANCE: FAIL - %d failing check(s): %s"
              % (len(_failures), ", ".join(_failures)))
        return 1
    print("ACCEPTANCE: PASS - %d CATPart/CATProduct sample(s) with V5_CFV2 "
          "identity verified." % len(_entries))
    return 0


if __name__ == "__main__":
    sys.exit(main())
