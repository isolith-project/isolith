# UI-D9 UI-PERF-PROFILE: per-stage frame cost split and the render-seam performance ADR

Card: t_375155c9, branch ui/d9-perf-profile (cut from github/wave/ui-d 59d96e1).
Host: u1, aarch64 20 cores, rustc/cargo 1.98.1, NVIDIA GB10 (Vulkan, driver
580.178.04), X :1 1920x1080. Scene: 1,002,528 triangles (709x709 grid) at
1600x1000, 20 warm-up + 120 measured frames (the UI-D5 protocol).
Date: 2026-11-25 (the committed runs finished at ~2026-11-26T05:44Z).

## STEP 0

```
git fetch github
git merge github/wave/ui-d
```
This worked and brought content: fast-forward 59d96e1..d8e200f, which lands the
UI-D5 harness (scripts/ui/perf.sh, crates/oep-viewer/tests/perf_budget.rs,
proof/UI-D5/) onto the card branch. No conflicts, no rebase, no reset.

## What was measured and how

The card's preferred path, `perf record`/`perf report` on the harness, is
BLOCKED on this host and could not be used:

```
$ perf record -e cycles -a -o /tmp/pf.data -- sleep 0.1
Error: Failure to open any events for recording.
Access to performance monitoring and observability operations is limited.
```

Cause: kernel.perf_event_paranoid = 4 (unprivileged sampling denied); no
passwordless sudo, no setcap on /usr/bin/perf. So the stage split uses the
card's sanctioned fallback: env-gated wall-clock stage timers added to
crates/oep-viewer/src/gpu/mod.rs behind ISOLITH_PERF_STAGES=1, inert when the
variable is unset (proven below). `scripts/ui/perf-profile.sh` (this card's new
script) runs:

1. fast smoke, variable unset (cargo test -p oep-viewer --test perf_budget
   -- --nocapture) -> green; prints CPU determinism hash and GPU echo hash.
2. fast smoke, ISOLITH_PERF_STAGES=1 -> green; same hashes + the demo render's
   stage line.
3. heavy 1M, unset, release -> clean p95 (the UI-D5 protocol number) written
   to proof/UI-D9/perf.json.
4. heavy 1M, ISOLITH_PERF_STAGES=1, release -> 142 per-frame stage lines
   (1 construct + 1 resize + 20 warm-up + 120 measured) in
   transcripts/04-heavy-set.txt, plus its own report perf-set.json.
5. aggregation -> proof/UI-D9/profile.json (stage means over the 120 measured
   frames, stage sum vs p95, both hashes, the perf-block reason).

The stage timers partition the frame exactly: assemble equals its six
sub-buckets (positions, normals, project+cull, sort, expand, other), and the
four seam stages (assemble + vertex-prep + submit + readback+w) sum to
frame_time within 0.004 ms (frame mean 388.17 ms, stage sum 388.17 ms).

## Result (quoted from proof/UI-D9/profile.json, means over 120 measured frames)

Clean p95 (unset, this host/run): 424.81 ms (committed UI-D5 reference 434.94
ms, the top of this session's 404.4 to 434.5 ms p95 band). Instrumented p95:
408.97 ms.

| Stage | mean ms | share |
|---|---|---|
| assemble | 293.34 | 75.6 percent |
| - positions | 0.31 | - |
| - normals | 3.37 | - |
| - project+cull | 85.51 | - |
| - sort | 50.14 | - |
| - expand | 98.65 | - |
| - other (incl. intermediate face-list teardown) | 55.37 | - |
| vertex-prep (pack 120.3 MB + create_buffer_init x2 + draw record) | 92.49 | 23.8 percent |
| submit | 0.07 | 0.0 percent |
| readback+w (copy + map + synchronous device.poll; owns GPU raster wait) | 2.26 | 0.6 percent |
| hash (post-frame oracle, after frame_time is captured) | 4.95 | (outside the seam bucket) |

Concrete median frame raw line (frame 36):
ISOLITH_PERF_STAGES frame=36 total=387.326ms assemble.positions=0.299ms assemble.normals=3.338ms assemble.project+cull=85.472ms assemble.sort=52.246ms assemble.expand=97.693ms assemble=294.777ms assemble.other=55.730ms vertex-prep=90.213ms submit=0.069ms readback+w=2.263ms hash=4.948ms

### Stage sum against the measured p95

Four seam stages sum to 388.17 ms mean; frame mean is 388.17 ms (gap 0.004 ms).
The gap to the clean p95 is 36.65 ms = the p95 tail, the small untimed
bookkeeping (shading::state, encoder creation, the assemble-return/vertex-prep
gap, begin-frame work, print_frame) and inter-leg variance; the
instrumented-to-stage-sum gap is 20.8 ms. `hash` (4.95 ms) is deliberately
excluded from the seam bucket because frame_time is captured before it;
end-to-end (seam + hash) is 393.12 ms.

Attribution: the CPU re-derivation (assemble, 75.6 percent) is the seam. The
120 MB re-upload artifact (expand + vertex-prep) is about 191 ms. The GPU-side
(submit + synchronous wait + readback) is under 3 ms.

## Determinism (unchanged by the timers; fast smoke green both ways)

- CPU: demo frame_hash 12597229237775278722 == committed golden
  12597229237775278722 (printed equal=true in leg 1 and leg 3 reports).
- GPU: gpu structural echo frame_hash 9287941873092625150 -> 9287941873092625150,
  identical = true with ISOLITH_PERF_STAGES unset (transcripts/01-fast-smoke-unset.txt)
  AND set (transcripts/02-fast-smoke-set.txt); profile.json
  gpu_frame_hash_identical_unset_vs_set = true.
- cargo test -p oep-viewer --test perf_budget -> ok with the variable unset
  and with it set (transcripts/01, 02). This card only touched gpu/mod.rs (the
  env-gated timers) + new files; crates/oep-viewer/tests/perf_budget.rs is
  byte-identical to wave/ui-d (git status clean for it).

## What could not be measured on this host, and why

- perf record/report CPU samples: denied (paranoid=4, no sudo, no setcap;
  full reason in profile.json perf_unavailable_reason). The per-stage split is
  wall-clock, not sample-based.
- The pure GPU-raster wall time is not isolated: it sits inside readback+w's
  synchronous device.poll and is small at this fragment load.
- The fine-grained identity of assemble.other (55.37 ms): measured as part of
  assemble; mostly the drop of the 1M-face intermediate list (the allocation
  teardown), with per-primitive colour lookup + line/grid/triad assembly + Vec
  bookkeeping also inside. It is not further split on this host.
- Run-to-run variance: a few percent on the shared integrated GPU (clean p95
  measured 404.4 / 407.9 / 434.5 / 424.8 ms across the four leg-3 runs this
  session; the committed UI-D5 number 434.94 ms is the top of the band). All
  numbers in the ADR/profile are the final committed run's, made on the exact
  committed tree.

## Prior-art brief

Delivered by the orchestrator and attached to the card; copied into
proof/UI-D9/prior-art.md in the same commit as this ADR. sha256 of the copy:
c5120c2b21bbbe55683a425eca55d6ec2bc7a340137cf866cbbfad595e6dc706
(identical to the attachment on the card). The ADR treats it as a primary
source and records ADOPT/REFUTE per stage (see the ADR, section 7).

## ADR

workstreams/modeler/ui/decisions/render-seam-performance.md (1.0.0, dated
2026-11-25) recommends Option A (GPU-resident geometry + CPU-side retention,
the prior-art brief's Stage 1 + Stage 2) and defers the depth-buffer change
(brief Stage 3b) to its own card. The ADR states the do-nothing floor
(424.81 ms p95, about 26x the 16.6 ms bar) and the scene sizes where the seam
meets the budget (~39k triangles as-is, ~121k after Option A keeping the CPU
sort).

Note to the ADR index owner: docs/decision-records.md is NOT on this card's
owned-file list, so it was left untouched. The index needs one new row under
"Technical decision records (ADRs)":
  [Render-seam performance](workstreams/modeler/ui/decisions/render-seam-performance.md)
  | 2026-11-25 | 1.0.0 | Per-stage profile picks GPU-resident geometry with
  CPU-side retention for the 1M-triangle interactive seam; depth buffer
  (Stage 3b) deferred as its own change.
