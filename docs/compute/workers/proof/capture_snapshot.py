#!/usr/bin/env python3
"""Capture a vault snapshot (catalogue + every revision + blob sha) for the
P9-M3-T02 proof. Uses urllib (stdlib) so no curl|python pipe.
"""
import json, sys, urllib.request

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8787"
OUT = sys.argv[2] if len(sys.argv) > 2 else "proof/vault-snapshot.json"

def get(path):
    with urllib.request.urlopen(BASE + path, timeout=20) as r:
        return json.load(r)

items = get("/api/v1/items")["items"]
snap = []
for it in items:
    revs = get(f"/api/v1/items/{it['id']}/revisions")
    for r in revs["revisions"]:
        snap.append({
            "item_id": it["id"],
            "name": it["name"],
            "kind": it["kind"],
            "revision": r["number"],
            "state": r["state"],
            "files": [
                {"filename": f["filename"], "format": f["format"],
                 "size": f["size"], "blob_sha256": f["blob_sha256"]}
                for f in r["files"]
            ],
        })
doc = {"format": "oep-vault-snapshot", "version": 1, "base": BASE, "entries": snap}
with open(OUT, "w") as fh:
    json.dump(doc, fh, indent=2)
    fh.write("\n")
print(f"wrote {OUT}: {len(snap)} revisions across {len(items)} items")
