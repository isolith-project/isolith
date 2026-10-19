# P3-M1-T03 — Mesh rep -> internal mesh reader (delivery note)

Status: verified on real material 2026-10-16 · P3 / oep-3dxml

## What this card delivers

`oep_3dxml::mesh` — the reader side of the tessellated-ASCII 3DXML branch.
Given the `.3DRep` member markup (the `<XMLRepresentation ...>` document
tree), it returns one internal `Mesh { positions, triangles }` per
`xsi:type="PolygonalRepType"` rep that carries actual geometry, in document
order. Mesh accessors return plain slices (`&[[f32;3]]`, `&[[u32;3]]`) — the
exact shape the P3-M4-T01 writer consumes, so a P3-M4-T02 round-trip can feed
reader output straight into `ascii::write_package`.

## Format facts relied on (all observed, hex notebook §4.2 + P1-M1-T02)

- Root `XMLRepresentation`; reps nested under `Root xsi:type="BagRepType"`.
- Geometry rep: `Rep xsi:type="PolygonalRepType"` with
  `<VertexBuffer><Positions>x y z,x y z,...</Positions>` (comma-separated
  triples, no count header) and `<Faces><Face .../>`.
- Two observed face encodings, BOTH decoded:
  - `triangles="i j k i j k ..."` — space-separated triplets, 3 per triangle
    (Abaqus-exporter branch: cube1/blocks).
  - `strips="r1,r2,..."` — comma-separated strip runs, each run
    space-separated vertex indices; N indices -> N-2 triangles (CATIA V5
    branch: machinist/cycloidal/FM1; our writer emits 3-index runs -> exactly
    1 triangle each, so writer output is unambiguous).
- Strip winding on runs >3 `[uncertain]` (no vendor viewer in clean-room).
  Pinned convention: standard OpenGL — triangle k of a run is
  `(i[k], i[k+1], i[k+2])` for even k and `(i[k+1], i[k], i[k+2])` for odd k.
  Triangle counts, index bounds and bbox are convention-independent and are
  the hard assertions.
- One `.3DRep` member may carry many polygonal reps (FM1: dozens); the parser
  returns one `Mesh` each. Edge-only / empty reps (silhouette `<Edges>
  <Polyline>`) are SKIPPED, never an error (tolerant-failure, P2-M1-T04).
  Unobserved rep types (`TessellatedRepType` etc.) are ignored the same way.

## Verification against known-good geometry

- Synthetic fixtures assert EXACT decoded positions/triangles for a quad via
  `triangles=`, via `strips=`, for a 5-index strip, nested BagRep wrappers,
  and multiple reps per document (crates/oep-3dxml/tests/mesh_rep.rs).
- Real material: the Rust test cross-checks a committed GOLDEN-FACTS table
  (positions count, triangle count, bbox, first-position class) derived by an
  INDEPENDENT stdlib-only Python decoder
  (tests/test_oep_3dxml_mesh_rep_acceptance.py) — no shared code between the
  two decoders. Members verified: Abaqus cube1 (10x10x10 cube, 12 tris,
  surface ~600), Abaqus blocks, CATIA machinist TessPart_2 (42 meshes, first
  134 pos / 86 tris), CATIA cycloidal TessPart_5 (long strips, mesh 8 =
  4884 pos / 4488 tris). The Rust leg cross-checks on the same corpus;
  otherwise SKIP and pass so CI stays green before the P1 corpus PR merges.
- Independent decoder footprint across the committed corpus: 3238 geometry
  meshes decoded, all with in-range indices.

## Errors and safety

- `MeshError` is typed (MalformedXml / NotXmlRepresentation / BadPosition /
  IndexOutOfRange), never panics. Fuzz-friendly (P2-M1-T05 discipline).
- `#![deny(missing_docs)]` crate-level upheld; `quick-xml 0.39` is the only
  new runtime dep (same pin as sibling P3-M1-T02), `zip` is a dev-dep only
  (self-contained PR, mergeable independently of T01/PR #56).

## License posture

Observed facts only + internal-use licensed spec (P1-M1-T02, never quoted).
No Dassault XSD text reproduced; no vendor library reads for us.

## Known limitations (honest)

- Strip winding on runs >3 is pinned, not independently confirmed against a
  3DS viewer (clean-room posture); counts/bbox/bounds are exact regardless.
- PCM/materials (`SurfaceAttributes`, color) inside `<Face>` are ignored —
  that is P3-M2/P3-M4 territory, not mesh extraction.
- Binary V5_CFV3 `.3DRep` members are out of scope here (P3-M3).

## Hotspots handled

- `crates/oep-3dxml/src/lib.rs` and `Cargo.toml` are shared with sibling P3
  cards (T02 rep-parse, T06 roundtrip, writer); this card only APPENDS a
  module + the quick-xml pin (identical line to T02) — no overwrite. Flagged
  in completion metadata.
