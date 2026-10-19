#!/usr/bin/env python3
"""Acceptance test for Keelworks P0-M5-T01 shared standards reference materials.

Asserts, against the REAL files in the repo, that:
  1. A standards index exists and covers exactly the four program standards:
     STEP AP242, QIF, MIL-STD-31000, ASME Y14.41.
  2. Each standard has a directory with README.md, keelworks-use.md, references.md.
  3. Each README carries the required metadata blocks (publisher, current edition,
     availability, license/redistribution status, official URL).
  4. Fair-quoting discipline: no vendored full-text spec; quoted extracts are marked
     with '>' and stay within a bounded budget; no file claims to include a proprietary
     PDF body.
  5. reference URLs are well-formed http(s) links (checked off-network as far as
     format; the live resolution pass is separate).
  6. Every standard is mapped to at least one Keelworks workstream/milestone in the
     index and in its keelworks-use.md.

Run:  python3 tests/standards_test.py
Exit 0 = pass; non-zero = fail. Zero dependencies (stdlib only).
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STANDARDS = os.path.join(ROOT, "standards")

# canonical short key -> (expected dir name, official spec name keyword)
EXPECTED = {
    "ap242": "ap242",
    "qif": "qif",
    "mil-std-31000": "mil-std-31000",
    "asme-y14.41": "asme-y14.41",
}

REQUIRED_FILES = ["README.md", "keelworks-use.md", "references.md"]

REQUIRED_README_SECTIONS = [
    "Standard",       # official name + document id
    "Publisher",      # issuing body
    "Current edition",
    "Availability",   # public / paywalled / freely downloadable
    "License / redistribution status",
    "Official source",  # canonical URL or where to obtain
    "Scope",            # what it covers (may cite public abstract, fair quote)
    "PMI / MBD relevance",  # why keelworks cares
]

# Fair-quote budget: any single quoted extract must be <= this many chars.
MAX_QUOTE_LENGTH = 1200
# Hard cap on any markdown file length to prevent accidental spec vendoring.
MAX_FILE_LENGTH = 20000

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def has_heading(text, heading):
    """Heading present as a markdown heading (any level) containing the text."""
    return any(line.strip().startswith("#") and heading.lower() in line.lower()
               for line in text.splitlines())


def analyze_quotes(text):
    """Return (count, max_len) of '>'-quoted lines."""
    count = 0
    max_len = 0
    cur = 0
    in_quote = False
    for line in text.splitlines():
        if line.strip().startswith(">"):
            in_quote = True
            count += 1
            cur += len(line)
        else:
            if in_quote:
                max_len = max(max_len, cur)
                cur = 0
                in_quote = False
    if in_quote:
        max_len = max(max_len, cur)
    return count, max_len


def url_ok(u):
    return re.match(r"^https?://[^\s]+$", u) is not None


def main():
    check(os.path.isdir(STANDARDS), "standards/ directory missing")

    index_path = os.path.join(STANDARDS, "README.md")
    check(os.path.exists(index_path), "standards/README.md (index) missing")
    index_text = None
    if os.path.exists(index_path):
        index_text = open(index_path, encoding="utf-8").read()
        for key in EXPECTED:
            check(key in index_text.lower(),
                  "index does not mention expected standard key: %s" % key)

    for key, dirname in EXPECTED.items():
        d = os.path.join(STANDARDS, dirname)
        check(os.path.isdir(d), "standards/%s/ missing" % dirname)
        for f in REQUIRED_FILES:
            fp = os.path.join(d, f)
            check(os.path.exists(fp), "standards/%s/%s missing" % (dirname, f))
            if os.path.exists(fp):
                text = open(fp, encoding="utf-8").read()
                size = len(text)
                check(size <= MAX_FILE_LENGTH,
                      "standards/%s/%s too large (%d chars) - possible spec vendoring"
                      % (dirname, f, size))

        readme = os.path.join(d, "README.md")
        if os.path.exists(readme):
            text = open(readme, encoding="utf-8").read()
            for section in REQUIRED_README_SECTIONS:
                check(has_heading(text, section),
                      "standards/%s/README.md missing section: %s" % (dirname, section))
            qcount, qmax = analyze_quotes(text)
            check(qmax <= MAX_QUOTE_LENGTH,
                  "standards/%s/README.md quoted extract too long (%d > %d)"
                  % (dirname, qmax, MAX_QUOTE_LENGTH))
            # no vendored pdf/xsd binary reference inside the repo tree
            for f in os.listdir(d):
                if f.lower().endswith((".pdf", ".xsd", ".step", ".stp", ".model")):
                    check(False, "standards/%s/%s appears to vendor a copyrighted "
                          "artifact in-tree (public specs go in references, not files)"
                          % (dirname, f))

        refs = os.path.join(d, "references.md")
        if os.path.exists(refs):
            ref_text = open(refs, encoding="utf-8").read()
            urls = re.findall(r"https?://[^\s\)\]\>]+", ref_text)
            check(len(urls) >= 1,
                  "standards/%s/references.md has no URLs" % dirname)
            bad = [u for u in urls if not url_ok(u)]
            check(not bad, "standards/%s/references.md has malformed URLs: %s" % (dirname, bad))

        use = os.path.join(d, "keelworks-use.md")
        if os.path.exists(use):
            use_text = open(use, encoding="utf-8").read()
            has_ws = bool(re.search(r"\b(P[0-9]|P0-[A-Z])\b", use_text))
            check(has_ws, "standards/%s/keelworks-use.md maps to no workstream "
                  "(no P# milestone token found)" % dirname)

    # index must map each standard to a workstream
    if index_text is not None:
        for key in EXPECTED:
            m = re.search(r"%s.{0,400}" % re.escape(key), index_text, re.IGNORECASE | re.DOTALL)
            check(m and re.search(r"\bP[0-9]", m.group(0)),
                  "index does not map %s to a workstream (P#) token" % key)

    if failures:
        print("STANDARDS ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("STANDARDS ACCEPTANCE PASSED: all 4 standards present, "
          "metadata complete, fair-quote discipline holds.")


if __name__ == "__main__":
    main()
