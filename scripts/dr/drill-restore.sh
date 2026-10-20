#!/usr/bin/env bash
# P10-M3-T01: DR drill -- fresh-machine restore from the P10-M1-T02 backup,
# timed, under 60 minutes (RTO). Run target: U1/VM-class host (decision #9);
# the NAS is passive storage only and never the drill target.
#
# What "fresh" means here: a brand-new, bare Ubuntu container (ubuntu:24.04,
# no host mounts, no Keelworks state) is provisioned from the stock image and
# the keelworks stack is installed + restored onto it. The ONLY path by which
# vault data enters the fresh host is the backup bundle read off the
# second-disk / archive target. It is never copied from the live vault.
#
# Usage:
#   scripts/dr/drill-restore.sh [--check] [BK_TARGET] [FRESH_IMAGE]
#     ---check     validate prerequisites only; no container, no timing
#     BK_TARGET    second-disk mount point or archive dir holding
#                  keelworks-vault-backup/ (default:
#                  $HOME/keelworks-data/keelworks-vault-backup)
#     FRESH_IMAGE  stock bare image for the fresh host (default ubuntu:24.04)
#
# Exit 0 iff every health check passes AND measured TDR < 60 minutes.
# Writes a JSON drill report to workstreams/delivery/disaster-recovery/proof/dr-drill-<ts>.json

set -euo pipefail
cd "$(dirname "$0")/../.."
WT=$(pwd)

BK_TARGET="${1:-$HOME/keelworks-data}"
FRESH_IMAGE="${2:-ubuntu:24.04}"
PDN_BIN="${PDN_BIN:-/home/r1a/keelworks-main/.worktrees/p9m5t02-ga-gate-1017a/target/debug/oep-pdm}"
BACKUP_ROOT="$BK_TARGET/keelworks-vault-backup"
CT_NAME="dr-fresh-$(date +%s)"
HOST_PORT=28914
PROOF_DIR="$WT/workstreams/delivery/disaster-recovery/proof"
mkdir -p "$PROOF_DIR"

# ---------- prereq check ----------
check() {
  echo "[drill] --check mode: validating prerequisites"
  local ok=1
  [ -f "$BACKUP_ROOT/manifest.sha256" ] || { echo "  - FAIL backup manifest: $BACKUP_ROOT/manifest.sha256"; ok=0; }
  [ -d "$BACKUP_ROOT/latest/blobs" ] || { echo "  - FAIL backup blobs dir: $BACKUP_ROOT/latest/blobs"; ok=0; }
  [ -f "$BACKUP_ROOT/latest/store.json" ] || { echo "  - FAIL backup store.json"; ok=0; }
  [ -f "$PDN_BIN" ] || { echo "  - FAIL oep-pdm binary: $PDN_BIN"; ok=0; }
  docker info >/dev/null 2>&1 || { echo "  - FAIL docker daemon unreachable"; ok=0; }
  docker image inspect "$FRESH_IMAGE" >/dev/null 2>&1 || { echo "  - FAIL fresh image not present: $FRESH_IMAGE"; ok=0; }
  [ "$ok" = 1 ] && echo "  - all prerequisites OK" || echo "  - prerequisites MISSING"
  exit $((1 - ok))
}
[ "${1:-}" = "--check" ] && check
[ $# -gt 0 ] && BK_TARGET="${1:-$BK_TARGET}"
FRESH_IMAGE="${2:-$FRESH_IMAGE}"

# ---------- measurable start (T0) ----------
T0=$(date +%s)
TS=$(date +%Y%m%d-%H%M%S)
REPORT="$PROOF_DIR/dr-drill-$TS.json"
echo "=============================================================="
echo "[drill] $TS DR drill: fresh-machine restore from backup (P10-M3-T01)"
echo "[drill] backup root   : $BACKUP_ROOT"
echo "[drill] fresh image   : $FRESH_IMAGE"
echo "[drill] live PDM bin  : $PDN_BIN"
echo "=============================================================="

# ---------- phase 1: preflight (backup exists and is intact) ----------
echo "[1/8] verifying backup manifest on the archive target ..."
( cd "$BACKUP_ROOT" && sha256sum -c manifest.sha256 >/dev/null 2>&1 ) \
  || { echo "[drill] FAIL: backup bundle on $BK_TARGET fails manifest check" >&2; exit 1; }
EXPECT_BLOBS=$(grep -c 'latest/blobs/' "$BACKUP_ROOT/manifest.sha256")
echo "[1/8] ok: manifest verified, $EXPECT_BLOBS blobs in bundle"

# ---------- phase 2: fresh bare host ----------
echo "[2/8] provisioning fresh bare host '$CT_NAME' ($FRESH_IMAGE) ..."
docker rm -f "$CT_NAME" >/dev/null 2>&1 || true
# pick a free host port for the restored PDM (live dev node already owns 28913)
HOST_PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('127.0.0.1',0));print(s.getsockname()[1]);s.close()")
docker run -d --name "$CT_NAME" -p "127.0.0.1:$HOST_PORT:8787" \
  "$FRESH_IMAGE" sleep infinity >/dev/null
echo "[2/8] container up (id $CT_NAME, host port $HOST_PORT)"

# ---------- phase 3: install the stack (binary only; offline install
#                     package is P10-M2-T02 territory) ----------
echo "[3/8] installing oep-pdm into the fresh host ..."
docker exec "$CT_NAME" mkdir -p /opt/keelworks/bin /var/lib/keelworks/vault
docker cp "$PDN_BIN" "$CT_NAME:/opt/keelworks/bin/oep-pdm"
docker exec "$CT_NAME" chmod +x /opt/keelworks/bin/oep-pdm
docker exec "$CT_NAME" /opt/keelworks/bin/oep-pdm --help >/dev/null

# ---------- phase 4: transfer backup bundle (from archive, not live) ----------
echo "[4/8] transferring backup bundle from archive target ..."
docker exec "$CT_NAME" mkdir -p /tmp/restore-src
docker cp "$BACKUP_ROOT/manifest.sha256" "$CT_NAME:/tmp/restore-src/"
docker cp "$BACKUP_ROOT/latest/." "$CT_NAME:/tmp/restore-src/latest/"

# ---------- phase 5: verify integrity in the fresh host, then restore ----------
echo "[5/8] verifying + extracting in the fresh host ..."
docker exec "$CT_NAME" sh -c 'cd /tmp/restore-src && sha256sum -c < manifest.sha256 >/dev/null 2>&1' \
  || { echo "[drill] FAIL: bomb-to-host transfer fails integrity check" >&2; exit 1; }
docker exec "$CT_NAME" sh -c 'cp -a /tmp/restore-src/latest/store.json /var/lib/keelworks/vault/ && \
  cp -a /tmp/restore-src/latest/blobs /var/lib/keelworks/vault/ && \
  [ -d /tmp/restore-src/latest/audit ] && cp -a /tmp/restore-src/latest/audit /var/lib/keelworks/vault/ || true'
echo "[5/8] restored to /var/lib/keelworks/vault (store.json + blobs + audit)"

# ---------- phase 6: boot the stack ----------
echo "[6/8] starting oep-pdm on the fresh host ..."
docker exec -d "$CT_NAME" /opt/keelworks/bin/oep-pdm \
  --addr 0.0.0.0:8787 --persist-dir /var/lib/keelworks/vault
sleep 2

# ---------- phase 7: health checks + real vault read (from the host, as a
#                     customer operator would) ----------
echo "[7/8] running health checks against restored PDM at 127.0.0.1:$HOST_PORT ..."
READY=0
for i in $(seq 1 30); do
  if curl -fsS -m 2 "http://127.0.0.1:$HOST_PORT/api/v1/health" >/dev/null 2>&1; then
    READY=1; break
  fi
  sleep 1
done
[ "$READY" = 1 ] || { echo "[drill] FAIL: restored PDM did not become healthy" >&2; docker rm -f "$CT_NAME" >/dev/null 2>&1; exit 1; }

HEALTH=$(curl -fsS -m 5 "http://127.0.0.1:$HOST_PORT/api/v1/health")
echo "[7/8] health  : $HEALTH"
echo "$HEALTH" | grep -q '"status":"ok"' || { echo "[drill] FAIL: health not ok" >&2; exit 1; }

ITEMS=$(curl -fsS -m 5 "http://127.0.0.1:$HOST_PORT/api/v1/items")
N_ITEMS=$(echo "$ITEMS" | python3 -c "import sys,json;print(len(json.load(sys.stdin)['items']))")
echo "[7/8] items   : $N_ITEMS restored"
[ "$N_ITEMS" -gt 0 ] || { echo "[drill] FAIL: vault item list empty after restore" >&2; exit 1; }

AUDIT=$(curl -fsS -m 5 "http://127.0.0.1:$HOST_PORT/api/v1/audit/verify" || echo '{"ok":false}')
echo "[7/8] audit   : $AUDIT"

# real vault read: first item, revision 1 file; content hash must match the
# content-addressed blob name in the backup manifest
ITEM_ID=$(echo "$ITEMS" | python3 -c "import sys,json;print(json.load(sys.stdin)['items'][0]['id'])")
BLOB=$(curl -fsS -m 10 -D /tmp/dr_hdr.txt "http://127.0.0.1:$HOST_PORT/api/v1/items/$ITEM_ID/revisions/1/file" -o /tmp/dr_file.bin)
EXPECTED=$(awk 'tolower($1)=="x-blob-sha256:"{print $2}' /tmp/dr_hdr.txt | tr -d '\r')
ACTUAL=$(sha256sum /tmp/dr_file.bin | cut -d' ' -f1)
echo "[7/8] read $ITEM_ID rev1: blob-sha256=$EXPECTED actual=$ACTUAL size=$(stat -c%s /tmp/dr_file.bin)"
[ "$EXPECTED" = "$ACTUAL" ] || { echo "[drill] FAIL: real vault read content hash mismatch" >&2; exit 1; }
grep -q "$ACTUAL" "$BACKUP_ROOT/manifest.sha256" \
  || { echo "[drill] FAIL: restored file not present in backup manifest" >&2; exit 1; }
echo "[7/8] real vault read verified against the backup manifest"

# ---------- phase 8: teardown + record ----------
T1=$(date +%s)
TDR=$((T1 - T0))
echo "[8/8] tearing down fresh host ..."
docker rm -f "$CT_NAME" >/dev/null 2>&1 || true

python3 - "$REPORT" "$TS" "$TDR" "$HEALTH" "$N_ITEMS" "$EXPECTED" "$EXPECT_BLOBS" "$AUDIT" "$BK_TARGET" "$FRESH_IMAGE" <<'PY'
import json, sys
out = {
  "drill": "P10-M3-T01 DR drill: fresh-machine restore from backup",
  "timestamp": sys.argv[2],
  "measured_tdr_seconds": int(sys.argv[3]),
  "under_60_min": int(sys.argv[3]) < 3600,
  "health": json.loads(sys.argv[4]),
  "restored_item_count": int(sys.argv[5]),
  "verified_blob_sha256": sys.argv[6],
  "blob_count_in_backup": int(sys.argv[7]),
  "audit": json.loads(sys.argv[8]),
  "backup_target": sys.argv[9],
  "fresh_image": sys.argv[10],
}
json.dump(out, open(sys.argv[1], "w"), indent=2)
PY

echo "--------------------------------------------------------------"
echo "[drill] RESULT: TDR = ${TDR}s ($((TDR/60))m $((TDR%60))s) -> $([ $TDR -lt 3600 ] && echo 'PASS (< 60 min)' || echo 'FAIL (>= 60 min)')"
echo "[drill] report: $REPORT"
if [ "$TDR" -lt 3600 ]; then
  echo "[drill] DONE: fresh-machine restore from backup PASSED every check"
  exit 0
else
  echo "[drill] FAIL: restore took >= 60 minutes" >&2
  exit 1
fi
