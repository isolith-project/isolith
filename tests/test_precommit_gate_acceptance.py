#!/usr/bin/env python3
"""P0-M3-T04 acceptance test: precommit quality gate wired to CI.

The gate MUST be vendored into the repo (scripts/gate/) so the SAME adaptive
gate runs both as a local .git/hooks/pre-commit block AND as a Gitea Actions
check on every push. This test is the spec; it is written FIRST (RED) and the
gate + CI wiring make it green.

Parts:
  PART A (hermetic, no network): assert the gate scripts are committed,
    executable, bash-syntactically valid, and that a Gitea Actions workflow
    invokes the vendored gate on a push trigger with a run step.
  PART B (live fixture, real commit path): build a throwaway git repo, install
    the hook with scripts/gate/install-hook.sh, and prove on the ACTUAL git
    commit path that a staged .py file with a syntax error BLOCKS the commit
    (exit != 0) and a fixed staged file PASSES it (exit == 0).

The full gate pass against the real workspace (pytest + cargo test) is verified
by running the gate script directly on the repo (documented in scripts/gate/README.md),
not re-run from inside pytest to avoid recursion.

Pure stdlib, pytest-compatible, runs standalone as
`python3 tests/test_precommit_gate_acceptance.py`.

"Done" for P0-M3-T04 means PART A + PART B pass against the real tree, the gate
exits 0 against the workspace, and a Gitea Actions run of the gate workflow
concludes successfully on a push.
"""
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GATE_DIR = os.path.join(REPO, "scripts", "gate")
GATE_SH = os.path.join(GATE_DIR, "precommit-gate.sh")
INSTALL_SH = os.path.join(GATE_DIR, "install-hook.sh")
WORKFLOW_DIR = os.path.join(REPO, ".gitea", "workflows")

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    print("[%s] %s%s" % (status, name, (" -- " + detail if detail else "")))
    if not ok:
        _failures.append(name)


def bash_n(path):
    r = subprocess.run(["bash", "-n", path], capture_output=True, text=True)
    return r.returncode == 0, r.stderr.strip()


def with_exec(p):
    return os.path.isfile(p) and os.access(p, os.X_OK)


# --------------------------------------------------------------------------
# PART A — gate scripts committed, executable, valid, and wired into CI
# --------------------------------------------------------------------------

def test_gate_script_present():
    check("scripts/gate/precommit-gate.sh exists", os.path.isfile(GATE_SH), GATE_SH)
    if not os.path.isfile(GATE_SH):
        return
    check("precommit-gate.sh is executable", with_exec(GATE_SH))
    ok, err = bash_n(GATE_SH)
    check("precommit-gate.sh bash -n clean", ok, err)
    text = open(GATE_SH, encoding="utf-8").read()
    check("gate hard-fails (exit $fail / fail=1)",
          "fail=1" in text and "exit $fail" in text)
    check("gate runs python py_compile", "py_compile" in text)
    check("gate runs pytest when present", "pytest" in text)
    check("gate runs cargo test when present", "cargo test" in text)
    # The gate's own help is not required; the README carries usage.


def test_install_hook_present():
    check("scripts/gate/install-hook.sh exists", os.path.isfile(INSTALL_SH), INSTALL_SH)
    if not os.path.isfile(INSTALL_SH):
        return
    check("install-hook.sh is executable", with_exec(INSTALL_SH))
    ok, err = bash_n(INSTALL_SH)
    check("install-hook.sh bash -n clean", ok, err)
    text = open(INSTALL_SH, encoding="utf-8").read()
    check("install-hook points at the VENDORED gate (not a skill dir)",
          "precommit-gate.sh" in text and ".git/hooks/pre-commit" in text)


def test_ci_wiring():
    if not os.path.isdir(WORKFLOW_DIR):
        check("workflow dir exists: .gitea/workflows", False, "dir missing")
        return
    files = [f for f in os.listdir(WORKFLOW_DIR) if f.endswith((".yaml", ".yml"))]
    check("workflow dir has yaml(s)", bool(files), repr(files))
    gate_wf = os.path.join(WORKFLOW_DIR, "precommit-gate.yaml")
    check(".gitea/workflows/precommit-gate.yaml exists", os.path.isfile(gate_wf), gate_wf)
    if not os.path.isfile(gate_wf):
        return
    text = open(gate_wf, encoding="utf-8").read()
    check("workflow triggers on push", bool(re.search(r"(?m)^\s*push\s*[:|]", text)))
    check("workflow triggers on pull_request",
          bool(re.search(r"(?m)^\s*pull_request\s*[:|]", text)))
    check("workflow invokes the vendored gate script", "precommit-gate.sh" in text)
    check("workflow has a run step calling bash", "run:" in text and "bash" in text)


def test_gate_readme():
    readme = os.path.join(GATE_DIR, "README.md")
    check("scripts/gate/README.md documents usage", os.path.isfile(readme), readme)
    if not os.path.isfile(readme):
        return
    text = open(readme, encoding="utf-8").read()
    check("README says how to install the hook", "install-hook.sh" in text)
    check("README says how to remove the hook", "pre-commit" in text and "rm" in text)
    check("README documents the CI wiring", "CI" in text or "Gitea Actions" in text
          or ".gitea" in text)


# --------------------------------------------------------------------------
# PART B — live fixture: the hook blocks a broken staged .py on git commit
# --------------------------------------------------------------------------

def run_git(path, args, **kw):
    # Scrub git's hook env (GIT_DIR / GIT_INDEX_FILE are exported to pre-commit
    # hooks): a fixture repo created inside the hook must never inherit the
    # parent repo's git dir, or its commits would land in the real repo
    # (observed: 'should pass'/blocked' commits staged _gate_fixture.py into
    # the Isolith worktree).
    clean_env = {k: v for k, v in os.environ.items()
                 if not k.startswith("GIT_")}
    return subprocess.run(["git"] + args, cwd=path, capture_output=True,
                          text=True, env=clean_env, **kw)


def test_hook_blocks_bad_commit_and_passes_good():
    if not os.path.isfile(GATE_SH):
        check("PART B skipped (no gate script)", False, "gate missing")
        return
    # Belt-and-suspenders: the fixture must always live in the system temp dir,
    # never in the repo root. A generic fixture name (broken.py) once leaked
    # into the index; we use a unique name AND a leak-detector guard that fails
    # loudly instead of silently polluting a commit.
    stray = os.path.join(REPO, "_gate_fixture.py")
    if os.path.exists(stray):
        os.remove(stray)
    with tempfile.TemporaryDirectory(prefix="kgate-") as tmp:
        run_git(tmp, ["init", "-q", "-b", "main"])
        run_git(tmp, ["config", "user.email", "gate@isolith.test"])
        run_git(tmp, ["config", "user.name", "Isolith Gate"])
        # Install the vendored hook into the fixture repo. Scrub GIT_* env too:
        # install-hook.sh must act on the fixture's .git, not the parent's.
        clean_env = {k: v for k, v in os.environ.items()
                     if not k.startswith("GIT_")}
        subprocess.run(["bash", INSTALL_SH, tmp], capture_output=True, text=True,
                       env=clean_env)
        hook = os.path.join(tmp, ".git", "hooks", "pre-commit")
        # Symlink may fall back to a copy inside install-hook.sh.
        check("PART B: hook file exists after install", os.path.exists(hook), hook)
        if not os.path.exists(hook):
            return
        # 1) bad file: syntax error -> commit MUST be blocked.
        with open(os.path.join(tmp, "_gate_fixture.py"), "w", encoding="utf-8") as fh:
            fh.write("def broken(:\n    pass\n")
        run_git(tmp, ["add", "_gate_fixture.py"])
        commit = run_git(tmp, ["commit", "-m", "should be blocked"])
        check("PART B: bad staged .py BLOCKS commit",
              commit.returncode != 0, "rc=%s" % commit.returncode)
        out = (commit.stdout + commit.stderr).lower()
        check("PART B: blocker output mentions python syntax",
              "syntax" in out,
              (commit.stdout + commit.stderr).strip()[-200:])
        # 2) fixed file: valid python -> commit MUST succeed.
        with open(os.path.join(tmp, "_gate_fixture.py"), "w", encoding="utf-8") as fh:
            fh.write("def ok():\n    return 42\n")
        run_git(tmp, ["add", "_gate_fixture.py"])
        commit = run_git(tmp, ["commit", "-m", "should pass"])
        check("PART B: fixed staged .py PASSES commit",
              commit.returncode == 0,
              (commit.stdout + commit.stderr).strip()[-200:])
        # 3) the fixture never leaked into the repo root during the test.
        check("PART B: no fixture leak into repo root",
              not os.path.exists(os.path.join(REPO, "_gate_fixture.py")),
              os.path.join(REPO, "_gate_fixture.py"))


# --------------------------------------------------------------------------

def main():
    print("Isolith precommit quality gate (P0-M3-T04) acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_gate_script_present()
    test_install_hook_present()
    test_ci_wiring()
    test_gate_readme()
    test_hook_blocks_bad_commit_and_passes_good()
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
