#!/usr/bin/env python3
"""Acceptance test for the P12 wave integration and checkpoint policy.

Two things are asserted on real material:
  1. the policy doc carries the binding content (wave/N model, checkpoint
     schema, the two-repository hygiene bound, the changed definition of done,
     no em dashes / curly quotes);
  2. the tool actually round-trips: manifest a hermetic repo, then verify it.

Hermetic git repos are created in a temp dir with GIT_* environment scrubbed,
because a leaked GIT_DIR from the surrounding checkout can otherwise move the
outer repository's refs (a real incident in this program, see the P12-T05
hardening).

Run: python3 tests/test_wave_integration_policy.py   (and under pytest)
Exit 0 = pass.
"""
import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
POLICY = os.path.join(ROOT, "workstreams/release-engineering", "wave-integration-and-checkpoint-policy.md")
TOOL = os.path.join(ROOT, "python", "oep_tooling", "wave_checkpoint.py")

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def load_tool():
    spec = importlib.util.spec_from_file_location("wave_checkpoint", TOOL)
    if spec is None or spec.loader is None:
        raise SystemExit("cannot load tool module: %s" % TOOL)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def clean_env():
    env = dict(os.environ)
    for k in list(env):
        if k.startswith("GIT_"):
            del env[k]
    return env


def run(cmd, cwd, env):
    return subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True)


def hermetic_repo(tmp):
    env = clean_env()
    repo = os.path.join(tmp, "repo")
    os.makedirs(repo)
    for c in (["init", "-q", "-b", "main"],
              ["config", "user.email", "t@example.invalid"],
              ["config", "user.name", "test"]):
        r = run(["git"] + c, repo, env)
        check(r.returncode == 0, "git %s failed: %s" % (c, r.stderr))
    # main gets one base commit
    open(os.path.join(repo, "README"), "w").write("base\n")
    run(["git", "add", "-A"], repo, env)
    run(["git", "commit", "-q", "-m", "chore: base"], repo, env)
    # a wave branch with two card commits and one uncarded fixup
    run(["git", "checkout", "-q", "-b", "wave/9"], repo, env)
    for i, (f, msg) in enumerate((
            ("a.txt", "card(P1-T01): first thing (#101)"),
            ("b.txt", "card(P1-T02): second thing (#102)"),
            ("c.txt", "chore: fixup after merge"))):
        open(os.path.join(repo, f), "w").write("x\n")
        run(["git", "add", "-A"], repo, env)
        run(["git", "commit", "-q", "-m", msg], repo, env)
    return repo, env


def test_policy_doc():
    check(os.path.isfile(POLICY), "policy doc missing: %s" % POLICY)
    if not os.path.isfile(POLICY):
        return
    text = open(POLICY, encoding="utf-8").read()
    low = text.lower()
    for needle in ("wave/n", "keelworks.wave-checkpoint.v1",
                   "keelworks/keelworks-canary", "definition of done",
                   "anti-proliferation", "wave-checkpoints"):
        check(needle in low, "policy missing required content: %r" % needle)
    check("card(<card-id>)" in text or "card(" in text,
          "policy must define the card commit message form")
    check(text.count(EM_DASH) == 0, "policy contains em dash(es)")
    check(sum(text.count(c) for c in CURLY) == 0, "policy contains curly quotes")
    check(len(re.findall(r"repositor(y|ies)", low)) >= 4,
          "policy must discuss repositories (hygiene clause)")


def test_tool_roundtrip():
    mod = load_tool()
    check(mod.SCHEMA == "keelworks.wave-checkpoint.v1", "tool schema constant wrong")
    check(mod.ALLOWED_REPOS == {"keelworks", "keelworks-canary"},
          "tool must bound repos to exactly the canonical pair")
    tmp = tempfile.mkdtemp(prefix="wave-ckpt-")
    try:
        repo, env = hermetic_repo(tmp)
        rc = run([sys.executable, TOOL, "--repo", repo, "manifest",
                  "--wave", "9", "--branch", "wave/9", "--base", "main"],
                 ROOT, env)
        check(rc.returncode == 0, "manifest failed: %s" % rc.stderr)
        if rc.returncode == 0:
            import json
            m = json.loads(rc.stdout)
            check(m["counts"]["cards"] == 2, "expected 2 cards, got %s" % m["counts"])
            check(m["counts"]["uncarded"] == 1, "expected 1 uncarded commit")
            check(m["cards"][0]["card"] == "P1-T01", "card id parse wrong")
            check(m["cards"][0]["pr"] == 101, "pr number parse wrong")
            # emit + verify round trip
            rc2 = run([sys.executable, TOOL, "--repo", repo, "checkpoint",
                       "--wave", "9", "--branch", "wave/9", "--base", "main",
                       "--dir", os.path.join(tmp, "out")], ROOT, env)
            check(rc2.returncode == 0, "checkpoint failed: %s" % rc2.stderr)
            jp = os.path.join(tmp, "out", "wave-9.json")
            check(os.path.isfile(jp), "checkpoint did not write manifest")
            rc3 = run([sys.executable, TOOL, "--repo", repo, "verify",
                       "--manifest", jp], ROOT, env)
            check(rc3.returncode == 0, "verify failed on a fresh manifest: %s" % rc3.stderr)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def test_selfcheck():
    rc = subprocess.run([sys.executable, TOOL, "selfcheck"], cwd=ROOT,
                        capture_output=True, text=True)
    check(rc.returncode == 0, "tool selfcheck failed: %s\n%s" % (rc.stdout, rc.stderr))


def run_checks():
    print("P12 wave integration policy acceptance")
    print("policy: %s" % POLICY)
    print("=" * 66)
    test_policy_doc()
    test_tool_roundtrip()
    test_selfcheck()
    return failures


def main():
    fails = run_checks()
    if fails:
        print("WAVE INTEGRATION POLICY ACCEPTANCE FAILED (%d)" % len(fails))
        for f in fails:
            print("  -", f)
        sys.exit(1)
    print("WAVE INTEGRATION POLICY ACCEPTANCE PASSED: wave/N model, checkpoint "
          "schema, two-repo hygiene bound, changed definition of done, and the "
          "tool manifest/verify round trip.")
    return 0


def test_wave_integration_policy_acceptance():
    fails = run_checks()
    assert not fails, "wave integration policy acceptance failed (%d): %s" % (
        len(fails), fails[:5])


if __name__ == "__main__":
    main()
