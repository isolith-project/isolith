# Keelworks P10-M5-T02 - Fresh Install Walkthrough and Checklist

Version: 1.0.0 (2026-10-18)
Status: P10 GA gate - verified on two approved machines (U1 + a U1/VM-class VM),
recorded in P10-M5/proof/ga-gate-p10-m5t02.json/.md + the recorded install
video under P10-M5/proof/video/.
Owner: P10 hybrid-cloud delivery lane (GA gate card t_e3843afe).
Card: P10-M5-T02 on the Keelworks board.
Ties to: decision #9 (run targets: U1 + a second U1/VM-class host; NAS stays
repo/storage only), decision #10 (on-prem / air-gap is the default posture),
decision #11 (update checker advisory + off by default).

## 1. What this walks you through

A brand-new machine with Docker installed but nothing Keelworks on it. You
follow this document top to bottom on that machine, and at the end the full
stack comes up with every health check green:

| service  | binary          | container       | host port | health surface                    |
|----------|-----------------|-----------------|-----------|------------------------------------|
| PDM      | oep-pdm         | keelworks-pdm   | 8787      | GET /api/v1/health                 |
| Viewer   | oep-web         | keelworks-viewer| 8790      | GET /  (explorer HTML, HTTP 200)   |
| P2P      | oep-opsdash     | keelworks-p2p   | 8080      | GET /healthz                       |

"Worker" (the task wording) is the P2P compute lane plus the actual worker
binaries shipped in the same image (oep-batch-convert, the convert worker, and
oep-render-worker, the render worker) - verified in steps 6-7. No component
phones home; the update checker is off by default (decision #11).

This is a GA gate, so it follows the same measured contract as the P2/P3/P4/
P8/P9 gates: measured criteria, honest classification, and both verifications
recorded as proof, not as a log line.

## 2. Prerequisites (on each target machine)

- Ubuntu 24.04 LTS (aarch64 or amd64) or Debian 12. Other distros work if the
  pieces below exist. A U1/VM-class host (KVM/libvirt/qemu VM on U1, or the
  garage PC via WoL) is the approved second-machine class (decision #9).
- Docker Engine 24+ running (`docker info` succeeds) and Docker Compose v2
  (`docker compose version`).
- ~5 GB free disk, `curl`, `git`, and (online path) outbound internet to
  fetch the build base images and crates. For the air-gap path instead of
  internet you need the signed offline bundle from P10-M2 (section 7).
- Nothing else. There is no separate database, no secret handshake, no admin
  bootstrap step.

## 3. Get the deploy directory onto the machine

Online path (clone the repo, then work from the deploy dir):

```bash
git clone http://<gitea>/keelworks/keelworks.git keelworks
cd keelworks/P10-M1/deploy
```

Air-gap path: copy the signed bundle + repo scripts off the USB/ISO exactly as
P10-M2/air-gap-install.md describes (section 7 below).

## 4. One-command install

```bash
./install.sh
```

That is the whole install. From a clean state it:

1. preflights docker + compose + disk and writes `.env` from `.env.example`
   (host binds default to 127.0.0.1 - nothing exposed to the LAN);
2. builds the single stack image `keelworks/oep-stack:0.1.0` (one image,
   three services: pdm, viewer, p2p);
3. `docker compose up -d`;
4. waits until all three compose healthchecks are green (up to 180s);
5. curls each health surface directly and prints the operator summary.

Exit code 0 iff all three surfaces answer green. To stop: `./install.sh --down`.

## 5. The checklist (run these, in order)

| # | Step | Command | Expected |
|---|------|---------|----------|
| 1 | Clean state confirmed | `docker ps --filter name=keelworks- --format '{{.Names}}'` | (empty - no keelworks containers) |
| 2 | Install | `./install.sh` | `All 3 services healthy.` and exit 0 |
| 3 | PDM health | `curl -fsS http://127.0.0.1:8787/api/v1/health` | `{"status":"ok","service":"oep-pdm","version":"1.0.0"}` |
| 4 | Viewer health | `curl -fsS -o /dev/null -w '%{http_code}' http://127.0.0.1:8790/` | `200` |
| 5 | P2P health | `curl -fsS http://127.0.0.1:8080/healthz` | `{"status":"ok"}` |
| 6 | Convert worker shipped | `docker run --rm --entrypoint oep-batch-convert keelworks/oep-stack:0.1.0 --help` | usage text, exit 0 |
| 7 | Render worker shipped | `docker run --rm --entrypoint oep-render-worker keelworks/oep-stack:0.1.0 --describe` | JSON descriptor, exit 0 |
| 8 | Update checker off | `grep -c 'OEP_UPDATE_CHECK=on' .env; echo $?` | `1` (grep finds nothing - feature stays off) and nothing in the compose stack phones home |

For step 8, the stronger statement: the generated `.env` has NO
`OEP_UPDATE_CHECK=on` line at all (the checker only runs when an operator
explicitly points it at a source), and the whole stack binds to 127.0.0.1 on
the host. The advisory checker itself (`oep-update-check`) with no config
reports `Off` and never blocks launch - that is pinned by
crates/oep-update/tests/update_checker_acceptance.rs.

## 6. Done

When all eight rows above match, the machine is a verified Keelworks P10
install. Run the same on the second machine. Record both runs (the proof
script does this for you) - a GA gate needs two recorded verifications, not
one.

## 7. Air-gap install (M2-T02, still green on this stack)

No internet on the target at all. On a connected build host make the signed
bundle (P10-M2 tooling):

```bash
cd scripts/airgap
./build-offline-bundle.sh --version 0.1.0 \
  --images keelworks/oep-pdm:airgap-proof-src nats:2-alpine \
  --compose docker-compose.airgap.yml --out dist/
```

Carry `keelworks-offline-0.1.0.tar.gz` + `.sha256` + `.sig` to the target by
USB/ISO. On the target (no network):

```bash
sha256sum -c keelworks-offline-0.1.0.tar.gz.sha256
sudo ./airgap-verify.sh --bundle keelworks-offline-0.1.0.tar.gz
sudo ./airgap-install.sh --bundle keelworks-offline-0.1.0.tar.gz \
     --install-dir /opt/keelworks --project oep
```

Result: signature + per-file checksums verified, images docker-loaded (no
pull), `OEP_UPDATE_CHECK=off` in the generated .env, same health checks green.
Details and the current proof: P10-M2/air-gap-install.md + P10-M2/proof/.

## 8. Proof on two machines (this gate)

The two recorded runs for this gate are machine A = U1
(promaxgb10-83f1, 192.168.0.67, aarch64) and machine B = a fresh U1/VM-class
QEMU/KVM VM (keelwb2, Ubuntu 24.04 cloud image) - per decision #9 approved
run targets, NOT the NAS. The proof script scripts/p10m5t02-ga-gate-proof.sh
re-runs this whole checklist on both machines and writes
P10-M5/proof/ga-gate-p10-m5t02.{json,md}; the install itself is recorded as a
terminal typescript + rendered video in P10-M5/proof/video/.

Layout of the deliverable:

```
P10-M5/
  install-walkthrough.md          this document (the checklist)
  ga-gate.md                      the gate write-up (measured contract)
  proof/
    ga-gate-p10-m5t02.json        measured rows, both machines, exit-code contract
    ga-gate-p10-m5t02.md          human-readable gate report
    video/                        install walkthrough recording
      machine-a-online.typescript (+ .timing)  U1 online install session
      machine-b-online.typescript (+ .timing)  VM online install session
      airgap-and-updateoff.typescript (+ .timing)
      keelworks-install-walkthrough.mp4        rendered replay of the above
```

## 9. Version history

- 1.0.0 (2026-10-18): initial GA-gate walkthrough; online on U1 + VM, air-gap
  re-proven, update checker off asserted.
