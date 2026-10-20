# 3DXML ASCII vs Binary (.3DRep): Hex-Level Observation Notebook

P1-M2-T04 · Isolith / OEP P1 Format Reconnaissance
Status: observation draft, verified on real material 2026-10-15
Version: 1.0

## 1. Purpose and method

This notebook records what a `3DXML` file looks like at the byte level,
derived ONLY from direct observation of real files. Where the public 3DXML
spec allows two storage branches, the files themselves pick one or the other;
the goal here is to map both branches to bytes so the P3 reader can detect
and enter the right one. No vendor documentation of the container layout is
used beyond what the bytes agree on (the licensed XSD from P1-M1-T02 remains
internal-use only and is not quoted here). Every number in this file was read
back from a real `.3dxml`/`.3DXML`/`.3DRep`. Byte values are not guesses;
claims about what a field *means* are marked `[uncertain]`.

The headline of the whole exercise, in one line:

> Inside the `ZIP` container, a part's geometry lives in a `.3DRep` member
> that is EITHER ASCII `XMLRepresentation` markup (format `TESSELLATED`,
> the open-tooling-friendly branch) OR a native-V5 binary container that
> starts with the magic `V5_CFV3` (format `UVR`). The two branches share
> the same `.3DRep` filename convention and are told apart by the first
> 8 bytes of the member.

Observation tool: `python -m oep_tooling.3dxml_observe <file.3dxml>` and the
acceptance test `tests/test_3dxml_hex_notebook_acceptance.py`, which
re-derives every fact in sections 4 and 5 from real files and fails loudly
on disagreement.

## 2. Real material observed

### 2.1 ASCII branch (committed corpus; P1-M1-T02/PR #26)

All six are in `corpus/data/3dxml/` (license-clean, MIT/Apache-2.0; sha256
verified 2026-10-15):

| File | Size (B) | sha256 | Container | .3DRep branch |
|---|---|---|---|---|
| blocks.3dxml | 2583 | 14ec76309f176fd5f70428d9f8b766640446b61cab8530c8708069dad4c1ead9 | ZIP | ASCII XML v1.0 |
| cube1.3dxml | 4834 | 8a4d4daef69baf11f674f41d9e022724f0b5a510bf36f26847479e6df6f04e6b | ZIP | ASCII XML v1.0 |
| quad.3dxml | 14317 | 24459eb1e786451d614c21f6a5cbca3b88eb226479e931b51673481613319d79 | standalone XML (no ZIP) | external refs (not embedded) |
| machinist.3DXML | 38849 | 2203d90f73d47cbb088a9e3b5f4e61780c0f65e575f0f27182c10be3f57c6753 | ZIP | ASCII XML v1.2 |
| cycloidal.3DXML | 742428 | 007751ab8639097ca146431b0d0c605a2523a269a8dbdc81facd966ddbc0a2de | ZIP | ASCII XML v1.2 |
| FM1_satellite.3DXML | 1763813 | 7ff05256666d63199ac16aa041b2869acde06df3ee03d839b5c0436eee2663ee | ZIP | ASCII XML v1.2 |

Abaqus/CAE-authored samples carry reps that are `XMLRepresentation version="1.0"`
and model docs at schema 4.0 with `\n` line endings; CATIA V5-authored samples
carry schema-4.2 docs (with one 4.3 exception, the gallery sample) and
`version="1.2"` reps with `\r\n` line endings. Line endings and writer
`<Generator>` are fingerprints, not format requirements `[uncertain]` (only
two writers observed).

### 2.2 Binary branch (reference-only material, NOT committed)

Observed on the Dassault-hosted public gallery sample, fetched 2026-10-15
(reference-only per the P1-M1-T02 license verdict: downloadable but no stated
redistribution license, so it is catalogued, not checked into git):

| File | Size (B) | sha256 |
|---|---|---|
| press_rink_axe_fta.3dxml | 609574 | 28778061da869c2a1e70a21e527a86c4de7dbed9a18028ea8a100de979ea26b4 |

Provenance (from its own `Manifest.xml` + `Transmission.3dxml`): authored in
CATIA V5 (Imagine & Shape lineage), created 2009-05-26, schema 4.3, with
`Preview2D.png` (PNG magic `89 50 4E 47`) and a 13-byte `.IntegrityCertificat`
(`c8 f0 1b 4a 00 00 00 00 23 20 1c 00 00`). The archive contains 22 entries:
Manifest.xml, the model doc `Transmission.3dxml`, `Preview2D.png`,
`.IntegrityCertificat`, and 18 `.3DRep` parts. Every one of the 18 `.3DRep`
parts is a binary `V5_CFV3` container (section 5). The 18 parts:

| .3DRep member | Size (B) | u32BE@0x08 | u32BE@0x0C |
|---|---|---|---|
| 32x18x220.3DRep | 3146 | 2400 | 746 |
| R125.3DRep | 7014 | 6268 | 746 |
| R170.3DRep | 7121 | 6375 | 746 |
| Connecting_Rod_Bush1_P808_.3DRep | 4553 | 3807 | 746 |
| Connecting_Rod_Bush_P808_.3DRep | 8475 | 7729 | 746 |
| Connecting_Rod_P808_.3DRep | 26740 | 25994 | 746 |
| Connecting_Rod1_P808_.3DRep | 14271 | 13525 | 746 |
| 9011513.3DRep | 19045 | 18299 | 746 |
| Chain_Sprocket_P406_.3DRep | 29589 | 28843 | 746 |
| I2005_120.3DRep | 47266 | 46500 | 766 |
| Worm_Holder_P405R_.3DRep | 40995 | 40229 | 766 |
| 6918.3DRep | 93968 | 93202 | 766 |
| 6316.3DRep | 108376 | 107610 | 766 |
| P404_worm_wheel_.3DRep | 195979 | 195213 | 766 |
| P402_crank_shaft_holder_.3DRep | 51695 | 50777 | 918 |
| P402_1_crank_shaft_holder_.3DRep | 41205 | 40287 | 918 |
| P401_crank_shaft____All_FTA.3DRep | 84496 | 83578 | 918 |
| PressRinkAxe_3DAnnotations.3DRep | 72578 | 71868 | 710 |

For every row above `u32BE@0x08 + u32BE@0x0C == Size` EXACTLY. The 0x0C
values cluster at {710, 746, 766, 918} `[uncertain: correlates with the rep's
contained sections/annotation payload, not the part name]`. Annotations
(`PressRinkAxe_3DAnnotations.3DRep`) and the FTA parts (crank shaft family)
use the larger tails.

## 3. Top-level branches (what the file extension does NOT tell you)

A `.3dxml`/`.3DXML` file is one of two things:

1. **ZIP-archive 3DXML** (dominant: 5 of 6 corpus files, plus the gallery
   sample). First four bytes are `50 4B 03 04` (string `PK\x03\x04`, the ZIP
   local-file header). Everything else lives inside the ZIP.
2. **Standalone ASCII model document** (observed: quad.3dxml). Plain XML, first
   bytes `3C 3F 78 6D 6C` (`<?xm`), single file, no ZIP wrapper.

Full ZIP-archive member inventory (observed across all samples):

| Member | Role | Observed size (B) |
|---|---|---|
| `Manifest.xml` | container index; `<Root>` names the primary model doc | 194-265 |
| `<Name>.3dxml` / `NameSetTree.3dxml` | primary model document (ProductStructure + rep wiring) | 1279-42607 |
| `*_N.3DRep` / `Abaqus_Geometry.3DRep` | part geometry: ASCII or binary (this notebook) | 1048-2820544 |
| `MaterialDef_N.3DRep` / `material_Rendering.3DRep` | OSM material rep (ASCII) | 1048-1881 |
| `CATMaterialRef.3dxml` | material references (Model_3dxml root, no xml decl) | 1161-19179 |
| `NameSetTree.3dxml` | ACTUAL primary model doc in the CATIA V5 writer family (see 4.1) | 1273-42607 |
| `CATRepImage.3dxml` / `image.bmp` / `Preview2D.png` | preview imagery | 598-7862 |
| `.IntegrityCertificat` | checksum token (gallery sample only) | 13 |

The primary model doc is named by `Manifest.xml` `<Root>`. Two writer
families are observed, and they name it differently:

- Abaqus/CAE exporter (blocks.3dxml, cube1.3dxml): primary doc is
  `Abaqus_Model.3dxml`; `Abaqus_Geometry.3DRep` holds all tessellation.
- CATIA V5 exporter (machinist/cycloidal/FM1): primary doc is named
  `NameSetTree.3dxml` (a `<Model_3dxml>` document carrying ProductStructure
  and the `urn:3DXML:` wiring despite its "naming-tree" filename), and
  geometry lives in `TessPart_N.3DRep` parts. `CATMaterialRef.3dxml` is a
  second `<Model_3dxml>`-rooted document that astonishingly has NO `<?xml`
  declaration (observed in the CATIA family); it only carries material
  references.

So a reader must not assume the primary model doc's member name; it must
follow `Manifest.xml` `<Root>`.

`Manifest.xml` is always ASCII XML: `<?xml version="1.0" encoding="utf-8" ?>`
then `<Manifest ... xsi:noNamespaceSchemaLocation="Manifest.xsd">` with
`<Root>...<model doc>.3dxml</Root>`. The 2009 CATIA-authored sample adds
`<Date>` and `<WithAuthoringData>false</WithAuthoringData>`; the newer corpus
manifests carry only `<Root>`. `[uncertain: <WithAuthoringData> is optional
per writer]`.

## 4. ASCII branch: XMLRepresentation markup

### 4.1 Containers (ZIP archive)

- ZIP magic `50 4B 03 04`; members are STORED/deflated per the local header.
- Primary model document (name per writer family, see section 3): the Abaqus
  exporter produces `<Name>.3dxml` (e.g. `Abaqus_Model.3dxml`); the CATIA V5
  exporter produces `NameSetTree.3dxml`. Both are `<Model_3dxml ...>` documents:
  - root `<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML" ...>`
  - `<Header><SchemaVersion>4.0</SchemaVersion>..4.3</SchemaVersion>` with
    observed values {4.0 Abaqus, 4.2 CATIA V5 corpus, 4.3 quad + gallery}
  - `<Title>`, `<Author>`, `<Generator>`, `<Created>` (writer provenance;
    e.g. `Generator` = `CATIA V5` or `Abaqus/CAE 6.23`)
  - `<ProductStructure root="1">`: `<Reference3D>` (the part), `<Instance3D>`
    with `<IsAggregatedBy>`, `<IsInstanceOf>`, and `<RelativeMatrix>` of 12
    decimals (row-major 4x3 transform)
  - Rep wiring: `<ReferenceRep ... format="TESSELLATED" version="1.1|1.2"
    associatedFile="urn:3DXML:<name>.3DRep"/>` plus a matching `<InstanceRep>`.
    The `associatedFile` URN is how the model doc names its geometry member;
    there is no in-ZIP path table other than this.
- Geometry parts `*.3DRep` are ASCII XML markup (section 4.2).
- Material parts are ASCII OSM markup (section 4.3).
- `CATMaterialRef.3dxml` is also `<Model_3dxml>`-rooted but has no `<?xml`
  declaration and carries only `CATMaterialRef`/material-relationship markup.

### 4.2 Tessellated ASCII .3DRep (the open-tooling branch)

Head of a CATIA V5-authored rep (cycloidal.3DXML member `TessPart_5.3DRep`,
2,820,544 B):

```
3C 3F 78 6D 6C 20 76 65 72 73 69 6F 6E 3D 22 31 2E 30 22 20 65 6E 63 6F 64 69 6E 67 3D 22 55 54 46 2D 38 22 3F 3E 0D 0A
<?xml version="1.0" encoding="UTF-8"?>\r\n
<XMLRepresentation version="1.2" xmlns="http://www.3ds.com/xsd/3DXML" ...>
  <Root xsi:type="BagRepType">
    <Rep xsi:type="BagRepType"><Rep xsi:type="PolygonalRepType">
      <Edges><Polyline vertices="x y z,x y z,..."/>
      <Faces>
        <Face strips="i i i...,j j j...,...">
          <SurfaceAttributes xsi:type="SurfaceAttributesType">
            <Color xsi:type="RGBAColorType" red=".." green=".." blue=".." alpha="1"/>
          </SurfaceAttributes>
        </Face>
        ...
      </Faces>
      <VertexBuffer>
        <Positions>x y z,x y z,...</Positions>
        <Normals>...</Normals>
      </VertexBuffer>
      ...
```

Observed encoding facts (all exact):

- Every tessellated ASCII rep opens with an XML declaration whose
  `encoding="UTF-8"` (case differs: CATIA V5 writes `"UTF-8"`, the Abaqus
  exporter writes `"utf-8"`).
- Root element is `<XMLRepresentation version="1.0">` (Abaqus 6.23 exporter)
  or `version="1.2"` (CATIA V5). Version is a string attribute, no binary
  length.
- Rep wrapper: `<Root xsi:type="BagRepType">` containing `<Rep>` bags whose
  leaves are `xsi:type="PolygonalRepType"` for tessellation and
  `xsi:type="TessellatedRepType"` variants were NOT observed in the corpus;
  only PolygonalRep was.
- `VertexBuffer` holds `<Positions>` then `<Normals>`. Positions are a single
  text chunk: comma-separated triples, each triple "x y z" space-separated
  doubles. No count header before the list (the reader must count).
- Faces are triangle strips: `<Face strips="...">` where the attribute is
  comma-separated runs and each run is space-separated vertex indices
  (observed runs of 25/27/29 indices in cycloidal).
- Silhouette/contour edges are `<Edges><Polyline vertices="..."/>` with the
  same comma/space encoding as Positions.
- Colors attach per-face via `<SurfaceAttributes><Color .../>` with RGBA
  doubles.
- The same chunk formats are used by the v1.0 Abaqus reps.
- Reps are stored in ZIP as binary-pre-deflate bytes, so on disk the file is
  the XML text compressed; delivery of the `.3DRep` member must decompress
  first (standard `zipfile`).

### 4.3 OSM material reps and aux XML (ASCII branch)

- `MaterialDef_N.3DRep` / `material_Rendering.3DRep` root:
  `<Osm xmlns="http://www.3ds.com/xsd/osm.xsd" xmlns:xsi=...>` with a
  `<Feature Alias= Id= StartUp=>` tree (observed `RenderingRootFeature`,
  `RenderingFeature` with `AmbientCoef`, `DiffuseCoef`, ... `Attr
  Name= Type= Value=`) and `<Feature Alias="MainDataSelection" ...>`. This is
  ASCII/Osm (OSM = "Object Specification Model" in the 3DXML family) markup;
  the same `<Osm>` root is also observed in `CATMaterialRef.3dxml` container
  variants.
- In the corpus `NameSetTree.3dxml` is the CATIA V5 family's *primary
  model document* (a `<Model_3dxml>` doc with ProductStructure and the
  `urn:3DXML:` wiring, section 3/4.1), not a separate naming tree, despite
  its filename. No standalone "naming/formula tree only" variant was observed
  separately.

### 4.4 Standalone ASCII model document (quad.3dxml)

- Not a ZIP; first bytes `3C 3F 78 6D 6C` (`<?xm`); `\n` line endings.
- Schema 4.3; `<Generator>CATIA V5</Generator>`, `<Author>PVO</Author>`,
  `<Created>2009-05-20</Created>`.
- ProductStructure only: 7 `Reference3D`, 8 `Instance3D`, matrix transforms,
  and 4 `<ReferenceRep format="TESSELLATED" version="1.2"
  associatedFile="urn:3DXML:Chassis.3DRep">` (also `Wheel.3DRep`,
  `Axle.3DRep`, and tellingly `NonAscii_208.3DRep`).
- The model doc contains NO inline geometry (`<VertexBuffer`/`<Faces>`
  absent): geometry lives in the externally-referenced `.3DRep` files, which
  are NOT shipped inside this document. That is the link between the
  standalone branch and the rep-level brand/catalog: the standalone document
  is only the product-structure slice.
- Consequence for the P3 reader: a standalone `.3dxml` may reference `.3DRep`
  files that are either branch; discovery must follow the URN to a real file,
  and classification must then be per-member (section 5/6), not per-document.

## 5. Binary branch: the V5_CFV3 .3DRep

### 5.1 Container and model-document side

The gallery sample is a ZIP archive (magic `50 4B 03 04`) identical in outer
shape to the ASCII ZIP branch. The difference is inside the `.3DRep` members
and in how the model doc declares them:

- Model doc `Transmission.3dxml` (schema 4.3): every part is declared with
  `format="UVR"` `version="1.0"` (18/18 refs), not `format="TESSELLATED"`.
  UVR = "Unified Visualization Representation", the native-V5 visualization
  rep family. This is the observed model-doc-side signature of the binary
  branch.
- Every referenced `.3DRep` member is a binary container (section 5.2); none
  is XML.

### 5.2 The .3DRep binary container (observed on 18 real parts)

First 0x60 bytes of `R125.3DRep` (7,014 B), annotated:

```
0000  56 35 5F 43 46 56 33 00 00 00 18 7C 00 00 02 EA   V5_CFV3....|....
      ^^^^^^^^^^^^^^^^ ^^ ^^^^^^^^^ ^^^^^^^^^^
      magic "V5_CFV3\0"  | u32BE@0x08 | u32BE@0x0C
                         +-- 5 zero bytes
0010  FF FF FF FF FF FF FF FF 00 00 00 00 00 00 00 00   ................
      ^ FF x8 (all parts)  ^ zero padding
0030  ...00 00 00 02 5F 0D 0A 00                        ......_...
                                   ^ ^^ ^^ ^^  '_' CR LF NUL
0040  43 41 54 49 41 5F 56 35 20 43 42 30 30 30 31 00   CATIA_V5 CB0001.
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ version token + NUL      ^
0050  4E 03 E8 00 40 00 0C 00 0A 00 0B 00 00 15 B6 00   N...@...........
      ^N^ u16BE 1000      ^... writer fields [uncertain]
0060  ...
```

Observed invariants across ALL 18 real parts (exact):

| Field | Offset | Value (all 18) |
|---|---|---|
| magic | 0x00..0x07 | `56 35 5F 43 46 56 33 00` = "V5_CFV3" + NUL |
| length A | 0x08..0x0B | u32 big-endian == main-region length |
| length B | 0x0C..0x0F | u32 big-endian == tail length |
| length invariant | -- | `u32BE@0x08 + u32BE@0x0C == file size` (exact, 18/18) |
| marker | 0x10..0x17 | `FF FF FF FF FF FF FF FF` (8 bytes) |
| prelude CRLF | 0x3C..0x3F | `5F 0D 0A 00` = `_` `\r` `\n` `\0` |
| version token | 0x40..0x4F | `43 41 54 49 41 5F 56 35 20 43 42 30 30 30 31 00` = "CATIA_V5 CB0001\0" |
| token follower | 0x50 | `4E` ('N') |
| count | 0x51..0x52 | u16BE `03E8` = 1000 |
| tail terminator | file[-8:] | `43 42 5F 5F 45 4E 44 00` = "CB__END" + NUL (18/18) |

Additional observations (all 18):

- Byte order is big-endian for the two length fields (LE reads give
  unreasonable values; BE sums check out exactly). This contrasts with the
  CGR header, which is little-endian throughout (P1-M2-T01) `[observation]`.
- The main region (0x10 up to `u32BE@0x08`) starts with the `FF x8` marker
  then zero padding up to the `_\r\n\0` prelude. The prelude + version token
  sit at the SAME offsets (0x3C/0x40) in every file (constant header size).
- Everything from `u32BE@0x08` (the tail) is a recursive section structure:
  shallow counters (`00 02 00 00 00 02 00 00 ...`), 8-byte GUID-like
  sequences `38 97 32 20 01 C9 DE 07` (invariant across parts, so likely a
  writer/container UUID `[uncertain]`), and UTF-16LE property names
  (`00 53 00 6B 00 65 00 6C 00 65 00 74 00 6F 00 6E` = "Skeleton";
  "SurfacicReps"; "Applicative Containers"; "MainDataStream"; `56 00 34 00
  56 00 35 00 5F 00 46 00 44 00 54` = "V4V5_FDT"), each prefixed by a
  u16/u32 length. A 16-char hex string in UTF-16LE is embedded near 0xC0
  (`38 38 66 32 31 63 31 39 32 38 31 39 30 30 30 30 63 37 66 30 31 62 34 61
  64 32 35 31` -> "88f21c1928190000c7f01b4ad251") `[uncertain: instance GUID]`.
- zlib/deflate: no part STARTS with a zlib stream (`78 9C`, `78 01`,
  `78 DA` never at offset 0). Incidental `78 9C` bytes DO occur deep inside
  larger parts (observed in the 72,578 B annotations part), so "starts with
  zlib" is the invariant, not "never present". The only compression layer is
  the ZIP above. No `PK\x03\x04` appears inside any part (no nested
  archive).
- `N \x03\xe8` at 0x50-0x52 and the `00 40 00 0C 00 0A 00 0B 00` block at
  0x53+ are consistent byte-for-byte within the part family but their meaning
  is unknown; `[uncertain]` (a scale/version/axis-set triplet is plausible:
  1000 appears as the classic CATIA "per-meter" unit factor).

### 5.3 The FTA (annotations) part is the same container

`PressRinkAxe_3DAnnotations.3DRep` (72,578 B) carries the 3D annotation
(FTA) representation of the archive and is STILL a `V5_CFV3` binary
container with identical header invariants (magic, length sum, FF x8,
version token, `CB__END`). Only its 0x0C tail length differs (710). So the
binary branch is not limited to tessellation: annotation/showability reps
share the container.

## 6. Detecting the branches from bytes (the P3 entry point)

Order a decoder must apply, per derived observation:

1. Read first 4 bytes of the outer file:
   - `50 4B 03 04` -> ZIP-archive 3DXML -> open ZIP, read `Manifest.xml` to
     find `<Root>` model doc.
   - `3C 3F 78 6D 6C` -> standalone ASCII model document -> parse XML.
   - anything else [error/unknown].
2. For each `.3DRep` member (or standalone referenced `.3DRep`), read its
   first 8 bytes after ZIP decompression:
   - lstrip begins `<?xml` -> ASCII branch -> parse `XMLRepresentation`
     markup (section 4.2); model doc declares it `format="TESSELLATED"`.
   - `56 35 5F 43 46 56 33 00` -> binary branch -> native-V5 `V5_CFV3`
     container (section 5.2); model doc declares it `format="UVR"`.
   - lstrip begins `<Osm` -> ASCII material rep (section 4.3).
3. The `format` attribute in the model doc is a reliable cross-check: it was
   `TESSELLATED` 6/6 on ASCII corpus archives and `UVR` 18/18 on the binary
   archive. `[uncertain: the attribute is descriptive, not a format
   requirement; a writer could in theory name either value]`.
4. The 4th signature to distinguish: binary parts end with `CB__END` +
   a NUL byte (last 8 bytes `43 42 5F 5F 45 4E 44 00`); ASCII parts end
   with `>` as the last XML element. Cheap and definitive.

## 7. Cross-format observations (feeds the P1 report)

- `V5_CFV3` (observed here) and `V5_CFV2` (CATPart/CATProduct corpus,
  P1-M1-T03 says magic `V5_CFV2` per file) are siblings in the `V5_CFV*`
  magic family. The CGR magic (P1-M2-T01) is different (`01 00 00 00` LE).
  So the OEP format family already shows three distinct container spines:
  CGR (LE, no string magic), V5_CFV2 native parts, and V5_CFV3 3DRep.
- Byte order differs: CGR header = little-endian; V5_CFV3 lengths =
  big-endian. A P2/P3 reader must not assume a single endianness globally.
- The `.3DRep` extension is ambiguous at the byte level (ASCII vs binary);
  any tool keying off the file extension alone will misclassify 50/50 of the
  binary material. Detection must be content-based (section 6).

## 8. Open questions / next probes

1. What exactly are `u32BE@0x08` and `u32BE@0x0C` beyond "sizes that sum to
   the file"? Is the main region bounded by `0x10..u32BE@0x08` and is the
   tail the section table? The UTF-16LE name table strongly suggests the tail
   is a small "rep header/dictionary" and `0x08` is the geometry-stream
   length. `[uncertain]`.
2. Does a non-V5 (e.g. modern 3DEXPERIENCE) binary `.3DRep` keep the same
   `V5_CFV3` header, or does it drift? Only 2009-era material observed. The
   P3-M3-T01 binary-branch milestone should re-run this probe on newer
   binary archives.
3. The `03 E8` unit factor (u16BE at 0x51, == 1000) and the
   `00 40 00 0C 00 0A 00 0B` block at 0x53+ need 2+ redecorated reps of the
   SAME part to separate writer fields from content fields (same need as
   P1-M2-T06 diff harness).
4. Do legacy 3DXML files (schema < 4.0) use a different `Manifest` or no
   `urn:3DXML:` scheme? Corpus has no pre-4.0 samples.
5. What geometry abstraction does the `V5_CFV3` main region hold? The
   observation tool stops at the header; extracting the tessellation is
   P2/P3 reader work, not notebook work.

## 9. Invariant

Clean-room only: all observations come from the byte content of the public
sample files (committed corpus + the Dassault-hosted gallery sample, which
is reference-only and NOT redistributed). No Dassault library read any file
for us and no vendor binary was executed. This notebook is evidence for the
P1 report and the P3 reader roadmap (P3-M1 ASCII reader, P3-M3 binary
branch), not a redistribution vehicle (that is the P1-M1 license lane).
