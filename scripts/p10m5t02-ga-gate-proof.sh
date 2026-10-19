#!/usr/bin/env bash
# P10-M5-T02 -- P10 GA gate live proof driver.
#
# Runs the measured P10 GA gate against the two approved machines (decision
# #9): machine A = U1 (the local host), machine B = a U1/VM-class host reached
# over SSH (default: the keelwb2 QEMU/KVM VM on U1; set MACHINE_B_HOST/PORT to
# point at any approved second host, e.g. op@192.168.0.43 for the garage PC).
#
# What it does, in the exact shape of the P8/P9 proof scripts:
#   1. asserts docker on the host, and that this is the RC head (no unmerged
#      conflict markers anywhere).
#   2. ships the repo (deploy dir + workspace source the compose build needs)
#      to machine B verbatim (documented physical-transfer path).
#   3. runs python/oep_tooling/ga_gate_p10.py with --docker and machine B:
#        DOCS     checklist self-consistent (committed)
#        INSTALL-A  clean-state one-command install on U1 -> 3 health green
#        INSTALL-B  clean-state one-command install on machine B -> 3 health green
#        STACK    PDM + viewer + worker (worker bins in the shipped image)
#        AIRGAP   signed offline bundle install on a --network none target
#                 (sig + checksums verified, docker load only, update off)
#        UPDATE   update checker off by default / non-blocking / no phone-home
#   4. writes workstreams/P10-M5/proof/ga-gate-p10-m5t02.{json,md}.
#
# Exit code 0 iff the gate reports no FAIL row (same contract as P2/P3/P4/P8/P9).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
ROOT="$PWD"

MACHINE_B_HOST="${MACHINE_B_HOST:-op@127.0.0.1}"
MACHINE_B_PORT="${MACHINE_B_PORT:-2229}"

echo "[proof] P10-M5-T02 GA gate (two-machine fresh-install walkthrough)"
echo "[proof] machine A = U1 (local); machine B = ${MACHINE_B_HOST}:${MACHINE_B_PORT}"

# 0. host prerequisites
command -v docker >/dev/null || { echo "[proof] docker required on host"; exit 1; }
docker info >/dev/null 2>&1 || { echo "[proof] docker daemon not running"; exit 1; }
if git grep -n -E '^(<<<<<<<|=======|>>>>>>>)(' -- "$ROOT" ':!Cargo.lock' >/dev/null 2>&1 \
   || grep -qE '^(<<<<<<<|=======|>>>>>>>)' "$ROOT/Cargo.lock" 2>/dev/null; then
  echo "[proof] unmerged conflict markers present on this head; aborting"; exit 1
fi

# 1. machine B reachable + repo present there (documented transfer path)
echo "[proof] checking machine B reachability"
if ! ssh -o BatchMode=yes -o ConnectTimeout=15 -o StrictHostKeyChecking=accept-new \
     -p "$MACHINE_B_PORT" "$MACHINE_B_HOST" 'true'; then
  echo "[proof] machine B unreachable (define MACHINE_B_HOST/PORT)"; exit 1
fi

echo "[proof] syncing the RC tree (deploy + workspace build inputs) to machine B"
src="$ROOT/"
# exclude heavy/derived artifacts; include everything the docs + compose build need
rsync -az --delete \
  -e "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -p $MACHINE_B_PORT" \
  --exclude '.git/' --exclude 'target/' --exclude '.worktrees/' \
  --exclude 'keelworksb2.qcow2' --exclude '*.typescript' --exclude '*.timing' \
  --exclude '*.mp4' --exclude 'workstreams/P10-M5/proof/video/' \
  "$src" "$MACHINE_B_HOST:~/keelworks/"

# 2. run the measured gate
echo "[proof] running ga_gate_p10 (docker + machine A + machine B)"
set +e
PYTHONPATH=python:python/oep_tooling python3 -m oep_tooling.ga_gate_p10 \
  --repo "$ROOT" \
  --report "$ROOT/workstreams/P10-M5/proof/ga-gate-p10-m5t02" \
  --docker \
  --machine-b-host "$MACHINE_B_HOST" \
  --machine-b-port "$MACHINE_B_PORT"
rc=$?
set -e

echo "[proof] -------- P10 GA gate report --------"
sed -n '1,40p' "$ROOT/workstreams/P10-M5/proof/ga-gate-p10-m5t02.md"
echo "[proof] gate exit code: $rc"
exit "$rc"
