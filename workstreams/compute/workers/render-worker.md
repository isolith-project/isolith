# Render Worker (P9-M3-T01): headless oep-geo renderer as a worker function

Version 1.0, 2026-10-17. Milestone: P9-M3-T01 (wave-2 downstream P9 compute
lane). Program: Isolith / Open Engineering Platform (OEP).

## 1. What this is

The render worker binary that turns a real `oep-geo` model into an image
artifact, headless, with parameterized resolution and fidelity, and registers
with the P9 scheduler as a worker function. It is the first concrete worker
of the P9 compute lane (master plan P9-M3-T01) and the downstream consumer of
the P6-M5-T01 renderer backend decision (WGSL/wgpu offline headless renderer,
ADR `workstreams/geometry-core/viewer/renderer-backend-wgpu-adr.md`, prereq t_cfbc5f03).

Crate: `crates/oep-render-worker`. Binary: `oep-render-worker`. Library:
`oep_render_worker`.

## 2. Contract placement (P9-M1 job spec)

The P9-M1 job/task spec (`docs/compute/spec/job-spec.md`) declares that a job's
`executor` names `kind` and `function`, and that worker registry / function
resolution is owned by P9-M3-T01 -- this card. The render worker therefore:

- Registers as `executor.kind = "render"`, `executor.function =
  "oep-geo-render"` through the `registry` module, the exact vocabulary the
  P9-M1 example render job uses.
- Accepts render parameterization in the P9-M1 `executor.args` shape:
  `--resolution WxH` and `--fidelity low|medium|high`.
- Returns bytes + metadata (the rendered PNG plus resolution/fidelity used)
  that the job broker maps onto the job's `output` refs (P9-M5-T02 GA gate).

The `registry` module is the seam the scheduler calls to resolve
`executor.function` onto this worker; `--describe` prints the same record as
JSON so discovery is CI-detectable without linking the in-flight scheduler
crate (oep-lane, P9-M2-T01, sibling card).

## 3. Worker function

`oep_render_worker::worker::render_document(doc: &Document, opts) ->
RenderResult`:

1. Build the viewer scene from the real `oep-geo` document
   (`oep_viewer::scene::build_scene`), which walks parts/bodies and the
   assembly instance transform chain into box-proxy primitives.
2. Frame an orbit camera to fit the scene bounds, then orbit to a readable
   angle.
3. Render the scene offscreen at the supersampled internal resolution
   (`resolution * fidelity.factor()`) with the deterministic CPU reference
   rasterizer behind the P6-M5-T01 seam (`oep_viewer::render::render`).
4. Box-downsample to the requested output resolution.

Headless by construction: no window, no display, no swapchain is ever
involved, which is exactly the ADR's headless contract. Today the deterministic
CPU backend is used (the ADR's documented escape hatch), so the worker runs on
the GPU-less CI container and produces real pixels there; the WGSL/wgpu
backend can slot in behind the same seam without touching the worker.

## 4. Parameterization

Resolution is the output pixel size (`WxH`). Fidelity is the supersample
anti-aliasing factor: Low = 1x, Medium = 2x, High = 4x internal render then
box-downsample. Higher fidelity keeps the same output resolution but smooths
box edges, so low and high renders of the same model produce different, higher
quality pixels -- a real quality knob, verified by the acceptance test and the
Rust contract test.

| option       | values                  | default  | effect                          |
|--------------|-------------------------|----------|---------------------------------|
| `--resolution` | `WxH` (e.g. `1920x1080`) | `640x480` | output pixel size               |
| `--fidelity`   | `low` / `medium` / `high` | `medium` | supersample factor (1x/2x/4x)   |

## 5. Registration

`oep_render_worker::registry`:

- `get(kind, function)` resolves a registration by the P9-M1 executor
  vocabulary; the render worker registers as `render/oep-geo-render`.
- `registry()` lists all workers this lane ships (currently the single render
  worker, the seam for P9-M3-T02/T03 to extend).
- `registration_json(&RENDER_WORKER)` produces the `--describe` payload:
  kind, function, binary, version, fidelity levels, accepted args.

## 6. CLI

```text
oep-render-worker --model <file.oepgeo.json> --output <out.png>
                  [--resolution WxH] [--fidelity low|medium|high]
oep-render-worker --describe
```

- `--describe` prints the worker registration as JSON and exits 0 (CI can
  assert the scheduler-facing registration without the scheduler crate).
- Otherwise the worker loads the real `oep-geo` document, renders it, and
  writes the PNG. Exit 0 = artifact produced.

## 7. Verification

- Acceptance test `tests/test_p9_m3_render_worker.py` (written first, TDD):
  crate exists + workspace member, registration vocabulary present,
  `--describe` parses and matches, worker renders the real golden
  example-bracket model to valid 640x480 PNGs at low and high fidelity that
  differ, live `cargo test -p oep-render-worker` passes, surface-language
  rule (no em dash, no curly quote).
- Rust contract test `crates/oep-render-worker/tests/worker_functions.rs`:
  registry resolves render/oep-geo-render, worker renders the golden model
  (non-blank), fidelity levels change pixels.
- Committed real-material proof: `workstreams/compute/workers/proof/render-worker-640x480-low.png`
  and `render-worker-640x480-high.png`, generated from the real golden
  document by the worker binary.
- Gates: `python3 tests/test_p9_m3_render_worker.py` PASS, pytest 34 passed,
  `cargo test --workspace` ok, `cargo fmt --all -- --check` clean, `cargo
  clippy --workspace --all-targets -- -D warnings` clean, vendored precommit
  gate PASS.

## Change log

- 1.0.0 (2026-10-17): initial. Headless render worker as a P9 worker function,
  registration under render/oep-geo-render, resolution/fidelity
  parameterization, real-material proof on the golden bracket model.
