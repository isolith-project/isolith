# P9-M2-T02 -- Fair share per team/node + quota enforcement

Status: implemented. Owner: P9 P2P compute lane. Version: 1 (2026-10-17).

## 1. Purpose

A P9 scheduler must not let one team monopolize the compute pool, and the
pool operator must be able to bound what each team may run concurrently. This
card delivers the **admission policy** the P9 scheduler (P9-M2-T01, the
`sibling oep-lane` crate) consults before dispatching a job. It is
self-contained (no cross-crate dependencies, per the pre-P6-M4-T02 monorepo
rule) because the scheduler spec, job spec and this policy live on separate
unmerged branches.

Three guarantees, matching the card acceptance:

1. **Quotas configured and enforced under load.** Every team gets a hard
   ceiling on concurrently-running CPU, memory and job count. A submission
   that would exceed a cap is throttled **even when the cluster is idle** --
   the `quota_configured_and_enforced_under_load` test proves an
   oversubscribed team is cut off at exactly its cap.
2. **Per-node fair-share scheduling.** On each node the scheduler recomputes a
   weighted max-min allocation among the teams with demand there. Two
   equal-weight teams saturate a node 50/50 no matter how aggressively one
   submits (`per_node_fair_share_saturates_50_50`); a 3x-weight team gets 75%.
3. **Conservative defaults.** A team with no explicit quota entry gets an
   equal slice of the whole cluster (total capacity / team count, at minimum
   a 50% split assumption) -- an unconfigured team can never hoard the pool.

## 2. Where it lives

- Code: `crates/oep-fairshare/` (`oep_fairshare`, workstream P9).
  - `config.rs` -- [`Node`]/[`NodeCapacity`]/[`TeamQuota`]/[`Config`] with
    `serde` wire round-trip and non-panicking validation.
  - `fair.rs` -- `weighted_max_min`: exact-integer water-filling (the
    per-node fairness primitive).
  - `enforce.rs` -- `FairShareScheduler`: the live admission gate (quota
    gates, node capacity gates, per-node fair-share gates) plus
    `complete()` to release finished work.
- Wire format: `docs/compute/scheduler/fairshare.sample.json` -- the committed,
  operator-visible config the acceptance tests load byte-for-byte.
- This doc.

## 3. The model

    TeamQuota      = { weight, max_concurrent_cpu_millicores,
                       max_concurrent_memory_mib, max_concurrent_jobs }
    NodeCapacity   = { cpu_millicores, memory_mib }
    Config         = { format=oep-fairshare, version=1,
                       nodes: [Node], teams: {team: TeamQuota} }

Every `admit(team, node, request)` answer is one of:

    Admitted
    Throttled { CpuQuota | MemoryQuota | JobQuota
              | NodeCpuCapacity | NodeMemoryCapacity
              | NodeFairShareCpu | NodeFairShareMemory
              | UnknownNode | UnknownTeam }

Gates are checked in this order (first violation wins):

1. Cluster-wide quota (CPU, memory, job count) -- **quota enforcement**.
2. Node physical capacity -- a node can never be oversubscribed.
3. Per-node weighted max-min fair share (CPU and memory independently) --
   **per-node fair-share scheduling**.

## 4. Conservative defaults

- Explicit `teams` entries are used exactly as configured.
- An **undeclared** team (one the scheduler has never seen) gets
  `weight = 1` and caps equal to `total_cluster / max(2, declared + 1)`, so a
  brand-new team can never claim a majority of the pool. This is deliberately
  strict; operators raise it via `teams` config.
- Weights must be `>= 1`; zero-weight configs are rejected at build time.

## 5. Test evidence (all green, real integers, no fakes)

- `quota_configured_and_enforced_under_load`: alpha requests 5x4000 cpu on an
  8000-cpu node; exactly 2 admit, 3 throttle; beta admitted freely. Proves
  the card's "oversubscribed teams get throttled".
- `per_node_fair_share_saturates_50_50`: alpha+beta both try to claim a
  16000-cpu node; each lands at 8000, node at 16000, further requests hit
  `NodeFairShareCpu`. Proves per-node fair share.
- `weights_drive_fair_share`: 3x-weight beta vs alpha on 16000 cpu => 12000 /
  4000. Weights scale shares.
- `unconfigured_team_gets_conservative_default`: a new "ghost" team cannot
  run past its equal-slice default on an empty cluster.
- `per_node_never_over_allocates_capacity`: interleaved load wave never
  oversubscribes either node.
- `fair.rs` unit suite: exact water-filling edge cases (empty, low demand,
  weights, three-way, overflow-scale capacities).
