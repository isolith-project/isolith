#!/usr/bin/env bash
# P10-M1-T02 canonical vault backup producer.
#
# Snapshots the oep-pdm vault (store DB + content-addressed blobs +
# tamper-evident audit chain) to a second-disk / archive target, with a sha256
# manifest and a machine-readable meta record. This is the OTHER half of the
# DR lane: the P10-M3-T01 drill (scripts/dr/drill-restore.sh) restores from
# EXACTLY this layout, so the contract below must not change without a
# matching runbook-change (see workstreams/P10-M3/dr-runbook.md).
#
# Layout (interchange contract, shared with P10-M3-T01):
#   <BK_TARGET>/keelworks-vault-backup/
#     latest/
#       store.json                # vault item/revision metadata DB
#       blobs/<sha256>            # content-addressed object store
#       audit/audit.log           # append-only tamper-evident chain
#       audit/audit.head
#     manifest.sha256             # sha256sum over latest/ (relative paths)
#     meta.json                   # created_at, source, counts, source_audit_head
#
# Run target (decision #9): U1/VM-class hosts or an attached second disk on
# U1/VM. The NAS is NEVER a backup target: this script refuses any target
# whose path belongs to the NAS mounts.
#
# Usage:
#   scripts/backup/vault-backup.sh [SOURCE] [BK_TARGET] [LABEL]
#     SOURCE      oep-pdm --persist-dir vault to snapshot
#                 (default: $HOME/keelworks-data/keelworks-vault)
#     BK_TARGET   second-disk mount point or archive dir
#                 (default: $HOME/keelworks-data)
#     LABEL       free-form snapshot label (default: scheduled-YYYYMMDD-HHMMSS)
#
# Exit 0 iff a checksum-verified snapshot landed under
#   $BK_TARGET/keelworks-vault-backup/  (latest/ + manifest + meta).
set -euo pipefail
cd "$(dirname "$0")/../.." || exit 1
WT=$(pwd)

SOURCE="${1:-$HOME/keelworks-data/keelworks-vault}"
BK_TARGET="${2:-$HOME/keelworks-data}"
LABEL="${3:-scheduled-$(date +%Y%m%d-%H%M%S)}"

# ---- Decision #9 guard: the NAS is passive storage, never a backup target ----
case "$BK_TARGET" in
  /mnt/nas*|/mnt/vault*|//192.168.0.101*) 
    echo "[vault-backup] FAIL: NAS is not an approved backup target (decision #9)" >&2
    exit 1
    ;;
esac

BACKUP_ROOT="$BK_TARGET/keelworks-vault-backup"
LATEST="$BACKUP_ROOT/latest"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

echo "[vault-backup] source      : $SOURCE"
echo "[vault-backup] backup root : $BACKUP_ROOT"
echo "[vault-backup] label       : $LABEL"

# ---- 0. Preflight: source must be a real oep-pdm persist vault -------------
if [ ! -f "$SOURCE/store.json" ]; then
  echo "[vault-backup] FAIL: $SOURCE/store.json missing (not an oep-pdm persist dir)" >&2
  exit 1
fi
mkdir -p "$LATEST"

# ---- 1. Snapshot blobs first, then DB, then audit (quiet quiesce) ----------
echo "[vault-backup] snapshotting blobs ..."
mkdir -p "$STAGING/blobs"
cp -a "$SOURCE"/blobs/. "$STAGING/blobs/" 2>/dev/null || true
NBLOBS=$(find "$STAGING/blobs" -type f 2>/dev/null | wc -l)
echo "[vault-backup]   $NBLOBS blobs copied"

echo "[vault-backup] snapshotting store.json ..."
cp -a "$SOURCE/store.json" "$STAGING/store.json"

echo "[vault-backup] snapshotting audit chain ..."
AUDIT_HASH=""
if [ -d "$SOURCE/audit" ]; then
  mkdir -p "$STAGING/audit"
  cp -a "$SOURCE/audit/audit.log" "$STAGING/audit/audit.log" 2>/dev/null || true
  cp -a "$SOURCE/audit/audit.head" "$STAGING/audit/audit.head" 2>/dev/null || true
  if [ -f "$STAGING/audit/audit.head" ]; then
    AUDIT_HASH=$(tr -d '\r\n' < "$STAGING/audit/audit.head")
  fi
fi

# ---- 2. Content determinism: every blob filename == sha256 of its bytes ----
echo "[vault-backup] verifying content-addressed blob names ..."
python3 - "$STAGING/blobs" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
bad = []
count = 0
for p in root.iterdir():
    if not p.is_file():
        continue
    count += 1
    h = hashlib.sha256(p.read_bytes()).hexdigest()
    if p.name != h:
        bad.append(str(p))
if bad:
    print("[vault-backup] FAIL: content-addressed mismatch:", bad)
    sys.exit(1)
print(f"[vault-backup]   {count} blobs self-verified")
PY

# ---- 3. Install the snapshot as the latest roll -----------------------------
rm -rf "$LATEST"
mv "$STAGING" "$LATEST"
trap - EXIT

# ---- 4. Checksums over the whole snapshot (relative paths) ------------------
echo "[vault-backup] writing manifest.sha256 ..."
( cd "$BACKUP_ROOT" && sha256sum latest/store.json latest/blobs/* 2>/dev/null \
  | sort -k2 ) > "$BACKUP_ROOT/manifest.sha256"
if ls "$BACKUP_ROOT"/latest/audit/* >/dev/null 2>&1; then
  ( cd "$BACKUP_ROOT" && sha256sum latest/audit/* 2>/dev/null | sort -k2 ) \
    >> "$BACKUP_ROOT/manifest.sha256"
fi
sort -k2 -o "$BACKUP_ROOT/manifest.sha256" "$BACKUP_ROOT/manifest.sha256"

NITEMS=$(python3 -c "import json,sys; d=json.load(open('$BACKUP_ROOT/latest/store.json')); print(d.get('next_item_seq',0))")
# Write meta.json through python so the audit head (a JSON fragment) is
# correctly escaped inside the string value -- never raw-interpolated.
python3 - "$BACKUP_ROOT/meta.json" "$LABEL" "$SOURCE" "$AUDIT_HASH" "$NBLOBS" "$NITEMS" "$BK_TARGET" <<'PY'
import json, sys, datetime
out_path, label, source, audit_head, nb, nitems, target = sys.argv[1:]
meta = {
  "schema": "keelworks-vault-backup/1",
  "label": label,
  "created_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
  "source_host": __import__("socket").gethostname(),
  "source_persist_dir": source,
  "blob_count": int(nb),
  "store_seq": int(nitems),
  "source_audit_head": audit_head,
  "target": target,
}
json.dump(meta, open(out_path, "w"), indent=2)
print(f"[vault-backup]   seq={meta['store_seq']} blobs={meta['blob_count']}")
PY

# ---- 5. Final verification: manifest must self-verify -----------------------
echo "[vault-backup] verifying manifest ..."
( cd "$BACKUP_ROOT" && sha256sum -c manifest.sha256 >/dev/null )
echo "[vault-backup] verification OK"
echo "[vault-backup] blobs    : $NBLOBS"
echo "[vault-backup] meta     : $BACKUP_ROOT/meta.json"
echo "[vault-backup] manifest : $BACKUP_ROOT/manifest.sha256"
echo "[vault-backup] DONE"
