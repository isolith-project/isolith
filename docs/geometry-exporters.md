# P6-M4-T01: Exporters for `oep-geo` (STL, glTF/GLB, OBJ, SVG-2D, PDF, STEP)

Status: implemented and golden-verified (Isolith wave 1, P6).

## Scope

A single, dependency-free exporter module for the P6 geometry core
(`crates/oep-geo/src/export/`). All six exporters consume the canonical
internal tessellation as plain slices (`positions: &[[f32; 3]]`,
`triangles: &[[u32; 3]]`), which is the vertices/indices/triangles
representation named by P2-M2-T01 — so they are independent of any concrete
`Mesh` type and trivially callable once the geometry kernel lands
(`mesh.vertices()` / `mesh.triangles()`).

Formats covered:

| Format | Entry point | Kind |
|---|---|---|
| STL (binary + ASCII) | `write_binary_stl`, `write_ascii_stl` | triangle mesh |
| glTF 2.0 | `write_glb`, `write_gltf_json` + `write_gltf_bin` | triangle mesh |
| Wavefront OBJ | `write_obj` | triangle mesh |
| SVG (2D drawing) | `write_svg_2d` | orthographic XY projected-edge drawing |
| PDF (drawing path) | `write_pdf_drawing` | same projection as one-page PDF 1.4 |
| STEP (faceted) | `write_step` + `StepOpts` / `StepStats` | AP214 PART 21 faceted B-Rep passthrough |

## Reference material and TDD

The reference tessellation is the CC0-1.0 isolith unit box
(`corpus/data/box.stl`, P0-M4-T02 fixture): 8 vertices / 12 triangles.

TDD order followed:

1. **RED** — the acceptance contract was written first:
   * `tests/test_oep_geo_exporters_acceptance.py`: re-parses each committed
     golden with pure-stdlib decoders written from the public format specs
     (STL struct, GLB header/accessors, OBJ records, SVG XML, PDF
     object/xref structure + a real-reader leg with qpdf/pdfinfo/pdftoppm,
     STEP PART 21 token scan) and asserts each golden *is* the unit box.
   * `crates/oep-geo/tests/exporters_contract.rs`: byte-locks each Rust
     writer's output to the same goldens (compiled only after GREEN exposed
     `oep_geo::export`).
   * `crates/oep-geo/tools/gen_golden_box_*.py`: INDEPENDENT encoders that
     produce each golden from the public spec (NOT by calling the Rust
     writers), so the byte-lock proves conformance, not an echo.
2. **GREEN** — `crates/oep-geo/src/export/{base,stl,obj,gltf,svg,pdf,step}.rs`
   implemented to produce byte-identical output to the goldens.
3. **Verify on real material** — `test_binary_stl_matches_corpus_box_when_present`
   compares the binary STL export (36 vertex coordinates) against the real
   `corpus/data/box.stl` when present (`OEP_CORPUS` or the standard corpus
   location); SKIPs cleanly when absent so CI stays green.

## Design decisions

### Decision: one shared `ExportError`, format-module aliases
All exporters return the same typed, never-panic error
(`ExportError`: `IndexOutOfRange { index, len }`, `NonFiniteVertex`,
`DegenerateTriangle`, `EmptyMesh`, `Io`). Each format module re-exports it
under its familiar name (`StlError`, `GltfError`, `ObjError`, `SvgError`,
`PdfError`, `StepError`) per the sibling exporter discipline
(P2-M2-T02 / P2-M2-T03). No panic path exists on hostile input.

### Decision: writers take slices, not a `Mesh`
The exporters deliberately accept `&[[f32;3]]` + `&[[u32;3]]` so they are
type-independent of the (not-yet-shipped) geometry kernel and testable with
a literal array. The P6-M4-T02 importer hub reconciles `Mesh` to these
slices. This matches how the P6-M2-T01..T03 cards kept modules
self-contained (`[f32;3]`/`[f64;3]` points).

### Decision: 2D drawing = projected-edge wireframe, not HLR
The SVG and PDF exporters draw the orthographic XY projection (drop Z) of
the mesh's UNIQUE UNDIRECTED edges as vector line segments. This is a
projected-edge wireframe ("top view"); hidden-line removal is a later
drawing milestone (P6 drawing path continues there). Deterministic dedup:
edges are keyed by their two projected endpoints in canonical order; the
first-seen edge wins; zero-length projected edges are dropped.

### Decision: STEP export is an HONEST tessellated passthrough
`write_step` emits a valid AP214 PART 21 file whose B-Rep is a
`SHELL_BASED_SURFACE_MODEL`: one planar `FACE_SURFACE` per triangle grouped
into a `CLOSED_SHELL` (watertight) / `OPEN_SHELL` (open) per connected
component. The header (`FILE_DESCRIPTION`), product description and
shape-representation identifier all state explicitly that this is a
TESSELLATED representation and NOT an exact analytic B-Rep (per the
P2-M5-T02 / P4-lane honesty contract). The STEP *export* lane is owned by P4
(`crates/oep-step`); this card provides the faceted passthrough now.

### Decision: normals are computed, not trusted
For STL facets, the normal is the right-hand-rule unit normal
`normalize(cross(v1-v0, v2-v0))` — STL consumers should recompute anyway,
but we emit it for interchange. glTF writes no normals (position + index
only; a normals pass is P2-M3-T01).

### Decision: no new dependencies
Everything (including the PDF writer with xref offsets and the STEP
emitter with connected-component detection) is std-only. No crates.io
fetch at build time — important on the offline CI host.

## Golden contract

Committed under `crates/oep-geo/tests/golden/`:

* `box-reference.bin.stl`      — 684 B, binary STL, 12 facets
* `box-reference.glb`          — 724 B, GLB (JSON chunk 528, BIN 168)
* `box-reference.obj`          — Wavefront OBJ, 8 v / 12 f
* `box-reference.svg`          — 2D SVG, 5 projected edges, viewBox -0.5 -0.5 2 2
* `box-reference.pdf`          — 522 B, min PDF 1.4, valid under qpdf
* `box-reference.step`         — AP214 PART 21, 12 faces / 1 closed shell

Regenerate (only when the REFERENCE box changes, which it should not
lightly): `python3 crates/oep-geo/tools/gen_golden_box_{stl,glb,obj,svg,pdf,step}.py`.

## Verification

* `cargo test --workspace` — 12 new integration tests byte-lock all six
  writers to the goldens + the typed-error model; whole workspace green.
* `python3 -m pytest -q` — 22 acceptance tests pass, including the six
  exporter-parsing tests and the real-corpus STL comparison.
* `scripts/gate/precommit-gate.sh` — PASS (pytest + cargo test).
* `cargo clippy --workspace --all-targets -- -D warnings` — clean.
* Real-material leg (manual): with `OEP_CORPUS` pointing at the corpus
  box.stl, the binary STL export's 36 vertex coordinates match the corpus
  fixture within 1e-6.
