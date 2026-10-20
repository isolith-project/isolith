#!/usr/bin/env python3
"""Isolith paper-trail archive harness (Don 2026-10-19).

The program's internal paper trail lives OUT of the repo, canonically at the
NAS archive. This script verifies archive integrity against the shipped
SHA256SUMS + manifest.json, and will grow to run the record-content checks
(Stage 1 of the relocation plan) against the archive instead of in-repo copies.

Usage:
    python3 scripts/paper-harness.py verify --root <archive-dir>
"""
import argparse, hashlib, json, sys

def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

def verify(root):
    sums = {}
    missing, bad = [], []
    with open(root + "/SHA256SUMS", encoding="utf-8") as fh:
        for line in fh:
            if not line.strip():
                continue
            digest, name = line.split(None, 1)
            sums[name.strip()] = digest
    for name, expect in sums.items():
        path = root + "/" + name
        if name == "SHA256SUMS":
            continue
        try:
            have = sha256(path)
        except FileNotFoundError:
            missing.append(name)
            continue
        if have != expect:
            bad.append(name)
    ok = len(sums) - len(missing) - len(bad)
    print("[paper-harness] archive: %s" % root)
    print("[paper-harness] files ok=%d missing=%d tampered=%d total=%d"
          % (ok, len(missing), len(bad), len(sums)))
    for m in missing:
        print("  MISSING %s" % m)
    for b in bad:
        print("  TAMPERED %s" % b)
    return 0 if (not missing and not bad) else 1

def main():
    ap = argparse.ArgumentParser(prog="paper-harness")
    sub = ap.add_subparsers(dest="cmd", required=True)
    v = sub.add_parser("verify", help="verify archive integrity")
    v.add_argument("--root", required=True)
    args = ap.parse_args()
    if args.cmd == "verify":
        sys.exit(verify(args.root))

if __name__ == "__main__":
    main()
