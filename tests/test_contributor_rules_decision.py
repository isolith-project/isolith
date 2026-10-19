#!/usr/bin/env python3
"""Acceptance test for Keelworks P0-M1-T04: contributor rules (CLA, DCO, CODEOWNERS).

Written FIRST (TDD). The decision record at P0-M1/contributor-rules-decision.md
plus the root CODEOWNERS file must make this test pass. It verifies the real
material, not mocks:
  decision        => contributions are certified with the Developer Certificate
                     of Origin (DCO, a Signed-off-by line), NOT a standing CLA
  codeowners      => per-workstream ownership at the repo root, matching the
                     real Cargo workspace member list, enforced before merge
  enforcement     => branch protection + CI sign-off check are named so the
                     decision survives into P0-M4-T01
  handoff         => the record hands the CONTRIBUTING prose to P0-M1-T05 and
                     governance refinement to P13-T01, without doing it here
  provenance      => the DCO choice is tied to the clean-room invariant

Plus the program surface-language rule (no em dashes), matching the other P0
deliverables.

Run:  python3 tests/test_contributor_rules_decision.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECISION = os.path.join(ROOT, "P0-M1", "contributor-rules-decision.md")
CODEOWNERS = os.path.join(ROOT, "CODEOWNERS")
CARGO_TOML = os.path.join(ROOT, "Cargo.toml")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
OWNER = re.compile(r"^@[A-Za-z0-9][A-Za-z0-9_-]*(/[A-Za-z0-9][A-Za-z0-9_-]*)?$")
EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_dec() -> str:
    with open(DECISION, encoding="utf-8") as fh:
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


def section_text(body, heading):
    """Text under a '## [N.] Heading', before the next heading of equal/higher
    level. Tolerates numbered headings ('## 1. Decision')."""
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def workspace_members():
    """Real Cargo workspace members that claim a crate path (crates/<name>)."""
    with open(CARGO_TOML, "rb") as fh:
        data = tomllib.load(fh)
    members = (data.get("workspace") or {}).get("members") or []
    return [m for m in members if m.startswith("crates/")]


def codeowners_rules():
    """Return [(pattern, [owners])] from the root CODEOWNERS file.
    A rule is a non-comment, non-empty line; pattern is the first token,
    owners are the remaining tokens (patterns here carry no spaces).
    Regex escapes like '\\\\.' are not used in our file, so a plain split is
    faithful to the real material."""
    rules = []
    with open(CODEOWNERS, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            tokens = line.split()
            if len(tokens) < 2:
                check(False, "CODEOWNERS rule must carry an owner: %r" % line)
                continue
            rules.append((tokens[0], tokens[1:]))
    return rules


def run_checks():
    print("P0-M1-T04 contributor rules decision acceptance")
    print("document: %s" % DECISION)
    print("=" * 66)

    # 1. The decision record exists.
    check(os.path.isfile(DECISION), "missing: P0-M1/contributor-rules-decision.md")
    if not os.path.isfile(DECISION):
        return failures

    text = read_dec()
    meta, body = split_front_matter(text)
    low = text.lower()

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta["status"].strip(), "front matter must carry a status")
    check("date" in meta and meta["date"].strip(), "front matter must carry a date")
    check("owner" in meta and meta["owner"].strip(), "front matter must carry an owner")

    # 3. The DCO decision is stated: Developer Certificate of Origin, sign-off.
    check("dco" in low or "developer certificate of origin" in low,
          "must adopt the Developer Certificate of Origin (DCO)")
    check("signed-off-by" in low or "sign off" in low or "sign-off" in low,
          "must require a Signed-off-by (-s) sign-off line on commits")

    # 4. CLA is explicitly rejected as a standing requirement.
    check("cla" in low, "must address the CLA question explicitly")
    check("no standing cla" in low,
          "must state there is no standing CLA requirement")
    check("copyright assignment" in low or "assign" in low,
          "must clarify no copyright assignment is required")

    # 5. Why: the decision is tied to the license posture.
    check("apache-2.0" in low, "must reference the Apache-2.0 license class")
    check("agpl" in low, "must reference the AGPL-3.0 license class")
    check("charter" in low, "must reference the program charter")

    # 6. CODEOWNERS policy is decided in the record.
    co = section_text(body, "CODEOWNERS policy")
    col = co.lower()
    check(bool(col), "must have a CODEOWNERS policy section")
    if col:
        check("workstream" in col or "crate" in col,
              "CODEOWNERS policy must be scoped per workstream")
        check("review" in col or "approval" in col or "approve" in col,
              "CODEOWNERS policy must require owner review/approval")
        check("root" in col, "CODEOWNERS policy must point at the repo root file")

    # 7. The root CODEOWNERS file exists and mirrors the real workspace.
    check(os.path.isfile(CODEOWNERS), "missing: CODEOWNERS at repo root")
    if os.path.isfile(CODEOWNERS):
        rules = codeowners_rules()
        check(bool(rules), "CODEOWNERS must contain at least one rule")
        patterns = [r[0] for r in rules]
        owners = [o for _, os_ in rules for o in os_]
        # 7a. A catch-all/default rule must exist.
        check(any(p == ".*" for p in patterns),
              "CODEOWNERS must carry a catch-all '.*' default rule")
        # 7b. Every workstream crate from Cargo.toml gets an owner row.
        for member in workspace_members() or ["crates/oep-cgr"]:
            want = (member + "/.*") if member.endswith("/") else (member + "/.*")
            check(any(p == want for p in patterns),
                  "CODEOWNERS missing row for workspace member %s (want %r)"
                  % (member, want))
        # 7c. Governance directories are covered.
        for want in ("python/.*", "docs/.*", "tests/.*", "standards/.*"):
            check(any(p == want for p in patterns),
                  "CODEOWNERS missing governance row %r" % want)
        # 7d. All owner tokens are valid @user or @org/team handles.
        for o in owners:
            check(OWNER.match(o), "CODEOWNERS invalid owner token %r" % o)
        # 7e. No em dashes in the policy file's own prose/comments.
        nd = open(CODEOWNERS, encoding="utf-8").read().count(EM_DASH)
        check(nd == 0, "CODEOWNERS contains %d em dash(es)" % nd)

    # 8. Enforcement is named and concrete.
    enforce = section_text(body, "Enforcement")
    check(bool(enforce.strip()), "must have an enforcement section")
    if enforce.strip():
        el = enforce.lower()
        check("branch protection" in el or "protection" in el,
              "enforcement must name main branch protection")
        check("codeowner" in el.replace("code owner", "codeowner"),
              "enforcement must require codeowner approval before merge")
        check("ci" in el or "check" in el,
              "enforcement must name a CI/sign-off check")
    check("P0-M4-T01" in text, "must hand branch protection/CI to P0-M4-T01")

    # 9. Handoff is explicit, not absorbed.
    check("P0-M1-T05" in text, "must hand CONTRIBUTING prose to P0-M1-T05")
    check("P13-T01" in text, "must hand governance refinement to P13-T01")

    # 10. Provenance / clean-room tie-in (the program's hard invariant).
    check("clean-room" in low.replace("clean room", "clean-room"),
          "must tie DCO provenance to the clean-room invariant")
    check("provenance" in low or "original" in low,
          "must describe the per-commit provenance attestation")

    # 11. Surface language rule: no em dashes (program discipline, see P0-M2).
    ndashes = text.count(EM_DASH)
    check(ndashes == 0, "document contains %d em dash(es); ban applies to P0 prose too" % ndashes)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("CONTRIBUTOR RULES DECISION ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("CONTRIBUTOR RULES DECISION ACCEPTANCE PASSED: "
          "DCO not CLA, per-workstream CODEOWNERS, enforcement + handoff "
          "recorded on real material (workspace + CODEOWNERS file).")
    return 0


def test_contributor_rules_decision_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "contributor rules decision acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
