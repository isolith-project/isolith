# AP242 — Keelworks Use

## Why Keelworks cares

AP242 is the neutral, vendor-independent carrier for the geometry + PMI that Keelworks
must read and write to be a serious interoperable engineering platform. The master
plan's P4 workstream (STEP AP242 / QIF / JT Interop) is built around it. It is also
the format defense customers most often require (tied to MIL-STD-31000 TDPs).

## Workstream mapping

- **P4-M1 (STEP import)** — `oep-step` crate over an existing OSS STEP library
  (Open CASCADE or pythonOCC hooks), vendor-neutral wrapper. AP242 is the primary
  AP target alongside AP203/214.
- **P4-M1-T02** — import path parity tests against NIST / CAx test suites using the
  AP242-based test cases published by the CAx-IF.
- **P4-M2 (Semantic PMI)** — import GD&T, datums, tolerances from AP242 into the
  internal PMI model (P6-M1-T02); re-export to AP242 with correctness tests; GD&T
  fidelity audit matrix (P4-M2-T03).
- **P4-M3 (STEP export)** — geometry-only export first, PMI later; validate against
  an independent reader (FreeCAD + OCC) with golden diffs.
- **P4-M5 (MBD validator)** — the wedge product surface: validate a model/drawing
  against Y14.41- and AP242-informed rule sets, and emit reports.

## What we take from the standard (observational, license-safe)

- **Semantic PMI entity model**: what GD&T/datum/tolerance entities exist, and how
  they attach to geometry. This informs the internal PMI model schema without
  copying entity texts.
- **AP scope / AR (application requirements)**: which geometry representations are
  in scope (B-Rep, tessellated, CSG, etc.) so `oep-geo` supports at least the same
  surface.
- **Recommended practices**: the CAx-IF implementation guidance is the practical
  interop spec (how vendors actually write AP242); it is our reference for what
  real files look like.

## Risks / notes

- ISO 10303-242 is copyrighted, so we must not copy standard text into `oep-step`
  or documentation beyond fair quotation. Our implementation follows the EXPRESS
  schema's public domain informational summaries and recommended practices, not
  copied textual extracts.
- The 2025 Edition 4 is the current one; Edition 3 (2022) files still circulate —
  the reader must accept both (P4-M1 parity covers this).
