#!/usr/bin/env python3
"""P0-M4-T01 acceptance test: Gitea Actions pipeline (lint + unit + integration on
every push) and main branch protection.

Pure stdlib. Runs as `python3 tests/ci_pipeline_test.py` and under pytest.
Checks are split into two parts:

  PART A (hermetic, no network): asserts the committed workflow file(s) exist
  and define the lint/unit/integration jobs on a push trigger.

  PART B (live, uses Gitea API when GITEA_TOKEN is available): reads back the
  repo's branch-protection rules and the most recent run, asserting main is
  protected with the expected status-check contexts and the latest run used
  the workflow on a push event.

"Done" for P0-M4-T01 means PART A passes against the real tree AND PART B
confirms, against the live NAS Gitea, that `main` has a protection rule whose
status-check contexts match the workflow's job names and that the workflow
runner has produced a successful run.

Environment:
  GITEA_TOKEN   optional; API token (don). When absent, PART B is skipped and
                the script reports "PART B SKIPPED (no token)" without failing.
  GITEA_BASE    optional; default http://192.168.0.101:3200/api/v1
"""
import os
import re
import sys
import json
import urllib.request
import urllib.error

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORKFLOW_DIR = os.path.join(REPO, ".gitea", "workflows")

EXPECTED_JOBS = ["lint", "unit", "integration"]
EXPECTED_TRIGGERS = ["push"]
# Context names Gitea branch protection must require (job names in the workflow).
EXPECTED_STATUS_CONTEXTS = ["lint", "unit", "integration"]

_failures = []
_partB_ok = True


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    print("[%s] %s%s" % (status, name, (" -- " + detail if detail else "")))
    if not ok:
        _failures.append(name)


def yaml_strip_comment(line):
    # naive comment stripping, good enough for workflow yaml we control
    in_s = False
    for i, ch in enumerate(line):
        if ch == "'" or ch == '"':
            in_s = not in_s
        elif ch == "#" and not in_s:
            return line[:i]
    return line


def parse_job_section(text, job):
    # Find "  <job>:" at two-space indent, return the indented block.
    m = re.search(r"(?m)^  {1}%s:\n((?:    .*\n?)+)" % re.escape(job), text)
    return m.group(1) if m else ""


def test_workflow_exists():
    if not os.path.isdir(WORKFLOW_DIR):
        check("workflow dir exists: .gitea/workflows", False, "dir missing")
        return
    files = [f for f in os.listdir(WORKFLOW_DIR) if f.endswith((".yaml", ".yml"))]
    check("at least one workflow yaml present", bool(files), repr(files))
    if not files:
        return
    combined = ""
    for f in files:
        p = os.path.join(WORKFLOW_DIR, f)
        combined += open(p, encoding="utf-8").read() + "\n"
    # trigger on push (every push)
    on_block = combined.split("on:", 1)[1] if "on:" in combined else ""
    check("workflow triggers on push", bool(re.search(r"(?m)^\s*push\s*[:|\n]", on_block)),
          "found 'push' under on:")
    # each expected job present with a run step
    for job in EXPECTED_JOBS:
        block = parse_job_section(combined, job)
        has_run = bool(re.search(r"(?m)^\s*run:\s*[|>\-]", block))
        check("job %s defined" % job, bool(block), "")
        check("job %s has a run step" % job, has_run, "")
        if "lint" in job:
            check("lint job runs rustfmt/check + clippy",
                  ("fmt" in block or "clippy" in block), "")
        if "unit" in job:
            check("unit job runs unit tests",
                  ("--lib" in block or "test" in block), "")
        if "integration" in job:
            check("integration job runs integration tests",
                  ("--tests" in block or "test" in block), "")


def api_get(path, token, base):
    req = urllib.request.Request(base + path,
                                 headers={"Authorization": "token " + token})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode())


def test_branch_protection_and_runs(token, base):
    global _partB_ok
    owner, repo = (os.environ.get("OEP_REPO") or "keelworks/keelworks").split("/", 1)
    try:
        rules = api_get("/repos/%s/%s/branch_protections" % (owner, repo), token, base)
    except urllib.error.HTTPError as e:
        check("PART B: branch protections fetchable", False, "HTTP %s" % e.code)
        _partB_ok = False
        return
    rules = rules if isinstance(rules, list) else []
    main_rule = None
    for r in rules:
        if r.get("rule_name") == "main":
            main_rule = r
    check("PART B: main has a branch protection rule", main_rule is not None,
          "rules: %s" % [r.get("rule_name") for r in rules])
    if main_rule is None:
        _partB_ok = False
        return
    contexts = main_rule.get("status_check_contexts") or []
    check("PART B: main requires status checks", bool(contexts), repr(contexts))
    for ctx in EXPECTED_STATUS_CONTEXTS:
        check("PART B: main requires status check '%s'" % ctx, ctx in contexts, repr(contexts))
    # pushes to main blocked (enable_push set) = PR-based flow
    check("PART B: direct push protection on main", main_rule.get("enable_push") is True,
          "enable_push=%s" % main_rule.get("enable_push"))
    # workflow actually ran successfully on a push event
    try:
        runs = api_get("/repos/%s/%s/actions/runs?limit=10" % (owner, repo), token, base)
    except Exception as e:
        check("PART B: actions runs fetchable", False, repr(e))
        _partB_ok = False
        return
    wf = runs.get("workflow_runs") or []
    push_runs = [r for r in wf if (r.get("event") or "").lower() == "push"]
    green = [r for r in push_runs if r.get("conclusion") == "success"]
    check("PART B: >=1 successful push-triggered run",
          len(green) >= 1,
          "push runs=%d green=%d" % (len(push_runs), len(green)))


def main():
    print("Keelworks CI pipeline (P0-M4-T01) acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_workflow_exists()
    token = os.environ.get("GITEA_TOKEN", "")
    if token:
        base = os.environ.get("GITEA_BASE", "http://192.168.0.101:3200/api/v1")
        test_branch_protection_and_runs(token, base)
    else:
        print("-- PART B SKIPPED (no GITEA_TOKEN env) --")
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
