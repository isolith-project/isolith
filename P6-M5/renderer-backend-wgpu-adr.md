---
title: Keelworks Renderer Backend Decision (WGSL/wgpu, offline, headless)
decision: adopt_wgpu_wgsl_offline_headless_renderer_shared_by_preview_and_p2p_render_jobs
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks program governance, P6-M5 (renderer backend)
applies_to: oep-geo (adopting surface), oep-render (new renderer crate behind a clean seam), the P6-M5-T02 preview viewer, the P9-M3-T01 headless render worker, and any offline/P2P render job
supersedes: nothing (first renderer backend decision record)
---

# Keelworks Renderer Backend Decision

This record is the binding product decision for how Keelworks renders geometry:
it names the renderer backend, the shading language, and the render target
model, and it answers the P6-M5-T01 question "WGSL/wgpu offline renderer for
previews and P2P render jobs; headless render target". It is versioned like the
boolean-ops decision (P6-M3-T01), the telemetry decision (P0-M5-T02), and the
release train decision (P0-M5-T03); any material change bumps the version in
the front matter and adds a line to the change log at the bottom.

The decision is ratified against a recorded real-material proof: the committed
spike in `P6-M5/spike/headless-render/` builds and runs on the reference host
(U1) with no display session at all, draws a triangle with a WGSL shader into
an offscreen texture, copies the pixels back to the CPU, and writes a verified
`proof.png`. The acceptance test in `tests/test_renderer_backend_wgpu_adr.py`
asserts this document and the spike artifact together, on real material, not on
mocks.

## 1. Decision

Keelworks ADOPTS **wgpu** as its renderer backend and **WGSL** as its shading
language, driving an **offline** renderer with a **headless** render target.
The same backend serves the two consumers named in the P6-M5-T01 card:

1. **Previews** (P6-M5-T02 preview viewer, and the P8 web viewer embed later):
   the interactive surface presents a wgpu-rendered image when a display is
   present, and can equally present the identical image without one.
2. **P2P render jobs** (P9-M3-T01 headless render worker): geometry comes in,
   the worker renders offscreen with wgpu, and a PNG/BMP/whatever frame comes
   out. No window, no display, no windowing system is ever required.

Because both consumers share one backend, one shader codebase (WGSL), and one
render path, preview fidelity and job-output fidelity cannot drift apart: the
preview is exactly what the render job will produce. This single-codebase
property is the primary reason the decision is worth making now rather than
lazily in each milestone.

Building an own offscreen rasterizer, binding a different GPU API, or pushing
rendering into a browser runtime are the explicitly weighed alternatives and
are each rejected or deferred with reasons (section 4).

## 2. Why this decision

Four facts drive it:

1. **Headless is the hard requirement and wgpu satisfies it natively.** The
   card mandates a headless render target: a render job must complete on a
   worker node with no display, no X/Wayland session, and no on-screen
   swapchain. wgpu renders to an offscreen texture without ever creating a
   surface; CPU readback is a first-class copy, which is exactly the offline
   job loop. The real-material spike proves this path on the reference GPU
   (section 6).

2. **One backend for interactive preview and offline jobs.** wgpu is the WebGPU
   implementation for Rust. The same device, pipeline, and WGSL shaders feed a
   windowed presentation (via a surface) or a headless render target, with the
   surface as an optional add-on rather than a precondition. This is what makes
   "preview equals job output" a property we get by construction.

3. **WGSL over GLSL/HLSL/SPIR-V keeps shader code portable and ours.** WGSL is
   the modern, backend-neutral shading language; wgpu (via naga) compiles it to
   Vulkan, Metal, DirectX 12, and OpenGL backends. Shaders written once in WGSL
   run on the U1 (Vulkan/NVIDIA GB10), on a Metal Mac, and, through the GL
   backend, on nodes with only OpenGL. We ship our own WGSL assets in-repo; we
   are not bundling a proprietary shader compiler.

4. **Rust-native and license-clean.** wgpu is the idiomatic Rust GPU library
   (the crate the ecosystem builds on), matching the Rust-first core decision
   (wave-1 decision 6). It is MIT OR Apache-2.0, compatible with the
   Apache-2.0 apps/libraries posture without a copyleft obligation (section 7).

## 3. Headless contract

The renderer MUST be able to produce output with:

- no display server (no X11, no Wayland, no Quartz, no Windows session),
- no window and no on-screen swapchain,
- no windowing-system dependency in the crate (the window/context connection is
  an optional feature for the interactive viewer only).

The render loop for a headless job is: create a device from a suitable adapter,
create an offscreen texture as the render target, encode and submit the render
pass with WGSL pipelines, copy the texture to a buffer, map it, and hand the
bytes to an encoder (PNG writer) on the CPU. The `proof.png` in the spike is
produced by exactly this loop. When a display IS available to the preview
viewer, the same pipeline presents through a surface by creating a swapchain
texture and running the identical render pass into it; the job and the preview
share the pipeline, differing only in where the color attachment lives.

## 4. Options considered

1. **wgpu + WGSL (ADOPTED).** Modern WebGPU API, Rust-native, headless by
   construction, single shader language that compiles to all target backends,
   MIT OR Apache-2.0. Real-material headless proof committed with this record
   (section 6).

2. **Raw Vulkan via ash.** Full control but no WGSL (SPIR-V toolchain instead),
   far more unsafe boilerplate per pipeline, and a much larger surface to keep
   headless-safe (swapchain-free) ourselves. REJECTED for wave 1: the extra
   code buys nothing the product needs, and it would slow the P6-M5-T02 viewer
   and P9 worker cards. Kept in mind only as a low-level escape hatch (section
   9) behind the clean seam.

3. **OpenGL/EGL directly (glow/glutin-style).** GLSL only, an older API with
   more driver divergence, and a weaker headless posture than wgpu's; we would
   carry the surfaceless-EGL plumbing ourselves. REJECTED as the primary
   backend. The GL backend of wgpu remains available as a compatibility
   fallback (section 9) without us touching GL directly.

4. **Metal or DirectX 12 directly.** Platform-locked; Keelworks targets the
   self-hosted/Linux/heterogeneous-node world (U1, NAS/Docker, P2P GPU nodes)
   where Vulkan is the common denominator. REJECTED as a primary; serving as
   optional wgpu backends is free.

5. **CPU software rasterizer (tiny-skia / own raster).** Simplest and fully
   deterministic, but it cannot meet the P6-M5-T03 interactive budget (1M
   triangles on U1-class hardware) and would put every render job on the CPU.
   DEFERRED as the escape hatch (section 9), not the primary.

6. **Browser-embedded renderer (three.js / WebGL / browser WebGPU).** Pulls a
   browser runtime into the desktop and the headless worker, which is wrong for
   offline P2P jobs and heavy for previews. REJECTED for the core. Web
   previews later (P8 web viewer embed) can reuse the same committed WGSL
   shaders and a small wasm/served renderer if we choose; that is an additive
   surface, not a replacement for this decision.

## 5. Clean interface

The renderer lives behind a clean seam: a new `oep-render` crate (or an
`oep-geo::render` module) exposes the public surface that callers see, and
wgpu/WGSL concepts never cross that seam.

- Downstream code (P6-M5-T02 viewer, P9-M3-T01 worker, P8 viewer embed, tests)
  depends on the seam's model-level types: a scene (geometry + materials +
  camera), a render target (an image/byte buffer or a window handle), and a
  `render()` that returns pixels.
- wgpu devices, pipelines, the WGSL source, adapter selection, and the
  readback plumbing stay inside the seam implementation, exactly like OCCT
  stays behind its seam in the P6-M3-T01 boolean-ops decision.
- The seam is what makes the section 9 escape hatch real: swapping wgpu for a
  software rasterizer or a different backend touches only the implementation
  behind the interface, never the callers.

## 6. Real-material verification

The spike `P6-M5/spike/headless-render/` is committed source plus a generated
proof image, and it is the acceptance-time evidence for this decision:

- Hardware: U1 reference host, NVIDIA GB10, Vulkan backend, driver 580.178.04,
  no display session at all (the run printed the adapter and rendered to
  completion with no surface and no windowing system).
- Shader: one WGSL pipeline draws a triangle into a 256x256 offscreen
  `Bgra8UnormSrgb` texture.
- Readback: the texture is copied to a CPU buffer, mapped, decoded, and saved
  as `proof.png`; the run asserted the center pixel is lit and the top-left
  corner is clear, so the image is not blank and has the expected coverage.
- Reproduce: `cargo run --release` inside `P6-M5/spike/headless-render/` on a
  GPU/Vulkan-capable host regenerates `proof.png`.

The acceptance test asserts both the ADR and the artifact: it verifies
`proof.png` exists, has the PNG magic bytes, and is a plausible non-empty
render, and it verified on this host that a headless run produces real pixels.

## 7. License

wgpu and its core crates (naga, wgpu-hal, wgpu-types, wgpu-core) are dual
licensed MIT OR Apache-2.0. That is compatible with the program posture decided
for wave 1: Apache-2.0 for applications and libraries. There is no copyleft
obligation: linking wgpu into `oep-render` does not contaminate the
Apache-2.0 library or the crates that depend on it. The AGPL-3.0 posture stays
scoped to the PDM server and P2P orchestration components; the renderer library
and the worker binary remain Apache-2.0. The `image` crate used for PNG
encoding in the spike is MIT OR Apache-2.0 as well.

## 8. Clean-room

wgpu is a public, open-source, permissively licensed library, not a vendor
binary and not copied source. Binding it is a normal dependency inclusion and
does not violate the program clean-room invariant (doctrine rule 5), the same
position recorded for the Open CASCADE binding in P6-M3-T01. We write our own
WGSL shaders, our own scene/target model, and our own render orchestration on
top; the library supplies the standard GPU API plumbing, exactly as OCCT
supplies standard geometry primitives.

## 9. Guardrails

- **Version pin.** `oep-render` pins a concrete wgpu version (wave-1 adoption:
  the 24.x line exercised by the spike) in Cargo.toml with Cargo.lock committed,
  mirroring the P6-M3-T01 OCCT pin. Upgrades are deliberate, tested PRs, not
  drifts.
- **Escape hatch.** The clean seam (section 5) keeps a swap path open: if wgpu
  at some point stops fitting (licensing drift, platform loss, unfixable bug),
  a software rasterizer (section 4 option 5) or a raw Vulkan backend (option 2)
  can replace the implementation behind the interface without touching callers.
- **Fallback backend.** wgpu's GL backend stays enabled where practical so a
  node with only OpenGL can still run render jobs (CPU/slow but functional).
- **Headless test gate.** A CI-reproducible headless render test is required:
  the committed acceptance test runs on every push (the precommit-quality-gate
  pytest), and the spike must remain buildable and its proof image valid. When
  a GPU runner is available, the spike itself runs in CI; on CI containers
  without a GPU, the committed proof artifact plus the spike source satisfy the
  gate and the seam keeps the GPU-touching path isolated.
- **Shaders in-repo.** WGSL assets are versioned with the repo, so the exact
  shader that produced a job's output is always recoverable (traceability for
  P9 render jobs and P11 compliance).

## 10. Cross-references

- P6-M5-T01 (this card): decision and proof.
- P6-M5-T02: preview viewer builds on this backend; camera/selection/clipping
  added over the seam.
- P6-M5-T03: viewer performance budget (1M triangles) is the acceptance number
  the backend must meet on U1-class hardware, and is measured over the seam.
- P9-M3-T01: the headless render worker consumes this backend as its worker
  function; job input = scene refs, job output = rendered frame.
- P6-M2: geometry/math lane feeds the scene model the renderer draws.
- P8-M4-T01: web viewer embed reuses the same WGSL shaders and seam.

## Change log

- 1.0.0 (2026-10-16): ratified. WGSL/wgpu offline headless renderer shared by
  previews and P2P render jobs; headless contract, options, clean interface,
  license, clean-room, real-material spike proof, and guardrails recorded.
