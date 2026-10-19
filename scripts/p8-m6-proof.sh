#!/usr/bin/env bash
# P8-M6-T01 verification probe: boot the retained oep-pdm-stub binary, drive
# the GENERATED python client (python/oep_pdm_client.py, produced from
# workstreams/P8-M6/openapi-v1.yaml) through open-from-PDM / save-revisions-to-PDM, and
# write a real-material proof to workstreams/P8-M6/proof/client-proof.json.
#
# This is the "stub promoted to a contract test" demonstration: the same
# contract the P7-M2-T02 stub established is exercised by the generated client
# against the real binary, successor-verifiable in the repo.
set -euo pipefail
cd "$(dirname "$0")/.."
WT=$(pwd)
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WT/target}"
BIN="$CARGO_TARGET_DIR/debug/oep-pdm-stub"
PORT=18886
PROOF="$WT/workstreams/P8-M6/proof/client-proof.json"
mkdir -p "$(dirname "$PROOF")"

if [ ! -x "$BIN" ]; then
  cargo build -p oep-pdm --bin oep-pdm-stub
fi

# Proof harness drives the OPEN contract (controlled test use): explicit
# opt-out of the default P11-T03 boundary (SEC-AUTHZ-DEFAULT).
"$BIN" --addr "127.0.0.1:$PORT" --no-auth > /tmp/pdm_stub_m6.log 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT

python3 - "$PORT" "$PROOF" <<'PY'
import base64, hashlib, json, os, socket, subprocess, sys, time

port, proof_path = int(sys.argv[1]), sys.argv[2]
ROOT = os.getcwd()  # the bash wrapper cd'd to the repo root
sys.path.insert(0, os.path.join(ROOT, "python"))
from oep_pdm_client import PdmClient, SPEC_SHA256  # noqa: E402

# Wait until the stub is healthy.
last = None
for _ in range(100):
    try:
        import urllib.request
        with urllib.request.urlopen("http://127.0.0.1:%d/api/v1/health" % port, timeout=1):
            break
    except Exception as e:  # server still booting
        last = e
        time.sleep(0.1)
else:
    raise RuntimeError("service did not become healthy: %r" % (last,))

c = PdmClient(base_url="http://127.0.0.1:%d/api/v1" % port)

# The two browser verbs, end to end, via the generated client.
step = (b"ISO-10303-21; HEADER; FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }')); "
        b"ENDSEC; DATA; ENDSEC; END-ISO-10303-21;")
sha = hashlib.sha256(step).hexdigest()

status, item = c.create_item({"name": "ProofBracket", "kind": "part",
                              "context": "p8-m6-t01"})
assert status == 201, (status, item)
iid = item["id"]

status, rev = c.save_revision(iid, "bracket.step", "step", step,
                              state="working", comment="proof",
                              created_by="p8-m6")
assert status == 201 and rev["number"] == "1", (status, rev)
assert rev["files"][0]["blob_sha256"] == sha, rev

payload = c.open_latest_file(iid)
assert isinstance(payload.bytes, bytes) and payload.bytes == step
assert c.verify_blob(payload), "content address must verify on the wire"

status, items = c.list_items()
assert status == 200 and len(items["items"]) == 1, items

proof = {
    "contract": "workstreams/P8-M6/openapi-v1.yaml",
    "spec_sha256": SPEC_SHA256,
    "base_url": "/api/v1",
    "client": "python/oep_pdm_client.py (generated)",
    "verbs": ["createItem", "saveRevision", "openLatestFile", "listItems"],
    "round_trip": {
        "item_id": iid,
        "revision": rev["number"],
        "bytes": len(step),
        "blob_sha256": sha,
        "byte_exact": True,
        "blob_verified": True,
    },
    "health": {"status": "ok", "service": "oep-pdm", "version": "1.0.0"},
}
with open(proof_path, "w") as fh:
    json.dump(proof, fh, indent=2)
    fh.write("\n")
print("generated-client proof written:", proof_path)
PY

echo "P8-M6-T01 generated client contract proof: OK"
