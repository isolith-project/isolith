#!/usr/bin/env python3
"""Keelworks release-train tooling (P12-T05).

Pure-stdlib CLI behind the policy in P12/release-train.md. Never mutates git
state unless a mutating subcommand is explicitly invoked.

Subcommands
-----------
  release_train.py validate --version <V>
      Exit 0 iff <V> matches the Keelworks calver+semver scheme YYYY.N.P[-PRE].

  release_train.py freeze --repo DIR --version <V> --trunk <REF>
                          [--since <REF>] [--out PATH]
      Read-only: resolve <REF>, compute the frozen commit set and tree hash,
      and write the freeze manifest (default
      P12/freeze-manifests/<V>.json under the repo). Exit 0 on success.

  release_train.py verify --repo DIR --manifest PATH
      Recompute the freeze for the recorded cut_from.sha and FAIL if the
      manifest no longer matches the git object database.

  release_train.py release-cut --repo DIR --version <V> --trunk <REF>
                    --canary <OWNER/REPO> [--since <REF>] [--dry-run]
      Full candidate cut: freeze + create release/<train> at the cut sha +
      annotated tag v<V> + push branch(tagged as canary/<V>) and tag to the
      canary remote. --dry-run prints every step without running the git
      mutating commands or the push.
"""
import argparse
import datetime
import json
import os
import re
import socket
import subprocess
import sys
import urllib.parse

# YYYY.N.P[-prerelease]  e.g. 2027.1.0, 2027.1.0-rc1, 2027.1.0-alpha.2
# Prerelease restricted to the candidate labels the program uses:
# rc<N> (release candidates) or <alpha|beta|gamma>.<n>.
PRERELEASE_RE = r"(?:rc\d+|(?:alpha|beta|gamma)\.[1-9]\d*)"
VERSION_RE = re.compile(
    rf"^(20\d\d)\.([1-9]\d?)\.(\d+)(?:-({PRERELEASE_RE}))?$"
)
SCHEMA = "keelworks.release-freeze.v1"


class ReleaseTrainError(RuntimeError):
    pass


def git(repo, *args):
    """Run git in repo; return (rc, stdout, stderr).

    Scrub every GIT_* var from the environment: a parent process that sets
    GIT_DIR (a git hook, a nested pytest run, CI) would override `-C repo`
    and make these commands operate on the wrong repository, breaking both
    the hermetic release-cut test and any real nested invocation.
    """
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    proc = subprocess.run(
        ["git", "-C", repo] + list(args),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def git_rev(repo, *args):
    rc, out, err = git(repo, *args)
    if rc != 0:
        raise ReleaseTrainError(f"git {' '.join(args)} failed: {err or out}")
    return out


def parse_version(version):
    m = VERSION_RE.match(version)
    if not m:
        raise ReleaseTrainError(
            f"invalid version {version!r}; expected YYYY.N.P[-prerelease], "
            "e.g. 2027.1.0 or 2027.1.0-rc1"
        )
    year, train, patch, pre = m.groups()
    return {
        "year": year,
        "train": f"{year}.{train}",
        "patch": int(patch),
        "prerelease": pre or "",
        "full": version,
    }


def validate_version(version):
    """Return True if version matches the scheme, False otherwise."""
    return bool(VERSION_RE.match(version))


def _root_commit(repo):
    roots = git_rev(repo, "rev-list", "--max-parents=0", "--all").splitlines()
    return roots[0] if roots else None


def compute_freeze(repo, version, trunk_ref, since_ref=None):
    """Resolve the frozen commit set for version at trunk_ref."""
    cut_sha = git_rev(repo, "rev-parse", "--verify", f"{trunk_ref}^{{commit}}")
    cut_tree = git_rev(repo, "rev-parse", f"{trunk_ref}^{{tree}}")
    since = since_ref if since_ref else _root_commit(repo)
    if not since:
        raise ReleaseTrainError("no commits and no --since ref to freeze from")
    since_sha = git_rev(repo, "rev-parse", "--verify", f"{since}^{{commit}}")

    rc_full, out_full, err_full = git(
        repo, "rev-list", "--count", f"{since_sha}..{cut_sha}"
    )
    rc_fp, out_fp, err_fp = git(
        repo, "rev-list", "--first-parent", "--count", f"{since_sha}..{cut_sha}"
    )
    if rc_full != 0:
        raise ReleaseTrainError(f"rev-list full failed: {err_full or out_full}")
    if rc_fp != 0:
        raise ReleaseTrainError(f"rev-list first-parent failed: {err_fp or out_fp}")

    return {
        "schema": SCHEMA,
        "version": parse_version(version)["full"],
        "train": parse_version(version)["train"],
        "cut_from": {"ref": trunk_ref, "sha": cut_sha},
        "cut_tree": cut_tree,
        "frozen_commits_full": int(out_full),
        "frozen_commits_first_parent": int(out_fp),
        "frozen_newest": cut_sha,
        "frozen_oldest": since_sha,
        "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "host": socket.gethostname(),
    }


def _manifest_default_path(repo, version):
    return os.path.join(repo, "P12", "freeze-manifests", f"{version}.json")


def write_manifest(manifest, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
        fh.write("\n")
    return path


def verify_manifest(repo, manifest_path):
    """Recompute the freeze at the recorded sha and compare against the file."""
    with open(manifest_path) as fh:
        recorded = json.load(fh)
    recomputed = compute_freeze(
        repo,
        recorded["version"],
        recorded["cut_from"]["sha"],
        since_ref=recorded["frozen_oldest"],
    )
    # cut_from.ref is an advisory label (e.g. "main"); the binding fact is the
    # sha, so normalize the label pathologically and compare on the sha.
    if "cut_from" in recorded and "ref" in recorded["cut_from"]:
        recomputed["cut_from"]["ref"] = recorded["cut_from"]["ref"]
    # Normalize the run-specific fields before comparing. If a field is absent
    # from the recorded manifest it must be absent from the recomputed one too
    # (a phantom "canary": null key makes the dicts unequal invisibly).
    for key in ("canary", "cut_branch", "created_utc", "host"):
        if key in recorded:
            recomputed[key] = recorded[key]
        else:
            recomputed.pop(key, None)
    ok = recomputed == recorded
    if not ok:
        diffs = {k: (recorded.get(k), recomputed.get(k)) for k in set(recorded) | set(recomputed)
                 if recorded.get(k) != recomputed.get(k)}
        return False, diffs
    return True, {}


def _canary_url(owner_repo):
    """Build the NAS Gitea URL from ~/.git-credentials, so no raw IP ever
    appears on the command line and no credentials are embedded in the remote
    URL (git resolves them itself at push time)."""
    cred = os.path.expanduser("~/.git-credentials")
    netloc = None
    if os.path.exists(cred):
        for line in open(cred):
            u = urllib.parse.urlsplit(line.strip())
            if u.scheme == "http" and u.netloc:
                # strip userinfo (if any); the stored host may percent-encode
                # its port (e.g. 192.168.0.101%3a3200), so unquote it back
                hostport = u.netloc.rsplit("@", 1)[-1]
                netloc = urllib.parse.unquote(hostport)
                break
    if not netloc:
        raise ReleaseTrainError("cannot resolve Gitea host from ~/.git-credentials")
    return f"http://{netloc}/{owner_repo}.git"


def release_cut(repo, version, trunk_ref, canary_owner_repo, since_ref=None,
                dry_run=False):
    info = parse_version(version)
    cut = compute_freeze(repo, version, trunk_ref, since_ref=since_ref)
    rel_branch = f"release/{info['train']}"
    tag = f"v{info['full']}"
    canary_branch = f"canary/{info['full']}"
    cut["cut_branch"] = rel_branch
    cut["canary"] = {
        "repo": canary_owner_repo,
        "branch": canary_branch,
        "tag": tag,
    }

    steps = []
    steps.append(f"manifest -> P12/freeze-manifests/{info['full']}.json (write)")
    steps.append(f"branch  -> git branch {rel_branch} {cut['cut_from']['sha']}")
    steps.append(f"tag     -> git tag -a {tag} -m 'Keelworks {info['full']} freeze' {cut['cut_from']['sha']}")
    steps.append(f"push    -> git push canary {rel_branch}:{canary_branch} {tag}")
    if dry_run:
        print("DRY RUN for", info["full"])
        for s in steps:
            print("  would run:", s)
        return cut, None

    manifest_path = _manifest_default_path(repo, info["full"])
    write_manifest(cut, manifest_path)

    def ensure_ref(refname, create_args, kind):
        rc, out, err = git(repo, "rev-parse", "--verify", "--quiet", f"{refname}^{{commit}}")
        if rc == 0:
            if out != cut["cut_from"]["sha"]:
                raise ReleaseTrainError(
                    f"{kind} {refname} exists at {out}, not the cut sha "
                    f"{cut['cut_from']['sha']}; refusing to rewrite an existing ref")
            return  # already at the cut sha, idempotent no-op
        git_rev(repo, *create_args)

    ensure_ref(rel_branch, ["branch", rel_branch, cut["cut_from"]["sha"]], "branch")
    ensure_ref(tag, ["tag", "-a", tag,
                     "-m", f"Keelworks {info['full']} release candidate freeze",
                     cut["cut_from"]["sha"]], "tag")
    remote_url = _canary_url(canary_owner_repo)
    existing = git_rev(repo, "remote").splitlines()
    if "canary" not in existing:
        git_rev(repo, "remote", "add", "canary", remote_url)
    else:
        cur = git_rev(repo, "remote", "get-url", "canary")
        # heal only URLs that are equipment-bad (credentials embedded in the
        # URL, or a percent-encoded host); never clobber a working remote
        # (e.g. a local file:// canary used by hermetic tests)
        if "@" in cur or "%" in cur:
            git_rev(repo, "remote", "set-url", "canary", remote_url)
    rc, out, err = git(repo, "push", "canary", f"{rel_branch}:{canary_branch}", tag)
    if rc != 0:
        raise ReleaseTrainError(f"canary push failed: {err or out}")
    return cut, manifest_path


def main(argv=None):
    ap = argparse.ArgumentParser(description="Keelworks release-train tooling")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("validate")
    p.add_argument("--version", required=True)

    p = sub.add_parser("freeze")
    p.add_argument("--repo", default=os.getcwd())
    p.add_argument("--version", required=True)
    p.add_argument("--trunk", required=True)
    p.add_argument("--since")
    p.add_argument("--out")

    p = sub.add_parser("verify")
    p.add_argument("--repo", default=os.getcwd())
    p.add_argument("--manifest", required=True)

    p = sub.add_parser("release-cut")
    p.add_argument("--repo", default=os.getcwd())
    p.add_argument("--version", required=True)
    p.add_argument("--trunk", required=True)
    p.add_argument("--canary", required=True)
    p.add_argument("--since")
    p.add_argument("--dry-run", action="store_true")

    args = ap.parse_args(argv)

    if args.cmd == "validate":
        ok = validate_version(args.version)
        print(f"VALID {args.version}" if ok else f"INVALID {args.version}")
        return 0 if ok else 1

    if args.cmd == "freeze":
        cut = compute_freeze(args.repo, args.version, args.trunk,
                             since_ref=args.since)
        path = args.out or _manifest_default_path(args.repo, args.version)
        write_manifest(cut, path)
        print(json.dumps(cut, indent=2, sort_keys=True))
        print(f"manifest written: {path}")
        return 0

    if args.cmd == "verify":
        ok, diffs = verify_manifest(args.repo, args.manifest)
        if ok:
            print(f"VERIFY OK {args.manifest}")
            return 0
        print(f"VERIFY FAILED {args.manifest}")
        print(json.dumps(diffs, indent=2, sort_keys=True))
        return 1

    if args.cmd == "release-cut":
        cut, path = release_cut(
            args.repo, args.version, args.trunk, args.canary,
            since_ref=args.since, dry_run=args.dry_run,
        )
        print(json.dumps(cut, indent=2, sort_keys=True))
        if path:
            print(f"cut complete; manifest: {path}")
        return 0

    return 1


if __name__ == "__main__":
    sys.exit(main())
