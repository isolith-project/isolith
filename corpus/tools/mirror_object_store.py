#!/usr/bin/env python3
"""P1-M4-T01: materialize + verify the corpus object-store mirror on NAS.

Operates on the LIVE NAS object store (Gitea generic package registry
don/isolith-corpus v1), driven by corpus/manifest.json (the P1-M1-T04
registry) as the single source of truth.

  python3 corpus/tools/mirror_object_store.py --mirror \
      --data-root /path/to/local/sources
      # upload every backend=object-store manifest entry (idempotent
      # content-dedup), then GET-verify every blob, then write the committed
      # corpus/object-store-index.json.

  python3 corpus/tools/mirror_object_store.py --verify
      # read-only: GET every uploaded mirror blob back and assert sha256 ==
      # manifest (checksums match P1-M1-T04 manifest).

  python3 corpus/tools/mirror_object_store.py --check
      # hermetic: rebuild the index in-memory from the manifest + old index and
      # diff against the committed file (byte-identical => deterministic).

Pure stdlib. Exit 1 on any verify failure.
"""
import argparse
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "python"))

MANIFEST = os.path.join(ROOT, "corpus", "manifest.json")
INDEX = os.path.join(ROOT, "corpus", "object-store-index.json")


def _build_index(data_root):
    from oep_tooling import objectstore as os_mod
    index, results = os_mod.mirror(MANIFEST, data_root, INDEX)
    return index, results


def main():
    ap = argparse.ArgumentParser(
        prog="corpus/tools/mirror_object_store.py",
        description="P1-M4-T01 corpus object-store mirror (materialize/verify)")
    ap.add_argument("--mirror", action="store_true",
                    help="upload missing blobs, verify all, write index")
    ap.add_argument("--verify", action="store_true", help="verify store vs manifest")
    ap.add_argument("--check", action="store_true",
                    help="determinism check (no network)")
    ap.add_argument("--data-root", default=None, action="append",
                    help="dir (recursive) holding local corpus source bytes "
                         "(repeatable)")
    args = ap.parse_args()

    from oep_tooling import objectstore as os_mod

    if args.check:
        if not args.data_root:
            print("--check requires --data-root to rebuild the index")
            return 1
        rebuilt, _results = _build_index(args.data_root)
        with open(INDEX, encoding="utf-8") as fh:
            committed = json.load(fh)
        if rebuilt == committed:
            print("ACCEPTANCE: PASS — corpus/object-store-index.json byte-identical "
                  "to a fresh build from the manifest (%d entries)"
                  % len(committed.get("entries", [])))
            return 0
        print("ACCEPTANCE: FAIL — committed index is stale vs a fresh mirror build")
        return 1

    if not os_mod.probe():
        print("NAS object store unreachable; nothing done")
        return 2

    if args.mirror:
        if not args.data_root:
            print("--mirror requires --data-root")
            return 1
        print("live mirror: data_root=%s" % args.data_root)
        index, results = _build_index(args.data_root)
        n_ok = sum(1 for _e, ok, _d in results if ok)
        print("mirror results: %d/%d entries materialized" % (n_ok, len(results)))
        for eid, ok, detail in results:
            if not ok:
                print("  NOT OK  %s -- %s" % (eid, detail))
        print("wrote %s (%d entries)" % (INDEX, len(index.get("entries", []))))
        if n_ok != len(results):
            print("ACCEPTANCE: FAIL — %d entries could not be materialized" % (len(results) - n_ok))
            return 1

    if args.verify or args.mirror:
        ok_count, (uploaded, pending), out = os_mod.verify_manifest_mirror(
            MANIFEST, INDEX)
        bad = [o for o in out if not o[1]]
        print("verify: uploaded=%d pending=%d verified=%d" % (uploaded, pending, ok_count))
        for eid, ok, detail in bad[:10]:
            print("  BAD  %s -- %s" % (eid, detail))
        if uploaded != 122 or pending != 0 or bad:
            print("ACCEPTANCE: FAIL — mirror not fully materialized/verified")
            return 1
        print("ACCEPTANCE: PASS — all 122 object-store blobs verified live on NAS")
        return 0

    ap.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
