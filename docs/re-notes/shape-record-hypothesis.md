# "Shape Record" Hypothesis per Format

P1-M2-T05 · Isolith / OEP P1 Format Reconnaissance
Status: hypothesis note, byte-verified on real material 2026-10-15
Version: 1.0

## 1. What a "shape record" is (and why we need the hypothesis)

Every format in the Isolith target set is a *container* first: a wrapper
that carries identity, structure, metadata, and references. Inside that
wrapper, the one record that actually stores the geometric shape - the
vertices/triangles/topology a reader must extract - is what this note calls
the **shape record**. Locating it per format is the precondition for the
reader milestones (P2 CGR, P3 3DXML, P5 native).

Everything below is a HYPOTHESIS derived from real bytes observed on
2026-10-15 (the P1-M1 corpus lane owns provenance; samples are not
redistributed). Byte-level values that were read from real files are exact.
Any structural *meaning* is marked `[uncertain]`. This note feeds the
P1-M2-T06 save/modify/bisect diff harness, which is how a hypothesis becomes
a fact.

Observation tool: `python -m oep_tooling.shape_record_observe <file>`
(pure stdlib, read-only). Acceptance test:
`tests/test_shape_record_hypothesis_acceptance.py` (TDD: written RED before
this note, now GREEN on the real corpus).

## 2. CGR: the rep-graph payload is the shape record

Observed (all 11 real samples; 10 cfysa DMU doc-set + 1 piston 2007-era):

| Probe | Value | Samples |
|---|---|---|
| magic @0x00 | `01 00 00 00` (u32 LE == 1) | 11/11 |
| preamble 0x04..0x63 | 96 zero bytes | 11/11 |
| invariant template 0x68..0x77 | `00 07 00 00 00 07 00 00 00 b1 01 0b 01 08 02 4f` | 11/11 |
| size | multiple of 0x1000 | 11/11 |
| scalar encoding | little-endian | 11/11 |

Family A (piston.cgr, 2007-era): ASCII class strings at the header tail
`CAT3DBagRep` @0x7c and `CATSurfacicRepy` @0xb9 - the industrial fingerprint
of a CATIA CAA `CAT3DRep`-family object graph. Family B (cfysa doc set,
R18/R19-era): no header class strings; resident f32 @0x7c clusters ~1.158
(a scale / sag / bounding-extent token, `[uncertain]`).

**Hypothesis:** the CGR payload after the header is a **depth-first
serialization of the CATIA 3D visualization rep graph** - a
`CAT3DBagRep` ("bag of reps", the container of child representations) owning
`CATSurfacicRep` surface representations - written with implicit lengths, NOT
a forward table of blocks (a search of the first ~0x200 bytes of every
sample found no repeating `(offset,length)` chain) `[uncertain]`.

- Where the actual triangle stream begins inside the surfacic rep is
  UNKNOWN: `[uncertain]` (P2-M2-T01 extraction + P1-M2-T06 bisect resolve).
- CGR carries tessellation only, no exact B-Rep (industry consensus, P1-M3
  audits; consistent with the class-name set observed: a rep graph, not a
  solid model).
- The two observed families (class-string vs float tail) likely share the
  rep-graph shape record but differ in writer options/version; P2 needs
  per-family parse paths (P1-M2-T02 drift note).

## 3. CATPart: the "Shape 1" rep with its geometry bag is the shape record

Observed on the canonical wave-1 corpus (114 CATPart + 7 CATProduct,
mohanevs knuckle-joint pack, writer V5R29SP0HF0 - the same set the sibling
P1-M2-T03 notebook verified), and re-checked this session on the wider
scratch corpus (158 parts + 48 products, see the variance note in 3.3):

- Container magic `V5_CFV2\x00`; length-prefixed strings; root token
  `ASMPRODUCT` + name; `catalogLinks` registry.
- A CATPart self-labels `MechanicalPart` (mechanical body model) and carries
  a default representation, the PRDREP `"Shape 1"`, plus the geometry tokens
  `_BagRepsList` and `BagRep` (a bag of geometric representations belonging
  to the part). Verified 114/114 on the canonical set; `BagRep` and
  `_BagRepsList` held on all 158/158 parts observed across scratch corpora.
- A CATProduct self-labels `CATMechProdCont` (mechanical product container),
  carries instance/rep edges (`_InstanceName`, `CtorToProd`, `ProdToRep`,
  `Coincidence`, verified 7/7 on the canonical knuckle-joint products) and NO
  `MechanicalPart` body; it does NOT embed child file paths (no `.CATPart`
  substring in any product - logical references only).
- Nuance observed this session: products also contain `_BagRepsList` /
  `BagRep` (the assembly-geometry bag, `ASMGEOMETRY`), but they never carry
  the `MechanicalPart` mechanical body. So the distinction holds at the body
  level, not the `BagRep` token level.

**Hypothesis:** the CATPart shape record is the **default representation
"Shape 1" of the MechanicalPart**, i.e. the `_BagRepsList`/`BagRep` geometry
block(s) reached from the part's PRDREP edge; exact B-Rep and its tessellated
display live inside that bag. A CATProduct has NO shape record of its own -
its shape is the composition graph of references to the parts' "Shape 1"
reps (plus an optional assembly-geometry bag) `[uncertain]`.

- Whether the `BagRep` block inside a part holds exact B-Rep, tessellation,
  or both is `[uncertain]` - P5 scope, and exactly what the P1-M2-T06 diff
  harness should bisect (re-save the part, watch which bytes move).
- The token set observed is a rep-graph fingerprint (BagRep list) consistent
  with a CATIA visualization/geometry rep model, parallel to CGR's bag-of-
  reps - `[uncertain]` whether CATPart's bag is the same record family as
  CGR's `CAT3DBagRep` (likely related by shared CAA object model, NOT
  confirmed on bytes).

### 3.3 Documented writer variance (why nothing here is a universal law)

Re-checking this session on the wider scratch corpus (city-set and friends,
158 parts + 48 products, all `V5_CFV2` magic) found two writer-family
differences from the canonical knuckle-joint pack. These are recorded so the
reader code treats them as batch writer statistics, not specification:

- `ASMPRODUCT` root token: present on 134/134 canonical + probe + commit
  files, but on only 15/72 city-set files. The root-product token is a
  writer/authoring-family marker, NOT a format law. `[uncertain]` on what
  its absence means structurally (different root naming path in that
  authoring run).
- `CATMechProdCont`/`_InstanceName`/`CtorToProd`/`ProdToRep`: present on the
  7/7 canonical products and 9/9 commit/probe products, but a handful of
  city-set SUB-product/assembly-tool files (Sub_Product1, ManagingComponents*,
  AssemblyTools01, ...) lack `CATMechProdCont` or `_InstanceName`. Those are
  sub-assembly documents of a different authoring shape; the reference-graph
  edge set is therefore a property of the knuckle-joint-style product
  authoring path, not of every `V5_CFV2` product. `[uncertain]`.
- `BagRep` / `_BagRepsList` on parts: universal (158/158 across ALL scratch
  corpora, 0 absent). This is the strongest repeatable marker - every part
  observed carries the geometry bag.

## 4. 3DXML ASCII branch: the PolygonalRep is the shape record (lowest uncertainty)

Observed on the six license-clean ASCII 3DXML samples (trimesh MIT +
cad-in-unity Apache-2.0 committed to corpus; Dassault gallery sample is
reference-only, see section 5):

Two outer forms:
1. ZIP 3DXML: `Manifest.xml` + a `<name>.3dxml` model document
   (`<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML">`,
   `<Header><SchemaVersion>…</SchemaVersion>`,
   ProductStructure with `Reference3D`/`Instance3D`, and
   `<ReferenceRep … associatedFile="urn:3DXML:<name>.3DRep">` edges that
   point at one or more byte `.3DRep` members) + material/rendering reps.
2. Plain ASCII 3DXML: the whole document is one XML file (quad.3dxml
   observed, 14,317 B), same schema, referencing external `.3DRep` geometry
   files.

The shape record, in both forms, is the inner representation XML:

    <XMLRepresentation version="1.0" xmlns=.../3DXML>
      <Root xsi:type="BagRepType">
        <Rep xsi:type="PolygonalRepType">
          <SurfaceAttributes>…material…</SurfaceAttributes>
          <VertexBuffer>
            <Positions> x y z, x y z, … </Positions>
            <Normals> … </Normals>
            <TextureCoordinates dimension="1D" channel="0">…</TextureCoordinates>
          </VertexBuffer>
          <Faces><Face triangles=" 0 1 2  2 3 0  … "/></Faces>
          <Edges><Polyline vertices="…"/></Edges>
        </Rep>
      </Root>
    </XMLRepresentation>

Observed facts (re-derived from real bytes by the acceptance test):
- The geometry `.3DRep` members of the ASCII samples are themselves ASCII XML
  beginning `<?xml` with `<XMLRepresentation>` (verified on all geometry reps
  of blocks, cube1, cycloidal, machinist, FM1_satellite).
- Vertex positions/normals/uv are whitespace-separated decimal numbers inside
  `<Positions>`, `<Normals>`, `<TextureCoordinates>`; triangles are
  whitespace-separated vertex indices inside `<Face triangles="…">`; edges
  are polylines. All text-decodable, no binary stepping needed.
- Not every `.3DRep` member is a shape record: the material and rendering
  reps (`MaterialDef_N.3DRep`, `material_Rendering.3DRep`) are ALSO ASCII XML
  but of the separate `Osm` schema (`<Osm xmlns="http://www.3ds.com/xsd/osm.xsd">`,
  a Feature/Attribute rendering tree), and carry no VertexBuffer/Faces. So a
  reader must dispatch on `.3DRep` CONTENT (XMLRepresentation vs Osm), not on
  the file extension alone - `[uncertain]` if Osm reps can carry anything
  beyond rendering attributes in other samples, they are material-only here.

**Hypothesis:** in a 3DXML ASCII tessellated rep, the shape record is the
**`PolygonalRep` element**: a `VertexBuffer` (positions + optional
normals/uv) indexed by the `<Face>` triangle list. This is the LOWEST
uncertainty shape record in the whole target set - the geometry is literally
readable XML, and the schema (`xsi:type`, version 1.0/1.1/1.2, `format=
"TESSELLATED"`) is self-describing.

- Surface (Gregory-patch) reps exist in the 3DXML spec family but are NOT
  exercised by the committed ASCII corpus samples (all committed samples are
  PolygonalRep/tessellated). Whether a Gregory surface rep appears in the
  ASCII branch of real parts is `[uncertain]` and is P3-M2 scope.

## 5. 3DXML binary branch: a V5_CFV3 container, NOT XML

The Dassault gallery sample `press_rink_axe_fta.3dxml` (2009-05-26, CATIA V5
Imagine & Shape; reference-only license, NOT redistributable, kept off the
repo by clean-room policy) shows a SECOND branch that the ASCII samples do
not:

Observed on all 18 `.3DRep` members (verified 18/18):

| Probe | Value |
|---|---|
| first 8 bytes | `56 35 5f 43 46 56 33 00` = ASCII `V5_CFV3\x00` |
| resident string @~0x40 | `CATIA_V5 CB0001` |
| terminator near EOF | `CB__END\x00` |
| model XML format tag | `format="UVR"` (18/18 refs) |
| internal content | packed LE float streams (dense, few printable runs) |

**Key finding:** the binary `.3DRep` is NOT a binary re-encoding of the XML
schema - it is a **`V5_CFV3` container**, a DIFFERENT container-magic family
from CATPart/CATProduct's `V5_CFV2` (and from the plain-header CGR). The
name similarity (`V5_CFV2` vs `V5_CFV3`) suggests the same object-store
container technology family at a later version, `[uncertain]` but consistent
with Dassault's serialized-CATIA-object-store history.

**Hypothesis:** the binary 3DXML shape record lives inside a `V5_CFV3`
container whose payload is a packed little-endian float stream (positions,
normals, likely material/uv) followed by an index/triangle region, the whole
object terminated by `CB__END` `[uncertain]`. The XML model views these
through `format="UVR"` references (meaning of "UVR" `[uncertain]`; likely
"Universal Representation" `[uncertain]`).

- Full slot layout inside the `V5_CFV3` container is NOT known - tagged for
  P3-M3 (`binary .3drep` branch) and P1-M2-T06 diff bisect. The CGR note
  (P1-M2-T01) and this note both flag that string-scanning will not locate
  sections here; block walking must follow offsets.
- The binary branch is the reference/architecture-relevant observation for P3;
  it does not need to be in the first 3DXML parcel (YAGNI: no committed
  sample exercises it), but it MUST be named in the RE notes so the P3
  milestone tracks it instead of rediscovering it.

## 6. Cross-format summary (the hypothesis in one table)

| Format | Container | Shape record hypothesis | Confidence |
|---|---|---|---|
| CGR | plain header (magic `01 00 00 00`) | CAT3DRep rep-graph payload, depth-first, implicit lengths | medium |
| CATPart | `V5_CFV2` | MechanicalPart default rep "Shape 1": `_BagRepsList`/`BagRep` geometry block(s) | medium |
| CATProduct | `V5_CFV2` | none of its own; composition-graph edges to parts' "Shape 1" (+ optional ASMGEOMETRY bag) | medium |
| 3DXML ASCII | ZIP or plain XML | `PolygonalRep` element: VertexBuffer + `<Face>` triangle indices | high |
| 3DXML binary `.3DRep` | `V5_CFV3` | packed LE float stream + index region, `CB__END`-terminated | low |

Confidence scale: high = byte-decoded/self-describing on real material;
medium = observed tokens + strong structural fingerprint, meaning not yet
proven; low = container + terminators observed, internal layout not yet
decoded.

## 7. What would upgrade each confidence level

1. **CGR / CATPart (medium -> high):** run the P1-M2-T06 diff harness (save /
   load / re-export a part in a free tool, bisect which bytes move) and
   produce ≥2 re-decorated versions of the SAME part; watch the `BagRep`
   region for the change window. Also compare the CGR `CAT3DBagRep` token
   with the CATPart `BagRep` token on bytes: same or different record
   family.
2. **3DXML ASCII (high already):** confirm a Gregory-patch surface rep in the
   ASCII branch of a real part to extend coverage beyond tessellation; this
   is P3-M2's input, not a correctness gap here.
3. **3DXML binary (low -> higher):** obtain a second `V5_CFV3` sample and
   `CB__END`-bisect; record the header u32 slots (observed values at 0x08,
   0x0c and `CATIA_V5 CB0001` @~0x40) across samples to separate
   writer-version fields from content fields.

## 8. Invariants

- Clean-room only: all observations above are from the byte content of the
  named public / reference sample files on 2026-10-15. No Dassault library
  read any file, no vendor binary was executed.
- Committed corpus samples are license-clean and redistributable
  (MIT/Apache-2.0); the Dassault gallery binary-branch sample is
  reference-only and stays OUT of the repo (P1-M1-T01 policy). The
  acceptance test SKIPs the binary lane unless the reference sample is
  present locally (`OEP_3DXML_BIN`).
- Every structural meaning in this note is marked `[uncertain]`; byte values
  are not guesses and are re-derived by the acceptance test from real files.

## 9. Version history

- 1.0 2026-10-15 initial per-format shape-record hypothesis note (P1-M2-T05).
