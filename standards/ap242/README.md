# STEP AP242 — Managed Model-Based 3D Engineering

## Standard

ISO 10303-242:2022, *Industrial automation systems and integration — Product data
representation and exchange — Part 242: Application protocol: Managed model-based 3D
engineering* (Edition 3). A newer ISO 10303-242:2025 (Edition 4) has been published;
Edition 3 was withdrawn by ISO. The STEP AP242 standard is the merger of aerospace
STEP AP203 (configuration-controlled 3D design) and automotive STEP AP214 (core data
for automotive mechanical design processes).

## Publisher

International Organization for Standardization (ISO), Technical Committee
ISO/TC 184/SC 4 (Industrial data). The industry-side implementer coordination for
AP242 is hosted by the CAx Implementor Forum (CAx-IF) / AFNeT / prostep ivip, with
project portal at ap242.org.

## Current edition

- ISO 10303-242:2025 (Edition 4) — current international standard (replaces the
  2022 Edition 3).
- ISO 10303-242:2022 (Edition 3) — withdrawn as of the 2025 edition.
- Earlier: Edition 2 (2020), Edition 1 (2014).

## Availability

- The ISO document is a **copyrighted paid standard**; the official product page is
  at iso.org/standard/84667.html. ISO sells the PDF/paper; it is not freely
  redistributable.
- Industry-side project and implementation material is **free to access**:
  ap242.org (one-page overview, use cases, benchmark reports) and the CAx-IF
  Recommended Practices (e.g., the PMI recommended practices PDF) are public.

## License / redistribution status

- The ISO standard text (ISO 10303-242) is under **ISO copyright** — do NOT vendor
  the ISO PDF into this repo. Record the official link instead.
- The CAx-IF Recommended Practices documents are public informational documents and
  are referenced, not copied, here.
- Clean-room posture: observation of the published standard and recommended
  practices is lawful; no vendor source code or binaries are used (program P0-M2
  legal workstream).

## Official source

- https://www.iso.org/standard/84667.html (ISO 10303-242 — official product page)
- https://www.ap242.org (project portal, overview, use cases, benchmarks)
- https://www.cax-if.org (CAx Implementor Forum, recommended practices)

## Scope

From the public ISO abstract (fair quote):

> ISO 10303-242 specifies the application protocol for Managed model based 3d
> engineering. ... The following are within the scope of ISO 10303-242: ...
> geometrically bounded surface geometry model; topologically bounded surface
> geometry model; faceted-boundary geometry model; boundary geometry model;
> compound shape geometry model; constructive solid geometry model; parametric
> and constrained geometry model; 2D-sketch model; 3D tessellated geometry model;
> 3D scan data; curved triangles.

In practice AP242 is the workhorse for exchanging **semantic PMI / MBD data** —
GD&T, datums, tolerances, and annotations attached to 3D geometry — plus B-Rep and
tessellated geometry. It is the de facto neutral format for "managed model-based 3D
engineering" across automotive, aerospace, and defense.

## PMI / MBD relevance

- AP242 is the primary carrier for **semantic PMI** (GD&T, datums, tolerances) in a
  Model-Based Enterprise. This is the exact interoperability pain point P4 targets
  (the CATIA round-trip problem noted in the master plan).
- P4-M1 builds `oep-step` over an existing OSS STEP library with clean wrappers;
  P4-M1-T02 requires AP203/214/242 import path parity against NIST/CAx test suites;
  P4-M1-T03 is the accuracy harness.
- P4-M2 semantic PMI import / re-export builds on AP242; P4-M2-T03 is the GD&T
  fidelity audit matrix.
- P4-M3 STEP export targets AP242 geometry (PMI later); P4-M5 MBD validator checks
  models against Y14.41-style rule sets.
