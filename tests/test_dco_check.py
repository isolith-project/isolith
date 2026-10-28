#!/usr/bin/env python3
"""Acceptance test for the Isolith DCO sign-off check (community intake, item 8).

Written FIRST (TDD). The module python/oep_tooling/dco_check.py plus the CI
workflow that runs it (.gitea/workflows/dco.yaml) plus the public-surface port
(.github/workflows/dco-check.yaml) must make these checks pass on real
material, not mocks:
  commits   => every commit in a git range must carry a Signed-off-by line;
               unsigned commits are reported by SHA and fail the check
  body      => a pull-request body must carry a DCO sign-off line
  wiring    => the live CI workflow (Gitea Actions) and the GitHub port both
               invoke the module on the PR base..head range and the PR body,
               so the CONTRIBUTING.md promise ("CI fails the status on the
               pull request") is enforced, not just documented

Hermetic: builds a throwaway git repo under the system temp dir; stdlib only.

Run:  python3 tests/test_dco_check.py   (and under pytest)
Exit 0 = pass; nonzero = fail.
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODULE = os.path.join(ROOT, "python", "oep_tooling", "dco_check.py")
GITEA_WF = os.path.join(ROOT, ".gitea", "workflows", "dco.yaml")
GH_WF = os.path.join(ROOT, ".github", "workflows", "dco-check.yaml")

failures = []


def check(cond: bool, msg: str) -> None:
    if not cond:
        failures.append(msg)


def sh(repo: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", repo, *args], capture_output=True, text=True)


def make_commit(repo: str, path: str, content: str, message: str,
                sign: bool) -> str:
    os.makedirs(os.path.dirname(os.path.join(repo, path)), exist_ok=True)
    with open(os.path.join(repo, path), "w", encoding="utf-8") as fh:
        fh.write(content)
    sh(repo, "add", path)
    cmd = ["git", "-C", repo, "-c", "user.name=Test User",
           "-c", "user.email=test@example.invalid", "commit", "-m", message,
           "-s" if sign else "--no-signoff"]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError("fixture commit failed: %s" % proc.stderr)
    return sh(repo, "rev-parse", "HEAD").stdout.strip()


def run_check(argv: list[str], repo: str) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["PYTHONPATH"] = os.path.join(ROOT, "python") + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run([sys.executable, MODULE, "--repo", repo, *argv],
                          capture_output=True, text=True, env=env)


def test_unsigned_commit_fails_and_signed_row_passes() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        sh(tmp, "init", "-q", "-b", "main")
        sh(tmp, "-c", "user.name=Test User", "-c", "user.email=test@example.invalid",
           "commit", "-q", "--allow-empty", "-m", "root", "-s")
        base = sh(tmp, "rev-parse", "HEAD").stdout.strip()
        # Signed commit must pass.
        signed = make_commit(tmp, "signed.txt", "signed", "add signed file", sign=True)
        # Unsigned commit must fail.
        unsigned = make_commit(
            tmp, "unsigned.txt", "unsigned",
            "add unsigned file without a sign-off line", sign=False)
        ok = run_check(["--base", base, "--head", unsigned], tmp)
        check(ok.returncode != 0,
              "range ending on an unsigned commit must fail, got rc=%d"
              % ok.returncode)
        check(unsigned in ok.stdout,
              "the unsigned SHA must be reported as missing sign-off")
        # A signed range is the baseline for the body checks.
        # A body with a sign-off passes; a body without one fails.
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as bf:
            bf.write("title\n\nSigned-off-by: Test User <test@example.invalid>\n")
            bf.flush()
            body_ok = run_check(["--base", base, "--head", signed,
                                 "--body-file", bf.name], tmp)
            os.unlink(bf.name)
        check(body_ok.returncode == 0,
              "a PR body with a Signed-off-by line must pass, rc=%d, stdout=%s"
              % (body_ok.returncode, body_ok.stdout.strip()))
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as bf:
            bf.write("title\n\nno sign-off here\n")
            bf.flush()
            body_bad = run_check(["--base", base, "--head", signed,
                                  "--body-file", bf.name], tmp)
            os.unlink(bf.name)
        check(body_bad.returncode != 0,
              "a PR body without a sign-off must fail")
        check("PR-BODY CHECK FAILED" in body_bad.stdout,
              "the failing body must be reported as a body failure")
        # A fully-signed range with no body file passes.
        range_ok = run_check(["--base", base, "--head", signed], tmp)
        check(range_ok.returncode == 0,
              "a fully-signed range must pass, rc=%d, stderr=%s"
              % (range_ok.returncode, range_ok.stderr.strip()))


def test_workflows_wire_the_module() -> None:
    for wf, port in ((GITEA_WF, "live Gitea Actions"), (GH_WF, "GitHub port")):
        check(os.path.isfile(wf), "missing %s workflow: %s" % (port, wf))
        if os.path.isfile(wf):
            text = open(wf, encoding="utf-8").read()
            check("oep_tooling/dco_check.py" in text,
                  "%s must invoke the DCO check module by path" % port)
            check("github.event.pull_request.base.sha" in text and
                  "github.event.pull_request.head.sha" in text,
                  "%s must scan the PR base..head commit range" % port)
            check("pull_request.body" in text,
                  "%s must pass the PR body to the check" % port)


def main() -> int:
    print("DCO sign-off check acceptance (community intake item 8)")
    print("module: %s" % MODULE)
    print("=" * 60)
    test_unsigned_commit_fails_and_signed_row_passes()
    test_workflows_wire_the_module()
    print("=" * 60)
    if failures:
        print("DCO CHECK ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        return 1
    print("DCO CHECK ACCEPTANCE PASSED: commits and PR bodies must carry a "
          "Signed-off-by line, enforcement wired into the live CI workflow "
          "and its public-surface port on real material.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
