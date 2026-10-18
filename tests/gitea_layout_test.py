#!/usr/bin/env python3
"""Acceptance test for the Keelworks NAS Gitea org + repo layout (P0-M1-T03).

Checks the live Gitea instance state, not a local copy. Run from anywhere with
network access to the NAS Gitea (192.168.0.101:3200) and a token.

Token resolution: $GITEA_TOKEN, else the don token in ~/.git-credentials.
API base: $GITEA_BASE or http://192.168.0.101:3200/api/v1

Exit 0 and print one PASS/FAIL line per check, plus a final verdict, when all
checks pass. Exit 1 otherwise.
"""
import json
import os
import sys
import urllib.error
import urllib.request
from urllib.parse import unquote, urlsplit

BASE = os.environ.get("GITEA_BASE", "http://192.168.0.101:3200/api/v1")
ORG = "keelworks"
META_REPO = "oep-meta"
CODE_REPO = "keelworks"
OLD_PATH = "don/keelworks"

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok, detail))
    print(("PASS" if ok else "FAIL") + f"  {name}" + (f"  [{detail}]" if detail else ""))


def get_token():
    tok = os.environ.get("GITEA_TOKEN")
    if tok:
        return tok
    cred = os.path.expanduser("~/.git-credentials")
    if os.path.exists(cred):
        with open(cred) as f:
            for line in f:
                if "192.168.0.101" in line:
                    u = urlsplit(line.strip())
                    return unquote(u.password) if u.password else None
    return None


def api(path, expect=200):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "token " + get_token()})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:200]
    except Exception as e:  # noqa: BLE001
        return -1, str(e)


def repo_exists(owner, name):
    code, body = api(f"/repos/{owner}/{name}")
    return code == 200 and isinstance(body, dict) and body.get("name") == name


def default_branch_of(owner, name):
    code, body = api(f"/repos/{owner}/{name}")
    if code == 200 and isinstance(body, dict):
        return body.get("default_branch")
    return None


def is_private(owner, name):
    code, body = api(f"/repos/{owner}/{name}")
    if code == 200 and isinstance(body, dict):
        return body.get("private") is True
    return False


def tree_paths(owner, name, branch):
    code, body = api(f"/repos/{owner}/{name}/git/trees/{branch}?recursive=true")
    if code == 200 and isinstance(body, dict):
        return {e["path"] for e in body.get("tree", [])}
    return set()


def main():
    if not get_token():
        print("ERROR: no Gitea token (set GITEA_TOKEN or fix ~/.git-credentials)")
        return 1

    code, body = api(f"/orgs/{ORG}")
    org_exists = code == 200 and isinstance(body, dict) and body.get("username") == ORG
    check("G1 org keelworks exists", org_exists)
    check("G2 oep-meta repo exists under org", repo_exists(ORG, META_REPO))
    check("G3 keelworks monorepo exists under org", repo_exists(ORG, CODE_REPO))
    check("G4 oep-meta is private", is_private(ORG, META_REPO))
    check("G5 keelworks repo is private", is_private(ORG, CODE_REPO))
    check("G6 oep-meta default branch == main", default_branch_of(ORG, META_REPO) == "main")
    check("G7 keelworks default branch == main", default_branch_of(ORG, CODE_REPO) == "main")

    markers = ["Cargo.toml", "crates/oep-cgr/Cargo.toml", "docs/layout.md",
               "tests/monorepo_layout_test.py"]
    paths = tree_paths(ORG, CODE_REPO, "main")
    missing = [m for m in markers if m not in paths]
    check("G8 keelworks monorepo markers on main", not missing,
          "missing: " + ",".join(missing) if missing else "all present")

    code, body = api(f"/repos/{OLD_PATH}")  # follows 3xx; a transfer leaves a 301 -> new owner
    moved = (code == 200 and isinstance(body, dict)
             and body.get("owner", {}).get("username") == ORG
             and body.get("name") == CODE_REPO)
    check("G9 don/keelworks no longer owned by don (redirects to org)", moved,
          "status " + str(code) + (f" owner={body.get('owner',{}).get('username')}" if isinstance(body, dict) else ""))

    meta_paths = tree_paths(ORG, META_REPO, "main")
    readme_ok = "README.md" in meta_paths
    if readme_ok:
        code, body = api(f"/repos/{ORG}/{META_REPO}/raw/README.md")
        if code == 200:
            low = body.lower()
            readme_ok = ("oep-meta" in low and "keelworks" in low and "monorepo" in low)
    check("G10 oep-meta README documents org/repo layout + monorepo", readme_ok)

    code, body = api(f"/repos/{ORG}/{CODE_REPO}/pulls?state=all&limit=50")
    found = False
    if code == 200 and isinstance(body, list):
        merged = [(p.get("title") or "") + "|" + str(p.get("merged")) for p in body if p.get("merged")]
        found = any("Monorepo" in t for t, m in [(x, True) for x in merged])
        pr_hist = f"merged PRs: {len(merged)}"
    else:
        pr_hist = "could not read PRs"
    check("G11 keelworks PR history preserved through transfer", found, pr_hist)

    failed = [c for c in checks if not c[1]]
    print()
    if failed:
        print(f"ACCEPTANCE FAILED: {len(failed)}/{len(checks)} checks failed")
        return 1
    print(f"ACCEPTANCE PASSED: {len(checks)}/{len(checks)} checks green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
