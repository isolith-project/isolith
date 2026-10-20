#!/usr/bin/env python3
"""Acceptance test for Isolith P11-T01: the stack threat model.

Written FIRST (TDD), mirroring the ADR-test convention. The document at
workstreams/security/threat-model.md must make this test pass. It verifies the committed real
material, not mocks:

  sections  => the card's acceptance list: assets, trust boundaries, threat
               actors, and mitigations mapped to the stack must each be a
               documented section
  risk map  => the risk register (workstreams/security/threat-model.md section 6) and the
               mitigation inventory (section 7) form THE mapping table; the
               table must have NO ORPHAN RISKS:
                 1. every risk names at least one mitigation, and every named
                    mitigation id exists in the inventory (no orphan risk, no
                    dangling reference)
                 2. every inventory entry is named by at least one risk (no
                    orphan mitigation)
                 3. every COMPONENT entry points at a path that exists in
                    this repository right now (no invented component)
                 4. every CARD entry is one of the eight committed P11 cards
                    (P11-T01..T08, per the program master plan P11 section)
  language  => program surface-language rule: no em dashes, no curly quotes

Run:  python3 tests/test_p11_threat_model.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "workstreams/security", "threat-model.md")

# The eight committed Wave-3 P11 cards (master plan P11 section).
P11_CARDS = {
    "P11-T01", "P11-T02", "P11-T03", "P11-T04",
    "P11-T05", "P11-T06", "P11-T07", "P11-T08",
}

RISK_ID = re.compile(r"^\| (?:NW|W|CL|AG|UC|SU|OP|DI)-\d+ \|")
MIT_ID = re.compile(r"(M-[A-Z0-9]+)")
CARD_REF = re.compile(r"P11-T0[1-8]")

EM_DASH = "\u2014"      # U+2014, the em dash the program bans from its prose
EN_DASH = "\u2013"      # U+2013 en dash, also banned by the ADR precedent
CURLY = "\u2018\u2019\u201c\u201d"  # straight quotes only

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def section(text, heading):
    """Body under a '## N.' style heading, up to the next '## ' heading."""
    m = re.search(r"\n## (?:[\d.]+\.\s*)?%s" % re.escape(heading), text)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n## ", text[start:])
    return text[start:] if nxt is None else text[start:start + nxt.start()]


def risk_rows(body):
    rows = []
    for line in body.splitlines():
        m = RISK_ID.match(line)
        if not m:
            continue
        rid = m.group(0).strip().strip("|").strip()
        rows.append((rid, MIT_ID.findall(line)))
    return rows


def mitigation_rows(body):
    rows = []
    for line in body.splitlines():
        if not line.startswith("| M-"):
            continue
        cols = [c.strip() for c in line.split("|")][1:-1]
        if len(cols) < 5:
            continue
        mid, name, kind, closes, why = cols[:5]
        rows.append((mid, kind, closes))
    return rows


def run_checks():
    print("P11-T01 threat model acceptance")
    print("doc: %s" % DOC)
    print("=" * 66)

    # 0. The document exists.
    check(os.path.isfile(DOC), "missing: workstreams/security/threat-model.md")
    if not os.path.isfile(DOC):
        return failures

    text = read(DOC)

    # 1. Required sections (the card's acceptance list).
    for heading in ("Assets", "Trust boundaries", "Threat actors", "System of interest"):
        body = section(text, heading)
        check(body.strip() != "", "missing required section: %s" % heading)
    mit_body = section(text, "Mitigation inventory")
    check(mit_body.strip() != "", "missing required section: Mitigation inventory")
    reg_body = section(text, "Risk register")
    check(reg_body.strip() != "", "missing required section: Risk register")

    # Separate the mitigation inventory from the accepted/residual prose that
    # follows it inside the same section: stop at the table terminator line
    # that ends the inventory's last row is hard to scope by heading, so pull
    # only the inventory's pipe rows (| M- ...).
    inv = mitigation_rows(mit_body)
    inv_by_id = {mid: (kind, closes) for mid, kind, closes in inv}
    check(len(inv) >= 15, "mitigation inventory too small: %d entries" % len(inv))

    risks = risk_rows(reg_body)
    check(len(risks) >= 15, "risk register too small: %d risks" % len(risks))

    # 2.1 Every risk names >=1 mitigation, and every ref exists (no orphans).
    risk_mit_pairs = []
    for rid, mids in risks:
        check(len(mids) >= 1, "risk %s has no mitigation (orphan risk)" % rid)
        for m in mids:
            if m not in inv_by_id:
                check(False, "risk %s references unknown mitigation %s" % (rid, m))
            else:
                risk_mit_pairs.append((rid, m))

    # 2.2 Every mitigation entry is named by at least one risk (no orphan
    #     mitigation), and kind/closes are valid.
    referenced = set()
    for rid, m in risk_mit_pairs:
        referenced.add(m)
    for mid, (kind, closes) in sorted(inv_by_id.items()):
        check(kind in ("COMPONENT", "CARD"),
              "mitigation %s has unknown kind %r" % (mid, kind))
        if kind == "COMPONENT":
            toks = [t.strip() for t in closes.split(",") if t.strip()]
            check(len(toks) >= 1, "mitigation %s has no component path" % mid)
            for tok in toks:
                p = os.path.join(ROOT, tok)
                check(os.path.exists(p),
                      "mitigation %s references missing component %s" % (mid, tok))
        elif kind == "CARD":
            refs = CARD_REF.findall(closes)
            check(len(refs) == 1,
                  "mitigation %s must reference exactly one P11 card, got %r"
                  % (mid, refs))
            check(refs and refs[0] in P11_CARDS,
                  "mitigation %s references non-committed card %r" % (mid, refs))
        check(mid in referenced,
              "mitigation %s closes nothing (orphan mitigation)" % mid)

    # 3. Surface-language rule: no em dash, no en dash, no curly quotes.
    for ch, name in ((EM_DASH, "em dash"), (EN_DASH, "en dash")):
        check(ch not in text, "document contains a %s (program rule)" % name)
    for ch in CURLY:
        check(ch not in text, "document contains a curly quote (program rule)")

    print("risks: %d, mitigations: %d, risk->mitigation links: %d"
          % (len(risks), len(inv), len(risk_mit_pairs)))
    return failures


if __name__ == "__main__":
    fails = run_checks()
    if fails:
        print("FAIL")
        for f in fails:
            print("  - " + f)
        sys.exit(1)
    print("PASS: risk register and mitigation inventory have no orphan risks "
          "and every closure resolves to a real component or a P11-T* card.")
    sys.exit(0)


def test_p11_threat_model_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    fails = run_checks()
    assert not fails, "P11-T01 threat model acceptance failed (%d): %s" % (
        len(fails), fails[:5])
