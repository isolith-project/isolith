# Re-note: P10-M1-T02 -- TLS/certs for the stack + vault DB/blob backup to a second disk/archive

What: the self-hosted Keelworks PDM gained native HTTPS termination (no reverse
proxy required) and a scheduled, checksummed backup of the vault (store DB +
content-addressed blobs + audit chain) to a second-disk / archive target on
U1/VM-class hosts, with the producer-side restore round-trip proven.

Key facts:
- TLS is native rustls (axum-server) in `oep-pdm`: `--tls-cert cert.pem
  --tls-key key.pem`; the router and API contract are unchanged. `base_url()`
  reports https when TLS is on. Local identities from
  `scripts/gen-stack-certs.sh` (CA + leaf, SANs hostname/localhost/127.0.0.1).
- Backup producer `scripts/backup/vault-backup.sh` writes the DR-shared bundle
  contract `<archive>/keelworks-vault-backup/{latest/{store.json, blobs/,
  audit/}, manifest.sha256, meta.json}`; manifest is sha256 and self-verified;
  NAS paths are refused (decision #9).
- Scheduling: `scripts/backup/install-vault-backup-cron.sh` (cron or systemd
  user timer). Restore round-trip proof on the host:
  `scripts/backup/vault-restore-check.sh`.
- Acceptance verified live on the U1 reference host (GB10) and in CI
  (`.gitea/workflows/p10-m1-t02.yaml`, rust:1-slim container with openssl):
  TLS handshake chains+hostname-verifies, health rides HTTPS, plain HTTP not
  served on the TLS listener, backup schedule installed, restore round-trips.
- Evidence: `workstreams/P10-M1/tls-and-backup.md`, `workstreams/P10-M1/proof/p10-m1-t02-proof.json`,
  `tests/test_p10_m1_t02_tls_and_backup.py`, `crates/oep-pdm/tests/
  tls_acceptance.rs`.
- DR handoff: the P10-M3-T01 drill consumes this exact bundle layout, so
  backups produced here are restorable by the quarterly drill.

2026-10-17/18: delivered + verified on promaxgb10-83f1 (U1-class, decision #9
run target). Schedule installed: daily 03:00 UTC cron entry.
