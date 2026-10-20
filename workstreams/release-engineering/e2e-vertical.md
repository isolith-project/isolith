# Isolith P12-T09 - E2E Scenario Suite (the reference vertical, headless)

Version: 1.0.0 (2026-10-18)
Status: active release-blocking E2E gate (P12 QA / CI-CD / release engineering).

## 1. What this is

The reference vertical demo, scripted as an automated end-to-end test that runs
headlessly against the real stack and asserts each stage's output:

    import CGR/STEP  ->  model (oep-geo / step2mesh)
      ->  release via PDM  ->  render/convert via P2P  ->  share (viewer)

The driver is `scripts/e2e-vertical.sh` wrapping
`python/oep_tooling/e2e_vertical.py`. It follows the same measured GA contract
as every gate in the program: legs with a closed SKIP taxonomy, honest
classification, exit 0 iff no FAIL row, self-consistent JSON + Markdown report
under `workstreams/release-engineering/proof/e2e-vertical-<ver>.{json,md}`.

Its pass is a **release-blocking CI check** (`.gitea/workflows/
ga-gate-p12e2e.yaml`) alongside the GA gates. A broken stage cannot go green:
the suite's own `--break-stage` repro is asserted non-zero in CI and committed
under `workstreams/release-engineering/proof/e2e-vertical-break-<ver>.json`.

## 2. The vertical, stage by stage

| Stage | What runs (real material)                  | What is asserted                              |
|-------|---------------------------------------------|-----------------------------------------------|
| BOOT  | real `oep-web` (PDM API + viewer over one  | `GET /api/v1/health` 200 `{"status":"ok"}`    |
|       | vault store) on the stack                   |                                               |
| IMPORT| committed STEP fixture (`box-reference.step)| item + revision 1 created; blob sha-256 ==    |
|       | as vault item (+ staged real `.cgr` when    | sha(fixture); re-open byte-identical          |
|       | present; SKIP `corpus-missing` otherwise)   |                                               |
| MODEL | committed golden oep-geo model              | `oep-geo` format item, blob sha-verified;     |
|       | (`example-bracket.oepgeo.json`) + real      | `oep-batch-convert run` step2mesh ok=1        |
|       | `oep-batch-convert` lane                     | hash_verified=1                               |
| RELEASE via PDM | PDM lifecycle: lock + transition    | state `released`; reopened byte-identical     |
|       | `working -> under_review -> released`       | (immutable); `released -> working` refused;   |
|       |                                            | audit chain re-verifies when exposed          |
| RENDER via P2P | real `oep-render-worker`           | a real PNG is produced (magic + >=512 bytes)  |
|       | (CPU rasterizer, ships in the image)        |                                               |
| SHARE | the viewer serves the released model        | explorer 200 + lists item; item page 200;     |
|       |                                            | `/ui/items/{id}/preview/{rev}.png` 200 +      |
|       |                                            | `image/png` + `X-Oep-Drawn-Pixels` > 0        |

Closed SKIP taxonomy (a SKIP row always carries exactly one of these, the same
honest contract as the P8/P9/P10/P12 gates):

- `bin-missing`        -- a real binary not built on this ref (CI builds them)
- `stack-unreachable`  -- the booted stack did not answer on its base URL
- `layer-skip`         -- a stage explicitly not applicable (selfcheck / no model)
- `corpus-missing`     -- real `.cgr` sample not staged (clean-room: real CGR is
                           never committed; the STEP lane always runs)
- `audit-unavailable`  -- the service build does not expose the tamper-evident
                           audit log
- `deliberate-break`   -- the `--break-stage` proof run (expected FAIL)
- `io-error`

## 3. Run it

```bash
# CI-safe degraded: contract check only, no stack (runs in CI + precommit)
./scripts/e2e-vertical.sh --selfcheck

# Headless vertical against real local binaries (what CI's gate does after
# `cargo build -p oep-web -p oep-render-worker -p oep-worker --bins`)
BIN_DIR=target/debug ./scripts/e2e-vertical.sh --version 0.1.0

# Against the packaged stack image on a docker host (U1 / a VM-class host)
./scripts/e2e-vertical.sh --version 0.1.0 --mode docker

# The deliberate-failure repro (proves the gate BLOCKS a broken stage)
./scripts/e2e-vertical.sh --version 0.1.0 --break-stage release

# Stage a real .cgr for the CGR import leg (clean-room dev material)
./scripts/e2e-vertical.sh --version 0.1.0 --cgr /path/to/VALVE.cgr
```

Exit code 0 iff the recorded report has ZERO FAIL rows. Non-zero = do not ship.

## 4. Release-blocking in CI

`ga-gate-p12e2e.yaml` runs on every push / PR (and therefore release/* and
v-tag refs too, like the P11 gate). Per ref it:

1. builds the real binaries from the triggering ref (`cargo build
   -p oep-web -p oep-render-worker -p oep-worker --bins`);
2. runs the acceptance test `tests/test_p12t09_e2e_vertical.py`;
3. runs the headless vertical (`scripts/e2e-vertical.sh --version ...`,
   local real binaries): a FAIL here is a red build = release blocked;
4. runs the `--break-stage release` repro and asserts it exits NON-zero (the
   "proves it blocks" check);
5. uploads the E2E reports as build artifacts.

The per-release recorded proof (packaged-stack docker mode, on U1) is committed
under `workstreams/release-engineering/proof/` so the release branch ships a green record alongside the
other gate proofs.

## 5. What makes this honest

- The vertical runs against the REAL stack (real binaries / packaged image),
  the REAL committed golden fixtures, and the REAL worker functions. Nothing is
  mocked or synthesized.
- Real `.cgr` binaries are clean-room dev material and never enter the repo;
  the CGR import leg honestly SKIPs when no sample is staged, exactly like the
  P2/P9 corpus legs.
- The `--break-stage` repro makes the blocking property testable: if any stage
  regresses to FAIL, the gate goes red and the release is blocked.

## 6. Ties

- P12-T01 test pyramid (E2E tier), P12-T02 golden corpus (fixtures it banks),
  P12-T05 release train / P12-T06 install rehearsal (per-release proof
  convention), P10-M1 stack (the deployed surfaces being driven),
  decision #9 (run hosts: U1 + a VM-class second host; NAS repo/storage only),
  decision #10 (on-prem 127.0.0.1 posture), P11-T02 (other release gates the
  E2E joins on every push).
