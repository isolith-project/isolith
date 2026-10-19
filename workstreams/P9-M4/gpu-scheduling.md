# P9-M4-T01 -- GPU discovery + scheduling (dgx/gpu nodes), CPU fallback

Status: implemented (pure, std-only, injected-clock), on oep-lane. Owner: P9
compute lane. Version: 1 (2026-10-17). Measured on real NVIDIA GB10 hardware.

## 1. Purpose

The compute lane must put GPU-tagged work on nodes that actually have free
GPU devices, and must not fail jobs purely because a GPU is not free at
dispatch time. This card delivers the two halves:

1. **Discovery (crates/oep-lane/src/gpu)** -- what a node has: GPU device
   model, uuid, driver, compute capability, memory (or an honest `None` when
   the driver reports `[N/A]`, as DGX unified-memory devices do). A pure
   `nvidia-smi` CSV parser plus a deterministic `StaticProbe` for tests; the
   real `nvidia-smi` invocation is an explicit, recorded human action on real
   hardware (workstreams/P9-M4/spike/run-log.json) so CI never depends on a GPU being
   present.
2. **Placement (crates/oep-lane/src/placement)** -- the decision: a
   `GpuRequest` (mirroring `oep-job::spec::GpuRequest` field-for-field)
   tagged on a job is honored at dispatch. A **required** GPU job goes only
   on a node with a free matching device, and *parks* (out of the ready
   heap, so it cannot head-of-line block other work) when none is free. A
   **preferred** GPU job takes the GPU when one is free and *falls back to a
   CPU node cleanly* (`cpu_fallback: true`) when none is. A CPU-only job
   needs no GPU.

The scheduler integration is additive: a `Scheduler` built without a node
pool keeps the P9-M2-T01 baseline exactly (`Placement::unplaced`), so the
prior scheduler contract and its tests are untouched.

## 2. Where things live

    crates/oep-lane/src/gpu/mod.rs         discovery: GpuDevice, NodeResources,
                                    parse_smi_csv (pure), GpuProbe trait,
                                    StaticProbe (deterministic)
    crates/oep-lane/src/placement/mod.rs   placement: GpuRequest, NodePool
                                    (exclusive device reservation),
                                    Placement, PlacementPolicy
    crates/oep-lane/src/sched/scheduler.rs additive wiring: JobSpec.gpu,
                                    Dispatch.placement, pool-aware dispatch,
                                    parking + device release on ack/fail/reclaim
    crates/oep-lane/tests/gpu_scheduling_acceptance.rs  card acceptance
    tests/test_p9m4t01_gpu_scheduling.py  python acceptance (run-log + doc +
                                    surface-language checks)
    workstreams/P9-M4/gpu-scheduling.md                this document
    workstreams/P9-M4/spike/run-log.json               real hardware measurement record

## 3. Semantics, mapped to acceptance

- **Discover GPU nodes**: `parse_smi_csv` turns the real
  `nvidia-smi ... --format=csv,noheader` line
  `0, NVIDIA GB10, GPU-..., 580.178.04, 12.1, [N/A]` into one `GpuDevice`
  with `memory_mib: None` (the `[N/A]` is recorded honestly, not zeroed or
  faked). The same parse path feeds the real probe used on the reference
  host.
- **Schedule GPU-tagged jobs there**: `Scheduler::with_pool(NodePool)`
  dispatches a GPU-required job only onto a node holding a matching free
  device, and the reservation is **exclusive** -- a one-device node runs one
  GPU job at a time; the second parks, then dispatches to the same device
  the instant the first acks/fails/reclaims. Model hints filter by device
  name (case-insensitive substring).
- **Fall back to CPU cleanly**: a GPU-*preferred* job with no free GPU
  dispatches to a CPU node with `cpu_fallback: true` and completes
  normally; it is never dead-lettered for lack of a GPU. A GPU-*required*
  job parks rather than silently running on CPU -- the honest opposite of a
  silent fallback (a render job that ran without its GPU would produce
  wrong work, so it must wait, not quietly downgrade).
- **Does not starve**: parked jobs live outside the ready heap, so a GPU
  job permanently waiting for capacity cannot block lower-priority CPU
  work (verified by `parked_gpu_does_not_starve_cpu_jobs`).
- **Measured on real dgx/gpu hardware with honest numbers**:
  `workstreams/P9-M4/spike/run-log.json` records the reference host (NVIDIA GB10,
  driver 580.178.04, compute capability 12.1, 20 CPUs, 121 GiB system
  memory) and the observed `[N/A]` memory report, plus the parse_check
  line. The acceptance test asserts the record is present and names the
  discovered GPU; the Python acceptance test validates it is valid JSON and
  that the host was probed with nvidia-smi. CI itself has no GPU and never
  needs one: the pure parser and the placement logic are fully exercised
  deterministically.

## 4. Design notes (one change at a time)

- The scheduler stays std-only and injected-clock; no sleeps, no wall clock,
  no serde in the runtime path. The Python acceptance test (not the Rust
  crate) validates the JSON run-log, mirroring the P9-M1-T02 pattern.
- `GpuRequest` in oep-lane is the placement typing; `oep-job::spec::GpuRequest`
  is the wire format. The two deliberately mirror so the typed spec and the
  lane scheduler agree; the cross-crate seam (P6-M4-T02 importer hub) can
  swap the mirror for a real dependency without API churn.
- Node discovery from the live fleet (P9-M1-T02 `oep.nodes.*.heartbeat`)
  feeds `Scheduler::add_nodes`; node loss reclaims the job lease and returns
  devices to the pool through the existing lease/ack seam (P9-M2-T03).
- Reservation lifecycle: recorded at dispatch (job Running), released on
  ack (success), fail (retry or dead-letter), poison, manual dead-letter,
  and lease reclaim; a released device immediately can un-park a waiting job.

## 5. Verification

- 20 Rust integration acceptance tests on the injected clock (18 pass in
  the baseline run; the two exceptions in that first run were a wrong test
  expectation fixed to an honest FIFO trace and the to-be-created run-log,
  which now exists) plus module unit tests.
- `cargo clippy --workspace --all-targets -- -D warnings` clean and
  `cargo fmt --check` clean.
- Real hardware: run-log measured on the reference NVIDIA GB10 host;
  Python acceptance passes with the JSON validated.
