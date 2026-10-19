# oep-jt container and LSG decode notes (P4-M4-T03)

Reverse-engineering notes for the JT reader (N/A for CGR: this reader is
built from the published specification, not from observation. Cite the spec
section for every layout). Author: P4 lane wedge 2026-10-16. Living document:
new milestones append, they do not rewrite history.

## Grounding

Implementing surface is the published format description, clean-room:

- Siemens JT v10 File Format Reference Rev-B (primary byte layouts).
- JTIAP PSI 14-1 V3 (2024-03-04, JT 10.5 based) - cross-checked where it
  is the newer revision.
- DIN SPEC 91383:2024 and ISO 14306 are the same format family; no layout
  difference material to this wedge was found between the two documents.

## File header (Ref 5.1.1)

- 80-byte version string. Padding and the final five bytes are the
  ASCII/binary translation detection bytes: ` \n \r \n ` (0x20 0x0a 0x0d
  0x0a 0x20). A valid JT file starts with `Version ` or a `V<digit>` form.
- 1 byte Byte Order: 0 = LSB first, 1 = MSB first. Anything else is a
  typed BadHeader.
- I32 empty field (ignored, tolerant).
- U64 block-directory offset.
- GUID root reference (the LSG block id).
- GUID empty field (ignored).

Header total fixed size 125 bytes. Shorter input cannot be a JT file
(NotJt / Truncated); JPEG or other magic is not accepted.

## Block directory (Ref 5.1.2)

- I32 entry count (bounded to 4096; larger is a typed BadDirectory).
- Per entry: GUID block id (16), U64 offset, U32 length, U32 attributes.
  Bits 24-31 of attributes are the block type (Ref Table 6): 1 LSG,
  2 JT B-Rep, 3 PMI, 4 Meta Data, 6 Shape, 7-16 Shape LOD0-9, 17 XT
  B-Rep, 18 Wireframe, 20 ULP, 24 LWPA.

The wedge locates the root LSG block by the header root reference; when the
reference is absent it falls back to the first kind-1 block (tolerant), and
rejects a root whose block type is not 1.

## Block header (Ref 5.1.3.1)

- GUID block id (16), I32 block type, I32 block length (includes the 24
  header bytes). A declared length that overruns the file is a typed
  BadStructure.

## Element stream (Ref 5.1.3.2 + Figure 19)

LSG blocks support whole-segment compression, so their elements use the
"compressed" element header form: the first element is preceded by a U32
Compression Flag, then every element carries I32 Compressed Data Length +
U8 Compression Algorithm (1 = none, 3 = LZMA) before the Logical Element
Header (I32 element length + GUID object type + U8 object base type + I32
object id).

- Compression Algorithm 3 (LZMA) is a typed UnsupportedFeature in this
  wedge (documented, never guessed).
- Unknown object type GUIDs are skipped by element length with a warning
  (tolerant never-crash, P2-M1-T04 lineage).
- End-of-elements marker: the all-0xFF GUID (Annex A).

Shape LOD blocks (types 6-16) do NOT support whole-segment compression, so
their elements use the plain (non-flag) header form.

## LSG nodes (Ref 6.1)

- Base Node Data (6.1.1.1.1): U8 version, U32 node flags, I32 attribute
  count + I32 attribute object ids.
- Group Node Data (6.1.1.3.1): Base Node Data + U8 version + I32 child
  count + I32 child ids.
- Partition Node (6.1.1.2): group data + U8 local version + I32 partition
  flags + MbString file name + BBoxF32 + F32 area + 3 count ranges. The
  wedge reads children and, when present, the declared transformed bbox.
- Part Node (6.1.1.5): Meta Data Node Data (group) + U8 version + I32
  empty. The wedge reads children.
- Instance / LOD / Range LOD / Switch / Group nodes are recognized and
  skipped with a warning (out of the visualization wedge's leaf mapping).
- Property atoms: Base Property Atom Data = U8 version + U32 state flags.
  String Property Atom (6.2.2) = base + U8 version + MbString value.
  Late Loaded Property Atom (6.2.7) = base + U8 version + GUID block id +
  I32 block kind + I32 payload object id + I32 reserved (>=1).
- Property Table (6.3): I16 version + I32 element-property-table count,
  then per element: I32 element object id + key/value property atom id
  pairs terminated by key == 0.

Part resolution: the partition's children are parts; each part's property
table values provide the name (String Property Atom) and the shape reference
(Late Loaded Property Atom). Orphan parts are surfaced with a warning.

## Shape decode (Ref 7.2 Primitive Set)

- Element payload reads: U8 base shape version, U8 base primset version,
  U64 vertex bindings (coords only in wedge), U8 version (=1), I32 bits per
  vertex, I8 texcoord gen type.
- Bits per vertex == 0 => Lossless Compressed Primitive Set Data: I32
  uncompressed size, I32 compressed size (< 0 => raw primitive records;
  >= 0 => LZMA, unsupported-typed).
- Raw primitive record (Table 51): I32 reserved, CoordF32 params1,
  DirF32 params2, Quaternion params3, RGB colour, I32 type.
- Box (type 0, Table 52): params1 = min corner, params2 = side lengths,
  params3 = orientation quaternion. The wedge tessellates identity-rotated
  boxes into 8 vertices / 12 triangles (documented wedge convention);
  non-identity quaternions are a typed UnsupportedFeature (honesty budget:
  no fabricated orientation).
- Other primitive types (cylinder 1, pyramid 2, sphere 3, tri-prism 4):
  typed UnsupportedFeature in this wedge.

Topologically compressed representations (v9/v10 TriStripSet/PolygonSet
via Annex D) and the lossy quantized path are documented-but-deferred and
return typed errors, never fabricated geometry.

## Bitlength codec (Ref 13.2.2 + Annex B.2)

Prefix codes implemented and tested against the documentation's worked
examples: `0` keep width, `111110` = +4 step sizes, `1001` = -2 step
sizes (2-bit steps). Full symbol-stream decode requires the nibble-coded
bias (min/max/mean) which this wedge reports as a typed UnsupportedFeature
pending a reference vector; see crates/oep-step/src/jt/codec.rs.

## Future work (next wedges)

- LZMA / arithmetic / Deering codecs and the nibble bias used by the
  bitlength codec.
- Topological shape decode (Annex D) for real v9/v10 TriStripSet /
  PolygonSet files.
- Meta Data Segment PMI Manager (ADR gated second milestone).
- Real-writer-acquired corpus under the decision-7 dev-only posture (the
  current golden is synthesized to the documented byte layout; no third
  party JT file is redistributed).
