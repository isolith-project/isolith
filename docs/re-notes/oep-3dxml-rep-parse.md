# oep-3dxml XML Rep Parse: Nodes, References, Instances

P3-M1-T02 · Keelworks / OEP P3 3DXML Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

The second working slice of the `oep-3dxml` Cargo crate: parse the primary
model document of a 3DXML container (the `<Model_3dxml>` XML, named by
`Manifest.xml` `<Root>`, or a standalone `.3dxml`) into a typed rep model of
the assembly graph — **nodes** (`Reference3D`), **references**
(`ReferenceRep` with the `associatedFile` URN), and **instances**
(`Instance3D` with `<IsAggregatedBy>`/`<IsInstanceOf>` links and the 12-double
`<RelativeMatrix>` transform), plus the closing `InstanceRep` wiring. It uses
a real XML engine (`quick-xml`) per the P3-M1-T01 RE note — the manifest stays
std-only, the model document gets a full parser.

Deliverables:

- `crates/oep-3dxml/src/rep.rs` — `parse_model(bytes)` → `ModelRep`
  (`Header` + `ProductStructure`); typed `RepError`, never panics.
- `crates/oep-3dxml/tests/rep_parse.rs` — TDD acceptance test (written RED
  against the skeleton crate, then GREEN) anchored to real license-clean
  corpus files with a golden-facts map.
- `crates/oep-3dxml/Cargo.toml` — adds `quick-xml` (rep parse engine, no
  serde/async features) and `zip` as a *dev*-dependency (test-only: opens
  real ZIP-archive corpus 3DXMLs to reach the model document; T01 carries
  `zip` as a full dependency in the container code).
- This RE note (evidence + provenance + verification transcript).

## 2. Grounding and clean-room posture

All facts asserted here come from the license-clean MIT/Apache-2.0 corpus of
P1-M1-T02 (`corpus/data/3dxml`) and the hex notebook P1-M2-T04
(`docs/re-notes/3dxml-hex-notebook.md` §4.1, §4.4). The element/attribute
names are the public `3DXML.xsd` vocabulary observed on the corpus — no vendor
documentation is quoted, no proprietary bytes are committed. The only new
library linked is `quick-xml` (pure-Rust, zero-dependency-default streaming
XML reader; supply-chain posture P11-T06).

## 3. Observed model-document shape (locked by the acceptance test)

The primary model document (Abaqus-written `Abaqus_Model.3dxml`, CATIA/SOLIDWORKS
written `NameSetTree.3dxml`, or the standalone `quad.3dxml`) is a
`<Model_3dxml>` rooted at namespace `http://www.3ds.com/xsd/3DXML`:

```xml
<Model_3dxml ...>
  <Header><SchemaVersion>4.2</SchemaVersion><Title>..</Title> ...</Header>
  <ProductStructure root="1">
    <Reference3D xsi:type="Reference3DType" id="1" name="..."/>      <!-- node -->
    <ReferenceRep xsi:type="ReferenceRepType" id="2" format="TESSELLATED"
                  version="1.2" associatedFile="urn:3DXML:TessPart_2.3DRep"/>  <!-- reference -->
    <Instance3D xsi:type="Instance3DType" id="3" name="...">
      <IsAggregatedBy>1</IsAggregatedBy>
      <IsInstanceOf>4</IsInstanceOf>
      <RelativeMatrix>12 doubles, row-major 4x3</RelativeMatrix>
    </Instance3D>                                                        <!-- instance -->
    <InstanceRep xsi:type="InstanceRepType" id="5">
      <IsAggregatedBy>4</IsAggregatedBy><IsInstanceOf>2</IsInstanceOf>
    </InstanceRep>
  </ProductStructure>
  <DefaultView>...</DefaultView>
</Model_3dxml>
```

Golden map (all counts verified this session on the corpus, 2026-10-15):

| File | Schema | Ref3D | RefRep | Inst3D | InstRep | root | first associatedFile |
|---|---|---|---|---|---|---|---|
| blocks.3dxml | 4.0 | 2 | 1 | 1 | 1 | 1 | Abaqus_Geometry.3DRep |
| cube1.3dxml | 4.0 | 2 | 1 | 1 | 1 | 1 | Abaqus_Geometry.3DRep |
| machinist.3DXML | 4.2 | 1 | 1 | 0 | 1 | 1 | TessPart_2.3DRep |
| quad.3dxml | 4.3 | 7 | 4 | 8 | 4 | 1 | Chassis.3DRep |
| cycloidal.3DXML | 4.2 | 17 | 13 | 67 | 13 | 1 | TessPart_5.3DRep |
| FM1_satellite.3DXML | 4.2 | 72 | 36 | 94 | 36 | 1 | TessPart_3.3DRep |

`[observation]` Writers differ in cosmetics: SOLIDWORKS' exporter omits
`<ReferenceRep>`/`<Instance3D>` `name` (None), CATIA adds authoring metadata
children (`<PLM_ExternalID>`, `<V_discipline>`, `<V_usage>`, `<V_nature>`) and
per-instance `<PLM_ExternalID>`, quad embeds per-instance material matrices
(`<CATMaterial>`, `<V_Matrix_1..12>`). The parser must tolerate all of these
(via unknown-element skip) and still count nodes/references/instances exactly.
Line endings LF vs CRLF and one-line vs pretty-printed documents are handled
by the engine.

## 4. Detection / API contract

- `oep_3dxml::rep::parse_model(&[u8]) -> Result<ModelRep, RepError>`
- `ModelRep { header: Header, structure: ProductStructure }`
- `ProductStructure { root_id, nodes: Vec<Reference3D>, references:
  Vec<ReferenceRep>, instances: Vec<Instance3D>, instance_reps:
  Vec<InstanceRep> }`
- `ReferenceRep::member_name() -> Option<String>` strips the `urn:3DXML:`
  prefix to yield the `.3DRep` member name (the rep/mesh join key).
- A `<RelativeMatrix>` that is not exactly 12 doubles → typed `RepError::Matrix`
  (never a panic; fuzz-friendly per P2-M1-T05 discipline). Non-UTF-8 input,
  truncated XML, or a bare prose blob with no `<Model_3dxml>` root → typed
  `RepError::Xml` / `RepError::InvalidUtf8`.
- Unknown elements anywhere (writer metadata, `<DefaultView>`, material refs)
  are skipped; an empty `<Model_3dxml/>` parses to an empty structure.

## 5. Verification transcript (real material, 2026-10-15)

`OEP_3DXML_SAMPLES=<corpus> cargo test -p oep-3dxml`

```
info  verifying rep parse on real material: .../blocks.3dxml
PASS  blocks.3dxml: schema=4.0 nodes=2 refs=1 instances=1 instreps=1
info  verifying rep parse on real material: .../cube1.3dxml
PASS  cube1.3dxml: schema=4.0 nodes=2 refs=1 instances=1 instreps=1
info  verifying rep parse on real material: .../machinist.3DXML
PASS  machinist.3DXML: schema=4.2 nodes=1 refs=1 instances=0 instreps=1
info  verifying rep parse on real material: .../quad.3dxml
PASS  quad.3dxml: schema=4.3 nodes=7 refs=4 instances=8 instreps=4
info  verifying rep parse on real material: .../cycloidal.3DXML
PASS  cycloidal.3DXML: schema=4.2 nodes=17 refs=13 instances=67 instreps=13
info  verifying rep parse on real material: .../FM1_satellite.3DXML
PASS  FM1_satellite.3DXML: schema=4.2 nodes=72 refs=36 instances=94 instreps=36
test result: ok. 9 passed; 0 failed
```

When no sample is supplied (e.g. Gitea CI before the P1 corpus PR merges) the
real-material leg prints a tolerant SKIP so CI stays green; the test searches
`$OEP_3DXML_SAMPLES`, `samples`/`../samples` next to the crate, and the repo
corpus `corpus/data/3dxml`.

## 6. Open questions / next probes

1. P3-M1-T03 (mesh rep → internal mesh) consumes `ReferenceRep::member_name()`
   to locate the `.3DRep` member inside the container and `Instance3D`
   `relative_matrix` for placement. The rep XML (`XMLRepresentation`
   `<VertexBuffer>`/`<Faces>`) is deliberately untouched here.
2. The binary V5_CFV3 `.3DRep` branch (format=`UVR`) is P3-M3-T01; the model
   doc parse here already records `format="UVR"` so the two branches route on
   the same join key.
3. `[uncertain]` whether a writer can nest `<Reference3D>` elements (corpus has
   only flat, flat-sibling ProductStructure). If a nested form appears, the
   flat siblings assumption needs revisiting.

## 7. Invariant

Clean-room only: the repo records observed facts, never proprietary bytes.
Synthetic test fixtures are built from public element names and observed
shape. The corpus itself is committed by P1-M1-T02, not duplicated here.
