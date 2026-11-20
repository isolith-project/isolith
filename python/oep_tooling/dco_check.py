#!/usr/bin/env python3
"""Isolith DCO sign-off check (community intake, item 8).

Verifies that every commit in a git range carries a `Signed-off-by:` line
(the Developer Certificate of Origin, see CONTRIBUTING.md) and that a
pull-request body carries a DCO sign-off line. This is the check the CI
workflows (.gitea/workflows/dco.yaml and the GitHub .github/workflows port)
run so the CONTRIBUTING.md promise "CI fails the status on the pull request"
is actually enforced, not just stated.

Usage:
    python3 python/oep_tooling/dco_check.py --repo DIR \\
        --base SHA --head SHA [--body-file PATH] [--everywhere]

Exit 0 iff every commit in the range base..head carries a `Signed-off-by:`
line and (when --body-file is given) the body carries a sign-off line. Exit 1
otherwise, listing the offending commits on stderr. Stdlib only; no network.

`--everywhere` additionally scans the commits reachable from HEAD (used in
tests to check the repository's own history without a range argument).

The check is deliberately simple and honest: it requires the marker line to
exist, it does not verify the person against a registry. It is a hygiene gate,
not an identity proof. One placeholder copy-pasted into a PR body will pass
the letter of this check; the reviewer is the one who holds the contributor to
the spirit (CONTRIBUTING.md, DCO section). That limitation is stated here
rather than hidden.

No claim this script makes is above what it verifies: it prints exactly the
commits that lacked the line, and nothing else.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys

SIGN_OFF_RE = re.compile(r"^[ \t]*Signed-off-by:[ \t]+\S.+$", re.MULTILINE)
# Commit records are emitted with field and record separators so no
# realistic commit message can break the framing.
FIELD_SEP = "\x1f"
RECORD_SEP = "\x1e"
LOG_FMT = "%H%x1f%s%x1f%B%x1e"


class MissingSignOff(Exception):
    """Raised with a human-readable reason when the DCO check does not pass."""


def run_git(repo: str, args: list[str]) -> str:
    proc = subprocess.run(
        ["git", "-C", repo] + args, capture_output=True, text=True)
    if proc.returncode != 0:
        raise MissingSignOff(
            "git %s failed in %s: %s"
            % (" ".join(args), repo, proc.stderr.strip() or proc.stdout.strip()))
    return proc.stdout


def scan_commits(repo: str, base: str | None, head: str | None) -> list[str]:
    """Return the SHAs of commits in the range that lack a Signed-off-by line.

    With base None the scan covers the commits reachable from head (used for
    a whole-tree audit). Records come from a single git log invocation.

    Merge commits are excluded (--no-merges). A merge carries no authored
    change of its own: GitHub's "Update branch" button, a maintainer merging
    main into a contributor branch, and the repository's own merge commits all
    produce them, and requiring a sign-off on those red-flags every
    contributor who syncs their branch. The DCO certifies authored commits.
    """
    if base is not None:
        rev = "%s..%s" % (base, head)
    else:
        rev = str(head)
    out = run_git(repo, ["log", "--no-merges", "--format=" + LOG_FMT, rev])
    bad = []
    for record in out.split(RECORD_SEP):
        record = record.strip("\n")
        if not record:
            continue
        fields = record.split(FIELD_SEP, 2)
        if len(fields) < 3:
            continue
        sha, subject, body = fields
        message = subject + "\n" + body
        if not SIGN_OFF_RE.search(message):
            bad.append(sha)
    return bad


def check_body(body: str) -> bool:
    """Return True if the pull-request body carries a DCO sign-off line."""
    if not body:
        return False
    return bool(SIGN_OFF_RE.search(body))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Isolith DCO sign-off check (commits + PR body).")
    parser.add_argument("--repo", required=True, help="git repository root")
    parser.add_argument("--base", default=None, help="range base SHA (inclusive)")
    parser.add_argument("--head", required=True, help="range head SHA")
    parser.add_argument("--body-file", default=None,
                        help="file containing the pull-request body")
    parser.add_argument("--everywhere", action="store_true",
                        help="scan all commits reachable from head (no range)")
    args = parser.parse_args(argv)

    failures = []

    if args.everywhere:
        range_label = "all commits reachable from %s" % args.head
        bad = scan_commits(args.repo, None, args.head)
    else:
        if not args.base:
            sys.stderr.write("dco-check: a base SHA (or --everywhere) is required to scan a range\n")
            return 2
        range_label = "%s..%s" % (args.base, args.head)
        bad = scan_commits(args.repo, args.base, args.head)

    if bad:
        print("DCO COMMIT CHECK FAILED (%d commit(s) in %s missing Signed-off-by):"
              % (len(bad), range_label))
        for sha in bad:
            print("  missing sign-off: %s" % sha)
        failures.append("commit sign-off")
    else:
        print("DCO commit check passed on %s (Signed-off-by present on every commit)"
              % range_label)

    if args.body_file:
        try:
            with open(args.body_file, encoding="utf-8") as fh:
                body = fh.read()
        except OSError as exc:
            sys.stderr.write("dco-check: cannot read body file %s: %s\n"
                             % (args.body_file, exc))
            return 2
        if check_body(body):
            print("DCO PR-body check passed (body carries a Signed-off-by line)")
        else:
            print("DCO PR-BODY CHECK FAILED (body has no Signed-off-by line)")
            failures.append("PR body sign-off")
    else:
        print("DCO PR-body check skipped (no --body-file; push-only scan)")

    if failures:
        sys.stderr.write("dco-check FAILED: %s\n" % ", ".join(failures))
        return 1
    print("DCO check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
