# P9-M5-T02 -- P9 GA gate: 100-job render/convert wave across 3+ nodes, outputs verified in PDM

Status: implemented + verified live on the real GB10 fleet (2026-10-17).
Owner: P9 compute lane. Version: 1.

## 1. What this gate is

This is the **P9 General-Availability gate**, the same shape as the P2/P3/P4/P8
gates before it: a measured, honest acceptance harness with a closed SKIP
taxonomy and the exit-code contract (0 iff no FAIL row). What it measures is
the P9 wave itself:

> run a **100-job render/convert wave across 3+ nodes**, zero manual
> intervention, every output **hash-verified in the P8 vault**.

The difference from every earlier gate is the unit of "real material": here
the real material is a **live fleet** (three physical NVIDIA GB10 nodes on the
LAN), a **real scheduler** (oep-lane), **real worker functions** (the P9-M3
render + convert lanes), and the **real P8 vault** (oep-pdm, the P8-M6 GA-gate
service). Nothing is mocked or synthesized.

## 2. What was built

### crates/oep-gate (new)

The gate driver and node agent:

- **`oep-ga-gate`** (driver) -- connects to the live vault, seeds the real
  corpus, builds the real `oep_lane` Scheduler + NodePool from the fleet's
  real nodes, enqueues the 100 JobSpecs, runs the dispatch/ack loop on the
  wall clock, executes each dispatch on its placement node, re-hashes every
  output from the vault, and writes the wave report.
- **`oep-ga-node`** (agent) -- the uniform per-node worker: reads a JobSpec
  on stdin, resolves the input from the SHARED vault, runs the real worker
  function (convert via `oep_worker`, render via `oep_gate::render`), stores
  the output back to the vault, hash-verifies, prints the outcome as JSON.
  Deployed to remote fleet members by the proof script.
- **`render.rs`** -- `run_render_job`: the render peer of the convert lane's
  `run_job`, wiring the headless oep-geo renderer to the same vault + output
  target + hash-verification contract (the P9-M3-T01 doc named exactly this:
  render bytes map onto the job's output refs, P9-M5-T02).
- **`fleet.rs` / `manifest.rs` / `report.rs`** -- fleet model (real nodes),
  the deterministic 100-job wave (STEP->mesh, CGR->glTF, oep-geo render, each
  with a unique vault output item), and the self-consistent report.

### python/oep_tooling/ga_gate_p9.py

The measured gate harness in the exact P8-gate shape: BOOT (real vault),
WAVE (the driver run), PDM (independent re-hash), NODES (>=3 with real work),
AUDIT (tamper-evident chain). Writes `ga-gate-p9.{json,md}`.

### Other

- `scripts/p9m5t02-ga-gate-proof.sh` -- one command: builds, deploys the agent
  to the remote fleet (scripted SSH), boots the LAN-addressable vault, runs
  the wave, writes the proof.
- `P9-M5/fleet.example.json` (real nodes) + `fleet.local.json` (CI-safe
  single-node), `P9-M5/proof/*` (the committed measured proof).
- `crates/oep-gate/src/render.rs` render-to-vault worker.
- P8 seam fix surfaced by the >100-item wave: the PDM adapter's name
  resolution now pages the whole catalogue (a >100-item vault used to hide
  its earliest items from a single newest-first page). Additive; the existing
  v1 wire is byte-identical.

## 3. How a job runs

1. **Seed** -- the proof script seeds the real corpus into the shared vault
   (STEP fixtures + real CGR CATPart via the P9-M3-T02 seed manifest shape,
   plus the oep-geo golden model).
2. **Enqueue** -- the driver builds the real Scheduler over a NodePool of the
   fleet's real nodes (real GPU resources), and enqueues 100 JobSpecs with a
   GPU-preferred request so the real placement policy spreads the wave by
   exclusive device reservation.
3. **Dispatch/execute** -- the driver pulls dispatches on the wall clock;
   each dispatch's placement node executes the job: in-process worker
   functions on the coordinator, `oep-ga-node` over SSH on remote nodes, all
   against the **same** LAN-addressable vault. Node agents resolve inputs
   from the vault, run the real render/convert functions, store outputs, and
   hash-verify (vault blob sha == artifact sha). The driver acks/fails the
   scheduler with the real generation.
4. **Verify** -- after the wave, every output is reopened from the vault and
   re-hashed independently (the driver) and the audit chain is re-verified
   (the harness).

## 4. The measured proof (live, 2026-10-17)

Fleet: u1 (promaxgb10-83f1, 192.168.0.67), u2 (promaxgb10-9e14,
192.168.0.69), u3 (promaxgb10-9e46, 192.168.0.70) -- three physical NVIDIA
GB10 nodes.

```
summary: total=100 ok=100 skip=0 fail=0 hash_verified=100 rehash_mismatch=0
         nodes=3/3 dispatched=100
per-node: u1=34  u2=33  u3=33
executors: convert:step2mesh=34  convert:cgr2gltf=33  render:oep-geo-render=33
```

Every one of the 100 wave jobs produced an artifact, stored it in the real P8
vault, and was re-opened + re-hashed to its recorded content address: 100/100
hash-verified. The wave ran with **zero manual intervention** (one command
does build + deploy + vault + wave + verify).

## 5. Honesty boundaries

- Skip classification is preserved: a non-convertible/non-renderable input is
  a typed SKIP row (never a fabricated PASS, never a FAIL). The generated
  wave intentionally cycles mesh-producing STEP fixtures so the live run is
  dominated by verified outputs; the skip-capable fixtures are still seeded.
- A reachable node / vault is required for its leg to PASS; unreachable is a
  closed SKIP (tolerant doctrine), never an invented PASS.
- GPU-preferred placement is reported per job (`cpu_fallback` flag); the
  reference renderer is the documented CPU backend, so render jobs are never
  announced as GPU-*required*.
- `--local-only` mode (single-node, no fleet SSH) is the CI-safe degraded
  run: same real scheduler, workers, vault and verification, one node.

## 6. Verification

- `cargo test --workspace` green (355 tests) incl. the new `report`
  round-trip unit test; `cargo clippy --workspace --all-targets -- -D warnings`
  clean; `cargo fmt --check` clean.
- `python3 tests/ga_gate_p9_test.py` PASS (module present, binaries present,
  report self-consistency, live local-only gate).
- Live 3-node proof: `bash scripts/p9m5t02-ga-gate-proof.sh` exit 0 with
  `P9-M5/proof/ga-gate-p9.{json,md}` + `ga-gate-p9-wave.json` (measured rows).
