# P4-M4-T01: QIF import/export — basic cut (MBD quality data)

Status: delivered (wave-2 P4 QIF lane). Branch: `task/oep-qif-1016a`.
Crate: `crates/oep-qif`.

## What QIF is

QIF ("Quality Information Framework", ANSI QIF 3.0 / ISO 23952:2020, published
by DMSC) is a free-to-implementers XML framework standard for the metrology /
quality side of the Model-Based Enterprise. A QIF document carries the MBD
model (product) plus measurement plans, results, resources and statistics.
Our entry point is the **MBD quality data** (datums, datum reference frames,
GD&T characteristics and their nominals) — the design-side PMI that primes
"actually exchange" between CAD and inspection.

Sources used (observational, license-safe, standards/qif/keelworks-use.md):

- Official schema bundle (free to implementers): DMSC QIF3.0-2018-ANSI.zip at
  qifstandards.org; xsd/QIFApplications/QIFDocument.xsd + QIFLibrary/*.
- Official sample instance files: QualityInformationFramework/qif-community
  (Boost-1.0-licensed samples), incl. the NIST PMI Validation & Conformance
  (CTC-01 ASME1 AP242) model committed here as a dev-only fixture.

## The basic cut — what is IN

The crate reads and writes a **documented subset** of the QIF 3.0 model, byte-
stable and syntax-valid against the official XSD. Modeled elements:

| QIF document part | What we carry |
|---|---|
| Root QIFDocument | namespace qif3, `versionQIF="3.0.0"`, `idMax`, QPId |
| Header | Application/Name, Author/Name + Organization |
| StandardsDefinitions | Standard id, organization enum (ASME...), designator |
| FileUnits | Primary linear unit (SI + unit name + optional factor) |
| DatumDefinitions | DatumDefinition id + DatumLabel (A/B/C...) |
| DatumReferenceFrames | DatumReferenceFrame + SimpleDatum (definition id, material modifier, referenced component) + Precedence |
| Product | Part identity (PartSet/Part id + ModelNumber) + RootPart |
| Characteristics | FormalStandardId + **6-kind GD&T definitions** (position, surface profile, perpendicularity, flatness, diameter size, angle) with tolerance value/bounds, unit, DRF ref, material condition, zone shape, extent — **plus** matching nominals (definition ref, name, designator, target value) |

The model (`src/model.rs`) is deliberately QIF-native: it keeps QIF ids and
text verbatim (ids as strings for references, tolerance values as raw text so
decimalPlaces/units survive round trips byte-exactly) and is 100%
deterministic (ordered `Vec`s, no hashmaps, no sorting — document order in,
document order out).

## What is OUT (tolerantly skipped — never a crash, never an invented value)

Everything else in a real QIF file is stepped over and **counted** in the
parse report (`ParseReport.skips`), which is part of the API, not hidden:

- GeometrySet / TopologySet (curves, surfaces, NURBS, tessellation, binary
  streams) — geometry carriers, out of scope for the quality-data cut.
- ViewSet / CameraSet / AnnotationViewSet / VisualizationSet / PMIDisplay /
  Fonts — display/annotation layout.
- Features (FeatureDefinitions / FeatureNominals) — semantic features are a
  later P4-M2 / P6 mapping concern; we carry their id references only when the
  element itself is modeled (nominal feature ids are skipped with a reason).
- NoteSet / PartNoteSet — geometry-anchored free text (documented OUT).
- Plan / Results / Statistics / Rules / MeasurementResources /
  ManufacturingProcessTraceabilities — other QIF sub-models beyond MBD.
- Vendor Attributes, ExternalIds, EntityInternalIds, UserDataXML, Signature.

The skip taxonomy is a closed set of reasons ("outside basic cut", "unknown
document section", feature-links, namespace note). A file that is NOT QIF at
all, or a QIF version other than 3.0, returns a typed `Error` (NotQif /
UnsupportedVersion) instead of a best-effort parse.

## Round-trip contract

- `parse(bytes) -> Result<(QifDoc, ParseReport), Error>` imports the basic cut.
- `write(&QifDoc) -> Result<String, Error>` emits the canonical normalized
  QIF document with strict official element order and n= counts.
- Contract: `parse(write(parse(x)))` equals `parse(x)` on the model
  (structural equality via `PartialEq`); the export is **byte-deterministic**
  (locked to the committed golden `nist-mbd-basic-export.qif`).
- Honesty: the export is a *normalized basic-cut document*, not a
  byte-copy of the incoming file. Geometry/plan/result content the source may
  have is deliberately not re-emitted; the scope of what survives the round
  trip is exactly the IN table above.
- `self_check(doc) -> Vec<String>` verifies reference integrity (datum ids,
  DRF ids, characteristic definition ids all resolve) and idMax coverage.

## Syntax validation (the acceptance's "output syntax-validated")

Two layers, both executed in this card's verification:

1. **Official QIF 3.0 XSD**: the exported golden AND the source fixture are
   validated against DMSC's QIFDocument.xsd tree using lxml (with a local
   resolver for the include/import chain). Proven VALID for all three files:
   the fixture, the exported golden, and the canonical basic fixture. The
   acceptance test runs this leg whenever the official schema is on disk
   (`OEP_QIF_XSD_ROOT`), with a loud SKIP + one-line fetch command otherwise.
   `python/oep_tooling/qif_xsd.py` fetches the official bundle reproducibly
   (pinned sha256) and validates.
2. **Structural invariants**: `self_check` (reference integrity) + the Python
   test's independent xml.etree re-parse of the golden (content + element
   shape), which does not depend on the Rust writer at all.

## Fixture provenance (dev-only reference material, decision #7)

- `crates/oep-qif/tests/golden/nist_ctc_01_asme1_ap242.qif` — real NIST PMI
  Validation & Conformance CTC-01 model (ASME1, AP242 QIF), fetched from
  QualityInformationFramework/qif-community samples (Boost-1.0-licensed
  sample directory; NIST-origin model), sha256
  `103254361d09f47fc7498f21b8e9317c3112d8eb26c751cb64e0d6243c56bc9d`.
  Dev-only reference material, never shipped with a product release.
- `crates/oep-qif/tests/golden/basic-mbd.qif` — minimal canonical fixture
  covering all six GD&T kinds (XSD-valid).
- `crates/oep-qif/tests/golden/nist-mbd-basic-export.qif` — the byte-locked
  canonical export of the NIST fixture's basic cut (regenerate with
  `OEP_QIF_REGEN=1`).

## Mapping hook (later cards)

The QIF characteristics/tolerance model maps onto `oep-pmi` (the internal PMI
model, P6-M1-T02) and the `oep_geo` part identity during P4-M2 (semantic PMI
import) / the P6 importer hub. This card deliberately keeps `oep-qif`
standalone (no cross-crate path deps per docs/layout.md doctrine) so the
mapping lands in the adapter layer without coupling the format crate.

## Verified this run

- `cargo test -p oep-qif` — 5/5 (real NIST round trip, canonical fixture
  round trip, golden byte-lock, typed NotQif, typed UnsupportedVersion).
- Official XSD: fixture, exported golden, basic fixture all VALID.
- Python acceptance: independent reparse + XSD legs (schema present).
- Workspace build/test plus the full precommit gate below.
