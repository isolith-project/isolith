# Wave ui-d status record

Wave D of the editor parity program (`~/.hermes/plans/2026-11-09_210930-isolith-rust-ui-parity.md`, section 5): GPU viewport and visual quality. Branch `wave/ui-d`.

STATUS: OPEN — the UI-D6 integrate card (t_62060655) BLOCKED at the wave-close gate (2026-11-26). The merged wave head `1097f76` does NOT pass the full six-leg gate: fmt, clippy `-D warnings`, the precommit pytest (2 failures), and `shot.sh` are red, and every red leg's root cause is in files outside the integrate card's owned list (UI-D3/UI-D5 crate files, D1/D2/D3/D8/D10 em-dash text guards, and the MEDIA-RENDERER demo present PNGs). Full evidence: `workstreams/modeler/ui/proof/UI-D6/PROOF.md` and its `transcripts/`.

## Cards

| Card | Title | Status |
|---|---|---|
| UI-D1 | UI-GPU-BACKEND: wgpu viewport behind the seam, shared device with egui | done |
| UI-D2 | UI-GPU-SHADING: lighting, edges, view modes | done |
| UI-D3 | UI-PICK: id buffer picking, hover, box select | done |
| UI-D7 | UI-RENDER-FIX: correct the clip_w projection and guard empty face buffers | done |
| UI-D4 | UI-NAV: view cube, standard views, section planes | done |
| UI-D8 | UI-VEC4-PIPELINE: one homogeneous clip-space path, transform in the vertex stage | done |
| UI-D9 | UI-PERF-PROFILE: per-stage frame cost split and the render-seam performance ADR | done |
| UI-D10 | UI-RESIDENT-SCENE: keep geometry on the GPU, cull per primitive, re-upload only dirty meshes | done |
| UI-D5 | UI-PERF: interactive frame budget on U1, recorded numbers | done (budget MET on merged head: p95 6.408 ms; historical miss preserved) |
| UI-D6 | UI-WAVE-D-INTEGRATE: merge, gate, checkpoint, screenshots, tag | blocked (needs_input) 2026-11-26 |

Every wave-D card branch is an ancestor of the wave head (`proof/UI-D6/transcripts/08-ancestors.txt`).

## Head sha

- Wave head: `1097f76` (carries UI-D1, D2, D3, D7, D4, D8, D9, D10 and D5's landing; the wave was fast-forwarded to include UI-D5's merged-head re-gate proof at `1097f76` per the orchestrator's UI-D5 handoff).
- Not closed: no checkpoint merge into `main` exists. `github/main` (`bed3d50`) is not an ancestor of the wave head (main carries post-wave-C docs PRs #32/#33), so the checkpoint would be a real merge, not a fast-forward.

## Deferred

UI-D11 (UI-DEPTH-SORT, reverse-Z depth buffer) is deliberately NOT part of wave D per the orchestrator addendum 2026-11-25: the premise that the painter sort could not reach the budget is false (UI-D10 measured p95 6.366 ms at the full 1,002,528-triangle / 1600x1000 scene against the 16.6 ms bar), and UI-D11 is a correctness upgrade that changes the GPU output contract (per-pixel depth ordering, GPU regolden, id-parity and transparency re-verification), not a budget fix. Its branch is NOT merged; it is parked in `scheduled` for Don's decision.

## Render-seam performance numbers (from the cards' own proofs)

The interactive seam at 1,002,528 triangles / 1600x1000 on the real NVIDIA GB10:

- Before (UI-D5 measurement, pre-D10 seam): p95 434.94 ms (the committed historical miss in UI-D5's proof is p95 438.617 ms, ~26x over the bar, recorded 2026-11-25 and preserved as history; bar never lowered).
- After UI-D10 (resident scene, Option A of `workstreams/modeler/ui/decisions/render-seam-performance.md` v1.0.1): p95 6.366 ms (UI-D10 proof) and p95 6.408 ms (UI-D5 re-gate on the merged head, p50 6.313, p99 6.471, max 6.823, min fps 146.6, mean fps 158.7). Threshold 16.6 ms: MET.
- Steady-state upload per frame: 120,303,360 B before -> 64 B after (MVP uniform only).
- Device buffer allocations per frame: 0. Draw calls per frame: 2.
- Per-primitive frustum cull: 78.4 pct of a 1,331-primitive framed assembly dropped.
- Pixel oracle: 21/21 proof frames sha256-identical across the change; CPU golden 12597229237775278722 and GPU echo 9287941873092625150 unchanged.
- Prior-art brief: `workstreams/modeler/ui/proof/UI-D9/prior-art.md` (36 cited sources).

The render-seam fix, in one line: geometry uploaded once into device-resident buffers and referenced per frame, per-primitive world-AABB culling, dirty-only re-uploads, painter order preserved. (The lesson worth keeping for wave E.)
