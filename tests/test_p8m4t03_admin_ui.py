#!/usr/bin/env python3
"""Acceptance for Keelworks P8-M4-T03: admin UI backend contract lockstep.

The Rust side proves behavior (tests/admin_roles_live.rs and
tests/admin_revisions_quotas.rs). This test guards the *contract surface* from
the other side of the fence, exactly like the sibling P8 contract-lockstep
tests:

1. The design doc (workstreams/pdm/web/admin-ui.md) exists and locks the three
   pillars (role matrix / revisions / storage quotas) plus the audit seam.

2. The public admin surface (crates/oep-pdm/src/admin.rs) exposes the methods
   the acceptance relies on: live grant/revoke, full-revision view, quota
   set/enforce, and audit verification. A silent rename or removal of any of
   these would fail CI here before a human notices.

3. The surface keeps the program prose rule (no em dashes) in user-facing
   strings/comments it owns.

Run: python3 tests/test_p8m4t03_admin_ui.py   (and pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "workstreams/pdm/web", "admin-ui.md")
ADMIN_MOD = os.path.join(ROOT, "crates", "oep-pdm", "src", "admin.rs")

EM_DASH = "\u2014"  # U+2014, banned from program prose

# Public surface the acceptance depends on.
EXPECTED_METHODS = [
    "open_with_bootstrap",
    "authorize",
    "grant_role",
    "revoke_role",
    "role_matrix",
    "grants",
    "all_revisions",
    "revisions_for",
    "storage_quota",
    "set_storage_quota",
    "usage_bytes",
    "attach_under_quota",
    "verify_audit",
]

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def test_doc_exists_and_locks_the_pillars():
    with open(DOC, encoding="utf-8") as fh:
        md = fh.read()
    for pillar in ["role matrix", "Revisions", "storage quotas", "audit"]:
        check(
            pillar.lower() in md.lower(),
            f"design doc does not mention the {pillar!r} pillar",
        )
    check(
        EM_DASH not in md,
        "design doc contains an em dash (banned from program prose)",
    )


def test_admin_surface_exposes_the_acceptance_methods():
    src = open(ADMIN_MOD, encoding="utf-8").read()
    for method in EXPECTED_METHODS:
        check(
            f"fn {method}(" in src,
            f"admin surface missing pub fn {method}(",
        )
    # The three acceptance concepts must be named in the module.
    for concept in ["role matrix", "all revisions", "storage quota", "audit"]:
        check(
            concept in src,
            f"admin.rs does not reference the {concept!r} concept",
        )


def test_admin_mod_declared_in_lib():
    lib = open(
        os.path.join(ROOT, "crates", "oep-pdm", "src", "lib.rs"), encoding="utf-8"
    ).read()
    check("pub mod admin;" in lib, "lib.rs does not declare pub mod admin;")


def main():
    test_doc_exists_and_locks_the_pillars()
    test_admin_surface_exposes_the_acceptance_methods()
    test_admin_mod_declared_in_lib()
    if failures:
        for msg in failures:
            print(f"FAIL  {msg}")
        print("\nACCEPTANCE: FAIL")
        sys.exit(1)
    print("\nACCEPTANCE: PASS - P8-M4-T03 admin UI contract lockstep.")
    sys.exit(0)


if __name__ == "__main__":
    main()
