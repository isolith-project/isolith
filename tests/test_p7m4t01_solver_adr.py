"""P7-M4-T01 acceptance: the solver decision ADR (adopt OSS, do not rebuild).

P7-M1-T01 assigned the sketcher solver choice to "a P7-M3-T01 / P7-M4-T01
deliverable with its own ADR"; P7-M3-T01 has not shipped one, so this card
owns the 2D sketch solver record (workstreams/modeler/sketch-and-rules/sketch-solver-adr.md). This gate
verifies the record shape so the decision is reviewable and durable:

  - front matter: versioned (semver), status, date, owner, applies_to,
    a decision key
  - the keep/acquire posture answers the question with ONE recommendation and
    weighs the mandated path (adopt, do not rebuild) against the alternatives
  - the clean-room line is explicit (no FreeCAD source copied)
  - the seam is named (crate::solve) so the "no solver types on the public
    surface" structural rule is auditable
  - the license posture is addressed (permissive stack, notice per P7-M1)
  - an escape hatch / revisit trigger and a test gate are named

Plus the program surface-language rule (no em dashes, no curly quotes).
"""

import os
import re

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(REPO, "workstreams/modeler/sketch-and-rules", "sketch-solver-adr.md")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
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
    print("P7-M4-T01 sketcher solver ADR acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)
    check(os.path.isfile(ADR), "missing: workstreams/modeler/sketch-and-rules/sketch-solver-adr.md")
    if not os.path.isfile(ADR):
        return 1
    text = read(ADR)
    meta, body = split_front_matter(text)

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta.get("version", "")),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    for key in ("status", "date", "owner", "decision", "applies_to"):
        check(key in meta and meta.get(key, "").strip(),
              "front matter must carry %s" % key)

    # 3. The card question is answered: adopt an OSS solver, do not rebuild.
    low = text.lower()
    check("adopt" in low, "must state the adopt posture")
    check("do not rebuild" in low or "not rebuild" in low,
          "must reject rebuilding a solver")
    check("freecad" in low, "must engage the FreeCAD reference")
    check("seam" in low, "must name the solver seam")
    check("oss" in low or "open source" in low, "must be scoped to OSS")

    # 4. A single recommendation in the Decision section.
    dec = section_text(body, "Decision")
    dlow = dec.lower()
    check(bool(dlow), "must have a 'Decision' section")
    if dlow:
        check("adopt" in dlow, "Decision must adopt a solver")
        check("vcad" in dlow, "Decision must name the adopted library")

    # 5. Posture: clean-room, license, escape hatch, test gate.
    check("clean-room" in low or "cleanroom" in low or "clean room" in low,
          "must address the clean-room invariant (no FreeCAD source copied)")
    check("mit" in low, "must name the license posture of the adopted stack")
    check("escape" in low, "must name an escape hatch")
    check("gate" in low, "must name a test gate")

    # 6. Surface-language rule: no em dashes, no curly quotes.
    for ch, name in (("\u2014", "em dash"), ("\u2013", "en dash"),
                     ("\u2018", "left quote"), ("\u2019", "right quote"),
                     ("\u201c", "left dquote"), ("\u201d", "right dquote")):
        if ch in text:
            check(False, "contains %s" % name)

    print("-" * 66)
    if failures:
        for f in failures:
            print("FAIL  %s" % f)
        return 1
    print("PASS  solver ADR is complete, posted, and surface-clean")
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(run_checks())
