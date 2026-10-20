# P2-M5-T02 — `oep-cgr2step` CLI (tessellated mesh -> STEP B-Rep passthrough)

Status: implemented. Version 1.0 (2026-10-15).
Owner: Isolith program, P2 CGR reader workstream (milestone P2.5 "CLI + regression").

## Purpose

`oep-cgr2step` opens a CGR file, extracts the tessellated triangle mesh
(`oep_cgr::mesh::extract`, P2-M2-T01), and writes it to an ISO 10303-21
(STEP / PART 21) file **as a faceted B-Rep** — a shell of planar faces derived
from the source tessellation.

## Honesty contract (the core requirement of this card)

The output is marked as **tessellated** and is **NOT claimed to be an exact
analytic B-Rep**. This contract is enforced by the acceptance test
(`crates/oep-cgr/tests/step_writer.rs`, written before the implementation):

- `FILE_DESCRIPTION` in the PART 21 header states the file is a tessellated
  mesh passthrough and is not an exact analytic B-Rep.
- The `PRODUCT` name/description and the `SHAPE_REPRESENTATION` identifier
  carry the `tessellated` marker.
- The B-Rep uses `SHELL_BASED_SURFACE_MODEL` with planar `PLANE` faces bounded
  by `POLY_LOOP`s — a faceted representation. No `ADVANCED_FACE`, no
  `B_SPLINE`/NURBS, no analytic surfaces are emitted.
- The CLI prints a runtime warning on stderr reminding the user the output is
  tessellated and not an exact representation.
- Downstream geometry/tolerance tools (CAM, FEA, GD&T) must treat the model
  accordingly — this is a visualization/interchange artifact, not a precision
  B-Rep. Exact geometry conversion is a future workstream (P4/P6), not this
  passthrough.

## Usage

```
oep-cgr2step INPUT.cgr OUTPUT.step
```

Reads `INPUT.cgr` bytes, extracts the mesh, writes `OUTPUT.step`. On success a
one-line summary (vertices, triangles, faces written, shells open/closed,
degenerate facets skipped) plus the honesty warning are printed to stderr.

### Exit codes (documented contract)

| Code | Meaning |
|------|---------|
| 0    | success — STEP written |
| 1    | I/O error (input unreadable or output unwritable) |
| 2    | usage error (bad/missing arguments) |
| 3    | input is not a recognized CGR (header spine mismatch) |
| 4    | classified skip — no usable tessellated mesh found in the CGR payload |
| 5    | internal mesh-geometry invariant failure (defensive; should not occur on extracted meshes) |

## Output structure (what a reader will find)

- PART 21 envelope, `AUTOMOTIVE_DESIGN` (AP214) schema.
- One `SHELL_BASED_SURFACE_MODEL` containing:
  - one `CLOSED_SHELL` per watertight connected component,
  - one `OPEN_SHELL` per open/non-manifold component.
- Per triangle: `FACE_SURFACE` (geometry = `PLANE` normal to the facet, built
  from `AXIS2_PLACEMENT_3D` + `CARTESIAN_POINT` + `DIRECTION`), bounded by
  `FACE_OUTER_BOUND` -> `POLY_LOOP` of the triangle's three `CARTESIAN_POINT`s.
- Units: millimetre (SI), documented as an assumption — CGR carries no units
  field; the source tessellation coordinates are emitted verbatim (f32
  shortest round-trip).
- Degenerate (zero-area) triangles are skipped with a counted warning, never
  fatal; a mesh that is entirely degenerate is a classified skip (exit 4).

## Validation performed

- TDD acceptance in Rust (`crates/oep-cgr/tests/step_writer.rs`): PART 21
  envelope, honest tessellation markers, faceted structure, closed/open shell
  detection, per-component shells, byte-determinism, typed errors, -full
  writer no-panic mini-fuzz, CLI exit-code table, and a real-material leg
  that runs the actual binary on every supplied sample (skip-clean when
  `OEP_CGR_SAMPLES` is unset).
- Independent-reader leg in Python
  (`tests/test_oep_cgr2step_acceptance.py`): the produced STEP is parsed by
  the independent ISO 10303-21 parser `steputils` (syntax + entity-structure
  validation), with a full entity-count assert. Skips cleanly when `steputils`
  is not installed, so CI stays green.
- Real-material leg: every sample in `OEP_CGR_SAMPLES` reaches a *classified*
  CLI outcome (STEP produced, or a documented non-zero code) — the P2-M1-T04
  tolerant-failure doctrine. As of this milestone the current R11/R19-era
  corpus yields classified skips at the mesh-extraction stage (see Known
  limits), exactly as `P2-M2-T01` documents.

## Known limits (honest)

- **Mesh extraction is upstream of this card.** Whether a CGR yields a mesh at
  all is decided by `oep_cgr::mesh::extract`, whose triangle-stream location
  in the payload is [uncertain] until the P2-M1-T03 block walker and the
  P2-M1-T06 diff harness land. This CLI honors that faithfully: no mesh found
  = classified skip (exit 4), never a fabricated file.
- **Not an exact B-Rep.** Facets are copied from the CGR tessellation; no
  NURBS/analytic surface fitting is performed or claimed.
- **Units assumed millimetre.** CGR embeds no unit metadata.
- **No OCC/FreeCAD smoke test yet**: the produced file is validated against
  the independent PART 21 parser `steputils`; a full CAD-reader load test is
  planned in P4-M3-T02 (STEP export validation against FreeCAD + Open CASCADE)
  and can then be pointed at this passthrough output.
