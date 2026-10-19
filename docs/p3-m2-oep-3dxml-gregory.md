# P3-M2-T01: Gregory-patch surface decode to NURBS

## Deliverable

`crates/oep-3dxml` now decodes Gregory-patch surface representations into
exact NURBS surfaces using own math (no Open CASCADE dependency).

- `oep_3dxml::gregory` — bicubic `GregoryPatch` (20 control points), direct
  rational evaluation, cubic `BezierCurve3` boundaries, and the EXACT decode
  to a rational biquartic `NurbsSurface` (degree 4x4, clamped single span,
  control points + weights).
- `oep_3dxml::lattice` — the "surface rep" data model: `LatticeMesh`
  (vertices / faces / edges + per-vertex and per-edge rounding weights +
  per-edge endpoint tangent vectors), a minimal `XvlShell` X3D parser
  (DEF/USE aware), and the per-face reconstruction to `GregoryPatch`.

Real material verified: the license-clean Web3D Consortium LatticeXvl
`XvlShell` examples committed at `corpus/data/3dxml/surface/` (Star, Dolphin,
Baby, Octopus, Hand + weight/vector attribute demos). Every decoded patch of
every model is finite and its NURBS evaluation equals the Gregory evaluation on
a sampling grid (the decode is exact; tolerance 1e-9 absorbs float rounding).

## How it is verified

- TDD: `crates/oep-3dxml/tests/gregory_decode.rs` (10 tests) written first —
  exactness, planarity, corner limits, boundary-vs-Bezier, weight
  non-negativity, per-model golden counts (Star 78v/80f -> 96 patches;
  Dolphin 193 mixed-n-gon faces), real edge-vector tangent exactness.
- `tests/test_gregory_decode_acceptance.py`: independent stdlib-only parse of
  the committed corpus (sha256 registry, ring/edge index validity, weight
  cardinality, DEF/USE resolution, topology goldens).
- Both run in CI: pytest gate + `cargo test --workspace` (lint/clippy deny
  warnings).

## Honesty limits (see docs/re-notes/gregory-patch-decode.md)

The Gregory -> NURBS step is exact and complete. The lattice -> Gregory
reconstruction reproduces, exactly and on real material, everything the public
data pins down: vertex interpolation and the boundary cubics from edge tangent
vectors. The CATIA/Lattice rounding-weight-to-interior stencil is proprietary;
v1 uses a documented harmonic-equilibrium interior and marks the parity
`[uncertain]`, deferred until the licensed 3DXML XSD is in hand
(P1-M1/3dxml-spec-process.md).

## Base / stack

Branch `task/gregory-decode-nurbs-1015a` off `gitea/main` (9dd24b0). The
`NurbsSurface` output is the handoff type for P3-M2-T02 (tessellation) and the
P6 geometry core / STEP exporter.

## Files

- crates/oep-3dxml/src/gregory.rs      (Gregory patch, NURBS decode, curves)
- crates/oep-3dxml/src/lattice.rs      (surface-rep model + XvlShell parser)
- crates/oep-3dxml/src/vec3.rs         (dependency-free vector math)
- crates/oep-3dxml/tests/gregory_decode.rs
- tests/test_gregory_decode_acceptance.py
- corpus/data/3dxml/surface/           (LICENSE-clean real material + README)
- docs/re-notes/gregory-patch-decode.md
