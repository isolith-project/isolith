#!/usr/bin/env python3
"""P12-T05 acceptance test: release train policy, version scheme, freeze
manifest, and the mechanical release cut.

Pure stdlib; runs under `python3 tests/test_p12release_train.py`, pytest, and
the P0-M3-T04 precommit gate. The release-cut machinery is exercised end to
end against a hermetic local (file://) canary bare repo -- no network, no
mutating the real Isolith checkout.

Checks
------
1. Version scheme (YYYY.N.P[-pre]) accepts/rejects the right strings.
2. workstreams/release-engineering/release-train.md exists with the required policy content and front
   matter (feature freeze, canary staging, patch-vs-next-train triggers).
3. Freeze manifest generation + verify round-trip on a real temp git repo.
4. verify detects a tampered manifest.
5. release-cut performs the full mechanical cut (manifest, release branch,
   annotated tag, canary push of canary/<v> + tag) against a bare canary repo.
6. Any committed workstreams/release-engineering/freeze-manifests/*.json conforms to the schema.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
from oep_tooling import release_train as rt  # noqa: E402

_failures = []


def check(name, ok, detail=""):
    print(f"[{'PASS' if ok else 'FAIL'}] {name}" + (f" -- {detail}" if detail else ""))
    if not ok:
        _failures.append(name)


def run(cmd, cwd=None, env=None):
    env = env or os.environ
    # never let the parent repo's git dir leak into a fixture repo
    clean = {k: v for k, v in env.items() if not k.startswith("GIT_")}
    proc = subprocess.run(cmd, cwd=cwd, env=clean, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def assert_isolated(tmp, label):
    """Fail loudly if a fixture repo's git dir is NOT the tmp dir itself."""
    rc, gd, err = run(["git", "rev-parse", "--absolute-git-dir"], cwd=tmp)
    assert rc == 0, f"{label}: could not resolve git dir: {err}"
    if os.path.realpath(gd) != os.path.realpath(os.path.join(tmp, ".git")):
        raise AssertionError(
            f"{label}: fixture repo NOT isolated: git dir {gd} != {tmp}/.git. "
            "A leaked GIT_* env var would corrupt the real isolith repo.")


def make_repo(path, ncommits=3, author="release-train test <t@kw>"):
    """Create a git repo at path with ncommits on branch 'main'."""
    env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@kw",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@kw")
    os.makedirs(path, exist_ok=True)
    rc, out, err = run(["git", "init", "-q", "-b", "main"], cwd=path, env=env)
    assert rc == 0, err
    assert_isolated(path, "make_repo")
    run(["git", "config", "user.name", "t"], cwd=path, env=env)
    run(["git", "config", "user.email", "t@kw"], cwd=path, env=env)
    for i in range(ncommits):
        with open(os.path.join(path, "file.txt"), "a") as fh:
            fh.write(f"line {i}\n")
        run(["git", "add", "-A"], cwd=path, env=env)
        run(["git", "commit", "-q", "-m", f"commit {i}"], cwd=path, env=env)
    return env


def test_version_scheme():
    good = ["2027.1.0", "2027.1.0-rc1", "2027.12.4-rc1", "2027.2.7-rc10",
            "2027.1.0-alpha.2", "2099.52.0"]
    bad = ["2027.1", "2027", "27.1.0", "2027.01.0", "2027.1.0-rc", "2027.1.0-",
           "2027.1.0 rc1", "2027.0.1", "2027.1.0.1"]
    for v in good:
        check(f"valid version accepted: {v}", rt.validate_version(v), v)
    for v in bad:
        check(f"invalid version rejected: {v}", not rt.validate_version(v), v)


def test_doc_present_and_covers_policy():
    doc = os.path.join(REPO, "workstreams/release-engineering", "release-train.md")
    check("release-train doc exists at workstreams/release-engineering/release-train.md", os.path.isfile(doc), doc)
    if not os.path.isfile(doc):
        return
    text = open(doc).read()
    check("front matter carries version+status", "version: 1.0.0" in text
          and "status: Ratified" in text)
    for needle in ("## 3. Feature freeze", "## 5. Staged rollout",
                   "## 6. Patch / hotfix policy", "**A patch release",
                   "goes to the next train", "isolith/isolith-canary",
                   "## Change log"):
        check(f"doc covers required policy: {needle!r}", needle in text)


def test_freeze_and_verify_roundtrip():
    with tempfile.TemporaryDirectory() as tmp:
        env = make_repo(tmp)
        cut = rt.compute_freeze(tmp, "2027.3.0", "main")
        check("freeze resolves cut sha", len(cut["cut_from"]["sha"]) == 40)
        check("freeze records frozen commits", cut["frozen_commits_first_parent"] >= 1)
        check("freeze records train from version", cut["train"] == "2027.3")
        manifest_path = os.path.join(tmp, "m.json")
        rt.write_manifest(cut, manifest_path)
        ok, diffs = rt.verify_manifest(tmp, manifest_path)
        check("freeze/verify round-trip is faithful", ok, str(diffs))


def test_verify_detects_tamper():
    with tempfile.TemporaryDirectory() as tmp:
        env = make_repo(tmp)
        cut = rt.compute_freeze(tmp, "2027.3.0", "main")
        manifest_path = os.path.join(tmp, "m.json")
        rt.write_manifest(cut, manifest_path)
        data = json.load(open(manifest_path))
        data["frozen_commits_full"] = data["frozen_commits_full"] + 7
        json.dump(data, open(manifest_path, "w"))
        ok, diffs = rt.verify_manifest(tmp, manifest_path)
        check("verify flags a tampered manifest", not ok,
              "expected mismatch in frozen_commits_full")


def test_release_cut_end_to_end_hermetic():
    with tempfile.TemporaryDirectory() as tmp:
        env = make_repo(tmp)
        bare = os.path.join(tmp, "canary.git")
        rc, out, err = run(["git", "init", "-q", "--bare", bare], cwd=tmp)
        assert rc == 0, err
        # the bare canary must be an isolated bare store, not the parent repo
        rc2, gd, err2 = run(["git", "rev-parse", "--absolute-git-dir"], cwd=bare)
        assert rc2 == 0, err2
        assert os.path.realpath(gd) == os.path.realpath(bare), gd
        run(["git", "remote", "add", "canary", bare], cwd=tmp, env=env)
        head_sha = run(["git", "rev-parse", "HEAD"], cwd=tmp, env=env)[1]
        cut, manifest_path = rt.release_cut(
            tmp, "2027.1.0-rc1", "main", "isolith/isolith-canary")
        # manifest written
        check("release-cut writes the manifest", manifest_path
              and os.path.isfile(manifest_path), manifest_path)
        # local branch + annotated tag exist
        branches = run(["git", "branch"], cwd=tmp, env=env)[1]
        check("release branch created locally", "release/2027.1" in branches, branches)
        tags = run(["git", "tag", "-n"], cwd=tmp, env=env)[1]
        check("annotated tag created locally", "v2027.1.0-rc1" in tags, tags)
        # canary bare repo received branch + tag at the cut sha
        refs = run(["git", "--git-dir", bare, "for-each-ref"], cwd=tmp, env=env)[1]
        check("canary received canary/2027.1.0-rc1 branch",
              "refs/heads/canary/2027.1.0-rc1" in refs, refs)
        check("canary received v2027.1.0-rc1 tag",
              "refs/tags/v2027.1.0-rc1" in refs)
        cand_sha = run(["git", "--git-dir", bare, "rev-parse",
                        "canary/2027.1.0-rc1"], cwd=tmp, env=env)[1]
        check("canary candidate sha equals freeze sha", cand_sha == head_sha,
              f"{cand_sha} vs {head_sha}")


def test_committed_manifests_conform():
    mdir = os.path.join(REPO, "workstreams/release-engineering", "freeze-manifests")
    if not os.path.isdir(mdir):
        check("no committed freeze manifests to validate",
              True, "workstreams/release-engineering/freeze-manifests empty or absent")
        return
    files = sorted(f for f in os.listdir(mdir) if f.endswith(".json"))
    if not files:
        check("no committed freeze manifests", True, "none present")
        return
    required = ["schema", "version", "train", "cut_from", "cut_tree",
                "frozen_commits_full", "frozen_commits_first_parent",
                "frozen_newest", "frozen_oldest", "canary"]
    for f in files:
        with open(os.path.join(mdir, f)) as fh:
            data = json.load(fh)
        missing = [k for k in required if k not in data]
        check(f"manifest {f} conforms to schema", not missing,
              "missing: " + ",".join(missing))
        check(f"manifest {f} schema id matches",
              data.get("schema") == rt.SCHEMA, str(data.get("schema")))


def main():
    test_version_scheme()
    test_doc_present_and_covers_policy()
    test_freeze_and_verify_roundtrip()
    test_verify_detects_tamper()
    test_release_cut_end_to_end_hermetic()
    test_committed_manifests_conform()
    print()
    if _failures:
        print(f"ACCEPTANCE FAILED: {len(_failures)} check(s) failed")
        return 1
    print("ACCEPTANCE PASSED: all release-train checks green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
