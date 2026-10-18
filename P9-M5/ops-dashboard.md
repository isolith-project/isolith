# P9-M5-T01 -- Ops dashboard: queue depth, node health, job logs, cost/energy

Status: implemented (std-only, served over HTTP). Owner: P9 compute lane.
Version: 1 (2026-10-17).

## 1. Purpose

The P9 compute lane needed an operations surface: a live view of **queue
depth**, **node health**, **job logs** and a **cost/energy view**, readable
by a human on the NAS and by any automation that needs the lane's state. This
card delivers that surface as `crates/oep-opsdash`.

The data-reality contract is the whole point: **every number the dashboard
shows is read from the real `oep-lane` Scheduler (P9-M2-T01) or derived from
its real lease/ack lifecycle events. Nothing is mocked.**

- Queue counters (`waiting`/`ready`/`running`/`retrying`/`succeeded`/
  `dead-lettered`) come from the scheduler's own counters (`pending_count`,
  `ready_count`, `running_count`, `dead_letters`).
- The dead-letter log is the scheduler's `dead_letters()` record.
- Per-job statuses come from `Scheduler::status`, generation from
  `dispatch_generation_of`.
- Node health is a **projection of real lifecycle events**: a node is online
  when it receives a dispatch, `degraded` when the scheduler reclaims one of
  its jobs (lease expired without an ack), `down` when its heartbeat goes
  stale.
- The job log is a ring buffer the dashboard writes when it observes each real
  transition (enqueue, dispatch, ack, fail, poison, reclaim).
- Cost/energy is a documented, integer-only model over **real measured
  runtimes** (the dashboard clock delta between dispatch and ack/reclaim),
  times each node's declared draw (W) and rate (cents/kWh). It is pure u64/u128
  math so it is exact and reproducible in CI.

## 2. What was built

    crates/oep-opsdash/Cargo.toml            std-only crate, depends on oep-lane
    crates/oep-opsdash/src/dashboard.rs      Dashboard: owns the real Scheduler,
                                             NodeRegistry, JobLog, EnergyLedger;
                                             ops API (enqueue/dispatch/ack/fail/
                                             poison/reap) + snapshot()
    crates/oep-opsdash/src/model.rs          serde snapshot wire format (the
                                             JSON contract, one source of truth)
    crates/oep-opsdash/src/registry.rs       NodeRegistry: health from real events
    crates/oep-opsdash/src/log.rs            JobLog ring buffer
    crates/oep-opsdash/src/cost.rs           EnergyLedger + pure energy/cost model
    crates/oep-opsdash/src/clock.rs          injected Clock (SystemClock/ManualClock)
    crates/oep-opsdash/src/demo.rs           deterministic proof wave + live driver
    crates/oep-opsdash/src/html.rs           render snapshot -> single-page HTML
    crates/oep-opsdash/src/http.rs           std-only HTTP/1.1 server
    crates/oep-opsdash/src/main.rs           CLI: serve / snapshot / render
    crates/oep-opsdash/tests/opsdash_acceptance.rs   card acceptance (real code)
    tests/test_p9_m5_ops_dashboard.py        python acceptance on the real binary
    docs/layout.md / P9-M5/proof/*           registered + proof artifacts

The crate is deliberately std-only (plus serde/serde_json for the JSON
contract and the real `oep-lane` dependency), matching the lane doctrine from
P9-M2-T01: hermetic CI, deterministic tests via the injected clock.

## 3. CLI

    oep-opsdash serve --listen 127.0.0.1:8080 [--live] [--lease-ms N]
        serve the dashboard over std HTTP (endpoints below)
    oep-opsdash snapshot [--out snapshot.json] [--lease-ms N]
        run the deterministic proof wave, dump the real snapshot JSON
    oep-opsdash render   [--out dashboard.html] [--lease-ms N]
        run the proof wave, render the HTML dashboard

HTTP endpoints:

    GET /                rendered HTML dashboard (all four views)
    GET /api/snapshot    full JSON snapshot
    GET /api/queue       queue counters + dead-letter log
    GET /api/nodes       node health rows
    GET /api/jobs        job statuses + log lines
    GET /api/cost        cost/energy ledger + totals
    GET /healthz         liveness (200 ok)

`--live` spawns a background driver so the served dashboard visibly moves
(enqueue -> dispatch -> real 30ms work -> ack -> reap, on the wall clock);
without it the dashboard serves whatever state exists. On a NAS deploy you
would run `oep-opsdash serve --listen 0.0.0.0:8080 --live` behind the
existing reverse proxy and point firewalls at 8080.

## 4. The proof wave (deterministic, real)

`demo::run_wave` drives a real scheduler with a fixed mix, exactly:

1. happy render job on u1            -> succeeded (real 500ms clock runtime)
2. happy convert job on nas          -> succeeded (real 250ms clock runtime)
3. job on u1 fails once, retries     -> succeeded (backoff window honored)
4. poison job on u1                  -> dead-lettered (terminal, DLQ row)
5. job on u1 whose lease is never acked -> reclaimed by the scheduler; u1
   is marked `degraded` for it (real `reclaim_expired`)

The snapshot at the end of this wave is `P9-M5/proof/snapshot.json`
(committed) and shows: 3 succeeded, 1 dead-lettered, 1 still-ready reclaimed
job; u1 degraded with 1 reclaimed job; and honest positive cost/energy
(mWh + micro-USD) accumulated only from the runtimes that really elapsed.

## 5. Acceptance mapping

- queue depth           -> `snapshot.queue` counters equal the scheduler's own
                           counts (asserted in opsdash_acceptance.rs)
- node health           -> u1 shows `degraded` + `jobs_reclaimed: 1` after the
                           real reclaim; a freshly registered node is online
- job logs              -> real lifecycle lines: 5 enqueues, 6 dispatches
                           (one job retried), 3 acks, 1 poison, 1 reclaim,
                           time-ordered; all from observed transitions
- cost/energy view      -> energy/cost > 0 derived from real measured runtimes
                           (test: 350W x 3.6s = 350 mWh exactly); HTML renders
                           "Cost / Energy" with node rows and totals
- served on NAS deploy  -> std HTTP server (healthz, /api/*, /) proven by the
                           python acceptance test against the real binary
- data real from the scheduler, not mocked -> every count comes from
                           `Scheduler::*`; the anti-mock guarantee is the
                           assertion that snapshot counters == scheduler
                           counters job-by-job

## 6. Verification

- 14 Rust tests (8 unit + 6 integration) all green; clippy -D warnings and
  fmt clean; works within the P9-M2-T01 scheduler spine.
- Python acceptance `tests/test_p9_m5_ops_dashboard.py` passes against the
  real binary (build, wave, HTTP, HTML, JSON).
- Proof artifacts: `P9-M5/proof/snapshot.json` + `P9-M5/proof/dashboard.html`
  generated from the real wave on U1.
