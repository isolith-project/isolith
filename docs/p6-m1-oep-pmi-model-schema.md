# oep-pmi model schema and serialization format (P6-M1-T02)

Version 1.0, 2026-10-16. Milestone: P6-M1-T02 (PMI model schema: GD&T types,
datums, notes; importable from P4). Program: Keelworks / Open Engineering
Platform (OEP), P6 geometry core. Clean-room, our own format — nothing here is
a vendor format, and no Dassault/Siemens/PTC PMI schema is embedded.

Reference implementation: `crates/oep-pmi` (Rust, serde).
Real-material gate: `crates/oep-pmi/tests/golden/example-bracket.oeppmi.json`,
byte-locked by `crates/oep-pmi/tests/pmi_serde.rs` and independently re-parsed
by `tests/test_oep_pmi_acceptance.py`.

---

## 1. Why a standalone PMI crate

The P4 interop workstream (`oep-step`, STEP AP242/QIF/JT) must import semantic
PMI (GD&T, datums, tolerances, notes) into an *internal* PMI model
(P4-M2-T01). That internal model is this crate. It is a **leaf** dependency —
it references oep-geo entities (parts, bodies, features) **by id string only**
and carries no cross-crate Rust dependency (layout.md: no path deps until the
P6 importer hub P6-M4-T02), so both oep-geo and oep-step can build on it
without a chain. This is also why T02 is not folded into the oep-geo crate:
the oep-geo `Document` (P6-M1-T01) reserves PMI as a later card and its golden
is byte-locked, so the PMI model lives as its own importable schema.

## 2. The four model classes

A PMI document holds four classes, stored flat (one id-keyed entry each) and
linked by id, never by ownership embedding:

| class                | module      | role |
|----------------------|-------------|------|
| Datum                | `datum`     | a real feature designated as a datum by a unique letter label |
| DatumFrame (DRF)     | `datum`     | ordered primary->secondary->tertiary block of datum refs |
| GeometricTolerance   | `tolerance` | a GD&T feature control frame: characteristic + value + refs |
| Note                 | `note`      | free-text note (general note or leader callout) |

### 2.1 Datum

| field        | type     | notes |
|--------------|----------|-------|
| `id`         | string   | unique document-wide |
| `label`      | string   | single-letter label (e.g. `A`); unique per document, non-blank |
| `feature_ids`| string[] | oep-geo feature ids this datum designates |
| `body_ids`   | string[] | oep-geo body ids this datum designates |

### 2.2 DatumFrame + DatumFrameRef

| field        | type            | notes |
|--------------|-----------------|-------|
| `id`         | string          | unique |
| `name`       | string          | e.g. `A-B-C` |
| `refs`       | DatumFrameRef[] | ordered; precedence validated 1..=3, no duplicates |

`DatumFrameRef`:

| field       | type          | notes |
|-------------|---------------|-------|
| `datum_id`  | string        | ref into `datums` (validated) |
| `precedence`| integer 1..=3 | 1 = primary, 2 = secondary, 3 = tertiary |
| `modifier`  | string?       | `mmc` \| `lmc` \| `rfs`; omitted when `rfs`-default-not-spelled |

### 2.3 GeometricTolerance (GD&T)

| field                 | type    | notes |
|-----------------------|---------|-------|
| `id`                  | string  | unique |
| `characteristic`      | string  | closed 14-value Y14.5/ISO 1101 set, see 2.3.1 |
| `value`               | number  | primary tolerance value in doc units; finite, >= 0 |
| `diameter_zone`       | bool    | true = diameter zone (`Ø` prefix on the value) |
| `material_condition`  | string? | `mmc` \| `lmc` \| `rfs`; omitted when None |
| `frame_id`            | string? | ref into `frames` (validated); omitted when None |
| `second_value`        | number? | composite/profile second value; omitted when None |
| `feature_ids`         | string[]| applied oep-geo feature ids |
| `body_ids`            | string[]| applied oep-geo body ids |
| `note`                | string? | callout/explanation note; omitted when None |

#### 2.3.1 The closed characteristic set (ASME Y14.5 / ISO 1101)

Serialized kebab-case:

| category   | characteristics |
|------------|-----------------|
| form       | `straightness`, `flatness`, `circularity`, `cylindricity` |
| profile    | `profile-of-line`, `profile-of-surface` |
| orientation| `angularity`, `perpendicularity`, `parallelism` |
| location   | `position`, `concentricity`, `symmetry` |
| runout     | `circular-runout`, `total-runout` |

A reader may switch on the characteristic soundly — it is a committed enum, not
an open string (14 variants, `#![deny(missing_docs)]`-covered).

### 2.4 Note

| field        | type      | notes |
|--------------|-----------|-------|
| `id`         | string    | unique |
| `text`       | string    | free text (non-blank) |
| `feature_ids`| string[]  | annotated oep-geo feature ids |
| `body_ids`   | string[]  | annotated oep-geo body ids |
| `position`   | number[3]?| optional 3D anchor `[x,y,z]`; omitted when None |

---

## 3. Ids and the reference model

- Ids are non-empty strings; the convention mirrors oep-geo:
  `datum_A`, `drf_abc`, `gt_1`, `note_holes`, ... (schema-agnostic).
- One PMI document = one id space: ids unique across all four collections.
- Cross-document references (feature_ids/body_ids pointing at an oep-geo
  Document) are **not** resolved here — they are id strings checked for
  duplicates only. Resolution against a real oep-geo document happens at the
  importer hub (P6-M4-T02) or the P4 importer, which owns both documents.

## 4. Serialization format (our own)

### 4.1 Header

```json
{
  "format": "oep-pmi",
  "version": 1,
  "units": "mm"
}
```

- `format`: fixed tag `oep-pmi`.
- `version`: schema version, currently `1`; readers reject unknown versions
  (rule `unsupported-version`).
- `units`: plain string (`mm`, `in`, ...); the tolerance/units **model** is
  P6-M1-T03.

### 4.2 Orientation

- JSON (RFC 8259), UTF-8. Canonical pretty (2-space indent, trailing newline)
  or compact; writers emit canonical, readers accept both.
- Numbers are IEEE-754 doubles, shortest-form round-trippable (serde_json);
  NaN/Inf must not appear in `value`/`second_value`/`position` (validated).
- Optional fields are **omitted when None** (`#[serde(skip_serializing_if =
  "Option::is_none")]`), matching the oep-geo-dependent key-omission contract
  T01 established. Presence of a key implies a value; absence implies None.

### 4.3 Determinism

Byte output of `PmiDocument::to_json_pretty()` is deterministic: object key
order = Rust struct declaration order (serde), and the model uses only `Vec`
collections in declaration order (no HashMap anywhere serialized). Same logical
document → same bytes → meaningful golden-byte locking and diff-stable history.

### 4.4 Round-trip guarantee

`serialize -> deserialize` is `==` at the value level (serde `PartialEq`).
Forward-compatible within a version: new optional fields default to
None/empty.

---

## 5. Validation rules (`PmiDocument::validate()`)

`validate()` returns every violation as `{code, message}`, never panics, and
rejects nothing at load time. Codes:

| code                    | condition |
|-------------------------|-----------|
| `bad-format`            | `format != "oep-pmi"` |
| `unsupported-version`   | `version != 1` |
| `duplicate-id`          | an id appears twice in the document id space |
| `duplicate-label`       | two datums share a label |
| `duplicate-ref`         | a reference list contains the same id twice |
| `unresolved-datum`      | a frame ref names a datum id not in `datums` |
| `unresolved-frame`      | a tolerance names a frame id not in `frames` |
| `invalid-precedence`    | a frame ref precedence is outside 1..=3 |
| `duplicate-precedence`  | a frame has two refs with the same precedence |
| `non-finite-value`      | tolerance value / second value not finite or negative |
| `invalid-position`      | a note position is not exactly three finite numbers |

---

## 6. Importable from P4 (destination-model contract)

The acceptance test includes `_p4_flavoured_source()` (a STEP-AP242-style
semantic skeleton: geometric tolerance entities with a characteristic string, a
datum reference frame with precedence, datum identifications, notes) and
collapses it into the oep-pmi settlement. The test proves the mapping target
agrees with the committed golden on: id sets per collection, characteristic,
value, diameter-zone flag, material condition, frame id, and the DRF
precedence/datum order. This is the fixpoint the real `oep-step` AP242 importer
(P4-M2-T01) will implement; this card locks the destination schema so the
importer teams against a stable, documented contract.

Deliberately out of scope for v1 (documented for a later card): composite
tolerance segments beyond one `second_value`, datum-target zones (Y14.5 4.6),
bonus-tolerance arithmetic, and cross-document body/feature resolution.

---

## 7. The canonical example (real material)

`crates/oep-pmi/tests/golden/example-bracket.oeppmi.json` models PMI on the
same bracket the oep-geo golden (P6-M1-T01) documents:

- datums `A` (top face), `B` (mount hole), `C` (mount hole);
- one datum reference frame `drf_abc` = `A | B | C`;
- a position tolerance `Ø0.2` **MMC** on the mount hole wrt `A|B|C`; a flatness
  `0.05` tolerance on the top face; a perpendicularity `0.1` on the mount hole;
- two notes: `4X Ø6.00 THRU` (with 3D anchor) and `ALL DIMENSIONS IN MM`.

Held to exact bytes by the Rust snapshot test and independently re-parsed
(stdlib) by the Python acceptance test.

## 8. Extensibility notes

- Unknown optional fields load fine (serde default), preserving forward compat
  within a version.
- New `version` means new parse rules; old readers reject loudly
  (`unsupported-version`) instead of misreading.
- The characteristic set is committed; adding a characteristic is a version
  bump, not a silent extension.

See also: `docs/layout.md` (crate map), P6-M1-T01 (oep-geo model schema),
P6-M1-T03 (units/tolerance model), P4-M2-T01 (STEP PMI import), P6-M4-T02
(importer hub).
