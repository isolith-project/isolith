# P5-M1-T02 · Native CATPart entity-class observability: B-Rep vs tessellation vs parameter graph

Format: hex-level reverse-engineering observation report, grounded in observed bytes only
Task: P5-M1-T02 (Keelworks wave-2, P5 native lane) - feeds the P5-M1-T03 alpha-target decision
Date: 2026-10-16
Status: verified on real material (114-part native CATPart corpus; see Verification record)
Owner: Keelworks P5 native lane. Sibling notes: docs/re-notes/catpart-catproduct-container-notebook.md (P1-M2-T03), docs/re-notes/shape-record-hypothesis.md (P1-M2-T05).

---

## 1. Scope, method, clean-room

Objective: answer the P5 alpha-target question - which *entity classes* are observable in a
native CATPart (CATIA V5 .CATPart container) and with what confidence, so P5-M1-T03 can pick
a first extraction target. Entity classes assessed:

1. B-Rep - exact boundary representation (faces/edges/vertices, analytic + freeform surfaces)
2. Tessellation - precomputed faceted mesh for display
3. Parameter graph - feature tree, feature instances, attribute/parameter keys

Method: pure byte observation with Python stdlib (never a Dassault library, never a vendor
binary). Every claim below maps to observed bytes at concrete offsets on a named corpus file.
Where meaning cannot be proven from bytes alone it is marked [uncertain].

Clean-room invariant (board.program.json): evidence (offsets, hashes, token tables) is recorded;
the sample binaries stay on the observation machine / corpus object store, not in the repo.

## 2. Corpus used

- 114 native .CATPart files, all verified magic `V5_CFV2\0` (56 35 5f 43 46 56 32 00), all
  carrying the writer marker `V5R29SP0HF0` (CATIA V5 R29 SP0 HF0) - single writer generation.
- 131 native rows in the P1-M1-T04 manifest (123 catpart-v5 + 8 catproduct-v5) collapse to 121
  unique native FILES (114 CATPart + 7 CATProduct): 10 rows are duplicate listings of the same
  sha256 under both a flat name (e.g. hex-nut.CATPart) and an upstream path name
  (SM Application__hex nut.CATPart). All census counts in this report are per unique file.
- Source lineage: mohanevs/solid-modeling-catia-v6 (MIT) mechanical-part tutorial pack, the same
  set the P1-M2-T03 container notebook and P1-M2-T05 shape-record note verified; plus the 10
  repo-backend CATPart/CATProduct under corpus/data/catpart/.
- The 7 CATProduct files are excluded from the geometry census (products carry no geometry of
  their own; their content is the composition/reference graph - established in P1-M2-T03).
  Feature-model tokens (`MechanicalPart`) also appear in products at lower density; the parts are
  the geometry carriers.

## 3. Executive findings (the three entity classes)

| Entity class | Observable in native CATPart? | Confidence | Best evidence |
|---|---|---|---|
| Parameter graph (feature tree + instance names + attribute keys) | YES | high | readable length-prefixed ASCII records: feature instances `Pad/Pocket/Hole/Sketch/Shaft/...` with attribute dictionaries (`Hole_Diameter`, `Pocket_EndLimit_Type`, `Offset`, `Depth`, `_FtrList`, `MechanicalPart`), 114/114 |
| B-Rep exact geometry | YES, payload present and coordinate-consistent | medium-high | nested `V5_CFV2` geometry container inside 114/114 parts (CB__END-terminated, 2.8%..91.7% of file, correlates r=0.79 with file size), containing IEEE-754 coordinate streams that match real model dimensions; analytic-kind names live in the feature schema, not inside the blob |
| B-Rep record-level semantics (which bytes = which face/surface/topology) | PARTIAL | medium / [uncertain] | the geometry blob is binary-coded (no topology value-class strings: CATFace/CATVertex/CATVertex absent inside the blob); type discriminators appear to be numeric, so full decode needs record-level RE, not token reading |
| Freeform surfaces (NURBS/Gregory) | NOT observed in this corpus | low (corpus-consistent) / [uncertain] general | 0/114 files contain Nurbs/Gregory/Bezier strings; knot-vector scans found no reliable freeform parameter data; strong evidence the corpus is analytic-only (textbook parts) |
| Tessellation (embedded display mesh) | NOT observed in this corpus | high for these bytes / [uncertain] cross-writer | 0/114 files contain Mesh/Tessell/Facet/VertexBuffer/Polygon tokens; no triangle-index buffer found; CAT3DBagRep absent; the CATCGRCont container exists 114/114 but its observable octet payload is a property/path string table, and the file's only raster is a JPEG thumbnail (JFIF) |

Bottom line for P5-M1-T03: native CATPart's *readable, byte-grounded* surface is the
**parameter graph** (feature tree names/order + attribute keys) and a **binary exact-geometry
payload** (B-Rep, coordinates matching the model). It is NOT a tessellation-observable format in
this corpus, and its B-Rep is not name-tagged the way CGR/3DXML rep graphs are - it is a binary
serialization. An alpha reader should therefore target (a) feature/parameter-graph extraction
from the ASCII object-store records, or (b) B-Rep coordinate extraction via a byte-level walker
with numeric-record decoding as the T03 spike; visualization-grade triangles will come from
same-part B-Rep tessellation, not from embedded mesh bytes.

## 4. Evidence: parameter graph (confidence: high)

Observed on 114/114 parts:

- Feature-instance records are readable ASCII, length-prefixed per the P1-M2-T03 idiom
  (prefix byte == strlen+1). Concrete offset dump (part 8eb1ffe2..., a textbook prism part at
  0xd760): `PRTSketch \x07 Sketch \r SolveManager ...`, `\x06 pos_y \x06 pos_z \x07 Offset \x04 Pad
  \n ResultOUT \n MergeThin`, `\x07 Pocket \x06 Depth \t VPGlobal`. So a reader can walk `Sketch ->
  Pad -> Pocket -> ...` in file order and read feature names + follow-on attribute keys.
- 439 standalone feature-instance hits across the corpus (Pad, Pocket, Hole, Sketch, Shaft,
  Chamfer, PartBody, ...) as standalone length-prefixed tokens, not substring coincidences
  (verified byte-level: they sit on a length-prefix boundary, e.g. \x04Pad, \x07Pocket).
- Feature-type token census (parts, n=114): Sketch 114/114, Thickness 114/114, Shell 114/114,
  _FtrList 114/114, MechanicalPart 114/114, Feature 114/114, then content-driven: Pocket 77,
  Thread 68, Hole 66, Mirror 39, Rib 34, Shaft 25, Fillet 24, Chamfer 14, Groove 7, Draft 4,
  Revolution 3, Slot 3, Loft 1. The feature tree is serialized with ENUMERABLE type names.
- Attribute-name dictionaries: `Hole_Diameter`, `Hole_Depth`, `Hole_BottomType`, `Hole_ThreadType`,
  `Pocket_EndLimit_Type`, `GSMPlaneValues`, `GSMPlaneOffset`, `EdgeFillet_Radius`,
  `MechanicalPartProperties` (VOLUME/AREA/MASS/Density), `_FtrList`, `CurrentFeature`,
  `_BagRepsList`, `BagRep`, PRDREP `Shape 1` - a parameter schema is present as readable bytes.

What is NOT readable as a string: the numeric parameter VALUES. Only two unit strings
(`0deg`, `100mm`) were found corpus-wide; numeric values are stored as binary IEEE-754 or in a
Value-record indirection (`Value` keys observed, e.g. `Layer = 0`). So the graph's *shape* and
*key names* are high-confidence observable; exact numeric parameter recovery is a binary-decode
step [uncertain] but structurally anticipated.

## 5. Evidence: B-Rep exact geometry (confidence: medium-high payload, medium record-level)

Observed on 114/114 parts:

- Every part embeds a nested `V5_CFV2` container reachable at a second magic offset (e.g.
  fork-end part 317760 B: magics at 0x000000 and 0x40a4e; the blob runs 0x40a4e..0x499bd = 36 719 B
  = 11.6% of file, terminated by `CB__END`). Blob fraction ranges 2.8%..91.7% across the corpus,
  median ~12%; blob length correlates with file size r = 0.79 (114 parts) - i.e. the blob scales
  with part complexity and is the geometry carrier, not a constant header.
- Inside the blob are dense IEEE-754 little-endian coordinate streams whose values match the
  model's real dimensions. Jet engine example - fork-end part (64 mm bore spacing), hex dump at
  0x40df4/0x40e14/0x41268: f32/f64 triples like (−23.99, −32.0), (0.685, −14.0), (0.685, 32.0),
  ±(23.79..), ±(106.38..), ±(1569..) - i.e. radii/centres/limits in model units. A 22-cluster
  scan of the blob region found 22 coordinate clusters of ≥ 24 contiguous plausible floats.
- Container registration: `CGMGeom`, `CATSeeBodyCont`, `CATBRepModeContainer`, `CATMFBRP`,
  `CATSeeBodyCont` occur 114/114 (the container catalogue at ~0x1cdf and per-container headers),
  structural labels for a CATIA Geometric Modeler ("SeenBody") exact-geometry store.
- Analytic-kind vocabulary (Plane, Line, Circle, Cylinder, Sphere, Ellipse, Conic, Point) is
  present 114/114 but - decisive nuance - those tokens live in the FEATURE SCHEMA/identifier
  region (offsets < ~0x2xxxx and the defaults region ~0x3d000), NOT inside the nested geometry
  blob (0/8 checked tokens appear inside the blob). So the exact B-Rep records are binary-coded,
  not name-tagged: a reader cannot key a face/surface record off an ASCII type string the way it
  keys off `CAT3DBagRep` in CGR.
- The container header of the nested blob differs per part (u32@+0x40 = 0x71faffa3 on the
  fork-end), vs 0x01ACF45E on the outer container - it is a per-save-state/generation token,
  not a stable spec value ([uncertain] what it encodes).

Consequence: exact geometric coordinates are present and internally consistent, so a
boundary-representation surface/curve extraction is feasible by walking the blob's record
structure - but the field layout (surface discriminators, topology arrays) is NOT readable from
strings and will require record-level binary RE (the P5-M1-T03 spike scope). Until that layout
is derived, per-surface semantics (this record == cylindrical face F17) stay [uncertain].

## 6. Evidence: tessellation (confidence: not observable in this corpus)

Observed across 114/114 parts:

- Zero hits for mesh vocabulary: Mesh, Tessell/Tessellation, Facet, VertexBuffer, Polygon,
  CAT3DFacetRep, CATMesh. The single "Triangle" hit corpus-wide is inside a drafting-symbol
  table (ISO_3D.DressUp.Symbols.Triangle), not geometry.
- No triangle index buffer: a scan for runs of ≥ 9 small u32 (0<v<4096) inside the geometry
  region found values that are field/length idioms (distinct small values: 2,1,17,130,66,9,512,
  …), not a shared-vertex index sequence; corpus-wide tight-index runs carry no index-reuse
  signature of a real mesh.
- `CAT3DBagRep` (the CGR rep-graph class string) is absent from every CATPart (checked 16-part
  spread). The CGR magic idiom `01 00 00 00 00 00 00 00` appears many times per file but is a
  common LE 1-value field, not a CGR block.
- `CATCGRCont` is registered 114/114, but the observable payload at those offsets is a
  property/path string table (e.g. `CATOctetArray` blob at 0x49c10 containing an assembly path
  `\Users\...\Desktop\assembly\...`), and the only pure raster in the file is a JFIF JPEG
  thumbnail (~0x1d37). So the CGR container is best explained as a registered-but-not-populated
  visualization slot on these saves.
- Layout scan of the whole file shows the dense-float geometry confined to the nested blob
  region, not a separate mesh stream.

Interpretation: in this writer/version corpus, a native CATPart does not embed a display
tessellation for direct extraction - the file is exact-geometry + feature model + thumbnail.
Confidence that THIS corpus carries no observable mesh: high. Whether other CATIA writers/save
options embed a tessellated rep (e.g. "save with CGR"), or whether any of the 122 object-store
blobs deviate, is [uncertain] - the corpus is single-writer (V5R29SP0HF0) and single-pack
(mohanevs tutorials), so cross-writer variation is untested. Pending corpus growth (P5-M2
reference-heavy / other writers) this finding is corpus-scoped, not a format law.

## 7. Confidence summary and [uncertain] register

| # | Claim | Confidence | Grounding |
|---|---|---|---|
| C1 | Feature tree is present, enumerable, ASCII (instances + attribute keys) | high | length-prefixed records + 114/114 token census, offsets reproduced |
| C2 | Numeric parameter values are binary, not ASCII | high (corpus) / [uncertain] general | only 2 unit strings found; Value-indirection keys observed |
| C3 | A nested V5_CFV2 geometry blob exists in every part | high | 114/114 second-magic + CB__END, blob-size↔file-size r=0.79 |
| C4 | The blob holds IEEE-754 coordinates matching model dimensions | high | value match to real part dimensions at reproduced offsets |
| C5 | B-Rep record semantics (face/surface/topology mapping) derivable from strings | low | no topology class strings inside the blob; numeric discriminators |
| C6 | Freeform (NURBS/Gregory) surfaces present | low in corpus / [uncertain] general | 0/114 strings, no knot vectors; analytic-only corpus |
| C7 | Embedded tessellation extractable | low in corpus / [uncertain] cross-writer | 0/114 mesh tokens, no index buffer, CGR slot unpopulated |
| C8 | CATCGRCont holds a mesh | low (empty/property table observed here) [uncertain] | octet payload = path table; JPEG thumbnail only |
| C9 | Feature values are recoverable | medium / [uncertain] until value-record decode | Value-key indirection observed, layout not derived |

Meaning of confidence labels (same scale as P1-M2-T05): high = byte-decoded/self-describing on
real material; medium = strong structural fingerprint, meaning not fully proven; low = container
or coordinates observed, record layout not yet decoded.

Uncertainty flags carried into P5-M1-T03 by design: blob field layout (C5), feature-value
encoding (C9), presence of tessellation/NURBS in other writers (C6/C7), and the semantics of
the nested-container header token (u32@+0x40 = 0x71faffa3 vs outer 0x01ACF45E).

## 8. Alpha-target recommendation input (decision support only)

This report was consumed by P5-M1-T03, which ratified the alpha-target ADR
(workstreams/P5-M1/alpha-target-brep-first-adr.md, version 1.0.0, 2026-10-16): the native
reader alpha targets **B-Rep and exact geometry first** from the "Shape 1"
bag, with feature-tree semantics deferred to the P5-M4 research lane. The byte
evidence in this report supports that ratification on every relevant axis and
adds the observable basis that was previously only hypothesized:

- C3/C4 show the nested V5_CFV2 geometry blob with model-consistent IEEE-754
  coordinates is present in 114/114 parts - the exact-geometry payload the ADR
  makes the alpha target exists at the byte level (upgrades the P1-M2-T05
  medium confidence for the geometry carrier).
- C7/C8 show tessellation is not observable in this corpus - consistent with
  the ADR treating mesh as a derived byproduct (P2 parity), not the target.
- C1 shows the parameter graph IS observably present as readable records, so
  deferring it to P5-M4 is a resource/sequencing choice, not an admission it
  is absent; the P5-M4 memo has real bytes to assess.

The two defensible alpha candidates, in order of byte-grounded safety, are
recorded here for the decision's change-log completeness:

1. Parameter-graph reader first (feature instances to names to attribute keys
   to data offsets): entirely ASCII-driven, testable against the corpus
   immediately, zero [uncertain] record layout underneath it. Downstream
   value: PDM/metadata, feature history, and a locator for where each
   feature's B-Rep payload lives.
2. B-Rep coordinate walker as the follow-on spike: requires record-level
   decode of the nested blob (C5), is the higher-reward but higher-risk
   target, and is the natural P5-M3 spike to gate on. Tessellation is not a
   viable alpha target on these bytes (C7).

P5-M1-T03 owns the decision; this section records the evidence input for
auditability. It does not re-state or override the ratified ADR.

## 9. Reproducibility and verification record

Observation commands (Python stdlib, re-run on corpus; all in this task's scripts):
1. identity: first 8 bytes == b"V5_CFV2\x00" (114/114 parts, 7/7 products).
2. token census: re.finditer(b"[\x20-\x7e]{4,}") substring count per file for the feature /
   analytic / mesh / CGR vocabularies ([/tmp/probe_one.py /tmp/census.py /tmp/corpus_census.py]).
3. nested-blob locate: all b"V5_CFV2\x00" offsets and b"CB__END" offsets; blob = 2nd magic ..
   first CB__END after it (114/114).
4. coordinate extraction: struct.unpack_from("<f"/"<d") over the blob; value-match vs model dims.
5. length-prefix verification: prefix byte before a standalone token == strlen+1 (e.g. \x04Pad).
6. index-buffer scan: runs of small u32 in geometry region; no shared-vertex mesh signature.
7. raster: file contains exactly one JFIF JPEG thumbnail (~0x1d37 region).

Acceptance checks (all green on real material):
- feature-model tokens present 114/114 parts
- nested V5_CFV2 blob present 114/114, CB__END present 114/114
- blob-size vs file-size Pearson r = 0.79 (114)
- mesh vocabulary 0/114; CAT3DBagRep 0/16-spread; no triangle index runs
- NURBS/Gregory strings 0/114
- analytic-kind tokens 114/114, all located outside the nested blob (0/8 inside)
- coordinate streams in blob match real model geometry on fork-end (64 mm jaw) part

## 10. Version history

- 1.0 2026-10-16 initial P5-M1-T02 observability report (native CATPart, single-writer corpus).
