# P4-M4-T03 JT reader (visualization-first wedge): implementation plan

Card: t_6d40b761, gated on P4-M4-T02 ADR (done, PR #88).
Branch: task/jt-reader-1016a (based on oep-step-1016a read-lane stack + merged ADR).

## Decision restated (from the ADR)

Build, do not adopt. GNU General Public License (GPL) readers are license
incompatible; permissive candidates are immature; no license-clean maintained
Rust-usable standalone JT reader exists. JT is documented (ISO 14306 / PSI
14-1 JTIAP V3.1 / Siemens JT v10 File Format Reference Rev-B / DIN SPEC
91383), so the build is spec-driven and clean-room compatible. The reader
lives behind the oep-step vendor-neutral seam; no JT segment/codec vocabulary
crosses the public surface.

## Spec sources (grounded 2026-10-16)

- Siemens JT v10 File Format Reference Rev-B (downloaded, 32325-line text
  extract held under scratch; sections cited below as "Ref sec N").
- JTIAP (PSI 14-1 V3 / JT 10.5) 2024-03-04 PDF (download held in scratch;
  sections cited as "JTIAP N").
- The two documents agree on every layout implemented here; where they differ
  in wording, JTIAP is the newer revision and the cited section is checked
  against it.

Specific layouts grounded from the specs:

- File header (Ref 5.1.1 / JTIAP 5.1.1): 80-char version string (last 5
  bytes are the ASCII/binary translation detection bytes ` \n \r \n `), 1-byte
  Byte Order (0 LSB first, 1 MSB first), I32 Empty Field, U64 TOC Offset,
  GUID LSG Segment ID, GUID Empty Field.
- TOC segment (Ref 5.1.2): I32 entry count then per entry GUID Segment ID,
  U64 Segment Offset, U32 Segment Length, U32 Segment Attributes (bits 24-31
  = segment type; table: 1 LSG, 2 JT B-Rep, 3 PMI, 4 Meta Data, 6 Shape,
  7-16 Shape LOD0-9, 17 XT B-Rep, 18 Wireframe, 20 ULP, 24 LWPA).
- Segment header (Ref 5.1.3.1): GUID Segment ID, I32 Segment Type,
  I32 Segment Length (includes header).
- Element header (Ref 5.1.3.2 Logical Element Header): I32 Element Length,
  then Element Header: GUID Object Type ID, U8 Object Base Type, I32 Object
  ID. The "Compressed" variant (used inside segment types that support whole
  segment compression) adds, before the element header, U32 Compression Flag
  (=3 LZMA on, else off), I32 Compressed Data Length, U8 Compression
  Algorithm (1 none, 3 LZMA). End-of-elements marker GUID is
  0xffffffff-ffff-ffff-ff-ff-ff-ff-ff-ff-ff (Ref Annex A).
- LSG nodes (Ref 6.1.1): Base Node Data = U8 Version, U32 Node Flags, I32
  Attribute Count + Attribute Object IDs. Partition/Group have Group Node Data
  with child counts and child Object IDs. Part/Meta Data wrap Group Node Data.
  Background: LSG is a DAG of object-id linked records; the first element in
  an LSG segment is a Partition node.
- Property atoms (Ref 6.2): Base Property Atom Data + String version gives
  names; Late Loaded Property Atom carries GUID Segment ID + I32 Payload
  Object ID -> links a shape node to its Shape LOD segment and element.
- Attributes (Ref 6.1.2): Base Attribute Data = I8 Version, U8 State Flags,
  U32 Field Inhibit, U32 Field Final. Material Attribute Element object type
  0x10dd1030-... carries RGBA ambient/diffuse/specular/emission + shininess.
  Geometric Transform Attribute object type 0x10dd1083-... carries a 16-bit
  Stored Values Mask + stored F32 matrix elements (row-major), identity-padded.
- Shape LOD (Ref 7.1 / 7.2): Base Shape LOD Data holds the version; a shape
  segment holds one Shape LOD element. Primitive Set Shape Element
  (Ref 7.2, object type 0xe40373c2-...) carries, for Bits Per Vertex = 0 and
  negative Compressed Data Size, raw per-primitive records: I32 reserved,
  CoordF32 params1, DirF32 params2, Quaternion params3, RGB colour, I32
  primitive type (0 Box, 1 Cylinder, 2 Pyramid, 3 Sphere, 4 Tri-Prism).
  Interpretation (Ref Table 52): Box = min corner (params1) + length vector
  (params2) + orientation quaternion; this is a deterministic facet source.
- Bitlength CODEC (Ref 13.2.2 + Annex B.2): fixed / adaptive-width bitfield
  integer coding with prefix codes (0 = same width, 1 + inc/dec bits +
  complement terminator, step size 2 bits). Implemented for decode and unit
  tested against the reference examples.

## Milestone 1 scope (honest boundary)

In scope (implemented and tested against a synthesized license-clean golden):
container spine (header / TOC / segment headers / element headers, including
the compressed-variant recognition), LSG traversal (partition/group/part/
instance/LOD/switch/shape nodes by object id, names via string property,
bounding data, material RGBA + geometric transform attributes), shape decode
for the documented uncompressed (negative Compressed Data Size) Primitive Set
elements (Box at minimum, feature-count/bbox pinned), the Bitlength CODEC, and
mapping onto the canonical oep-geo Document/part/body/feature model. The
reader is tolerant: unknown element GUIDs are skipped with warnings, never
panics, and returns typed errors (HeaderError / SegmentError / ElementError /
CodecError / ShapeError).

Documented-but-deferred to later milestones (return typed errors, never
fabricate geometry):
- LZMA segment/element decompression (=3 flags), CDP/arithmetic and Deering
  normal CODECs, topologically compressed Rep Data (v9/v10 TriStrip/Polygon
  sets), XT/STEP B-Rep segments, Meta Data / PMI Manager segment (the ADR's
  gated second milestone). The wedge deliberately pins geometry via the
  uncompressed primitive path so golden facts are real observables, not
  estimates.

## Corpus (decision-7 posture: dev-only, never shipped)

- Golden fixtures are synthesized to the documented byte layout by a committed
  Python writer (python/oep_tooling/jt_fixtures.py), license-clean (our own
  bytes), catalogued under the dev-only posture. No real production JT file is
  redistributed. The reader is written from the spec text (clean-room), so
  writer and reader are independent implementations of the documented layout.
- Fixtures committed under corpus/jt/ (header/TOC/LSG/shape) plus corrupt
  variants (truncated header, bad byte order, TOC offset out of range,
  unknown segment type, element length overflow). Golden test pins decoded
  facet counts + bounding volume within tolerance.

## Files

- crates/oep-step/src/jt/mod.rs - public seam: `parse`, `to_document`,
  `parse_to_document`, vendor-neutral carriers (no JT vocabulary on public
  items).
- crates/oep-step/src/jt/error.rs - typed Error enum.
- crates/oep-step/src/jt/container.rs - header / TOC / segment / element walk.
- crates/oep-step/src/jt/lsg.rs - LSG node/attribute/property traversal.
- crates/oep-step/src/jt/shape.rs - shape element decode (primitive set +
  structure of vertex-LOD paths, codec-gated).
- crates/oep-step/src/jt/codec.rs - Bitlength CODEC (implemented) + typed
  UnsupportedCodec for arithmetic/Deering/LZMA.
- crates/oep-step/tests/jt_golden.rs - golden + corrupt acceptance (Rust).
- tests/test_p4m4_jt_reader.py - pytest acceptance incl. seam scan (no JT
  segment/codec vocabulary on public items) and no-vendored-foreign-source
  scan (TKJT / JT Assistant / PyOpenJt identifier scan must FAIL the build).
- python/oep_tooling/jt_fixtures.py + corpus/jt/*.jt - license-clean golden
  and corrupt fixtures + provenance.
- docs/re-notes/oep-jt-container-and-lsg.md - decode notes citing spec
  sections (living doc, updated in later milestones).
- workstreams/P4-M4/jt-import-sources.md - append wedge delivery facts + fixture
  provenance (living evidence file; ADR stays at v1.0.0).

## Acceptance gate (ADR section 9)

1. Golden JT sample opens to a known mesh: vertex/triangle counts and bounding
   volume within tolerance (asserted in jt_golden.rs + pytest).
2. Unsupported/corrupt input -> typed errors, never a panic (corrupt fixture
   corpus + fuzz-ish junk tests).
3. Vendor-neutral seam stays clean: a CI-scan (pytest) rejects JT segment /
   codec vocabulary on public items.
4. No vendored foreign source: identifier scan rejects TKJT / JT Assistant /
   PyOpenJt tokens anywhere in the crate (build fails if present).
5. cargo test + pytest + precommit gate green; Gitea CI green (8/8).

## Execution order (TDD)

1. RED: write pytest acceptance (tests/test_p4m4_jt_reader.py) + Rust golden
   tests (crates/oep-step/tests/jt_golden.rs) defining the contract.
2. Corpus: python/oep_tooling/jt_fixtures.py writes golden + corrupt fixtures;
   commit bytes under corpus/jt/.
3. GREEN: implement container / lsg / shape / codec / error behind
   `oep_step::jt`, map to oep-geo.
4. Seam + no-vendored scans wired into pytest (CI + precommit gate run the
   same script).
5. Verify: cargo test --workspace, pytest, precommit-gate.sh; then push the
   branch and confirm Gitea Actions CI 8/8 green before completing the card.
