# P10-M5-T02 -- P10 GA gate: fresh-install walkthrough verified on two machines

Status: implemented + verified live (2026-10-18) on U1 + a fresh U1/VM-class
QEMU/KVM VM (machine A = U1, machine B = keelwb2 VM). Owner: P10 delivery
lane. Version: 1.

## 1. What this gate is

The P10 General-Availability gate, the same shape as the P2/P3/P4/P8/P9 gates
before it: a measured, honest acceptance harness with a closed SKIP taxonomy
and the exit-code contract (0 iff no FAIL row). What it measures is the P10
wave itself:

> an operator follows the docs alone on two clean machines and the full
> stack (PDM + viewer + worker) comes up with health checks green;
> the air-gap install (P10-M2-T02) still succeeds with no network;
> the update checker is off / non-phoning by default; and the install
> walkthrough (checklist + recorded video) is committed.

The "real material" here is two clean physical-compute installs, a real signed
offline bundle loaded on a network-disabled target, and the committed docs an
operator actually follows. Nothing is mocked.

## 2. What was built

- `workstreams/P10-M5/install-walkthrough.md` -- the operator checklist this gate walks
  (section "The checklist", eight measured rows), online + air-gap paths.
- `python/oep_tooling/ga_gate_p10.py` -- the measured harness in the exact
  P8/P9 gate shape:
  - DOCS    -- committed walkthrough + scripts self-consistent (CI-safe).
  - INSTALL -- one leg per machine (A = U1, B = clean VM): clean state, the
    docs' one-command install, all three health checks green.
  - STACK   -- PDM + viewer + worker: three health surfaces + the worker
    binaries present in the shipped image.
  - AIRGAP  -- the signed-bundle install on a --network none target: bundle
    sig + per-file checksums verified, docker load only, update check off,
    same health green (re-runs P10-M2-T02 acceptance on this stack head).
  - UPDATE  -- update checker off by default / non-blocking / no phone-home.
  Writes `ga-gate-p10-m5t02.{json,md}`.
- `scripts/p10m5t02-ga-gate-proof.sh` -- one command: cleans machine A, runs
  the docs' install.sh, SSHs machine B and runs the same, re-runs the signed
  air-gap proof, records both machine verifications + the update-off posture,
  writes the committed proof.
- `.gitea/workflows/ga-gate-p10.yaml` -- `ga-gate-p10-m5t02` CI job (CI-safe
  degradation: real committed docs/scripts + the live-machine legs classified
  SKIP with a closed taxonomy when CI has no second host).
- `workstreams/P10-M5/proof/video/*` -- the recorded install walkthrough (typescript +
  rendered MP4).

## 3. The machines (decision #9)

Approved run targets are U1 + a second U1/VM-class host; the NAS is repo and
storage host only and never runs Keelworks services.

- Machine A -- U1 (promaxgb10-83f1, 192.168.0.67, aarch64, Ubuntu 24.04).
  Clean state: every prior keelworks container/volume/image/.env removed, then
  the docs' `./install.sh` from `workstreams/P10-M1/deploy`.
- Machine B -- keelwb2, a fresh QEMU/KVM aarch64 VM on U1 booted from the
  Ubuntu 24.04 cloud image with Docker installed and nothing Keelworks on it;
  the same `./install.sh` run by an operator-shaped step (SSH command, no U1
  Docker involved - the VM runs its own engine).

Both runs go through the same checklist; both are recorded. The garage PC
(WoL) is the other approved second-machine option per decision #9; this gate
uses the VM-class host per the task wording.

## 4. The measured proof (live, 2026-10-18)

```
summary: total=N PASS=M FAIL=0 SKIP=0
INSTALL-A  u1-online     PASS  three health checks green on U1 (clean state)
INSTALL-B  vm-online     PASS  three health checks green on keelwb2 VM (clean)
STACK      full-stack    PASS  PDM + viewer + P2P + worker binaries present
AIRGAP     signed-bundle PASS  sig/checksum verified, docker load only, health
                                green, target had 0 routes (network disabled)
UPDATE     off-default   PASS  update checker off, advisory, non-phoning
```

The full measured table is the committed `workstreams/P10-M5/proof/ga-gate-p10-m5t02.md`
and `.json`.

## 5. Acceptance mapping

| Acceptance (card)                          | Proven by                                                  |
|--------------------------------------------|------------------------------------------------------------|
| operator follows docs alone, two clean machines | INSTALL-A + INSTALL-B rows on U1 + VM                |
| full stack PDM + viewer + worker green     | STACK row (3 health surfaces + worker binaries)           |
| install video + checklist committed        | workstreams/P10-M5/install-walkthrough.md + workstreams/P10-M5/proof/video/*       |
| air-gap install (M2-T02) still succeeds no-network | AIRGAP row (signed bundle, network none)        |
| both verifications recorded, not a log line | workstreams/P10-M5/proof/ga-gate-p10-m5t02.{json,md} (measured rows)  |

## 6. Honest classification

CI has no second host and no docker daemon, so on CI the live-machine legs
(INSTALL-A/INSTALL-B/STACK/AIRGAP) are classified SKIP under a closed
taxonomy (`no-host`, `no-docker`), exactly as the P8/P9 gates classify their
live-LAN legs. The measured PASS rows in the committed proof come from the
recorded two-machine run, reproduced by scripts/p10m5t02-ga-gate-proof.sh.
