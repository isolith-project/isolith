# P10-M1-T02 -- TLS/certs for the stack + vault DB/blob backup to a second disk/archive

Status: delivered + verified on the U1-class reference host (GB10, 2026-10-17/18).
Owner: Isolith self-hosted PDM lane (oep-pdm server). Version: 1.

## 1. What this is

Wave-2 downstream P10 delivery: the self-hosted Isolith PDM no longer has to
ride plain HTTP on the LAN. This card delivers two things for the stack:

1. **TLS + certs**: `oep-pdm` terminates HTTPS natively (rustls via
   axum-server), so the PDM web/API surface serves over TLS with a trustable
   chain. Certificates are provisioned by `scripts/gen-stack-certs.sh` (a zero-
   configuration local CA + leaf for the host), or you can point it at existing
   certs. No reverse proxy required -- the product terminates TLS itself.
2. **Backup to a second disk / archive**: `scripts/backup/vault-backup.sh`
   snapshots the vault (store.json metadata DB + content-addressed blobs + the
   tamper-evident audit chain) to a second-disk / archive target with a sha256
   manifest that is self-verified. `scripts/backup/install-vault-backup-cron.sh`
   schedules it daily (cron or systemd user timer). `scripts/backup/
   vault-restore-check.sh` restores a bundle into a fresh persist dir on the
   same host and proves the real vault read round-trips.

Run target (decision #9): U1/VM-class hosts or an attached second disk on
U1/VM. The NAS is passive storage and is NEVER a backup target (the producer
refuses NAS paths; see decision #9, amended x2).

## 2. Acceptance (from the card)

- **TLS terminates correctly**: `oep-pdm --tls-cert cert.pem --tls-key key.pem`
  serves the P8 REST API v1 contract over HTTPS. Verified by a real
  `openssl s_client -verify_return_error -CAfile` handshake (chain + hostname
  must validate, else non-zero) and by the health JSON riding over TLS. Plain
  HTTP is NOT served on the TLS listener (rustls rejects non-TLS bytes).
- **Backups are scheduled, checksummed**: a cron (or systemd user timer) entry
  runs the producer daily; the producer writes `manifest.sha256` over every
  bundle file and self-verifies with `sha256sum -c` before exiting 0; blob
  filenames are content-addressed and each name is checked against the sha256
  of its bytes.
- **A restore from the backup round-trips on U1/VM**: `vault-restore-check.sh`
  restores the bundle into a fresh persist dir, boots a fresh oep-pdm, and the
  served file bytes hash back to an entry in the backup manifest.
- **DR handoff to P10-M3-T01**: the DR drill card consumes exactly this bundle
  layout (`<BK_TARGET>/isolith-vault-backup/{latest, manifest.sha256,
  meta.json}`), so a backup produced here is restorable by that drill.

## 3. Bundle contract (shared with P10-M3-T01)

    <BK_TARGET>/isolith-vault-backup/
      latest/
        store.json               # vault item/revision metadata DB
        blobs/<sha256>           # content-addressed object store
        audit/audit.log          # append-only, tamper-evident chain
        audit/audit.head
      manifest.sha256            # sha256sum over latest/ (relative paths)
      meta.json                  # created_at, source, counts, source_audit_head

This matches `scripts/dr/backup-vault.sh` on the DR lane byte-for-byte; either
producer may write the bundle, the drill restores whatever is present.

## 4. TLS design notes

- `oep-pdm --tls-cert` and `--tls-key` must be given together; otherwise the
  server refuses to start (exit 2). Absent both, the server keeps serving plain
  HTTP (legacy default -- every existing test and client stays on `http://`).
- The router is byte-identical between the two paths: only the transport
  changes (axum-server rustls acceptor vs axum::serve).
- `base_url()` reports `https://` when TLS is on, so the contract client and
  tooling can tell which scheme to speak.
- Verified in CI on the container (openssl present) AND on the U1 host; the
  acceptance test `tests/test_p10_m1_t02_tls_and_backup.py` degrades legs to
  SKIP when openssl is unavailable, so CI never hard-fails on a tool gap.

## 5. Operate

    # 1. TLS identity for the stack (once per host, rotate before 27 months)
    scripts/gen-stack-certs.sh workstreams/delivery/deploy/certs                 # -> ca.pem, cert.pem, key.pem
    oep-pdm --persist-dir <vault> --tls-cert workstreams/delivery/deploy/certs/cert.pem --tls-key workstreams/delivery/deploy/certs/key.pem

    # 2. install the daily, checksummed backup on a second-disk / archive target
    scripts/backup/install-vault-backup-cron.sh <vault> <archive-or-2nd-disk>

    # 3. ad-hoc backup + restore round-trip check (proves the acceptance on this host)
    scripts/backup/vault-backup.sh <vault> <archive-or-2nd-disk>
    scripts/backup/vault-restore-check.sh <archive-or-2nd-disk>

## 6. Scope boundaries / honest limits

- The DR **drill** (timed restore to a fresh machine, quarterly cadence) is
  P10-M3-T01, not this card; this card supplies the producer + the producer-side
  round-trip proof on U1/VM.
- The offline/air-gap installer is P10-M2-T02. Certificates are generated
  locally; nothing phones home.
- The audit head hash is recorded in `meta.json` (as a strict JSON string);
  the audit chain remains verifiable through `GET /api/v1/audit/verify` on any
  restored vault.

Version: 1 (2026-10-17). Update this doc on any TLS arg, bundle layout, or
schedule change (mirrors the vault-conventions bump rule).
