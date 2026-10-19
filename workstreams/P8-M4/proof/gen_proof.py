#!/usr/bin/env python3
"""P8-M4-T01 proof generator.

Boots the built oep-web binary on an ephemeral port, seeds a fresh vault with
the reference vertical fixture (golden bracket assembly at two revisions), and
captures the four UI surfaces the card delivers:

  explorer.html     GET /                        item explorer
  bom.html          GET /ui/items/{id}/bom?rev=1 BOM tree (relationship graph)
  preview-1.png     GET /ui/items/{id}/preview/1.png  viewer embed rev 1
  preview-2.png     GET /ui/items/{id}/preview/2.png  viewer embed rev 2
  compare.html      GET /ui/compare?item={id}&a=1&b=2 compare view

Run: python3 workstreams/P8-M4/proof/gen_proof.py   (after `cargo build -p oep-web`)
Writes the artifacts into workstreams/P8-M4/proof/ (gitignored until committed).
"""
import base64
import json
import os
import subprocess
import sys
import time
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HERE = os.path.dirname(os.path.abspath(__file__))
PORT = 8790
BASE = f"http://127.0.0.1:{PORT}"
GOLDEN = os.path.join(ROOT, "crates", "oep-geo", "tests", "golden",
                      "example-bracket.oepgeo.json")


def api(method, path, body=None, raw=False):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=15) as r:
        payload = r.read()
        if raw:
            return payload
        try:
            return json.loads(payload)
        except json.JSONDecodeError:
            return payload.decode("utf-8", "replace")


def variant_doc(golden: str) -> str:
    """One fewer fastener and a renamed top assembly: differs from golden."""
    doc = json.loads(golden)
    for a in doc["assemblies"]:
        if a["id"] == "oep-assembly-fastener-set":
            a["instances"].pop()
        if a["id"] == "oep-assembly-bracket-assembly":
            a["name"] = "bracket-assembly-rev2"
    return json.dumps(doc, indent=2)


def main() -> int:
    # 1) Boot the server with a throwaway persist dir.
    persist = os.path.join(HERE, ".tmp-persist")
    if os.path.isdir(persist):
        import shutil
        shutil.rmtree(persist)
    os.makedirs(persist, exist_ok=True)
    srv = subprocess.Popen(
        [os.path.join(ROOT, "target", "debug", "oep-web"),
         "--addr", f"127.0.0.1:{PORT}", "--persist-dir", persist],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        time.sleep(1.2)
        # 2) Seed the reference vertical fixture.
        with open(GOLDEN, encoding="utf-8") as fh:
            golden = fh.read()
        item = api("POST", "/api/v1/items", {
            "name": "kw-bracket-assembly", "kind": "assembly",
            "description": "reference vertical fixture",
            "owner": "p8", "context": "w2",
        })
        iid = item["id"]
        for comment, body in (
            ("golden revision 1", golden),
            ("variant revision 2", variant_doc(golden)),
        ):
            api("POST", f"/api/v1/items/{iid}/revisions", {
                "state": "working", "comment": comment, "created_by": "p8-web-ui",
                "file": {"filename": "bracket.oepgeo.json", "format": "oep-geo",
                         "content_type": "application/json",
                         "content": base64.b64encode(body.encode()).decode()},
            })
        # 3) Capture the four surfaces.
        for name, path in (("explorer.html", "/"),
                           ("bom.html", f"/ui/items/{iid}/bom?rev=1"),
                           ("compare.html", f"/ui/compare?item={iid}&a=1&b=2")):
            with open(os.path.join(HERE, name), "w", encoding="utf-8") as fh:
                fh.write(api("GET", path))
        for name, rev in (("preview-1.png", "1"), ("preview-2.png", "2")):
            with open(os.path.join(HERE, name), "wb") as fh:
                fh.write(api("GET", f"/ui/items/{iid}/preview/{rev}.png", raw=True))
        print(f"proof artifacts written to {HERE}")
        return 0
    finally:
        srv.terminate()
        srv.wait(timeout=10)


if __name__ == "__main__":
    sys.exit(main())
