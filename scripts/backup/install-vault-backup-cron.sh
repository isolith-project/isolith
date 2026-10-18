#!/usr/bin/env bash
# P10-M1-T02: install the scheduled vault backup on U1/VM-class hosts
# (decision #9 run target). Registers a daily cron entry that runs the
# checksummed producer scripts/backup/vault-backup.sh against the live vault.
#
# The scheduling surface is genuinely opportunistic: it uses whichever local
# scheduler exists (cron if present, else a systemd user timer). It never
# touches the NAS. Idempotent: re-running replaces the keelworks-vault-backup
# line, never duplicates it.
#
# Usage:
#   scripts/backup/install-vault-backup-cron.sh [SOURCE] [BK_TARGET] [HOUR]
#     SOURCE     oep-pdm --persist-dir vault to snapshot
#                (default: $HOME/keelworks-data/keelworks-vault)
#     BK_TARGET  archive dir on the second disk/archive target
#                (default: $HOME/keelworks-data)
#     HOUR       daily UTC hour to run (default 3 -> 03:00 UTC)
set -euo pipefail
cd "$(dirname "$0")/../.." || exit 1
WT=$(pwd)

SOURCE="${1:-$HOME/keelworks-data/keelworks-vault}"
BK_TARGET="${2:-$HOME/keelworks-data}"
HOUR="${3:-3}"
BK=$WT/scripts/backup/vault-backup.sh

if [ ! -f "$BK" ]; then
  echo "FAIL: producer script missing: $BK" >&2
  exit 1
fi

# Refuse NAS target (decision #9) on install, not just at run time.
case "$BK_TARGET" in
  /mnt/nas*|/mnt/vault*|//192.168.0.101*)
    echo "FAIL: NAS is not an approved backup target (decision #9)" >&2
    exit 1
    ;;
esac

MARKER="### Keelworks vault backup (P10-M1-T02) ###"
LINE="$HOUR 0 * * * $BK '$SOURCE' '$BK_TARGET' >> $BK_TARGET/vault-backup.log 2>&1"

if command -v crontab >/dev/null 2>&1; then
  TMP=$(mktemp)
  crontab -l 2>/dev/null | grep -v "$MARKER" | grep -v "vault-backup.sh" > "$TMP" || true
  {
    echo "$MARKER"
    echo "# scheduled by scripts/backup/install-vault-backup-cron.sh on $(date -u +%F)"
    echo "$LINE"
    echo "$MARKER"
  } >> "$TMP"
  crontab "$TMP"
  rm -f "$TMP"
  echo "[schedule] installed cron entry (daily ${HOUR}:00 UTC):"
  echo "[schedule]   $LINE"
  crontab -l | grep vault-backup.sh
  echo "[schedule] next-run check: crontab -l | grep vault-backup.sh"
  exit 0
fi

if command -v systemctl >/dev/null 2>&1 && systemctl --user is-system-running >/dev/null 2>&1; then
  UNIT_DIR="$HOME/.config/systemd/user"
  mkdir -p "$UNIT_DIR"
  cat > "$UNIT_DIR/keelworks-vault-backup.service" <<EOF
[Unit]
Description=Keelworks vault backup (P10-M1-T02)
[Service]
Type=oneshot
ExecStart=$BK '$SOURCE' '$BK_TARGET' >> $BK_TARGET/vault-backup.log 2>&1
EOF
  cat > "$UNIT_DIR/keelworks-vault-backup.timer" <<EOF
[Unit]
Description=Daily Keelworks vault backup
[Timer]
OnCalendar=*-*-* ${HOUR}:00:00
Persistent=true
[Install]
WantedBy=timers.target
EOF
  systemctl --user daemon-reload
  systemctl --user enable --now keelworks-vault-backup.timer >/dev/null
  echo "[schedule] installed systemd user timer (daily ${HOUR}:00)"
  systemctl --user list-timers keelworks-vault-backup.timer
  exit 0
fi

echo "FAIL: no supported local scheduler (cron or systemd user) on this host" >&2
exit 1
