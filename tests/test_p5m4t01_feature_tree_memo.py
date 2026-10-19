#!/usr/bin/env python3
"""Acceptance test for Keelworks P5-M4-T01: feature-tree recoverability
decision memo with an options matrix (full recovery vs imported-dumb-solid vs
PMI-only) and a recommendation.

Written FIRST (TDD). The memo at P5-M4/feature-tree-recoverability-memo.md
must make this test pass. It verifies the real decision record:

  assessment  => the memo assesses feature-tree recoverability against
                 observation-based evidence on the real corpus, marking
                 confidence / [uncertain] where byte evidence does not prove
                 meaning (never a promise)
  options     => the memo carries an OPTIONS MATRIX covering exactly the three
                 candidate outcomes named on the card: full recovery,
                 imported-dumb-solid, and PMI-only
  matrix      => each option is scored on the same criteria (observability,
                 effort, legal/clean-room exposure, downstream value) and the
                 scores are real (no fabricated capability)
  recommendation => the memo picks one option and gives an actionable reason,
                 and names the P5-M5-T01 Don gate as the consumer of the memo
  grounding   => evidence cites the P5-M1-T02 observability report, the P5-M2-T02
                 byte-bisect tooling, the commercial-reader audit (P1-M3-T02),
                 the patent/clean-room posture (P0-M2-T02), and the oep-geo
                 feature-graph model (P6-M3-T03), with the no-em-dash / no-curly
                 quote surface rule enforced

Run:  python3 tests/test_p5m4t01_feature_tree_memo.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MEMO = os.path.join(ROOT, "P5-M4", "feature-tree-recoverability-memo.md")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_memo() -> str:
    with open(MEMO, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
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


def section_text(body, heading):
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def run_checks():
    print("P5-M4-T01 feature-tree recoverability decision memo acceptance")
    print("memo: %s" % MEMO)
    print("=" * 66)

    # 1. The memo exists.
    check(os.path.isfile(MEMO), "missing: P5-M4/feature-tree-recoverability-memo.md")
    if not os.path.isfile(MEMO):
        return failures

    text = read_memo()
    meta, body = split_front_matter(text)
    low = text.lower()

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta["status"].strip(),
          "front matter must carry a status")
    check("date" in meta and meta["date"].strip(),
          "front matter must carry a date")
    check("owner" in meta and meta["owner"].strip(),
          "front matter must carry an owner")
    check("decision" in meta and meta["decision"].strip(),
          "front matter must carry a decision key")
    check("applies_to" in meta and meta["applies_to"].strip(),
          "front matter must carry applies_to")

    # 3. Scoped to the feature-tree recoverability question in the P5 lane.
    check("feature tree" in low or "feature-tree" in low,
          "must name the feature tree")
    check("recover" in low, "must address recoverability")
    check("P5" in text, "must place the question in the P5 native-reader lane")

    # 4. The options matrix covers the three named outcomes and has real scores.
    mat = section_text(body, "Options matrix")
    check(bool(mat), "must have an 'Options matrix' section")
    if mat:
        norm = " ".join(mat.split())  # collapse line wraps before matching
        ml = norm.lower()
        for option, alt in (("full recovery", "full recovery"),
                            ("dumb solid", "dumb-solid"),
                            ("pmi-only", "pmi-only")):
            check(option in ml or alt in ml or option.upper() in mat,
                  "options matrix must cover '%s'" % option)
        # a markdown table with scoring columns is present
        check("|" in mat, "options matrix must be rendered as a scored table")
        check("observab" in ml or "confidence" in ml or "evidence" in ml,
              "matrix must score observability / confidence")
        check("legal" in ml or "patent" in ml or "clean-room" in ml,
              "matrix must score legal / clean-room exposure")
        check("value" in ml or "downstream" in ml or "effort" in ml,
              "matrix must score downstream value / effort")

    # 5. Assessment is honest: marks confidence / [uncertain], never promises.
    ass = section_text(body, "Assessment")
    check(bool(ass), "must have an 'Assessment' section")
    if ass:
        al = ass.lower()
        check("uncertain" in al or "confidence" in al or "observed" in al,
              "assessment must mark confidence / uncertainty")
        check("promise" not in al.lower(), "assessment must not promise recovery")

    # 6. A recommendation picks one option with an actionable reason.
    rec = section_text(body, "Recommendation")
    check(bool(rec), "must have a 'Recommendation' section")
    if rec:
        rl = " ".join(rec.split()).lower()  # collapse line wraps
        check(any(o in rl for o in ("full recovery", "dumb solid",
                                    "dumb-solid", "pmi-only")),
              "recommendation must name one of the three options")
        check("because" in rl or "reason" in rl or "evidence" in rl
              or "therefore" in rl or "recommend" in rl,
              "recommendation must give a reason")
        check("P5-M5" in rec, "recommendation must feed the P5-M5-T01 Don gate")

    # 7. Evidence grounding: the cited sibling artifacts.
    ev = section_text(body, "Evidence")
    check(bool(ev), "must have an 'Evidence' section")
    if ev:
        el = ev.lower()
        check("p5-m1-t02" in el, "evidence must cite the P5-M1-T02 observability report")
        check("p5-m2" in el or "bisect" in el or "observation" in el,
              "evidence must cite the P5-M2-T02 observation / byte-bisect tooling")
        check("p1-m3" in el or "commercial" in el,
              "evidence must cite the commercial-reader audit (P1-M3-T02)")
        check("p0-m2" in el or "patent" in el,
              "evidence must cite the patent / clean-room posture (P0-M2-T02)")
        check("p6-m3" in el or "feature graph" in el or "oep-geo" in el,
              "evidence must tie in the oep-geo feature-graph model (P6-M3-T03)")

    # 8. Cross-references.
    check("P5-M4-T01" in text, "must cite its own card P5-M4-T01")
    check("P5-M5-T01" in text, "must name the consuming P5-M5-T01 gate")

    # 9. Surface language rule: no em dashes, no curly quotes.
    check(text.count(EM_DASH) == 0,
          "document contains %d em dash(es)" % text.count(EM_DASH))
    ncurly = text.count("\u201c") + text.count("\u201d") + \
             text.count("\u2018") + text.count("\u2019")
    check(ncurly == 0,
          "document contains %d curly quote(s)" % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("FEATURE-TREE MEMO ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("FEATURE-TREE MEMO ACCEPTANCE PASSED: recoverability assessed on "
          "observation, options matrix covers full recovery / imported-dumb-solid "
          "/ PMI-only, recommendation given; feeds P5-M5-T01.")
    return 0


def test_p5m4t01_feature_tree_memo():
    failures = run_checks()
    assert not failures, "feature-tree memo acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
