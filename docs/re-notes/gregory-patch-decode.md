# Gregory-patch surface decode to NURBS (P3-M2-T01)

Version: 1.0, 2026-10-15. Author: Isolith default worker.
Re-notes discipline: written for the next worker/P6; `[uncertain]` marks every
part that is NOT byte-decoded from a proven spec.

## 1. Goal

A 3DXML/3DS surface representation is stored as a coarse lattice mesh (faces,
edges, vertices, rounding weights) that a compliant viewer reinterprets into a
smooth FREE-FORM SURFACE made of bicubic GREGORY patches. P3-M2-T01 decodes
each Gregory patch into an EXACT NURBS surface so P3-M2-T02 (surface->mesh
tessellation) and the STEP/Ap242 exporter can consume exact surfaces instead
of a triangle soup. Own math is used (no Open CASCADE dependency); the task
explicitly allowed either path.

## 2. The format family (lattice / Gregory)

- 3DXML surface data is stored as Gregory patches; the "surface mesh"
  (topology, faces, edges, vertices, ROUNDING WEIGHTS) is reinterpreted back
  into a surface by a compliant viewer (Wikipedia "3DXML", cross-referenced by
  Spatial Corp and CAD-interop glossaries).
- The technology is Lattice Technology's XVL **Lattice** structure, which
  Dassault licensed for 3DXML (Lattice itself describes a Lattice Mesh of
  topologies/faces/edges/vertices/rounding weights and a Lattice Surface of
  Gregory patches, interchanged by an "invertible rounding" algorithm; see the
  Web3D Consortium LatticeXvl proposal by M. Seki / Lattice Technology).
- Real material committed: the Web3D Consortium's public X3D `XvlShell`
  examples (BSD-style Web3D license, `corpus/data/3dxml/surface/`). They are
  the open, license-clean demonstration of exactly this representation family:
  coarse vertex/face/edge data plus `vertexRound` (0.5..1.5, default 1.0),
  `edgeRound`, and per-edge endpoint tangent VECTORS
  (`edgeBeginVector`/`edgeEndVector`).
- `[uncertain]` Whether the 3DXML ASCII 4.x schema carries this exact field
  layout byte-for-byte, or whether real CATIA surface data lands in the binary
  `V5_CFV3` branch (as observed on the Imagine & Shape gallery sample, see
  docs/re-notes/3dxml-hex-notebook.md): NOT proven from the licensed XSD (not
  yet obtained; see workstreams/format-corpus/samples/3dxml-spec-process.md). The math and the decode
  pipeline here are format-independent; the parser covers the public
  `XvlShell` spelling of the lattice, and the identical field set is what the
  3DXML binary branch is expected to serialize once P3-M3 decodes it.

## 3. Bicubic Gregory patch (the object being decoded)

20 control points: 4 corners, 8 edge-interior (2 per side), 8 interior twist
points (a 2x2 "(uv)" set + a 2x2 "(vu)" set). Evaluation is the rational
blend of two tensor-product bicubics:

    S(u,v) = ( u(1-v) * S1(u,v) + (1-u)v * S2(u,v) ) / ( u(1-v) + (1-u)v )

S1 uses the (uv) twist set, S2 the (vu) set; all boundary control points are
shared, so each boundary is a plain cubic Bezier and adjacent patches can be
G1 without matched twists (Gregory 1974). The blend ratios are 0/0 exactly at
the two corners (0,0) and (1,1); the limit there is the corresponding corner
point. `crates/oep-3dxml/src/gregory.rs` implements `eval` (with corner-limit
handling) and `boundary_curve(edge)`.

## 4. Exact decode to NURBS (the acceptance core)

The rational form is EXACTLY a single rational biquartic Bezier patch (degree
4x4). Derivation done from first principles (Bernstein algebra, no reference
copied):

- Numerator  N(u,v) = u(1-v)*S1 + (1-u)v*S2  -> biquartic, via Bernstein
  product of a bicubic grid by the bilinear weight (1,1 -> 4,4).
- Denominator D(u,v) = u(1-v) + (1-u)v  -> biquartic, by degree-elevating the
  (1,1) weight from degree 1 to 4 in each axis.
- NURBS control point at grid slot (i,j): weight = D's (4,4) Bernstein
  coefficient, position = N's coefficient / weight. Weights are >= 0; the two
  corner slots (0,0) and (4,4) carry weight 0 (the two singular corners).

The output `NurbsSurface` is a clamped single-span (Bezier) rational NURBS:
degree = (4,4), 5x5 control points + weights. Tests assert
`nurbs.eval(u,v) == gregory.eval(u,v)` to ~1e-9 over a full sampling grid on
every real-material patch (it is mathematically exact; the tolerance absorbs
float rounding only). This is the table the P6 geometry core and the STEP
exporter consume.

## 5. Lattice -> Gregory reconstruction (v1)

`crates/oep-3dxml/src/lattice.rs` parses `XvlShell` X3D documents into
`LatticeMesh` and reconstructs one `GregoryPatch` per quad face (n-gons fan
into quads around the face centroid -> n patches, documented). The per-face
decode:

- **Corners**: exactly the lattice vertices  (reproduced exactly; asserted).
- **Edges**: the boundary cubic is built from the lattice edge's endpoint
  tangent vectors with the analytic-tangent convention:
  `B1 = P0 + t@P0/3`, `B2 = P1 - t@P1/3`, each vector scaled by the edge
  rounding weight. An edge with no stored vector decodes to a straight chord
  (matches the `edgeBeginVector = (0 0 0)` demo exactly; asserted on real
  material via the `(0 2 0)` demo).
- **Interior twist points**: harmonic-equilibrium fill (each interior control
  point = mean of its four grid neighbours with the fixed boundary; a small
  4x4 linear solve). Twist sets coincide in v1 ("twist-free" patch), giving a
  smooth, boundary-exact, deterministic interior.

`[uncertain]` The exact CATIA / Lattice "invertible rounding" stencil for
mapping rounding weights to interior control points and to vector-less edge
curves is proprietary and NOT reproduced in v1. `vertexRound` is parsed and
carried on the model but does not yet alter the v1 interior; weight-vs-vector
interactions beyond the documented scaling are deferred. Anyone needing exact
CATIA parity must first obtain the licensed 3DXML XSD/user's guide
(workstreams/format-corpus/samples/3dxml-spec-process.md) and port its rounding formulas; the decode
pipeline and the exact Gregory->NURBS step will not change.

## 6. Verification (real material)

- Math: exactness, planarity, corner limits, boundary-vs-Bezier, weight
  non-negativity tests in `crates/oep-3dxml/tests/gregory_decode.rs`.
- Real corpus: every committed surface model (Star, Dolphin, Baby, Octopus,
  Hand) and the attribute demos parse and decode; every decoded patch is
  finite and `nurbs.eval == gregory.eval` on a 17x17 grid. Golden counts:
  Star 78 v / 80 f (72 quads + 8 tris -> 96 patches); Dolphin 193 f with
  mixed n-gons.
- Independent check: `tests/test_gregory_decode_acceptance.py` re-opens the
  corpus with stdlib-only XML parsing, verifies the sha256 registry, ring
  index validity, weight cardinality, DEF/USE resolution and the topology
  goldens.

## 7. Handoff notes for P3-M2-T02 / P6

- `NurbsSurface` is deliberately a plain Bezier NURBS: degree (4,4), clamped
  single-span knots, control points + weights. Tesselation (P3-M2-T02) should
  evaluate it directly; no knot surgery needed for single patches.
- The singular-corner caveat (denominator zero at (0,0)/(1,1) of each decoded
  patch) must be honored by tessellators (evaluate away from the exact
  corner, or use `GregoryPatch::eval` which returns the corner limit).
- Exactness guarantee only holds for Gregory decoded through
  `GregoryPatch::to_nurbs`; tessellation adds its own approximation budget.
