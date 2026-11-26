#!/usr/bin/env python3
"""UI-D12 mechanical em-dash text cleanup. Replaces every U+2014 em dash in the
granted viewer files with plain punctuation (house style: ':' or ',' or list '-')
so the P6-M5-T02 surface-language guard passes. TEXT ONLY: each replacement swaps
only the em-dash bytes plus adjacent spacing on that line; no code, identifier,
assertion or threshold changes."""
import sys

EM = "\u2014"

# per-file ordered list of (old_fragment, new_fragment). old_fragment must be
# unique in that file and contains the em dash.
EDITS = {
 "crates/oep-viewer/src/render.rs": [
   ("here \u2014 they use the same `transform_point4` formula but",
    "here: they use the same `transform_point4` formula but"),
   ("portion \u2014 exactly the D7 goldens). One homogeneous transform",
    "portion: exactly the D7 goldens). One homogeneous transform"),
   ("dropped here \u2014 the segment is clipped to the visible cube by",
    "dropped here: the segment is clipped to the visible cube by"),
 ],
 "crates/oep-viewer/src/gpu/resident.rs": [
   ("pre-card path drew \u2014 the renderer's one",
    "pre-card path drew: the renderer's one"),
   ("golden fixtures \u2014 the\n", "golden fixtures: the\n"),
   ("shading state) \u2014 the scene is owned by `GpuBackend` and",
    "shading state): the scene is owned by `GpuBackend` and"),
   ("dirty key \u2014 mesh bytes via `MeshData: PartialEq`,",
    "dirty key: mesh bytes via `MeshData: PartialEq`,"),
   ("frustum volume \u2014 rejected ONLY when",
    "frustum volume: rejected ONLY when"),
 ],
 "crates/oep-viewer/src/gpu/idbuffer.rs": [
   ("two queries from the grid \u2014 [`IdPass::pick_at`]",
    "two queries from the grid: [`IdPass::pick_at`]"),
   ("first \u2014 the painter's back-to-front order.",
    "first: the painter's back-to-front order."),
 ],
 "crates/oep-viewer/src/gpu/shading.rs": [
   ("light rig \u2014 one directional **key** light plus a weaker",
    "light rig: one directional **key** light plus a weaker"),
   ("**fill** light \u2014 whose strengths and directions are baked",
    "**fill** light, whose strengths and directions are baked"),
   ("boundary edge list \u2014 the\n", "boundary edge list: the\n"),
   ("normals) \u2014 the last two differ only in normal-driven pixels",
    "normals): the last two differ only in normal-driven pixels"),
   ("The edge pass only \u2014 the boundary edge list, nothing else.",
    "The edge pass only: the boundary edge list, nothing else."),
   ("The light rig \u2014 the single source of truth for the WGSL",
    "The light rig: the single source of truth for the WGSL"),
   ("Direction of the fill light \u2014 dimmer, from the opposite low side",
    "Direction of the fill light: dimmer, from the opposite low side"),
   ("list \u2014 the same 12 edges a tessellated box exposes.",
    "list: the same 12 edges a tessellated box exposes."),
   ("three vertex normals \u2014", "three vertex normals:"),
   ("renormalised \u2014 the classic Gouraud normal.",
    "renormalised: the classic Gouraud normal."),
   ("`Aabb::corners()` order \u2014 bottom ring 0..3, top ring 4..7, then",
    "`Aabb::corners()` order: bottom ring 0..3, top ring 4..7, then"),
 ],
 "crates/oep-viewer/src/gpu/device.rs": [
   ("**egui's device** \u2014 when the app runs `eframe` with the wgpu renderer,",
    "**egui's device**: when the app runs `eframe` with the wgpu renderer,"),
   ("**autonomous acquisition** \u2014 when no egui device was registered",
    "**autonomous acquisition**: when no egui device was registered"),
   ("**honest `None`** \u2014 if [`GPU_FLAG`] is `off` or no adapter is",
    "**honest `None`**: if [`GPU_FLAG`] is `off` or no adapter is"),
 ],
 "crates/oep-viewer/src/gpu/mod.rs": [
   ("[`device::register_egui_device`] \u2014 the shared device",
    "[`device::register_egui_device`]: the shared device"),
   ("transform \u2014 UI-D8)", "transform, UI-D8)"),
   ("screen space \u2014 byte-identical to",
    "screen space: byte-identical to"),
   ("`render::project_ndc` \u2014 one helper",
    "`render::project_ndc`: one helper"),
   ("tip letters X/Y/Z \u2014 the arms are drawn, the letters",
    "tip letters X/Y/Z: the arms are drawn, the letters"),
   ("`1 * src.a + 0 * dst.a` \u2014 REPLACE), so",
    "`1 * src.a + 0 * dst.a`: REPLACE), so"),
   ("128 instead of 255 \u2014 the number the UI-D2 acceptance prints",
    "128 instead of 255: the number the UI-D2 acceptance prints"),
   ("pre-projected NDC \u2014 the screen-constant arms are pixel-space by",
    "pre-projected NDC: the screen-constant arms are pixel-space by"),
   ("with and without the variable \u2014 UI-D10 re-proves",
    "with and without the variable, UI-D10 re-proves"),
   ("Upload bytes written in the last render (MVP + geometry + lines) \u2014",
    "Upload bytes written in the last render (MVP + geometry + lines):"),
   ("should not use the GPU path \u2014 the\n",
    "should not use the GPU path: the\n"),
   ("column-major f32 \u2014 one transform, one place, shared with the CPU.",
    "column-major f32: one transform, one place, shared with the CPU."),
   ("resident buffers (write_buffer; no device-buffer allocation) \u2014",
    "resident buffers (write_buffer; no device-buffer allocation):"),
   ("boundary edge list exactly \u2014 mesh edges for meshes, the 12 box",
    "boundary edge list exactly: mesh edges for meshes, the 12 box"),
   ("(steady state: 0 \u2014 the resident buffers were created once before).",
    "(steady state: 0; the resident buffers were created once before)."),
   ("f32 in native order (no bytemuck \u2014 the\n",
    "f32 in native order (no bytemuck; the\n"),
   ("per-vertex f32 normal bits \u2014 proven, see `resident` module docs).",
    "per-vertex f32 normal bits: proven, see `resident` module docs)."),
   ("correction \u2014 the combined matrix applied to the body-space point it",
    "correction: the combined matrix applied to the body-space point it"),
   ("(any component count \u2014 NDC `[f64;2]` for the",
    "(any component count: NDC `[f64;2]` for the"),
   ("`render::project_ndc` path (D7 \u2014 world point; one formula in the crate).",
    "`render::project_ndc` path (D7: world point; one formula in the crate)."),
   ("pipeline layout \u2014 the shaded pass consumes it in `shading.wgsl`'s vertex",
    "pipeline layout: the shaded pass consumes it in `shading.wgsl`'s vertex"),
   ("`f32::to_ne_bytes` \u2014 no bytemuck (the",
    "`f32::to_ne_bytes`: no bytemuck (the"),
 ],
 "crates/oep-viewer/tests/perf_budget.rs": [
   ("frame-time distribution \u2014 so \"interactive\" is a measured claim with a",
    "frame-time distribution: so \"interactive\" is a measured claim with a"),
   ("* `perf_budget_fast_smoke` \u2014 ALWAYS runs (never `#[ignore]`).",
    "* `perf_budget_fast_smoke`: ALWAYS runs (never `#[ignore]`)."),
   ("* `perf_budget_1m_gpu_heavy` \u2014 `#[ignore]`d by default (perf is a manual",
    "* `perf_budget_1m_gpu_heavy`: `#[ignore]`d by default (perf is a manual"),
   ("`capability` block on this device \u2014 the\n",
    "`capability` block on this device: the\n"),
   ("`CpuBackend` demo \u2014 the committed CPU\n",
    "`CpuBackend` demo: the committed CPU\n"),
   ("FAST SMOKE \u2014 always runs (`cargo test -p oep-viewer --test perf_budget`).",
    "FAST SMOKE: always runs (`cargo test -p oep-viewer --test perf_budget`)."),
   ("HEAVY \u2014 `#[ignore]`d by default. Runs the full 1M-triangle 1600x1000",
    "HEAVY: `#[ignore]`d by default. Runs the full 1M-triangle 1600x1000"),
   ("fails so the outcome is loud \u2014 the card must then be blocked `capability`",
    "fails so the outcome is loud: the card must then be blocked `capability`"),
   ("readback + GPU completion \u2014 the real\n",
    "readback + GPU completion: the real\n"),
   (" on {} ({}) \u2014 recorded in {}, bar NOT lowered",
    " on {} ({}) - recorded in {}, bar NOT lowered"),
 ],
 "crates/oep-viewer/tests/resident_scene.rs": [
   ("//!    \u2014 `create_buffer_init` per frame is gone (regression guard).",
    "//!    - `create_buffer_init` per frame is gone (regression guard)."),
   ("pre-D10 path drew \u2014 the sha256 golden tests pin the pixels.",
    "pre-D10 path drew: the sha256 golden tests pin the pixels."),
   ("culling legitimately removes it \u2014",
    "culling legitimately removes it:"),
 ],
 "crates/oep-viewer/tests/gpu_backend.rs": [
   ("    frames \u2014 the seam's determinism oracle re-proven on the GPU path;",
    "    frames: the seam's determinism oracle re-proven on the GPU path;"),
   ("whole body \u2014 creation\n", "whole body: creation\n"),
 ],
 "crates/oep-viewer/tests/render_axis_views.rs": [
   ("to the VIEW-SPACE point \u2014 the view\n",
    "to the VIEW-SPACE point: the view\n"),
   ("draws ANY model pixels \u2014 the all-culled frame is",
    "draws ANY model pixels: the all-culled frame is"),
 ],
}

total = 0
for path, pairs in EDITS.items():
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    for old, new in pairs:
        n = text.count(old)
        assert n == 1, f"{path}: anchor not unique (n={n}): {old!r}"
        text = text.replace(old, new)
        total += 1
    left = text.count(EM)
    assert left == 0, f"{path}: {left} em dashes left"
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)
    print(f"ok {path} ({len(pairs)} anchors)")
print(f"total em dashes replaced: {total}")
