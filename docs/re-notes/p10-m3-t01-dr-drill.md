# Re-note: P10-M3-T01 -- DR drill, fresh-machine restore under 1 hour

What: a timed disaster-recovery drill that restores the full oep-pdm vault
(store DB + content-addressed blobs + audit chain) from the P10-M1-T02 backup
onto a fresh machine from bare state, with the restore verified by health
checks and a real vault read. Measured TDR must be under 60 minutes.

Key facts:
- Run target: U1/VM-class hosts (decision #9; the NAS is passive storage only).
- Backup contract: `<archive>/keelworks-vault-backup/{latest/{store.json,
  blobs/, audit/}, manifest.sha256, meta.json}` -- produced by
  scripts/dr/backup-vault.sh, consumed by scripts/dr/drill-restore.sh.
- Drill definition of "fresh": a brand-new bare machine image (ubuntu:24.04
  container), no host mounts, no Keelworks state. Vault data enters the fresh
  host only via the backup bundle read off the archive target, never from the
  live vault.
- Integrity-first: the sha256 manifest is re-verified inside the fresh host
  before any file is placed; content-addressed blob names must equal the bytes
  sha256; the real-read check compares the served file hash to the manifest.
- Cadence: quarterly (next 2027-01-17), plus off-cycle on any vault schema /
  backup contract / install flow change or release train.
- Evidence committed to P10-M3/proof/dr-drill-<ts>.{json,md} (TDR, health,
  item count, verified blob sha256, audit verify, targets).

2026-10-17 drill (Q4-2026): PASS. Recorded in P10-M3/dr-runbook.md and
P10-M3/proof/. Live source vault: /tmp/p9ga-probe-vault (the P9 GA gate vault,
100 gate-out items, 12 content-addressed blobs, audit ok).
