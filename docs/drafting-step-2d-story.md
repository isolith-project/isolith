# Drafting and STEP: the honest 2D story (EDITOR-2D-DRAFTING)

Engineers hand drawings to fabrication. This note states plainly what
`oep-drafting` does with STEP (and what it does NOT), so nobody claims a 2D
drawing lives inside a STEP file.

## What STEP is, and is not

STEP (ISO 10303-21, AP203/AP214/AP242) is a **3D** B-Rep / semantic-PMI
exchange format. It exchanges solid and surface geometry, and (AP242) 3D
annotations. It has **no sheet drawing system**: there is no title block, no
2D projected sheet, no drawing layout concept in the STEP part 21 exchange
structure. CAD systems that "export drawings to STEP" are actually exporting
the 3D model (optionally with 3D PMI: the MBD path — see
`docs/pmi-model-schema.md`), not a 2D sheet.

## The honest consequence for 2D drafting

A 2D drawing is a **set of orthogonal projections arranged on a sheet**, and
the industry exchanges that as a document: PDF, DXF/DWG, or the native format.
There is no honest "STEP export of a 2D drawing." So:

- `oep-drafting` 2D export is **vector documents**: SVG and a minimal PDF 1.4
  (`write_drawing_svg` / `write_drawing_pdf`), both computed from the model's
  projected edges — deterministic, inspectable, printable.
- The **3D side** stays STEP-native: `oep-geo`'s STEP exporter
  (`oep_geo::export::write_step`) writes the solid model honestly as a
  faceted B-Rep with `StepOpts`, and the importer hub (`oep-import`) reads
  real STEP AP203/AP214/AP242 parts into the canonical model. That is the
  correct home for STEP, and it is where a downstream print of the part
  belongs, not inside the drawing file.
- MBD (model-based definition) is the modern "drawing-less" path and carries
  its annotations in the 3D model (AP242 PMI) — the P4 lane's
  `oep-step-pmi` importer feeds that (not 2D sheets).

## What the drafting workbench exports

| format | what | notes |
|--------|------|-------|
| SVG    | full drawing (views, title block, dimensions) | vector, orientation/dimension-consistent with the sheet model, committed golden |
| PDF    | full drawing (views, title block, dimensions) | minimal valid PDF 1.4, committed golden |
| STEP   | the 3D model, never the sheet | `oep_geo::export` / `oep-import`, faceted B-Rep, honest `StepOpts` |

DXF (the traditional 2D interchange with fab shops) is a **documented future
lane**, not silently promised: the primitive model (`export::Prim`) is the
seam a DXF writer would consume. See the workstream record
`workstreams/modeler/drafting/`.
