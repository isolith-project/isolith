# P9-M5-T02 -- P9 GA gate: 100-job render/convert wave across 3+ nodes, outputs verified in PDM

> Task: t_90343243 (P9 capstone). Status: implemented + verified live on the
> real GB10 fleet 2026-10-17. Version 1.
> Repo: isolith/isolith @ NAS Gitea; branch task/p9m5t02-ga-gate-1017a.
> Gate classification: **PASS** -- 100 jobs, 0 manual interventions, 100/100
> outputs hash-verified in the P8 vault, across 3 real nodes; exit-code
> contract + self-consistent JSON/MD report identical to the P2/P3/P4/P8 gates.

## 1. What this card delivers

The P9 General-Availability gate, built on the same measured, honest contract
as every GA gate before it:

| Acceptance | Measured result (live) |
|---|---|
| run a 100-job render/convert wave | 100 real jobs enqueued into the real oep-lane Scheduler (34 step2mesh + 33 cgr2gltf + 33 oep-geo-render), each executed by a real worker function |
| across 3+ nodes | 3 physical NVIDIA GB10 nodes (u1 192.168.0.67, u2 192.168.0.69, u3 192.168.0.70): u1=34 u2=33 u3=33 |
| zero manual intervention | one command (proof script) builds, deploys the node agent to the fleet over SSH, boots the shared vault, runs the wave, verifies |
| every output hash-verified in the P8 vault | 100/100 outputs reopened + re-hashed to their recorded content address; 0 rehash mismatch; audit chain re-verified |

## 2. New surface

- **`crates/oep-gate`** -- the gate crate: `oep-ga-gate` driver (real
  scheduler + node pool + dispatch/ack loop + node transport + independent
  re-hash), `oep-ga-node` uniform per-node worker agent, `render::run_render_job`
  (render outputs mapped onto the same vault/hash contract as convert),
  `fleet` / `manifest` / `report`.
- **`python/oep_tooling/ga_gate_p9.py`** -- the measured harness (P8-gate
  shape: BOOT/WAVE/PDM/NODES/AUDIT legs, closed SKIP taxonomy, exit 0 iff no
  FAIL).
- **`tests/ga_gate_p9_test.py`** -- TDD acceptance (module present, binaries
  present, report self-consistency, live local-only gate).
- **`scripts/compute-ga-gate-proof.sh`** + **`workstreams/compute/ops/fleet.example.json`**
  (real fleet) / **`fleet.local.json`** (CI-safe) / **`workstreams/compute/ops/proof/*`** (the
  measured proof).
- **`.gitea/workflows/ga-gate-p9.yaml`** -- separate CI gate workflow
  (P2/P3/P4/P8 convention) running the real vault + driver local-only on the
  committed corpus.
- **P8 adapter seam fix** (surfaced by the >100-item wave): PDM name
  resolution now pages the whole catalogue, so the wave's earliest seed items
  stay resolvable past the first 100-output page. Additive; v1 wire
  byte-identical.

## 3. Real-material verification transcript (2026-10-17 live)

`bash scripts/compute-ga-gate-proof.sh workstreams/compute/ops/fleet.example.json`

```
P9 GA gate: total=100 ok=100 skip=0 fail=0 hash_verified=100 rehash_mismatch=0
           nodes=3/3 dispatched=100
per-node: u1=34 u2=33 u3=33 | gate report total=5 PASS=5 FAIL=0 SKIP=0 exit 0
```

Every wave output was re-opened from the real P8 vault and re-hashed to its
`FileRef.blob_sha256` (independent of the worker's own check). The audit chain
(P8-M3-T03) re-verifies. Run timestamps and per-job rows are in
`workstreams/compute/ops/proof/ga-gate-p9-wave.json` / `.md`.

CI-safe degraded run (`--local-only`, single node, committed STEP + oep-geo
corpus): same real scheduler/workers/vault/verification path, one node;
`tests/ga_gate_p9_test.py` ALL CHECKS PASSED.

## 4. Honesty note

- Typed skips are preserved (non-convertible inputs are SKIP, never PASS);
  the generated wave cycles mesh-producing fixtures so the live run is
  100/100 verified outputs.
- GPU-preferred placement is honest: `cpu_fallback` recorded per job; render
  (CPU reference backend today) is never GPU-*required*.
- Fleet/vault unreachable is a closed SKIP, never a fabricated PASS.

## 5. What blocks closure, honestly

- NOTHING. Both P9 GA criteria -- 100-job wave across 3+ nodes with zero
  manual intervention, and every output hash-verified in PDM -- are evidenced
  above (PASS, exit 0, CI green, proof committed).

## 6. Boundary / anti-collision

Owns: `crates/oep-gate/**`, `python/oep_tooling/ga_gate_p9.py`,
`tests/ga_gate_p9_test.py`, `scripts/compute-ga-gate-proof.sh`,
`.gitea/workflows/ga-gate-p9.yaml`, `workstreams/compute/ops/ga-gate.md`, `workstreams/compute/ops/proof/ga-gate-p9.*`
+ `ga-gate-p9-wave.*`, `workstreams/compute/ops/fleet.*.json`, docs/layout.md + monorepo test
registration of oep-gate, and the additive PDM adapter catalogue-pagination
seam. Does not edit sibling-owned paths (oep-worker/oep-render-worker/oep-job/
oep-lane test surfaces, P8-M6 proof files). `pending-corpus/` stays gitignored
(clean-room dev-only material).
