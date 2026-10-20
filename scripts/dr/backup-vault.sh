#!/usr/bin/env bash
# P10-M1-T02-compatible vault backup: snapshot the oep-pdm vault (store DB +
# content-addressed blobs + tamper-evident audit) to a second-disk / archive
# target with a sha256 manifest and a machine-readable meta record.
#
# This is the backup shape the P10-M3-T01 DR drill restores from. Its layout is
# the contract between the backup task and the DR drill:
#
#   <BK_TARGET>/isolith-vault-backup/
#     latest/                     # latest snapshot (rolled)
#       store.json                # vault item/revision metadata DB
#       blobs/<sha256>            # content-addressed object store (V5/CGR/etc)
#       audit/audit.log           # append-only tamper-evident chain
#       audit/audit.head
#     manifest.sha256             # sha256sum over latest/ (relative paths)
#     meta.json                   # created_at, source, counts, source_audit_head
#
# Usage:
#   scripts/dr/backup-vault.sh [SOURCE] [BK_TARGET] [LABEL]
#     SOURCE      oep-pdm persist dir to snapshot (default:
#                 keeps a running --persist-dir vault alive; pass a directory)
#     BK_TARGET   second-disk mount point or archive dir (default:
#                 $HOME/isolith-data/isolith-vault-backup)
#     LABEL       free-form snapshot label (default: manual-YYYYMMDD-HHMMSS)
#
# Run target (decision #9): U1/VM-class hosts; the NAS is NOT a backup target.
#
# Exit 0 iff a checksum-verified snapshot landed under BACKUP/

set -euo pipefail
cd "$(dirname "$0")/../.."
WT=$(pwd)

SOURCE="${1:-/tmp/p9ga-probe-vault}"
BK_TARGET="${2:-$HOME/isolith-data}"
LABEL="${3:-manual-$(date +%Y%m%d-%H%M%S)}"

BACKUP_ROOT="$BK_TARGET/isolith-vault-backup"
LATEST="$BACKUP_ROOT/latest"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

echo "[backup] source            : $SOURCE"
echo "[backup] backup root       : $BACKUP_ROOT"

# ---- 0. Preflight: the source must be a real oep-pdm persist vault ----------
if [ ! -f "$SOURCE/store.json" ]; then
  echo "[backup] FAIL: $SOURCE/store.json missing (not an oep-pdm persist dir)" >&2
  exit 1
fi
mkdir -p "$LATEST"

# ---- 1. Snapshot: blobs first, then DB, then audit (idle/quiet quiesce). ----
echo "[backup] snapshotting blobs ..."
mkdir -p "$STAGING/blobs"
cp -a "$SOURCE"/blobs/. "$STAGING/blobs/" 2>/dev/null || true
NBLOBS=$(find "$STAGING/blobs" -type f | wc -l)
echo "[backup]   $NBLOBS blobs copied"
[ "$NBLOBS" -gt 0 ] || { echo "[backup] FAIL: no blobs in source" >&2; exit 1; }

echo "[backup] snapshotting store.json ..."
cp -a "$SOURCE/store.json" "$STAGING/store.json"

echo "[backup] snapshotting audit chain ..."
AUDIT_HASH=""
if [ -d "$SOURCE/audit" ]; then
  mkdir -p "$STAGING/audit"
  cp -a "$SOURCE/audit/audit.log" "$STAGING/audit/audit.log" 2>/dev/null || true
  cp -a "$SOURCE/audit/audit.head" "$STAGING/audit/audit.head" 2>/dev/null || true
  if [ -f "$STAGING/audit/audit.head" ]; then
    AUDIT_HASH=$(tr -d '\r\n' < "$STAGING/audit/audit.head")
  fi
fi

# ---- 2. Integrity: content determinism of the snapshot before it leaves host =
# Every blob file name must equal the sha256 of its own bytes.
echo "[backup] verifying content-addressed blob names ..."
python3 - "$STAGING/blobs" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
bad = []
for p in root.iterdir():
    if not p.is_file():
        continue
    h = hashlib.sha256(p.read_bytes()).hexdigest()
    if p.name != h:
        bad.append(str(p))
if bad:
    print("[backup] FAIL: content-addressed mismatch:", bad)
    sys.exit(1)
print(f"[backup]   {sum(1 for p in root.iterdir())} blobs self-verified")
PY

# ---- 3. Install the snapshot as the latest roll -------------------------------
rm -rf "$LATEST"
mv "$STAGING" "$LATEST"
trap - EXIT

# ---- 4. Checksums over the whole snapshot (relative paths) -------------------
echo "[backup] writing manifest.sha256 ..."
( cd "$BACKUP_ROOT" && sha256sum latest/store.json latest/blobs/* 2>/dev/null \
  | sort -k2 ) > "$BACKUP_ROOT/manifest.sha256"
# include audit files if present
if ls "$BACKUP_ROOT"/latest/audit/* >/dev/null 2>&1; then
  ( cd "$BACKUP_ROOT" && sha256sum latest/audit/* 2>/dev/null | sort -k2 ) \
    >> "$BACKUP_ROOT/manifest.sha256"
fi
sort -k2 -o "$BACKUP_ROOT/manifest.sha256" "$BACKUP_ROOT/manifest.sha256"

NITEMS=$(python3 -c "import json,sys; d=json.load(open('$BACKUP_ROOT/latest/store.json')); print(d.get('next_item_seq',0))")
cat > "$BACKUP_ROOT/meta.json" <<EOF
{
  "schema": "isolith-vault-backup/1",
  "label": "$LABEL",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "source_host": "$(hostname)",
  "source_persist_dir": "$SOURCE",
  "blob_count": $NBLOBS,
  "store_seq": $NITEMS,
  "source_audit_head": "$AUDIT_HASH",
  "target": "$BK_TARGET"
}
EOF

# ---- 5. Final verification: manifest must self-verify -------------------------
echo "[backup] verifying manifest ..."
( cd "$BACKUP_ROOT" && sha256sum -c manifest.sha256 >/dev/null )
echo "[backup] verification OK"
echo "[backup] label   : $LABEL"
echo "[backup] blobs   : $NBLOBS"
echo "[backup] meta    : $BACKUP_ROOT/meta.json"
echo "[backup] manifest: $BACKUP_ROOT/manifest.sha256"
echo "[backup] DONE"
