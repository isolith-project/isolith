#!/usr/bin/env bash
# P10-M1-T02: restore round-trip verification on U1/VM.
#
# Proves the "a restore from the backup round-trips" acceptance criterion: take
# the latest `isolith-vault-backup/` bundle produced by scripts/backup/
# vault-backup.sh, restore it into a FRESH persist dir on this host, boot a
# fresh oep-pdm against it, and verify with the exact checks the DR drill uses
# (health ok, item catalogue non-empty, audit chain verifies, one real vault
# read whose served bytes hash back to a blob in the manifest).
#
# This is the P10-M1-T02 producer-side proof; the full timed bare-metal drill
# to a container/other machine is the P10-M3-T01 drill (scripts/dr/).
#
# Usage:
#   scripts/backup/vault-restore-check.sh [BK_TARGET] [PDM_BIN]
#     BK_TARGET  archive dir holding isolith-vault-backup/
#                (default: $HOME/isolith-data)
#     PDM_BIN    oep-pdm binary to boot the restored vault with
#                (default: $WT/target/debug/oep-pdm)
# Exit 0 iff the restore round-trips on this U1/VM host.
set -euo pipefail
cd "$(dirname "$0")/../.." || exit 1
WT=$(pwd)

BK_TARGET="${1:-$HOME/isolith-data}"
PDM_BIN="${2:-$WT/target/debug/oep-pdm}"
BACKUP_ROOT="$BK_TARGET/isolith-vault-backup"
RESTORE_DIR="$(mktemp -d)"
HOST_PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('127.0.0.1',0));print(s.getsockname()[1]);s.close()")
trap 'kill "${SERVER_PID:-0}" 2>/dev/null || true; rm -rf "$RESTORE_DIR"' EXIT

echo "[restore-check] backup root : $BACKUP_ROOT"
echo "[restore-check] restore dir : $RESTORE_DIR"
echo "[restore-check] PDM binary  : $PDM_BIN"

# ---- 1. Preflight ----------------------------------------------------------
[ -f "$BACKUP_ROOT/manifest.sha256" ] || { echo "FAIL: no manifest" >&2; exit 1; }
[ -d "$BACKUP_ROOT/latest/blobs" ] || { echo "FAIL: no blobs" >&2; exit 1; }
[ -f "$BACKUP_ROOT/latest/store.json" ] || { echo "FAIL: no store.json" >&2; exit 1; }
[ -x "$PDM_BIN" ] || { echo "FAIL: oep-pdm binary not found: $PDM_BIN (cargo build -p oep-pdm)" >&2; exit 1; }

echo "[restore-check] verifying manifest integrity ..."
( cd "$BACKUP_ROOT" && sha256sum -c manifest.sha256 >/dev/null )
echo "[restore-check] manifest OK"

# ---- 2. Restore into a fresh persist dir ------------------------------------
echo "[restore-check] restoring store.json + blobs + audit ..."
mkdir -p "$RESTORE_DIR"
cp -a "$BACKUP_ROOT/latest/store.json" "$RESTORE_DIR/"
cp -a "$BACKUP_ROOT/latest/blobs" "$RESTORE_DIR/"
if [ -d "$BACKUP_ROOT/latest/audit" ]; then
  cp -a "$BACKUP_ROOT/latest/audit" "$RESTORE_DIR/"
fi

# ---- 3. Boot a fresh oep-pdm against the restored vault ----------------------
echo "[restore-check] booting oep-pdm on restored vault (port $HOST_PORT) ..."
"$PDM_BIN" --addr "127.0.0.1:$HOST_PORT" --persist-dir "$RESTORE_DIR" \
  >"$RESTORE_DIR/pdm.log" 2>&1 &
SERVER_PID=$!
sleep 1

READY=0
for i in $(seq 1 30); do
  if curl -fsS -m 2 "http://127.0.0.1:$HOST_PORT/api/v1/health" >/dev/null 2>&1; then
    READY=1; break
  fi
  sleep 1
done
[ "$READY" = 1 ] || { echo "[restore-check] FAIL: restored PDM did not become healthy" >&2; cat "$RESTORE_DIR/pdm.log" >&2; exit 1; }
echo "[restore-check] health : ok"

# ---- 4. The real vault read (the round-trip acceptance) ----------------------
# SEC-AUTHZ-DEFAULT (P2T-04-AZ, 2026-10-18): the REST v1 surface is
# authorization-bound by default. `/api/v1/health` stays public (the compose
# healthcheck depends on it); the data reads the drill performs (items,
# audit, file) must carry the identity headers. The drill authenticates as a
# per-run generated admin principal, no `--no-auth` anywhere: the restored
# vault must serve over the SAME guarded boundary as production.
RESTORE_USER="${RESTORE_USER:-restore-drill}"
RESTORE_ROLE="${RESTORE_ROLE:-admin}"
RESTORE_TENANT="${RESTORE_TENANT:-default}"
AUTH_HDRS=(-H "X-Oep-User: $RESTORE_USER" -H "X-Oep-Roles: $RESTORE_ROLE" -H "X-Oep-Tenant: $RESTORE_TENANT")

HEALTH=$(curl -fsS -m 5 "http://127.0.0.1:$HOST_PORT/api/v1/health")
echo "[restore-check] health json: $HEALTH"
echo "$HEALTH" | grep -q '"status":"ok"' || { echo "FAIL: health not ok" >&2; exit 1; }

ITEMS=$(curl -fsS -m 5 "${AUTH_HDRS[@]}" "http://127.0.0.1:$HOST_PORT/api/v1/items")
N_ITEMS=$(echo "$ITEMS" | python3 -c "import sys,json;print(len(json.load(sys.stdin)['items']))")
echo "[restore-check] restored item count: $N_ITEMS"
[ "$N_ITEMS" -gt 0 ] || { echo "FAIL: restored catalogue empty" >&2; exit 1; }

AUDIT=$(curl -fsS -m 5 "${AUTH_HDRS[@]}" "http://127.0.0.1:$HOST_PORT/api/v1/audit/verify" || echo '{"ok":false}')
echo "$AUDIT" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('ok'), d; print('[restore-check] audit chain ok (%d entries)' % d.get('count'))"

ITEM_ID=$(echo "$ITEMS" | python3 -c "import sys,json;print(json.load(sys.stdin)['items'][0]['id'])")
curl -fsS -m 10 "${AUTH_HDRS[@]}" -D "$RESTORE_DIR/hdr.txt" \
  "http://127.0.0.1:$HOST_PORT/api/v1/items/$ITEM_ID/revisions/1/file" \
  -o "$RESTORE_DIR/file.bin"
EXPECTED=$(awk 'tolower($1)=="x-blob-sha256:"{print $2}' "$RESTORE_DIR/hdr.txt" | tr -d '\r')
ACTUAL=$(sha256sum "$RESTORE_DIR/file.bin" | cut -d' ' -f1)
echo "[restore-check] real read $ITEM_ID rev1: expected=$EXPECTED actual=$ACTUAL size=$(stat -c%s "$RESTORE_DIR/file.bin")"
[ "$EXPECTED" = "$ACTUAL" ] || { echo "FAIL: served file hash mismatch" >&2; exit 1; }
grep -q "$ACTUAL" "$BACKUP_ROOT/manifest.sha256" \
  || { echo "FAIL: served file not present in backup manifest" >&2; exit 1; }
echo "[restore-check] real vault read round-trips against the backup manifest"

# ---- 5. Done ----------------------------------------------------------------
echo "[restore-check] PASS: restore round-trips on this U1/VM host"
exit 0
