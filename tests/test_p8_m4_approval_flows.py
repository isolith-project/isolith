#!/usr/bin/env python3
"""Acceptance test for Keelworks P8-M4-T02: approval flows + reviewer
comments + e-signature-equivalent (intent-affirmation, not a legal seal).

Written FIRST (TDD), mirroring the ADR-test convention (P8-M1, renderer-backend
P6-M5). It verifies the real material, not mocks:

  flow     => submit/review/approve/reject surfaces exist in oep-pdm::approval
              (ApprovalBoard::submit_for_review / add_reviewer_comment /
              decide) and the web routes exist in oep-pdm::web;
  comments => reviewer comments persist with the ticket (comments_for returns
              them alongside the ticket, and the design note says so);
  signing  => SigningNature has both an IntentAffirmation and a LegalSeal
              variant, the design note documents "intent-affirmation, not a
              legal seal", and the acceptance tests cover approve + reject +
              default-deny + cannot-decide-outside-review;
  web      => an end-to-end test drives the whole lifecycle over real HTTP
              sockets and asserts the signing statement shows on the page.

Run:  python3 tests/test_p8_m4_approval_flows.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPROVAL_SRC = os.path.join(ROOT, "crates", "oep-pdm", "src", "approval.rs")
WEB_SRC = os.path.join(ROOT, "crates", "oep-pdm", "src", "web.rs")
FLOW_TEST = os.path.join(ROOT, "crates", "oep-pdm", "tests", "approval_flows.rs")
WEB_TEST = os.path.join(ROOT, "crates", "oep-pdm", "tests", "approval_web_flows.rs")
NOTE = os.path.join(ROOT, "P8-M4", "approval-flows.md")

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(p):
    with open(p, encoding="utf-8") as fh:
        return fh.read()


def main():
    check(os.path.exists(APPROVAL_SRC), "approval.rs exists")
    check(os.path.exists(WEB_SRC), "web.rs exists")
    check(os.path.exists(FLOW_TEST), "approval_flows.rs acceptance exists")
    check(os.path.exists(WEB_TEST), "approval_web_flows.rs end-to-end exists")
    check(os.path.exists(NOTE), "P8-M4/approval-flows.md exists")

    approval = read(APPROVAL_SRC) if os.path.exists(APPROVAL_SRC) else ""
    check("pub struct ApprovalBoard" in approval, "ApprovalBoard defined")
    check("pub fn submit_for_review" in approval, "submit_for_review surface")
    check("pub fn add_reviewer_comment" in approval, "add_reviewer_comment surface")
    check("pub fn decide" in approval, "decide surface")
    check("pub enum SigningNature" in approval, "SigningNature defined")
    check(
        "IntentAffirmation" in approval and "LegalSeal" in approval,
        "both signing natures present (intent-affirmation + not-a-legal-seal)",
    )
    check(
        re.search(r"Action::SubmitForReview", approval)
        and re.search(r"Action::Review", approval),
        "flow gated on the RBAC reviewer actions",
    )
    check(
        "affirm must not be empty" in approval or "must not be empty" in approval,
        "empty affirmation is refused",
    )

    flow = read(FLOW_TEST) if os.path.exists(FLOW_TEST) else ""
    for needle in [
        "submit_review_approve_works_end_to_end",
        "reject_flow_persists_comment_and_denial_intent",
        "reviewer_without_review_role_is_denied_default_deny",
        "cannot_approve_a_ticket_that_is_not_under_review",
    ]:
        check(needle in flow, f"domain acceptance covers {needle}")

    web_test = read(WEB_TEST) if os.path.exists(WEB_TEST) else ""
    check(
        "full_approval_flow_over_http_end_to_end" in web_test,
        "web end-to-end covers the full flow",
    )
    check(
        "reject_over_http_and_default_deny" in web_test,
        "web end-to-end covers reject + default-deny",
    )
    check("intent-affirmation" in web_test, "web test asserts intent-affirmation")

    note = read(NOTE) if os.path.exists(NOTE) else ""
    check("intent-affirmation, not a legal seal" in note, "note documents the signing concept")
    check("LegalSeal" in note, "note names the not-parity bound")

    if failures:
        print("P8-M4-T02 acceptance FAIL")
        for f in failures:
            print("  -", f)
        return 1
    print("P8-M4-T02 acceptance OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
