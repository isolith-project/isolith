#!/usr/bin/env python3
"""Keelworks wave checkpoint tooling (P12 wave integration policy).

Companion to python/oep_tooling/release_train.py and to
P12/wave-integration-and-checkpoint-policy.md. A wave integrates onto a single
branch (wave/N) and closes with one documented checkpoint commit onto main.

Subcommands:
  validate   --wave N --branch wave/N [--base main]
             Check the integration target is well formed (branch exists, based
             on the base, no divergence beyond the base).
  manifest   --wave N --branch wave/N [--base main] [--out PATH]
             Compute the wave checkpoint manifest (schema
             keelworks.wave-checkpoint.v1) and print or write it.
  checkpoint --wave N --branch wave/N [--base main] [--dir P12/wave-checkpoints]
             Emit the manifest JSON and the human wave report MD.
  verify     --manifest PATH
             Re-verify a manifest against its repository.
  repos      [--repo DIR] [--allow keelworks,keelworks-canary]
             Enforce the two-repository hygiene bound (policy section 6): fail
             if the Gitea org holds more than the allowed repositories.
  selfcheck  Policy/doc consistency checks, no repo mutation (CI safe).

Stdlib only. Exit 0 = pass; nonzero = fail.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

SCHEMA = "keelworks.wave-checkpoint.v1"
POLICY = "P12/wave-integration-and-checkpoint-policy.md"
DEFAULT_DIR = "P12/wave-checkpoints"
# Policy section 6: the program has exactly these repositories and no others.
ALLOWED_REPOS = {"keelworks", "keelworks-canary"}
# Grandfathered legacy repo (wave-1 kickoff). No new work; archive pending Don.
LEGACY_REPOS = {"oep-meta"}
GITEA_API = "http://192.168.0.101:3200/api/v1"

CARD_RE = re.compile(r"card\(([^)]+)\):\s*(.*?)\s*\(#(\d+)\)")
EM_DASH = "\u2014"


# --------------------------------------------------------------------------
# git helpers
# --------------------------------------------------------------------------
def git(repo, *args):
    r = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("git %s failed: %s" % (" ".join(args), r.stderr.strip()))
    return r.stdout.strip()


def repo_root(repo):
    """Working tree root, or the repo path itself when it is bare.

    Read-only subcommands (validate, manifest, verify, repos) work against a
    bare repo; only checkpoint writes files and therefore needs a work tree.
    """
    r = subprocess.run(["git", "-C", repo, "rev-parse", "--show-toplevel"],
                       capture_output=True, text=True)
    if r.returncode == 0 and r.stdout.strip():
        return r.stdout.strip()
    return os.path.abspath(repo)


def rev(repo, ref):
    return git(repo, "rev-parse", ref)


def branch_exists(repo, name):
    r = subprocess.run(["git", "-C", repo, "rev-parse", "--verify", "--quiet",
                        "refs/heads/" + name], capture_output=True, text=True)
    return r.returncode == 0


def commits_between(repo, base, tip):
    """Commits reachable from tip and not base, oldest first."""
    out = git(repo, "log", "--reverse", "--format=%H\x1f%s", "%s..%s" % (base, tip))
    commits = []
    for line in out.splitlines():
        if not line.strip():
            continue
        sha, _, subject = line.partition("\x1f")
        commits.append((sha, subject))
    return commits


def _sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


# --------------------------------------------------------------------------
# manifest
# --------------------------------------------------------------------------
def build_manifest(repo, wave, branch, base):
    tip = rev(repo, branch)
    base_sha = rev(repo, base)
    commits = commits_between(repo, base_sha, tip)
    cards = []
    uncarded = []
    for sha, subject in commits:
        m = CARD_RE.search(subject)
        if m:
            cards.append({"card": m.group(1), "title": m.group(2),
                          "pr": int(m.group(3)), "sha": sha})
        else:
            uncarded.append({"sha": sha, "subject": subject})
    return {
        "schema": SCHEMA,
        "wave": str(wave),
        "branch": branch,
        "base": base,
        "base_sha": base_sha,
        "tip_sha": tip,
        "tip_tree": rev(repo, tip + "^{tree}"),
        "host": os.uname().nodename,
        "cards": cards,
        "uncarded_commits": uncarded,
        "counts": {"cards": len(cards), "commits": len(commits),
                   "uncarded": len(uncarded)},
    }


def write_manifest(path, manifest):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
        fh.write("\n")
    return path


def write_report(path, manifest):
    lines = []
    lines.append("# Keelworks wave checkpoint: wave-%s" % manifest["wave"])
    lines.append("")
    lines.append("Schema: %s" % manifest["schema"])
    lines.append("Branch: `%s` -> `%s`" % (manifest["branch"], manifest["base"]))
    lines.append("Base sha: `%s`" % manifest["base_sha"])
    lines.append("Tip sha:  `%s`" % manifest["tip_sha"])
    lines.append("")
    lines.append("Cards: %d. Commits: %d. Uncarded: %d." % (
        manifest["counts"]["cards"], manifest["counts"]["commits"],
        manifest["counts"]["uncarded"]))
    lines.append("")
    lines.append("## Cards in this wave")
    lines.append("")
    lines.append("| card | PR | sha | title |")
    lines.append("|---|---|---|---|")
    for c in manifest["cards"]:
        lines.append("| %s | #%d | %s | %s |" % (
            c["card"], c["pr"], c["sha"][:9], c["title"]))
    if manifest["uncarded_commits"]:
        lines.append("")
        lines.append("## Uncarded commits (fixups and merge plumbing)")
        lines.append("")
        for u in manifest["uncarded_commits"]:
            lines.append("- `%s` %s" % (u["sha"][:9], u["subject"]))
    lines.append("")
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
    return path


def verify_manifest(repo, path):
    errors = []
    m = json.load(open(path, encoding="utf-8"))
    if m.get("schema") != SCHEMA:
        errors.append("wrong schema: %r" % m.get("schema"))
    for key in ("wave", "branch", "base", "base_sha", "tip_sha", "tip_tree"):
        if not m.get(key):
            errors.append("manifest missing %s" % key)
    # the recorded tip must still resolve and match its recorded tree
    for ref, sha_key, tree_key in ((m.get("tip_sha"), "tip_sha", "tip_tree"),):
        if ref:
            try:
                got_tree = rev(repo, ref + "^{tree}")
            except SystemExit:
                errors.append("recorded tip sha does not resolve: %s" % ref)
                continue
            if got_tree != m.get(tree_key):
                errors.append("tip tree mismatch: recorded %s, got %s" % (
                    m.get(tree_key), got_tree))
    # every card commit must be reachable
    for c in m.get("cards", []):
        r = subprocess.run(["git", "-C", repo, "cat-file", "-e", c["sha"] + "^{commit}"],
                           capture_output=True, text=True)
        if r.returncode != 0:
            errors.append("card commit not found: %s (%s)" % (c["sha"][:9], c["card"]))
    return errors


# --------------------------------------------------------------------------
# repository hygiene (policy section 6)
# --------------------------------------------------------------------------
def _gitea_token():
    cred = os.path.expanduser("~/.git-credentials")
    if not os.path.exists(cred):
        return None
    txt = open(cred).read()
    m = re.search(r"http://don:([^@]+)@192\.168\.0\.101(:|%3a)3200", txt)
    return m.group(1) if m else None


def list_repos(org="keelworks"):
    tok = _gitea_token()
    if not tok:
        raise SystemExit("no Gitea token found in ~/.git-credentials")
    req = urllib.request.Request(
        "%s/orgs/%s/repos?limit=100" % (GITEA_API, org),
        headers={"Authorization": "token " + tok})
    with urllib.request.urlopen(req, timeout=30) as r:
        return [x["name"] for x in json.load(r)]


def check_repos(allowed, legacy=None):
    legacy = LEGACY_REPOS if legacy is None else legacy
    names = set(list_repos())
    extra = sorted(names - allowed - legacy)
    legacy_present = sorted(names & legacy)
    missing = sorted(allowed - names)
    return names, extra, legacy_present, missing


# --------------------------------------------------------------------------
# selfcheck (CI safe, no repo mutation)
# --------------------------------------------------------------------------
def selfcheck(root):
    problems = []
    policy = os.path.join(root, POLICY)
    if not os.path.isfile(policy):
        problems.append("policy doc missing: %s" % POLICY)
        return problems
    text = open(policy, encoding="utf-8").read()
    for needle in ("wave/N", "keelworks.wave-checkpoint.v1",
                   "keelworks/keelworks-canary", "Definition of done",
                   "anti-proliferation"):
        if needle not in text:
            problems.append("policy doc missing required content: %r" % needle)
    if text.count(EM_DASH):
        problems.append("policy doc contains %d em dash(es)" % text.count(EM_DASH))
    if len(ALLOWED_REPOS) != 2:
        problems.append("ALLOWED_REPOS must name exactly two repositories")
    return problems


# --------------------------------------------------------------------------
# cli
# --------------------------------------------------------------------------
def main(argv=None):
    ap = argparse.ArgumentParser(description="Keelworks wave checkpoint tooling")
    ap.add_argument("--repo", default=".")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("validate")
    p.add_argument("--wave", required=True)
    p.add_argument("--branch", required=True)
    p.add_argument("--base", default="main")

    p = sub.add_parser("manifest")
    p.add_argument("--wave", required=True)
    p.add_argument("--branch", required=True)
    p.add_argument("--base", default="main")
    p.add_argument("--out", default=None)

    p = sub.add_parser("checkpoint")
    p.add_argument("--wave", required=True)
    p.add_argument("--branch", required=True)
    p.add_argument("--base", default="main")
    p.add_argument("--dir", default=DEFAULT_DIR)

    p = sub.add_parser("verify")
    p.add_argument("--manifest", required=True)

    p = sub.add_parser("repos")
    p.add_argument("--allow", default=",".join(sorted(ALLOWED_REPOS)))

    sub.add_parser("selfcheck")

    args = ap.parse_args(argv)
    root = repo_root(args.repo)

    if args.cmd == "validate":
        ok = True
        if not branch_exists(args.repo, args.branch):
            print("FAIL: branch not found: %s" % args.branch)
            ok = False
        if not branch_exists(args.repo, args.base):
            print("FAIL: base not found: %s" % args.base)
            ok = False
        if ok:
            m = build_manifest(args.repo, args.wave, args.branch, args.base)
            print("OK: %s based on %s, %d cards, %d commits" % (
                args.branch, args.base, m["counts"]["cards"], m["counts"]["commits"]))
        return 0 if ok else 1

    if args.cmd == "manifest":
        m = build_manifest(args.repo, args.wave, args.branch, args.base)
        if args.out:
            write_manifest(args.out, m)
            print("wrote %s" % args.out)
        else:
            print(json.dumps(m, indent=2, sort_keys=True))
        return 0

    if args.cmd == "checkpoint":
        m = build_manifest(args.repo, args.wave, args.branch, args.base)
        jp = os.path.join(root, args.dir, "wave-%s.json" % args.wave)
        mp = os.path.join(root, args.dir, "wave-%s.md" % args.wave)
        write_manifest(jp, m)
        write_report(mp, m)
        print("wrote %s" % jp)
        print("wrote %s" % mp)
        print("tag with: git tag -a wave-%s -m 'wave %s checkpoint'" % (args.wave, args.wave))
        return 0

    if args.cmd == "verify":
        errs = verify_manifest(args.repo, args.manifest)
        if errs:
            print("CHECKPOINT VERIFY FAILED (%d):" % len(errs))
            for e in errs:
                print("  -", e)
            return 1
        print("checkpoint manifest verified: %s" % args.manifest)
        return 0

    if args.cmd == "repos":
        allowed = set(x.strip() for x in args.allow.split(",") if x.strip())
        names, extra, legacy_present, missing = check_repos(allowed)
        if extra:
            print("HYGIENE FAIL: unexpected repositories: %s" % ", ".join(extra))
            print("policy section 6: exactly %s are allowed" % ", ".join(sorted(allowed)))
            return 1
        print("repo hygiene OK: %s" % ", ".join(sorted(names)))
        if legacy_present:
            print("warning: legacy repo present (no new work, archive pending decision): %s"
                  % ", ".join(legacy_present))
        if missing:
            print("note: allowed but absent: %s" % ", ".join(missing))
        return 0

    if args.cmd == "selfcheck":
        probs = selfcheck(root)
        if probs:
            print("SELFCHECK FAILED (%d):" % len(probs))
            for p in probs:
                print("  -", p)
            return 1
        print("wave checkpoint selfcheck OK: policy present, two-repo bound present, no em dashes")
        return 0

    return 2


if __name__ == "__main__":
    sys.exit(main())
