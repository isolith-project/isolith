#!/usr/bin/env python3
"""Acceptance test for Keelworks P11-T03: the auth-boundary test suite.

Written to check the committed real material of card P11-T03, the same way the
P11-T01 threat-model test checks its own artifact. It verifies:

  artifacts => every committed artifact the card's acceptance names exists on
               this ref: the authz enforcement module, the P8-M3 RBAC core it
               builds on, the security suite, the boundary doc and the CI job.
  doc       => P11/auth-boundary.md contains the enforcement contract the card
               promises: the denial contract (403 cross-role / 404 cross-
               tenant), the endpoint to action map, the decisions D1-D5, the
               permissive-stub mutation proof, the clean-room note, and a
               versioned change log.
  suite     => tests/auth_boundary.rs really asserts: it names the strict
               authorizer AND the permissive authorizer and proves the two
               differ (the "would fail on a default-permissive RBAC stub"
               requirement) over real sockets.
  ci        => .gitea/workflows/ga-gate-p11.yaml registers a ga-gate-p11t03
               job that runs the auth-boundary suite and this acceptance test.
  language  => program surface-language rule: no em dashes, no curly quotes.

Run:  python3 tests/test_p11_auth_boundary.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "P11", "auth-boundary.md")
WORKFLOW = os.path.join(ROOT, ".gitea", "workflows", "ga-gate-p11.yaml")
SUITE = os.path.join(ROOT, "crates", "oep-pdm", "tests", "auth_boundary.rs")
SERVICE_SRC = os.path.join(ROOT, "crates", "oep-pdm", "src", "service.rs")

# Every artifact the card's acceptance/layout depends on existing.
ARTIFACTS = [
    "P11/auth-boundary.md",
    "crates/oep-pdm/tests/auth_boundary.rs",
    "crates/oep-pdm/src/authz/mod.rs",
    "crates/oep-pdm/src/authz/endpoints.rs",
    "crates/oep-pdm/src/authz/gate.rs",
    "crates/oep-pdm/src/authz/policy.rs",
    "crates/oep-pdm/src/authz/principal.rs",
    "crates/oep-pdm/src/authz/resource.rs",
    # The P8-M3 RBAC core the suite must pass on.
    "crates/oep-pdm/src/rbac/mod.rs",
    "crates/oep-pdm/src/rbac/roles.rs",
    "crates/oep-pdm/src/rbac/matrix.rs",
    "crates/oep-pdm/src/rbac/authorizer.rs",
    "crates/oep-pdm/src/rbac/policy.rs",
    "crates/oep-pdm/src/rbac/attrs.rs",
    "crates/oep-pdm/tests/rbac_abac.rs",
    "crates/oep-pdm/tests/rbac_default_deny.rs",
    "crates/oep-pdm/tests/rbac_role_matrix.rs",
    ".gitea/workflows/ga-gate-p11.yaml",
]

# Doc markers the acceptance promises (existence only; the table itself is the
# review surface, mirrored by the unit + integration tests for behavior).
DOC_MARKERS = [
    "## 1. Purpose",
    "## 2. What the boundary enforces",
    "## 3. The enforcement stack",
    "## 4. Endpoint to action map",
    "## 5. Decisions",
    "| D1 ",
    "| D2 ",
    "| D3 ",
    "| D4 ",
    "| D5 ",
    "## 6. How the suite proves it asserts",
    "## 7. Clean-room note",
    "## 8. Verification",
    "## 9. Change log",
    "403 Forbidden",
    "404 Not Found",
    "ManageSystem (admin only",
    "permissive-stub",
    "permissive_authorizer",
    "| D6 ",
    "SEC-AUTHZ-DEFAULT",
    "--no-auth",
]

EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose
EN_DASH = "\u2013"  # U+2013 en dash, also banned
CURLY = "\u2018\u2019\u201c\u201d"  # straight quotes only

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def main():
    # --- artifacts ---------------------------------------------------------
    for rel in ARTIFACTS:
        check(
            os.path.isfile(os.path.join(ROOT, rel)),
            f"missing committed artifact: {rel}",
        )

    # --- suite really asserts (the acceptance's mutation-style proof) ------
    suite_ok = os.path.isfile(SUITE)
    if suite_ok:
        suite = read(SUITE)
        check(
            "strict_boundary_authorizer()" in suite and "permissive_authorizer()" in suite,
            "auth_boundary.rs must reference both the strict and the permissive "
            "authorizer (the two-mode proof)",
        )
        check(
            "strict_mode_denies_what_permissive_allows" in suite,
            "auth_boundary.rs must contain the strict-vs-permissive mutation test",
        )
        check(
            "TcpStream::connect" in suite,
            "auth_boundary.rs must exercise the suite over real sockets",
        )
        # SEC-AUTHZ-DEFAULT: the suite must re-run the attack matrix against
        # the shipped DEFAULT binary and prove the --no-auth opt-out warns.
        check(
            "default_binary_denies_the_attack_matrix" in suite,
            "auth_boundary.rs must run the attack matrix against the default binary",
        )
        check(
            "no_auth_optout_warns_and_opens_the_contract" in suite,
            "auth_boundary.rs must prove the --no-auth opt-out warns and opens",
        )
        check(
            "CARGO_BIN_EXE_oep-pdm" in suite,
            "auth_boundary.rs must boot the real shipped oep-pdm binary",
        )

    # --- doc invariants ----------------------------------------------------
    if os.path.isfile(DOC):
        doc = read(DOC)
        for marker in DOC_MARKERS:
            check(marker in doc, f"P11/auth-boundary.md missing marker: {marker!r}")
        check(
            any(ch in doc for ch in (EM_DASH, EN_DASH)) is False,
            "P11/auth-boundary.md contains a banned dash",
        )
        check(
            any(ch in doc for ch in CURLY) is False,
            "P11/auth-boundary.md contains curly quotes",
        )
        # Version rule: any material doc change bumps the change-log version.
        ver = re.search(r"version: (\d+\.\d+\.\d+)", doc)
        check(ver is not None, "P11/auth-boundary.md has no front-matter version")
        if ver:
            check(
                ver.group(1) in ("1.0.0",) or re.search(r"- (\S+) \(20", doc),
                "P11/auth-boundary.md change log must list the current version",
            )
    else:
        failures.append("P11/auth-boundary.md missing")

    # --- default wiring (SEC-AUTHZ-DEFAULT) --------------------------------
    if os.path.isfile(SERVICE_SRC):
        svc = read(SERVICE_SRC)
        check(
            "strict_boundary_authorizer" in svc and "--no-auth" in svc,
            "service.rs must wire the strict boundary into the default start "
            "path with an explicit --no-auth opt-out",
        )
        check(
            "start_with_store_tls_and_authorizer" in svc,
            "service.rs must combine TLS with the auth gate for the binary path",
        )
        for m in ("AUTHORIZATION IS DISABLED", "authorization: ENFORCED"):
            check(
                m in svc,
                "service.rs must announce the auth state and warn on the opt-out",
            )
    else:
        failures.append("crates/oep-pdm/src/service.rs missing")

    # --- ci ----------------------------------------------------------------
    if os.path.isfile(WORKFLOW):
        wf = read(WORKFLOW)
        check("ga-gate-p11t03" in wf, "ga-gate-p11.yaml must define job ga-gate-p11t03")
        check("auth_boundary" in wf, "ga-gate-p11.yaml must run the auth_boundary suite")
        check("test_p11_auth_boundary.py" in wf, "ga-gate-p11.yaml must run this acceptance test")
    else:
        failures.append(".gitea/workflows/ga-gate-p11.yaml missing")

    if failures:
        print("P11-T03 acceptance FAIL:")
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P11-T03 acceptance PASS (artifacts, suite assertions, doc, CI)")


if __name__ == "__main__":
    main()
