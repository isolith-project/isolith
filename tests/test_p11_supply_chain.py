#!/usr/bin/env python3
"""Acceptance test for Keelworks P11-T06: supply-chain hardening.

Written FIRST (TDD), mirroring the ADR/threat-model test convention. It
machine-checks committed real material, not mocks:

  pinned set      => Cargo.lock is committed and every external package is
                     verifiable (registry+checksum, or git pinned to an exact
                     rev); the rust-toolchain pin names a concrete channel
  reproducible   => scripts/supply-chain/reproducible-build.sh exists and is
                     executable; a recorded workstreams/security/proof/reproducible-build-*
                     proof exists with identical=true
  gate           => scripts/supply-chain/verify-lockpin.sh exists/executable;
                     .gitea/workflows/supply-chain.yaml references both and
                     gates reproducible-build to the release branch line
  doctrine doc   => workstreams/security/supply-chain.md has the mandated sections (pinned
                     dependency set, reproducible build, tolerated
                     nondeterminism list, signed artifacts seam, CI
                     enforcement) and its front matter claims this card
  language       => program surface-language rule: no em dashes, no curly
                     quotes in the doc

Run:  python3 tests/test_p11_supply_chain.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "workstreams/security", "supply-chain.md")
REPRO_SH = os.path.join(ROOT, "scripts", "supply-chain", "reproducible-build.sh")
LOCKPIN_SH = os.path.join(ROOT, "scripts", "supply-chain", "verify-lockpin.sh")
WORKFLOW = os.path.join(ROOT, ".gitea", "workflows", "supply-chain.yaml")
LOCKFILE = os.path.join(ROOT, "Cargo.lock")

EM_DASH = "\u2014"
EN_DASH = "\u2013"
CURLY = "\u2018\u2019\u201c\u201d"

failures = []
def check(cond, msg):
    if not cond:
        failures.append(msg)

def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()

def section(text, heading):
    m = re.search(r"\n##{1,3} (?:[\d.]+\.\s*)?%s" % re.escape(heading), text)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##{1,3} ", text[start:])
    return text[start:] if nxt is None else text[start:start + nxt.start()]


def run_checks():
    print("P11-T06 supply-chain acceptance")
    print("doc: %s" % DOC)
    print("=" * 66)

    # --- Doctrine doc -------------------------------------------------------
    check(os.path.isfile(DOC), "missing: workstreams/security/supply-chain.md")
    if os.path.isfile(DOC):
        text = read(DOC)
        check("P11-T06" in text, "doc does not claim P11-T06")
        for req, heading in [
            ("pinned set", "Pinned dependency set"),
            ("reproducible", "Reproducible build"),
            ("nondeterminism", "Tolerated nondeterminism list"),
            ("signed", "Signed artifacts"),
            ("ci enforcement", "CI enforcement"),
        ]:
            check(section(text, heading).strip() != "",
                  "doc missing section: %s" % req)
        for ch, name in ((EM_DASH, "em dash"), (EN_DASH, "en dash")):
            check(ch not in text, "doc contains a %s (program rule)" % name)
        for ch in CURLY:
            check(ch not in text,
                  "doc contains a curly quote (program rule)")

    # --- Pinned dependency set ------------------------------------------------
    check(os.path.isfile(LOCKFILE), "missing: Cargo.lock (must be committed)")
    if os.path.isfile(LOCKFILE):
        lock = read(LOCKFILE)
        pkgs = []
        cur = None
        for line in lock.splitlines():
            if line.startswith("[[package]]"):
                cur = {"source": None, "checksum": None}
                pkgs.append(cur)
            elif cur is not None:
                m = re.match(r'source = "([^"]+)"', line)
                if m: cur["source"] = m.group(1)
                m = re.match(r'checksum = "([0-9a-f]+)"', line)
                if m: cur["checksum"] = m.group(1)
        ext = [p for p in pkgs if p["source"]]
        unver = []
        for p in ext:
            if p["source"].startswith("registry+") and not p["checksum"]:
                unver.append(p["source"])
            if p["source"].startswith("git+"):
                frag = p["source"].split("#")[-1] if "#" in p["source"] else ""
                if len(frag) != 40 or not all(c in "0123456789abcdef"
                                              for c in frag):
                    unver.append(p["source"])
        check(not unver, "unverifiable external dep entries: %r" % unver[:5])
        check(len(ext) > 0,
              "no external deps found in Cargo.lock (unexpected)")

        r = subprocess.run(["git", "ls-files", "--error-unmatch", "Cargo.lock"],
                           cwd=ROOT, capture_output=True, text=True)
        check(r.returncode == 0, "Cargo.lock is not tracked by git")

        tcf = os.path.join(ROOT, "rust-toolchain.toml")
        check(os.path.isfile(tcf), "rust-toolchain.toml is missing")
        if os.path.isfile(tcf):
            pin = read(tcf)
            chan = ""
            m = re.search(r'channel\s*=\s*"([^"]+)"', pin) or \
                re.search(r"channel\s*=\s*([0-9.]+)", pin)
            if m:
                chan = m.group(1)
            check(bool(chan) and chan != "stable",
                  "rust-toolchain pin should be a concrete channel, got %r"
                  % chan)

    # --- Reproducible build ----------------------------------------------------
    check(os.path.isfile(REPRO_SH) and os.access(REPRO_SH, os.X_OK),
          "reproducible-build.sh missing or not executable")
    proof_dir = os.path.join(ROOT, "workstreams/security", "proof")
    proofs = []
    if os.path.isdir(proof_dir):
        proofs = [f for f in os.listdir(proof_dir)
                  if f.startswith("reproducible-build-") and f.endswith(".json")]
    check(len(proofs) >= 1,
          "no workstreams/security/proof/reproducible-build-*.json proof recorded")
    for pf in proofs:
        try:
            with open(os.path.join(proof_dir, pf), encoding="utf-8") as fh:
                data = json.load(fh)
            check(data.get("identical") is True,
                  "proof %s records identical != true" % pf)
            check(data.get("artifacts", 0) > 0,
                  "proof %s has zero artifacts" % pf)
            check(bool(data.get("sha256sums")),
                  "proof %s missing SHA256SUMS path" % pf)
        except Exception as e:  # noqa: BLE001
            check(False, "proof %s unreadable/invalid: %s" % (pf, e))

    # --- CI gate ---------------------------------------------------------------
    check(os.path.isfile(LOCKPIN_SH) and os.access(LOCKPIN_SH, os.X_OK),
          "verify-lockpin.sh missing or not executable")
    check(os.path.isfile(WORKFLOW),
          ".gitea/workflows/supply-chain.yaml is missing")
    if os.path.isfile(WORKFLOW):
        wf = read(WORKFLOW)
        check("verify-lockpin.sh" in wf,
              "workflow does not run verify-lockpin.sh")
        check("reproducible-build.sh" in wf,
              "workflow does not run reproducible-build.sh")
        check("release/" in wf or "base_ref" in wf,
              "workflow does not gate reproducible-build to release branch")
        check("workflow_dispatch" in wf,
              "workflow lacks workflow_dispatch (on-demand repro)")

    print("proofs recorded: %s" % (", ".join(proofs) or "(none)"))
    return failures


if __name__ == "__main__":
    fails = run_checks()
    if fails:
        print("P11-T06 supply-chain ACCEPTANCE: FAIL")
        for f in fails:
            print("  - " + f)
        sys.exit(1)
    print("P11-T06 supply-chain ACCEPTANCE: PASS")
    sys.exit(0)


def test_p11_supply_chain_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    fails = run_checks()
    assert not fails, "P11-T06 supply-chain acceptance failed (%d): %s" % (
        len(fails), fails[:5])
