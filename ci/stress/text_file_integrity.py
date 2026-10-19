#!/usr/bin/env python3
"""Keelworks P0-M4-T03 — repo text-file integrity regression job.

Scans every text file in the repository and verifies, on real material:
  - strict UTF-8 decode (no mojibake / accidental binary in text files);
  - LF-only line endings (no stray CRLF from Windows tooling);
  - no NUL bytes or C0 control bytes that would corrupt a text pipeline.

This is the cheap long-tail safeguard a monorepo needs: it catches junk that
unit tests never see (bad encodings, mixed line endings, copy-paste artifacts)
and runs nightly as part of the stress suite. Deterministic: files scanned in
sorted order, one pass, exit 1 on the first problem report.

Exit 0 = all text files clean.
"""
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))          # repo root (ci/stress -> repo)

EXCLUDE_DIRS = {".git", ".worktrees", "target", ".venv", "__pycache__",
                "node_modules", ".cache", "build"}
SKIP_EXT = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".zip",
            ".gz", ".bz2", ".xz", ".7z", ".bin", ".git", ".lock", ".woff",
            ".woff2", ".ttf", ".otf", ".jar", ".wasm", ".so", ".o", ".a",
            ".exe", ".dll", ".db", ".sqlite", ".pyc", ".whl", ".tar"}


def is_binary(path):
    """Cheap heuristic: NUL byte in the first 8 KiB => binary."""
    try:
        with open(path, "rb") as fh:
            head = fh.read(8192)
    except OSError:
        return True
    return b"\x00" in head


def iter_text_files(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames
                             if d not in EXCLUDE_DIRS and not d.startswith("."))
        for name in sorted(filenames):
            ext = os.path.splitext(name)[1].lower()
            if ext in SKIP_EXT:
                continue
            path = os.path.join(dirpath, name)
            if is_binary(path):
                continue
            yield path


def check_file(path):
    problems = []
    with open(path, "rb") as fh:
        data = fh.read()
    try:
        data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as e:
        problems.append("not strict UTF-8: %s" % e)
    if b"\r" in data:
        problems.append("contains CR (LF-only expected)")
    for b in bytes(range(0x00, 0x09)) + bytes([0x0B, 0x0C]) + \
            bytes(range(0x0E, 0x20)) + bytes([0x7F]):
        if b in data:
            problems.append("contains C0 control byte 0x%02x" % b)
    return problems


def main(argv=None):
    files = sorted(iter_text_files(REPO))
    scanned = 0
    found = 0
    print("Keelworks text-file integrity scan (P0-M4-T03)")
    print("root: %s" % REPO)
    print("-" * 60)
    for path in files:
        rel = os.path.relpath(path, REPO)
        problems = check_file(path)
        scanned += 1
        if problems:
            found += 1
            print("[FAIL] %s" % rel)
            for p in problems:
                print("       - %s" % p)
    print("-" * 60)
    if found:
        print("INTEGRITY FAIL files=%d broken=%d" % (scanned, found))
        return 1
    print("INTEGRITY OK files=%d" % scanned)
    return 0


if __name__ == "__main__":
    sys.exit(main())
