#!/usr/bin/env bash
# P8-M6-T03 verification probe: boot the real oep-pdm service on a loopback
# port, run the P8 GA gate (multi-user edit/publish/release + concurrent lock
# test) against it over real sockets with N real client processes, then stage
# the built binary on the NAS object store and re-verify by sha-256.
#
# Writes the committed report workstreams/P8-M6/proof/ga-gate-p8-multiuser.{json,md} and
# the single-file proof workstreams/P8-M6/proof/p8m6t03-ga-gate-proof.json.
set -euo pipefail
cd "$(dirname "$0")/.."
WT=$(pwd)
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WT/target}"
BIN="${1:-$CARGO_TARGET_DIR/debug/oep-pdm}"
PORT="${PORT:-28893}"
PERSIST=$(mktemp -d)
PROOF="$WT/workstreams/P8-M6/proof/p8m6t03-ga-gate-proof.json"
mkdir -p "$(dirname "$PROOF")"

if [ ! -x "$BIN" ]; then
  echo "FATAL: no oep-pdm binary at $BIN (build first: cargo build -p oep-pdm)" >&2
  exit 2
fi

# Proof harness drives the OPEN contract (controlled test use): explicit
# opt-out of the default P11-T03 boundary (SEC-AUTHZ-DEFAULT).
"$BIN" --addr "127.0.0.1:$PORT" --persist-dir "$PERSIST" --no-auth > /tmp/p8m6t03_ga.log 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true; rm -rf "$PERSIST"' EXIT

# Wait for health.
for _ in $(seq 1 100); do
  if python3 - "$PORT" <<'PY'
import sys, urllib.request
try:
    urllib.request.urlopen("http://127.0.0.1:%s/api/v1/health" % sys.argv[1], timeout=1)
    raise SystemExit(0)
except urllib.error.HTTPError:
    raise SystemExit(0)
except Exception:
    raise SystemExit(1)
PY
  then break; fi
  sleep 0.1
done

# Run the full gate against the live service (N real client processes).
export PYTHONPATH="$WT/python:$WT/python/oep_tooling"
N="${N_CLIENTS:-4}"
python3 -m oep_tooling.ga_gate_p8 \
  --base "http://127.0.0.1:$PORT/api/v1" \
  --binary "$BIN" \
  --out "$WT/workstreams/P8-M6/proof" \
  --n "$N"
GATE_CODE=$?

# Single-file proof (successor-verifiable in the repo).
python3 - "$PROOF" "$GATE_CODE" <<PY
import json, sys, glob
path, code = sys.argv[1], int(sys.argv[2])
rep = json.load(open("workstreams/P8-M6/proof/ga-gate-p8-multiuser.json"))
proof = {
    "task": "P8-M6-T03",
    "gate": "python/oep_tooling/ga_gate_p8.py",
    "contract": "workstreams/P8-M6/openapi-v1.yaml (+ additive gate surface)",
    "n_clients": len([r for r in rep["rows"] if r["leg"] == "multiuser"]),
    "summary": rep["summary"],
    "exit_code": code,
    "honest": {
        "exit0_iff_no_fail": (code == 1) == (rep["summary"].get("FAIL", 0) > 0),
        "rollup_scales": rep["summary"]["total"] == len(rep["rows"]),
    },
    "report": "workstreams/P8-M6/proof/ga-gate-p8-multiuser.json",
}
json.dump(proof, open(path, "w"), indent=2)
print("gate proof written:", path, "exit", code)
PY
echo "P8-M6-T03 GA gate proof: done (exit $GATE_CODE)"
