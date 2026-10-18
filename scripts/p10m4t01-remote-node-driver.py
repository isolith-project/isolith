#!/usr/bin/env python3
"""P10-M4-T01 remote-node driver — runs ON the remote node (garage PC, WoL)
against the U1 LAN vault. Prints one classified leg row per line as JSON.

This is the "verified live across the two approved hosts" half of the card:
the garage PC is the remote node A (sync client replica); U1 hosts the vault
(node B in the driver's view). Both directions of replication plus the
deterministic concurrent-edit conflict are exercised over the real wire.

Usage (from U1 orchestrator):
  python3 scripts/p10m4t01-remote-node-driver.py --vault http://192.168.0.67:18789
  (exit 0 iff no FAIL row)
"""
import argparse
import base64
import hashlib
import json
import sys
import urllib.error
import urllib.request

def sha(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()

def upload(content: bytes) -> dict:
    return {"filename": "part.step", "format": "step",
            "content_type": "text/plain",
            "content": base64.b64encode(content).decode()}

def req(base, method, path, body=None):
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    r = urllib.request.Request(base.rstrip("/") + path, data=data,
                               headers=headers, method=method)
    try:
        with urllib.request.urlopen(r, timeout=15) as resp:
            raw = resp.read()
            try:
                return resp.status, json.loads(raw)
            except ValueError:
                return resp.status, raw
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except ValueError:
            return e.code, raw

def row(rid, leg, status, detail=""):
    print(json.dumps({"id": rid, "leg": leg, "status": status,
                      "detail": detail}, sort_keys=True))

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--vault", default="http://192.168.0.67:18789/api/v1")
    args = ap.parse_args()
    base = args.vault.rstrip("/")
    fail = False

    # health / reachability
    try:
        code, h = req(base, "GET", "/health")
        up = code == 200
    except Exception as e:  # noqa: BLE001
        up = False
        h = repr(e)
    row("remote-health", "replicate", "PASS" if up else "SKIP",
        "code=%r %r" % (code if up else "?", h))
    if not up:
        return 1  # cannot replicate against an unreachable vault

    # ---- Replicate leg (remote <-> LAN) ----
    code, item = req(base, "POST", "/items",
                     {"name": "RemoteNodePart", "kind": "part",
                      "context": "p10-m4-t01-live"})
    iid = item["id"]
    base_bytes = b"STEP base\nDIM A=10\nHOLE B=5\n"
    req(base, "POST", "/items/%s/revisions" % iid,
        {"state": "working", "file": upload(base_bytes)})
    code, rev = req(base, "GET", "/items/%s/revisions" % iid)
    base_rev = rev["revisions"][-1]["number"]
    base_hash = rev["revisions"][-1]["files"][0]["blob_sha256"]

    # LAN node (U1) makes an edit; remote node (garage) must see it.
    lan_edit = b"STEP base\nDIM A=12\nHOLE B=5\n"
    req(base, "POST", "/items/%s/revisions" % iid,
        {"state": "working", "created_by": "u1", "file": upload(lan_edit)})
    code, latest = req(base, "GET", "/items/%s/file/latest" % iid)
    ok = code == 200 and isinstance(latest, bytes) and \
         sha(latest) == sha(lan_edit)
    row("remote-sees-lan-edit", "replicate", "PASS" if ok else "FAIL",
        "u1-rev blob=%s.. vs expected %s.." % (sha(latest)[:10] if isinstance(latest, bytes) else "?", sha(lan_edit)[:10]))
    fail = fail or not ok

    # Remote node records an offline edit and syncs it back to U1's vault.
    remote_edit = b"STEP base\nDIM A=12\nHOLE B=8\n"
    code, push = req(base, "POST", "/sync/push",
                     {"item_id": iid, "base_revision": str(int(base_rev) + 1),
                      "base_blob_sha256": sha(lan_edit),
                      "created_by": "garage", "file": upload(remote_edit)})
    ok = code == 201 and push.get("status") == "synced"
    row("remote-edit-reaches-lan", "replicate", "PASS" if ok else "FAIL",
        "push code=%s status=%s" % (code, push.get("status")))
    fail = fail or not ok

    # ---- Conflict leg (deterministic) ----
    code, item2 = req(base, "POST", "/items",
                      {"name": "RemoteConflict", "kind": "part",
                       "context": "p10-m4-t01-live"})
    iid2 = item2["id"]
    cbase = b"CRIT=42\nKEEP=yes\n"
    req(base, "POST", "/items/%s/revisions" % iid2,
        {"state": "working", "file": upload(cbase)})
    code, rev2 = req(base, "GET", "/items/%s/revisions" % iid2)
    crev = rev2["revisions"][-1]["number"]
    chash = rev2["revisions"][-1]["files"][0]["blob_sha256"]

    ours = b"CRIT=44\nKEEP=yes\n"           # remote (garage) offline edit
    theirs = b"CRIT=42\nKEEP=theirs\n"      # LAN (U1) advances head meanwhile
    req(base, "POST", "/items/%s/revisions" % iid2,
        {"state": "working", "created_by": "u1", "file": upload(theirs)})
    code, push2 = req(base, "POST", "/sync/push",
                      {"item_id": iid2, "base_revision": crev,
                       "base_blob_sha256": chash, "created_by": "garage",
                       "file": upload(ours)})
    diverged = code == 409 and push2.get("status") == "conflict" and \
               push2.get("sync_id")
    row("remote-divergence-detected", "conflict",
        "PASS" if diverged else "FAIL",
        "push code=%s status=%s" % (code, push2.get("status")))
    fail = fail or not diverged
    if diverged:
        sid = push2["sync_id"]
        code, view = req(base, "GET", "/sync/conflicts/%s" % sid)
        snap = code == 200 and view["base"]["blob_sha256"] == chash and \
               view["ours"]["blob_sha256"] == sha(ours) and \
               view["theirs"]["blob_sha256"] == sha(theirs)
        row("remote-three-way-snapshots", "conflict",
            "PASS" if snap else "FAIL",
            "base=%.10s ours=%.10s theirs=%.10s" % (
                view["base"]["blob_sha256"], view["ours"]["blob_sha256"],
                view["theirs"]["blob_sha256"]))
        fail = fail or not snap
        code, resolved = req(base, "POST", "/sync/conflicts/%s/resolve" % sid,
                             {"choice": "ours", "merged": upload(ours)})
        ok = code == 201 and resolved["files"][0]["blob_sha256"] == sha(ours)
        row("remote-resolution-hash-verified", "conflict",
            "PASS" if ok else "FAIL", "resolved rev=%s" % resolved.get("number"))
        fail = fail or not ok

    return 1 if fail else 0

if __name__ == "__main__":
    sys.exit(main())
