#!/usr/bin/env bash
# P9-M5-T02 GA gate proof: run the real 100-job render/convert wave across the
# real GB10 fleet (3+ nodes), zero manual intervention, every output
# hash-verified in the P8 vault.
#
# Usage: scripts/compute-ga-gate-proof.sh <fleet.json>
#   fleet.json         real fleet (default workstreams/compute/ops/fleet.example.json)
#
# Stages: builds the gate binaries, deploys the node agent to the remote
# fleet members (SSH), boots the shared vault on the coordinator reachable on
# the LAN, generates the 100-job wave, runs the driver, verifies hashes, and
# writes workstreams/compute/ops/proof/ga-gate-p9.{json,md} + ga-gate-p9-wave.json.

set -euo pipefail
cd "$(dirname "$0")/.."
WT=$(pwd)
FLT="${1:-$WT/workstreams/compute/ops/fleet.example.json}"
PROOF_DIR="$WT/workstreams/compute/ops/proof"
mkdir -p "$PROOF_DIR"
PORT="${PORT:-28913}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WT/target}"

# 1. Build the gate binaries.
echo "[1/5] building oep-gate binaries..."
cargo build --offline -p oep-gate --bins
BIN="$CARGO_TARGET_DIR/debug/oep-ga-gate"
AGENT="$CARGO_TARGET_DIR/debug/oep-ga-node"

# 2. Extract the coordinator node id + gpu model from the fleet.
COORD=$(python3 -c "import json;print(json.load(open('$FLT'))['coordinator'])")
GPU_MODEL=$(python3 -c "import json;d=json.load(open('$FLT'));print(d.get('gpu_model',''))")
echo "[2/5] coordinator=$COORD gpu_model=$GPU_MODEL fleet=$FLT"

# 3. Deploy the node agent to every remote node (zero manual steps: scripted).
echo "[3/5] deploying oep-ga-node to remote fleet members..."
python3 - "$FLT" "$AGENT" <<'PY'
import json, subprocess, sys
flt, agent = sys.argv[1], sys.argv[2]
fleet = json.load(open(flt))
for n in fleet["nodes"]:
    if n.get("local") or not n.get("host"):
        print("  local node", n["id"], "skipped (in-process)")
        continue
    target_dir = n.get("agent_dir", "")
    host, user = n["host"], n.get("user", "r1a")
    mk = subprocess.run(["ssh","-o","BatchMode=yes","-o","StrictHostKeyChecking=accept-new",
                         f"{user}@{host}","mkdir -p",target_dir], capture_output=True, text=True)
    if mk.returncode != 0:
        print(f"  WARN: cannot mkdir {host}:{target_dir}: {mk.stderr.strip()[:120]}")
        continue
    cp = subprocess.run(["scp","-q","-o","BatchMode=yes","-o","StrictHostKeyChecking=accept-new",
                         agent, f"{user}@{host}:{target_dir}/oep-ga-node"], capture_output=True, text=True)
    print(f"  deployed oep-ga-node -> {host}:{target_dir}/oep-ga-node rc={cp.returncode}")
PY

# 4. Boot the shared vault on 0.0.0.0 (LAN-reachable so remote nodes hit it).
echo "[4/5] booting shared P8 vault on 0.0.0.0:$PORT..."
PERSIST=$(mktemp -d -t p9ga-proof-XXXXXX)
# Proof harness drives the OPEN contract (controlled test use): explicit
# opt-out of the default P11-T03 boundary (SEC-AUTHZ-DEFAULT).
"$CARGO_TARGET_DIR/debug/oep-pdm" --addr "0.0.0.0:$PORT" --persist-dir "$PERSIST" --no-auth \
    > /tmp/p9ga-proof-vault.log 2>&1 &
VPID=$!
trap 'kill $VPID 2>/dev/null || true; rm -rf "$PERSIST"' EXIT
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

# 5. Run the GA gate (driver + python harness both write the proof).
echo "[5/5] running the 100-job GA gate wave..."
export PYTHONPATH="$WT/python:$WT/python/oep_tooling"
export KEELWORKS_ROOT="$WT"
python3 -m oep_tooling.ga_gate_p9 \
    --repo "$WT" \
    --fleet "$FLT" \
    --jobs-count 100 \
    --driver "$BIN" \
    --agent "$AGENT" \
    --pdm "$CARGO_TARGET_DIR/debug/oep-pdm" \
    --persist "$PERSIST" \
    --port "$PORT" \
    --report "$PROOF_DIR/ga-gate-p9"
RC=$?
echo "P9 GA gate proof: exit $RC (0 = PASS)"
exit $RC
