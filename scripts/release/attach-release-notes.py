#!/usr/bin/env python3
"""Attach the generated changelog to the Gitea release for a release tag.

P12-T07. Called by .gitea/workflows/release-changelog.yaml after the
changelog for the release cut has been generated. Idempotent: if the release
already exists it is discovered and its notes updated; otherwise a release is
created for the tag. Always exits 0 so a flaky Gitea API call never fails the
CI job that already produced the changelog artifact (the markdown is printed
in the job log either way).

Usage:
  attach-release-notes.py <version> <changelog-file> <token> <api-url>

Env:
  GITHUB_REPOSITORY  owner/repo to attach to (fallback isolith/isolith)
"""
import json
import os
import sys
import urllib.error
import urllib.request

VERSION, CHANGELOG_FILE, TOKEN, API_URL = sys.argv[1:5]
REPO = os.environ.get("GITHUB_REPOSITORY", "isolith/isolith")

PRERELEASE_HINTS = ("pre", "rc", "alpha", "beta", "gamma")


def call(method, path, payload=None, raw=None):
    req = urllib.request.Request(API_URL.rstrip("/") + path, method=method)
    req.add_header("Authorization", "token " + TOKEN)
    req.add_header("Accept", "application/json")
    data = None
    if raw is not None:
        data = raw
        req.add_header("Content-Type", "text/markdown")
    elif payload is not None:
        data = json.dumps(payload).encode()
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data=data, timeout=60) as r:
            body = r.read().decode() or "{}"
            return r.status, json.loads(body)
    except urllib.error.HTTPError as e:
        return e.code, {}


def main():
    if not TOKEN or not API_URL or API_URL == "http:///api/v1":
        print("attach-release-notes: no GITHUB_TOKEN/GITHUB_API_URL; "
              "release notes stay in the artifact + job log (OK, non-fatal)")
        return 0
    with open(CHANGELOG_FILE, encoding="utf-8") as fh:
        body = fh.read()
    tag = VERSION if VERSION.startswith("v") else "v" + VERSION
    prerelease = any(h in tag.lower() for h in PRERELEASE_HINTS)

    status, rel = call("GET", "/repos/%s/releases/tags/%s" % (REPO, tag))
    if status == 200:
        rel_id = rel.get("id")
        print("attach-release-notes: release %s exists (id=%s)" % (tag, rel_id))
    elif status == 404:
        status, rel = call(
            "POST",
            "/repos/%s/releases" % REPO,
            {
                "tag_name": tag,
                "name": tag,
                "body": body,
                "draft": False,
                "prerelease": prerelease,
            },
        )
        rel_id = rel.get("id")
        print("attach-release-notes: created release %s (id=%s, status=%s)"
              % (tag, rel_id, status))
    else:
        print("attach-release-notes: GET release failed status=%s; giving up" % status)
        return 0

    if not rel_id:
        print("attach-release-notes: no release id resolved; skipping attach")
        return 0
    st, _ = call("PATCH", "/repos/%s/releases/%s" % (REPO, rel_id), {"body": body})
    print("attach-release-notes: release notes body PATCH -> %s" % st)
    fname = CHANGELOG_FILE.split("/")[-1]
    st2, _ = call(
        "POST",
        "/repos/%s/releases/%s/assets?name=%s" % (REPO, rel_id, fname),
        raw=body.encode(),
    )
    print("attach-release-notes: asset upload %s -> %s" % (fname, st2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
