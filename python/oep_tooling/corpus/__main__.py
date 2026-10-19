#!/usr/bin/env python3
"""oep_tooling.corpus CLI: manage the shared test-data registry + golden runner.

Subcommands:
  list             show registry entries
  verify           verify repo-backed entry checksums against the manifest
  fetch [--cache]  materialize object-store entries into the local cache
  golden --spec    run a golden-file test spec (corpus/golden_tests.json)
"""
import argparse
import os
import sys

from oep_tooling.corpus import registry, runner


def cmd_list(args):
    m = registry.load_manifest()
    print("%-24s %-12s %-12s %8s  %s" % ("ID", "FORMAT", "GEOM", "BYTES", "BACKEND"))
    for e in m["data"].get("entries", []):
        print("%-24s %-12s %-12s %8d  %s" % (
            e["id"], e.get("format", ""), e.get("geometry_class", ""),
            e.get("size", 0), e.get("backend")))
    return 0


def cmd_verify(args):
    m = registry.load_manifest()
    bad = 0
    for eid, ok, detail in registry.verify_repo_checksums(m):
        print("[%s] %s%s" % ("PASS" if ok else "FAIL", eid,
                             " -- " + detail if detail else ""))
        if not ok:
            bad += 1
    return 1 if bad else 0


def cmd_fetch(args):
    m = registry.load_manifest()
    any_err = 0
    for e in m["data"].get("entries", []):
        if e["backend"] != "object-store":
            continue
        path, err = registry.resolve_entry(e, manifest=m, cache_dir=args.cache)
        if err:
            print("[SKIP] %s -- %s" % (e["id"], err))
            any_err = 1  # fetch failure is a warning, not a hard fail (tolerant)
        else:
            print("[OK]   %s -> %s" % (e["id"], path))
    return any_err


def cmd_golden(args):
    results, code = runner.run_spec(args.spec, cache_dir=args.cache)
    runner.print_summary(results)
    return code


def main(argv=None):
    ap = argparse.ArgumentParser(prog="python -m oep_tooling.corpus",
                                 description="Keelworks test corpus harness")
    ap.add_argument("--cache", default=None,
                    help="cache dir for object-store blobs (default corpus/cache)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list", help="show registry entries")
    sub.add_parser("verify", help="verify repo-backed entry checksums")
    sub.add_parser("fetch", help="materialize object-store entries into cache")
    g = sub.add_parser("golden", help="run a golden-file test spec")
    g.add_argument("--spec", default="corpus/golden_tests.json", help="spec file path")
    args = ap.parse_args(argv)
    fn = {"list": cmd_list, "verify": cmd_verify, "fetch": cmd_fetch,
          "golden": cmd_golden}[args.cmd]
    return fn(args)


if __name__ == "__main__":
    sys.exit(main())
