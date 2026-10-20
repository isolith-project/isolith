#!/usr/bin/env bash
# P8-M1-T01 verification probe: boot the axum oep-pdm service on a fixed
# port, hit the contract endpoints with Python's stdlib urllib (no shell curl),
# write the health proof to workstreams/pdm/service/proof/health.json, and exercise the core
# open/save round trip. Real material for the ADR + service skeleton.
set -euo pipefail
cd "$(dirname "$0")/.."
WT=$(pwd)
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WT/target}"
BIN="$CARGO_TARGET_DIR/debug/oep-pdm"
PORT=18787
PROOF="$WT/workstreams/pdm/service/proof/health.json"
mkdir -p "$(dirname "$PROOF")"

# Proof harness drives the OPEN contract (controlled test use): explicit
# opt-out of the default P11-T03 boundary (SEC-AUTHZ-DEFAULT).
"$BIN" --addr "127.0.0.1:$PORT" --no-auth > /tmp/pdm_service.log 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT

python3 - "$PORT" "$PROOF" <<'PY'
import json, sys, time, urllib.request
port, proof_path = int(sys.argv[1]), sys.argv[2]
base = "http://127.0.0.1:%d/api/v1" % port

def req(method, path, body=None, ct="application/json"):
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = ct
    r = urllib.request.Request(base + path, data=data, headers=headers, method=method)
    with urllib.request.urlopen(r, timeout=10) as resp:
        raw = resp.read()
        try:
            return resp.status, json.loads(raw)
        except ValueError:
            return resp.status, raw

# Wait for liveness.
last = None
for _ in range(100):
    try:
        code, h = req("GET", "/health")
        if code == 200:
            break
    except Exception as e:  # server still booting
        last = e
        time.sleep(0.1)
else:
    raise RuntimeError("service did not become healthy: %r" % (last,))

print("health:", h)
assert h == {"status": "ok", "service": "oep-pdm", "version": "1.0.0"}, h
with open(proof_path, "w") as fh:
    json.dump(h, fh, indent=2)
    fh.write("\n")

# Catalog is empty, create an item, save a revision, download it.
code, items = req("GET", "/items")
assert code == 200 and items["total"] == 0, (code, items)
code, item = req("POST", "/items", {"name": "ProofPart", "kind": "part",
                                    "context": "p8-m1"})
assert code == 201, (code, item)
iid = item["id"]
import base64, hashlib
blob = b"oep-pdm axum skeleton proof payload"
sha = hashlib.sha256(blob).hexdigest()
code, rev = req("POST", "/items/%s/revisions" % iid,
                {"state": "working", "file": {"filename": "proof.txt",
                                              "format": "step",
                                              "content": base64.b64encode(blob).decode()}})
assert code == 201 and rev["number"] == "1", (code, rev)
assert rev["files"][0]["blob_sha256"] == sha, rev
code, latest = req("GET", "/items/%s/file/latest" % iid)
assert code == 200 and isinstance(latest, bytes) and latest == blob, (code, latest)
print("open/save round trip byte-exact on the axum skeleton: OK")
print("proof written:", proof_path)
PY
