# EDITOR-2D-DRAFTING RE note: drafting workbench on the real kernel

Task: EDITOR-2D-DRAFTING (t_49eeb2e7), the first pull-forward into the
editor spine (decision #19, 2026-10-22). Delivered on branch `wave/editor`.
Version 1 (2026-10-22).

## What landed

A 2D drafting workbench on the real kernel and `oep-geo` feature graph:

1. `crates/oep-drafting` — standard/projected views are COMPUTED from the
   model: `SolidEvaluator` replays the `oep-geo` feature graph into a solid
   mesh (`solid_from_graph`), `view::project_edges` projects the mesh's true
   edges onto each standard view plane, and `layout::layout_views` arranges
   the views on the sheet per the document's first/third-angle setting.
2. Dimension annotations wired to the `oep-sketch` variable set
   (FreeCAD-v1-variable-set behavior): a dimension's drawn value resolves, by
   variable name, from the SAME `VariableSet` entry that drives the model;
   `builder::drive_bracket_drawing` drives a bracket part (sketch + extrude)
   and its drawing from one variable set (width=80 / height=40 / depth=6).
3. Title block + sheet management + 2D export: deterministic SVG and a
   minimal valid PDF 1.4 over one shared primitive model
   (`export::Prim` + shared Helvetica text metrics), plus the honest
   STEP/2D story in `docs/drafting-step-2d-story.md` (STEP carries 3D B-Rep,
   never a 2D sheet; drawings export as vector documents; DXF is a
   documented future lane).
4. Class 2 versioned `oep-geo` extension: schema **v1 -> v2** adding the
   `drawing` class (sheets/views/dimensions/title blocks), with the
   P6-M1-T03 tolerant migration (v1 loads, promoted to v2 on parse;
   `Document::migrate`).

## Measured result (real material, live on the wave/editor worktree)

- `crates/oep-drafting/tests/golden/bracket-drawing.oepgeo.json` — canonical
  v2 Document: 1 part / 1 solid body bbox (0,0,0)-(80,40,6) / Sketch +
  VariableSet + Extrude features / one A4-sheet drawing with three standard
  views and three variable-driven dimensions.
- `bracket-drawing.svg` / `.pdf` — the rendered drawing; every view is a
  4-edge rectangle of the box (front 80x6, top 80x40, right 40x6) and the
  dimension labels read 80 / 40 / 6.
- Rust acceptance `crates/oep-drafting/tests/drawing_acceptance.rs` (6 tests)
  byte-locks the goldens, independently re-derives the projected rectangles,
  checks variable->dimension wiring AND the following edit (width 80 -> 120:
  drawing label and geometry both update), and checks first- vs third-angle
  arrangement.
- Python acceptance `tests/test_oep_drafting_acceptance.py` (7 tests)
  re-parses the goldens with pure stdlib (json / xml.etree / a structural
  PDF/xref walk) and re-derives per-view extents, layout order and labels.
- Workspace: `cargo test --workspace` green (exit 0, zero failures), including
  the schema-version bump across all crates that consume the canonical model
  (oep-geo, oep-sketch, oep-import, oep-kinematics, oep-shell, oep-step, ...);
  3 importer-hub goldens + 2 model goldens re-migrated to v2 and re-verified.

## Fidelity decision (this card owns it)

Views are projected-edge wireframes of the solid (no hidden-line removal —
that is the later drawing milestone, consistent with the established
SVG/PDF exporters' stance). "Trim" is not representable on the tessellated
lane and returns a typed error (kernel boolean is P6-M3-T04). The drawing
record is Class 2 (schema v2) because dimensions must reference the model's
variable set inside one canonical document.

## Handoff notes

- Schema migration pattern to reuse for the next Class 2 extension: keep
  `#[serde(default)]` on new collections, promote on parse via
  `Document::migrate`, migrate committed goldens through the canonical
  serializer, and assert versions against `oep_geo::document::VERSION` (never
  a literal) so a future bump fails loudly and obviously.
- The `export::Prim` primitive model is the seam for a DXF writer and for
  future section/HLR views.
