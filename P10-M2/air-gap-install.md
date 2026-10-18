# Keelworks Air-Gap Install Path (Offline / USB / ISO)

Version: 1.0.0 (2026-10-17)
Status: Implemented as task P10-M2-T02 (keelworks card t_98aa61ae)
Owner: Open Engineering Platform (OEP) program, P10 self-hosted delivery
Ties to: decision #10 (air-gap sovereignty centerpiece), decision #11 (update
checker advisory + off by default), P11 signing (signed artifacts)
Follows: the Bastion S2 offline-image pattern (build, transfer, load,
configure, start, verify, rollback — all with no internet)

## 1. What this path is

An air-gapped ITAR/defense shop has a machine with **no network access at
all** — typically no default route, no DNS, no registry. This document and the
tooling next to it deliver a signed, self-contained offline bundle that such a
machine can install *without ever needing the internet*, and that passes the
**same health checks** as an online install.

The bundle is the single artifact that crosses the air gap on a USB stick or
an ISO. Everything the target needs is inside it — container images, compose
stack, config templates, install/verify/rollback scripts, checksums, and the
signatures needed to verify it all. There is **no reach-back, no phone-home,
no registry pull, and no runtime that talks to a vendor cloud** (decision
#10, #11; telemetry stays off per P0-M5-T02).

## 2. The flow (build -> transfer -> load -> configure -> start -> verify -> rollback)

```
 BUILD HOST (connected)                     AIR-GAPPED TARGET (no internet)
 -------------                             --------------------------------
 build-offline-bundle.sh
   docker build  (from repo source)
   docker save -> images/*.tar.gz
   MANIFEST.json + SHA256SUMS (per-file)
   gpg --detach-sign (MANIFEST, SHA256SUMS, bundle)
   -> keelworks-offline-<ver>.tar.gz
      keelworks-offline-<ver>.tar.gz.sha256
      keelworks-offline-<ver>.tar.gz.sig
                    │
                    │  USB / ISO / couriered media
                    ▼
                                              airgap-verify.sh --bundle <tarball>
                                                "bundle signature OK"
                                              airgap-install.sh --bundle <tarball>
                                                1 verify   (bundle + manifest sig,
                                                            per-file sha256)
                                                2 extract  -> /opt/keelworks
                                                3 load     docker load (NO pull)
                                                4 configure .env from template
                                                            (update-check OFF)
                                                5 start    docker compose up -d
                                                6 verify   same health checks as
                                                            online install
                                                7 record   airgap-install-record.json
                                              airgap-rollback.sh  (on failure)
```

None of the target steps touch the network. `docker load` only reads local
tarballs; `docker compose up -d` never `--pull`s; the generated `.env` has
`OEP_UPDATE_CHECK=off` by default (decision #11).

## 3. Build the bundle (connected build host)

```bash
cd scripts/airgap
# 1. Build the component images from repo source (example: the PDM service).
docker build -f Dockerfile.oep-pdm -t keelworks/oep-pdm:<ver> ../../

# 2. Sign it (only the release lane holds the secret key; the public key ships
#    in this repo at scripts/airgap/keys/oep-airgap-signing.pub.asc).
gpg --import keys/oep-airgap-signing.pub.asc      # verify pubkey
export SIGN_KEY_ID=oep-airgap-signing             # your secret key uid

# 3. Assemble + sign the bundle (change --images/--compose to your stack).
./build-offline-bundle.sh \
    --version 0.1.0 \
    --images keelworks/oep-pdm:0.1.0 nats:2-alpine \
    --compose docker-compose.airgap.yml \
    --out dist/
```

Output:
- `dist/keelworks-offline-0.1.0.tar.gz` — the bundle (put on USB / burn to ISO)
- `dist/keelworks-offline-0.1.0.tar.gz.sha256` — checksum for media verification
- `dist/keelworks-offline-0.1.0.tar.gz.sig` — detached signature (P11)

## 4. Transfer to the air-gapped host

Copy the three files to the target by approved media. On the target, before
anything else:

```bash
sha256sum -c keelworks-offline-0.1.0.tar.gz.sha256
```

Then verify the signature and the on-media integrity (this does not need the
bundle to be unpacked yet):

```bash
cd scripts/airgap            # or the copy included in the bundle
./airgap-verify.sh --bundle /media/usb/keelworks-offline-0.1.0.tar.gz
```

## 5. Install (no network)

```bash
sudo ./airgap-install.sh \
    --bundle /media/usb/keelworks-offline-0.1.0.tar.gz \
    --install-dir /opt/keelworks \
    --project oep
```

`airgap-install.sh`:

1. Verifies the bundle tarball gpg signature and its sha256 (media integrity).
2. Extracts and verifies the **signed MANIFEST.json**, then checks **every**
   file's sha256 against the manifest — a tampered bit anywhere fails install.
3. `docker load`s every embedded image (<code>images/*.tar.gz</code>). No pull.
4. Generates `/opt/keelworks/.env` from the embedded template: a fresh local
   `OEP_LOCAL_SECRET` is created on the machine; `OEP_UPDATE_CHECK=off` by
   default.
5. Brings the stack up with `docker compose up -d`. The compose file is the
   one bundled (or `--compose`).
6. Waits for the **same health check** the online install uses — for the
   proof stack, `GET http://127.0.0.1:8791/api/v1/health` (the PDM service;
   the bus health is checked via `nats` `/healthz`).
7. Writes `proof/airgap-install-record.json` — host, bundle, health result,
   update-check posture, compose services. This is the audit record for the
   operator and for QA.

If the health check never comes up, the script invokes the rollback path and
fails loudly — it never leaves a half-broken stack pretending to be healthy.

## 6. Verify an installed air-gap stack

```bash
./airgap-verify.sh --bundle /opt/keelworks/../media/keelworks-offline-0.1.0.tar.gz \
    --install-dir /opt/keelworks --project oep
```

Re-runs the bundle signature check, the on-media manifest integrity check, and
the health check, and asserts `OEP_UPDATE_CHECK=off` in the installed `.env`.

## 7. Rollback

If an install must be undone, the stack is stopped and the previous local
image set is restored from the snapshot the installer took before loading the
new images:

```bash
./airgap-rollback.sh --install-dir /opt/keelworks --project oep
```

## 8. Update checker posture (decision #11) — non-phoning by default

In any offline deployment the update checker is **off by default**:

```ini
OEP_UPDATE_CHECK=off
OEP_UPDATE_MIRROR_URL=
OEP_UPDATE_OFFLINE_BUNDLE=
```

An admin may *explicitly* switch it on, and the only permitted sources for an
air-gapped shop are an on-site mirror URL or a path to another **offline
signed bundle** already on the machine. The checker, even when on, is advisory
and non-blocking: the program runs identically whether the source is found,
not found, or unreachable, and never fails closed (decision #11). Telemetry is
always off (P0-M5-T02). The same posture is emitted into every bundle's
`env.example` by `airgap-lib.sh::update_check_env`.

## 9. Proven on real material (this card)

- `keelworks/oep-pdm:airgap-proof-src` — the real P8 PDM service, built from
  this repository's source by `scripts/airgap/Dockerfile.oep-pdm`.
- `nats:2-alpine` — the real message bus the P9 lane adopted
  (P9-M1/T02 message-bus-adr.md).
- The whole path was exercised on the reference host U1 with the target
  network disabled (see `P10-M2/proof/` for the recorded proof):
  bundle built + signed, transferred to an isolated path, verified
  (signature + manifest + per-file sha256), images loaded with **no
  registry**, stack started, and the online-equivalent health checks passed.
- When the P10-M1-T01 one-command compose stack lands (it was unmerged while
  this card ran), the identical tooling wraps those images via `--compose` /
  `--images` — nothing in this path hard-codes the proof stack.

## 10. Files

- `scripts/airgap/build-offline-bundle.sh` — build + sign the bundle (build host)
- `scripts/airgap/airgap-install.sh` — air-gapped install (target)
- `scripts/airgap/airgap-verify.sh` — bundle + stack verification (target)
- `scripts/airgap/airgap-rollback.sh` — rollback to previous image set
- `scripts/airgap/airgap-lib.sh` — shared helpers + update-check posture
- `scripts/airgap/keys/oep-airgap-signing.pub.asc` — verification public key
- `scripts/airgap/Dockerfile.oep-pdm` — real PDM service image (proof subject)
- `scripts/airgap/docker-compose.airgap.yml` — proof stack (PDM + NATS bus)
- `P10-M2/proof/` — recorded proof of the air-gap run on U1
