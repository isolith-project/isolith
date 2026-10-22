#!/usr/bin/env python3
"""Acceptance test for the Isolith EDITOR SPINE decision (decision #19, 2026-10-22).

Verifies the real material, not mocks:

  decision  => the record at workstreams/governance/decisions/editor-spine.md
               carries the required front matter (title, decision, version
               1.0.0, date 2026-10-22, status Ratified, owner, applies_to,
               supersedes the decision #13 default-exclude for Class-A) and
               the canonical sections, with no em dash (program surface rule)
  retitle   => the 2027.1 story ships as "interop/PDM foundation release; the
               editor is the product spine" on every named surface: README,
               docs-site landing page, the format capability matrix page, and
               the CHANGELOG-2027.1.0-rc2 release notes. A reader-only release
               never claims to be the editor; the phrase is asserted so a
               future edit cannot silently re-frame the foundation release
  proof     => the CHANGELOG-2027.1.0-rc2.md checksum stays consistent with
               the committed file

Run:  python3 tests/test_editor_spine_decision.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import hashlib
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECISION = os.path.join(ROOT, "workstreams/governance/decisions/editor-spine.md")

# The exact retitle phrase every shipping surface must carry.
RETITLE = "interop/PDM foundation release; the editor is the product spine"

EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

REQUIRED_SECTIONS = [
    "Decision",
    "Why this decision",
    "Options considered",
    "What does not change",
    "Enforcement",
    "Coverage",
    "References",
    "Change log",
]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
    """Return (meta_dict, body) for simple top-level YAML front matter."""
    if not text.startswith("---"):
        return {}, text
    _, fm, body = text.split("---", 2)
    meta = {}
    for line in fm.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or ":" not in stripped:
            continue
        key, _, val = stripped.partition(":")
        meta[key.strip()] = val.strip()
    return meta, body


def plain_text(html):
    """HTML text with tags stripped and all whitespace collapsed."""
    no_tags = re.sub(r"<[^>]+>", "", html)
    return re.sub(r"\s+", " ", no_tags)


def main():
    print("EDITOR SPINE (decision #19) acceptance")
    print("=" * 60)
    print("repo root:", ROOT)

    # 1. Decision record present with the mandated front matter.
    check(os.path.isfile(DECISION), "missing %s" % DECISION)
    if os.path.isfile(DECISION):
        text = read(DECISION)
        check(EM_DASH not in text,
              "forbidden em dash (U+2014) in editor-spine.md")
        meta, body = split_front_matter(text)
        for key in ("title", "decision", "version", "date", "status",
                    "owner", "applies_to", "supersedes"):
            check(key in meta, "front matter missing key: %s" % key)
        check(meta.get("version") == "1.0.0",
              "version must be 1.0.0 (got %r)" % meta.get("version"))
        check(meta.get("date") == "2026-10-22",
              "date must be 2026-10-22 (got %r)" % meta.get("date"))
        check(meta.get("status") == "Ratified",
              "status must be Ratified (got %r)" % meta.get("status"))
        supersedes = meta.get("supersedes", "")
        check("decision #13" in supersedes and "Class-A" in supersedes,
              "supersedes must name the decision #13 default-exclude for Class-A")
        for h in REQUIRED_SECTIONS:
            check(re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(h), body),
                  "missing section: %s" % h)
        low = body.lower()
        check(RETITLE in re.sub(r"\s+", " ", body),
              "decision body must carry the retitle wording")
        check("same-or-greater editor functionality" in low,
              "decision body must state the product bar (same-or-greater editor functionality)")
    else:
        check(False, "cannot validate a missing decision record")

    # 2. Retitle phrase shipped where 2027.1 tells its story.
    retitle_surfaces = {
        "README.md": os.path.join(ROOT, "README.md"),
        "docs-site/index.html": os.path.join(ROOT, "docs-site/index.html"),
        "docs-site/format-matrix.html": os.path.join(ROOT, "docs-site/format-matrix.html"),
        "CHANGELOG-2027.1.0-rc2.md": os.path.join(ROOT, "CHANGELOG-2027.1.0-rc2.md"),
    }
    for label, path in retitle_surfaces.items():
        check(os.path.isfile(path), "missing retitle surface: %s" % label)
        if os.path.isfile(path):
            content = read(path)
            hay = plain_text(content) if path.endswith(".html") else content
            hay = re.sub(r"\s+", " ", hay)
            check(RETITLE in hay,
                  "%s does not carry the retitle phrase: %r" % (label, RETITLE))

    # 3. The changelog checksum proof stays consistent.
    changelog = os.path.join(ROOT, "CHANGELOG-2027.1.0-rc2.md")
    sha_file = changelog + ".sha256"
    if os.path.isfile(changelog) and os.path.isfile(sha_file):
        recorded = read(sha_file).strip().split()[0]
        actual = hashlib.sha256(read(changelog).encode()).hexdigest()
        check(recorded == actual,
              "CHANGELOG-2027.1.0-rc2.md.sha256 does not match the committed file")
    elif not os.path.isfile(sha_file):
        check(False, "missing checksum CHANGELOG-2027.1.0-rc2.md.sha256")

    print("=" * 60)
    if failures:
        for f in failures:
            print("FAIL %s" % f)
        print("EDITOR SPINE acceptance: %d failure(s)" % len(failures))
        return 1
    print("EDITOR SPINE acceptance: PASS (exit 0)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
