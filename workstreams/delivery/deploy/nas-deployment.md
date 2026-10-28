# Isolith OEP on a customer NAS / homelab server (DOCKER-NAS-DEPLOYMENT)

Version 1.0 (2026-10-27). Owner: Isolith delivery / P10 hybrid-cloud lane.
Card: `t_82f606dd` (DOCKER-NAS-DEPLOYMENT).

## 0. Corrected policy note (decision #9 boundary)

Decision #9 ("NAS is repo/storage host only, must not host Isolith services",
"never a backup target") applies to **our own** infrastructure: Don's NAS is a
storage and repository host, and no development, CI, build or service workload
runs on it.

That decision is **not amended by this document and not a conflict with it.**
This document is for **third-party customers** who want to host the Isolith
server component (PDM, P6 preview viewer, docs, workers) on **their own**
server or NAS (Synology Container Manager, QNAP, TrueNAS, Unraid, or plain
`docker compose` on a homelab server). Nothing here puts a workload on Don's
NAS, and nothing here makes the NAS a backup target for us.

Any "NAS" below means a third-party host. One consequence of #9's producer
rules carries over as guidance to customers: do not back up onto the same
volume/disk set that is the failure domain you are guarding against (see
section 5).

## 1. What ships

One multi-service image, `isolith/oep-stack`, carries the PDM, the P6 preview
viewer, the docs site, and the worker binaries (batch-convert, render worker,
gate node). A single image means one pull, one digest to pin, and one thing to
back up/roll back on a NAS.

On each release the image is published to **GHCR** (GitHub Container Registry)
with a stable version tag `v<YYYY.N.P>` plus a moving `latest`, and the image
digest is recorded in the release body. There is **no Docker Hub dependency**:
every consumer pulls from `ghcr.io/<org>/isolith/oep-stack`.

> Gate (2026-10-27, Don): the first **public** GHCR push only happens once Don
> opens the publish surface. The push path below is implemented, and the
> mechanism is verified against a local OCI registry during this card's run;
> the actual `ghcr.io` push is a one-line, explicit flag on `release.sh` that
> stays off until that signal.

## 2. Architecture support (honest)

The stack image is built from an arch-neutral multi-stage Dockerfile (Rust
release build in a pinned toolchain, copied into Debian slim). The GA-verified
build for this card is **linux/arm64, run-tested on U1 (aarch64,
promaxgb10-83f1)** with all four health checks green. A `linux/amd64` build was
attempted on the same host under QEMU emulation and **segfaulted inside the
emulated cargo build (exit 139); it was not treated as verified.** amd64 is
therefore stated as *supported by the same Dockerfile, not build-verified in
this run*; the amd64 manifest is produced on a native/CI amd64 runner before
an amd64-only claim is made.

In practice: arm64 hosts (most ARM Synology models, Apple-Silicon homelab,
ARM VPSes) run the verified image today; x86_64 hosts are expected to work but
need the first amd64-native build + a green health pass before the GA claim
extends to them.

## 3. Resource floor (measured on U1)

Measured, not guessed. `docker stats --no-stream` on the running stack
(arm64 image `isolith/oep-stack:0.1.0`, four containers, idle after one seeded
vault revision):

| service | RSS (idle) | CPU (idle) | container disk (writable layer) |
|---------|-----------|------------|---------------------------------|
| pdm     | 8.4 MiB   | ~1%        | 8.2 kB                          |
| viewer  | 6.0 MiB   | ~1.5%      | 8.2 kB                          |
| docs    | 7.6 MiB   | ~2%        | 4.1 kB                          |
| p2p     | 3.9 MiB   | ~1.5%      | 4.1 kB                          |
| **total (pdm+viewer+docs)** | **~22 MiB** | **~4% of one core** | ~20 kB |

Image on disk: 218 MB uncompressed, 47.8 MB compressed on the wire. Named
vault volume (`pdm_data`) at rest with one item/revision: 16 KB (grows with
CAD blob content; content-addressed blobs are stored once).

**Stated NAS floor:** 1 GB RAM free is far more than enough (the whole stack
idles under 30 MiB RSS); any modern NAS meets this. 1 GB free disk beyond the
image for the vault+blobs per project day-1. 1 vCPU is fine for the server
component; heavy batch-convert/render work is what burns CPU and should be
sized separately (workers are one-shot binaries, not always-on services).

## 4. Security posture for a NAS deployment

- **PDM auth on by default (no regression of P2T-04-AZ).** The REST v1
  surface is authorization-bound by default (SEC-AUTHZ-DEFAULT /
  SEC-AUTHZ-RESIDUAL, landed on the 2027.1 train). The shipped compose command
  for `pdm` and `viewer` does **not** pass `--no-auth`, so the RBAC boundary is
  enforced from the first boot. Verified during this card: an unauthenticated
  or wrong-role call is rejected `403` (`unknown role "engineer"`), and the
  data/audit/file surface returns 403 without identity headers. The compose
  must never add `--no-auth`; the only escape hatch is the documented,
  loudly-warned test/air-gap flag on the binary itself.
- **No internet exposure by default.** Every host bind defaults to
  `127.0.0.1` (`.env`/`.env.nas.example`), so nothing listens on the LAN until
  the operator publishes it. This is decision #10's posture carried into the
  compose.
- **LAN bind guidance.** To serve the team on the LAN, set each
  `OEP_*_BIND` to the NAS's LAN IP (or `0.0.0.0` behind the reverse proxy).
  Do not set `0.0.0.0` on a WAN-facing interface.
- **TLS termination options** (pick one):
  1. **Reverse proxy on the NAS** (Synology Reverse Proxy, QNAP reverse proxy,
     Nginx) terminating TLS in front of 8787/8790/8792. Simplest for a NAS.
  2. **Caddy in front** of the stack on the same host or a small VM, with a
     `Caddyfile` that reverse-proxies the three ports and terminates TLS.
  3. **Native `oep-pdm` TLS** (`--tls-cert/--tls-key` from
     `scripts/gen-stack-certs.sh` or real PKI) for the PDM surface when no
     reverse proxy is wanted.
  The docs service (`oep-docs`) and viewer serve over plain HTTP inside the
  LAN; put them behind the same proxy/edge if exposure beyond the NAS mounts.
- **Air-gap claim.** Isolith can run fully air-gapped **only when the host
  itself is isolated** (no default route, no DNS, no reachable registry). A
  NAS on your LAN that talks to the internet is NOT air-gapped. The air-gap
  install path is the signed offline bundle
  (`scripts/airgap/build-offline-bundle.sh`), not a GHCR pull. GHCR pulls, the
  update checker, and any telemetry are off/absent in the air-gap configuration
  (decision #11: checker advisory, off by default, never fails closed).

## 5. Backup and restore (does not violate decision #9)

- **What to back up:** the named volume `pdm_data` — that is the whole vault:
  `store.json` (item/revision metadata DB), `blobs/` (content-addressed),
  `audit/` (tamper-evident chain). Plus the config: `.env` and your compose
  override. The image itself is re-pullable (or kept as a tarball), so it is
  not part of the backup set.
- **Where it may go:** a different physical disk / different volume set /
  another NAS / off-site object storage. **Not** the same volume set that is
  the failure domain (e.g. not the same RAID-5 array hosting `pdm_data`, and
  for our own infrastructure the NAS is never a target at all — that guard
  is enforced in `vault-backup.sh` and refuses `/mnt/nas*`).
- **The producer** is `scripts/backup/vault-backup.sh` (sha256 manifest, blob
  names == content hashes, self-verifying) and the restore checker is
  `scripts/backup/vault-restore-check.sh`. These work against any
  second-disk/archive target. Command (on a host with the scripts):
  `scripts/backup/vault-backup.sh <vault-src> <backup-target> <label>`.
- **Proven restore (this card):** a clean-host round trip was executed on U1 —
  the live vault (1 item, 1 content-addressed blob, 2 audit entries) was
  snapshotted, backed up with the producer, then restored into a **fresh
  persist dir** and booted with a fresh `oep-pdm`; health was `ok`, the
  restored item count was 1, the audit chain verified (2 entries), and the
  served file bytes hash-matched `bc7d34…fd32` from the backup manifest. Full
  log: `proof/nas-restore-drill.log` (PASS).

## 6. Upgrade path

1. `docker compose -f docker-compose.nas.yml down` (or `--no-stop` for a
   rolling swap).
2. Edit `OEP_VERSION` in `.env` to the new release version.
3. `docker compose -f docker-compose.nas.yml pull` (fetches the new image from
   GHCR).
4. `docker compose -f docker-compose.nas.yml up -d` and wait for all four
   health checks green (`docker compose ps`, plus the per-service endpooints in
   section 7/quickstart).
5. **Migrations:** the vault schema is versioned and the store is
   forward-compatible (additive; `oep-vault-migrate` handles legacy V5-era
   tree imports, not in-place downgrade). A release that changes the on-disk
   schema bumps the format and the upgrade *adds* to the store; there is no
   destructive in-place migration step to orchestrate on the NAS. If a release
   notes say a migration is required, run it, then run the restore-check to
   confirm the vault still round-trips.
6. **Rollback:** keep the previous version in `.env` (or keep the old image
   tag available), `docker compose up -d` with the prior `OEP_VERSION`, and
   restart. The named volume is untouched by an image swap, so `pdm_data` is
   preserved across rollback; the vault format stays readable by the older
   binary for as long as the format is forward-compatible (documented per
   release).

## 7. Health-check contract (the accept surface)

```
curl -fsS http://127.0.0.1:8787/api/v1/health   # pdm   -> {"status":"ok",...}
curl -fsS -o /dev/null http://127.0.0.1:8790/    # viewer-> 200 (explorer HTML)
curl -fsS http://127.0.0.1:8080/healthz          # p2p   -> {"status":"ok"}
curl -fsS http://127.0.0.1:8792/healthz          # docs  -> {"status":"ok"}
```

## 8. Files

- `deploy/docker-compose.nas.yml` — pull-based NAS compose (no build; named
  volumes, explicit binds, health checks).
- `deploy/.env.nas.example` — environment template for NAS operators.
- `deploy/release.sh` — builds the image and, with `--push-ghcr`, tags/pushes
  to `ghcr.io/<org>/isolith/oep-stack:<ver>` and records the digest
  (`--digest-out`).
- `proof/nas-*` — measured floor, health output, restore drill log, build log.
- `../../docs-site/install-on-nas.html` — the NAS quickstart (this card item 7).

Version: 1.0. Bump on material change (compose shape, GHCR ref, resource
floor, or schema/migration behavior).
