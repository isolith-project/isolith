# oep-geo drawing model schema (Class 2 extension, EDITOR-2D-DRAFTING)

Version 2.0, 2026-10-22. Program: Isolith / Open Engineering Platform (OEP).
This is the Class 2 (model-extend) extension of the `oep-geo` model schema —
the drafting workbench's record model: sheets, standard views, dimensions and
titles. It supersedes the "1 additive" classification from the 2026-10-18
scope register: decision #19 (editor spine, 2026-10-22) makes the editor the
product, and drafting lives in the canonical document so dimensions reference
the SAME `oep-sketch` variable-set parameters that drive the 3D model
(FreeCAD-v1-variable-set behavior) and survive PDM round-trips in one file.

This is an **extension by design, not a rearchitecture**: `oep-geo` is a
versioned, consumer-additive format (scope-register cost class 2). The schema
version bumps `1 -> 2`; v1 files still load. Reference implementation:
`crates/oep-geo/src/drawing.rs`. Workbench (projection / layout / export):
`crates/oep-drafting`. Drafting goldens: `crates/oep-drafting/tests/golden/`.

---

## 1. Versioning and migration (P6-M1-T03 discipline)

- `version = 1` (2026-10-16): the original five classes (part, body, feature,
  assembly, instance).
- `version = 2` (2026-10-22): adds the sixth class **`drawing`**.

**Migration is tolerant and forward.** A v1 payload deserializes with the new
`drawings` collection defaulted to `[]`, and `Document::from_json_pretty`
promotes it to the current version in place (`Document::migrate`). A v1 file
therefore loads, validates, and re-serializes as v2. Committed v1 goldens
across the tree were migrated by re-serializing through the canonical
serializer (byte-identical output); consumers that asserted an absolute
version now assert against `oep_geo::document::VERSION`. See
`crates/oep-geo/tests/schema_serde.rs::v1_payload_migrates_to_current_version`.

## 2. The drawing class

| field    | type     | notes |
|----------|----------|-------|
| `id`     | string   | unique document-wide (one document = one id space; drawing/sheet/view/dimension ids all share it) |
| `name`   | string   | human-readable |
| `sheets` | sheet[]  | the sheets that make up the drawing |

A `Drawing` is stored flat in the `Document` alongside the original five
classes (`Document::drawings: Vec<Drawing>`, serde-defaulted empty).

### 2.1 Sheet

| field          | type                          | notes |
|----------------|-------------------------------|-------|
| `id`           | string                        | unique |
| `name`         | string                        | |
| `size`         | `SheetSize {width_mm, height_mm}` | paper size, unsigned whole mm; default `A4_LANDSCAPE` (297 x 210) |
| `projection`   | `first_angle` / `third_angle` | the per-document view-arrangement setting; default `third_angle` |
| `title_block`  | `TitleBlock`                  | the drawn-in-frame fields (see 2.4) |
| `views`        | `View[]`                      | standard views, in placement order |
| `dimensions`   | `Dimension[]`                 | annotate this sheet's views |

### 2.2 View

| field     | type                        | notes |
|-----------|-----------------------------|-------|
| `id`      | string                      | unique |
| `name`    | string                      | e.g. "Front" |
| `kind`    | `front` / `top` / `right`   | the standard view (closed vocabulary for this milestone) |
| `part_id` | string?                     | the part this view shows (document id space) |
| `scale`   | number                      | drawing scale applied to this view; default 1 |

A `View` is a **projection instruction**, not geometry. The projected edge
geometry is always computed at render time by the workbench from the model's
solid (never stored, never fabricated).

**Projection frames** (implemented in `oep-drafting::view`):

| kind    | normal | in-plane frame | shows |
|---------|--------|----------------|-------|
| front   | -Y     | u = X, v = Z   | length x height |
| top     | -Z     | u = X, v = Y   | length x width |
| right   | -X     | u = Y, v = Z   | width x height |

The first/third-angle convention changes only the **sheet arrangement** of the
(identical) projected contents: third-angle places TOP above FRONT and RIGHT to
the right; first-angle folds them to TOP below FRONT and RIGHT to the left.

### 2.3 Dimension

| field      | type                     | notes |
|------------|--------------------------|-------|
| `id`       | string                   | unique |
| `name`     | string                   | human label |
| `view_id`  | string                   | the view it annotates (same sheet) |
| `kind`     | `horizontal` / `vertical`| |
| `variable` | string?                  | FreeCAD-v1 variable-set entry name; omitted for a measured-only dimension |

The dimension **value is not stored**: it is resolved at render time from the
model's `oep-sketch` `VariableSet` feature when `variable` is bound (the SAME
entry that drives the model) and cross-checked against the measured projected
extent; otherwise from the measured extent. An edit to the variable re-solves,
re-drives the model, and the redrawn dimension follows.

### 2.4 TitleBlock

| field         | type   | notes |
|---------------|--------|-------|
| `title`       | string | required |
| `part_number` / `material` / `scale` / `revision` / `drawn_by` / `date` / `sheet` / `company` | string? | optional, omitted when absent |

## 3. Validation rules added (codes, see `oep-geo/src/validate.rs`)

- `bad-sheet-size` — a sheet size has a zero dimension.
- `bad-scale` — a view scale is not finite and positive.
- `unresolved-part` — a view names a missing `part_id`.
- `unresolved-view` — a dimension names a view not on its sheet.
- drawing/sheet/view/dimension ids join the one-document id space
  (`duplicate-id`).

## 4. Rendering and export (oep-drafting)

1. `SolidEvaluator` replays the model's feature graph into a solid mesh
   (`solid_from_graph`; MiniBox vocabulary, Trim is honestly not-kernel).
2. `view::project_edges` projects the mesh's true edges per standard view.
3. `layout::layout_views` arranges them on the sheet per first/third angle.
4. `render::render_sheet` resolves dimension labels from the variable set and
   places the title block, into a `RenderedDrawing`.
5. `export::svg` / `export::pdf` write deterministic vector documents.

## 5. Goldens and acceptance

Committed under `crates/oep-drafting/tests/golden/`:
`bracket-drawing.oepgeo.json` (the v2 Document), `bracket-drawing.svg`,
`bracket-drawing.pdf`. Byte-locked by `tests/drawing_acceptance.rs` and
independently re-parsed by `tests/test_oep_drafting_acceptance.py`, plus the
variable-edit-follows test (width 80 -> 120).
