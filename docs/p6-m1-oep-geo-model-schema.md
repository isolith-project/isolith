# oep-geo model schema and serialization format (P6-M1-T01)

Version 1.0, 2026-10-16. Milestone: P6-M1-T01 (`oep-geo` model schema:
part, body, feature, assembly, instance; serde serialization in our own
documented format). Program: Keelworks / Open Engineering Platform (OEP),
P6 geometry core. Clean-room, our own format — nothing here is a vendor
format (no 3DXML, STEP, CGR, or Dassault-derived schema is embedded).

Reference implementation: `crates/oep-geo` (Rust, serde).
Real-material gate: `crates/oep-geo/tests/golden/example-bracket.oepgeo.json`,
byte-locked by `crates/oep-geo/tests/schema_serde.rs` and independently
re-parsed by `tests/test_oep_geo_acceptance.py`.

---

## 1. The five model classes

An OEP document holds five geometry classes, stored flat (one id key per
entry) and linked by id, never by ownership embedding:

| class     | module     | role |
|-----------|------------|------|
| Part      | `part`     | an engineering artifact: identity, part number, owned bodies, feature history |
| Body      | `body`     | an independent lump of material (solid / sheet / wire) with an optional bounding box |
| Feature   | `feature`  | a modelling operation (open vocabulary kind + parameter map) that produced bodies |
| Assembly  | `assembly` | a named frame holding an ordered list of child instances |
| Instance  | `assembly` | one placement of a part or a nested sub-assembly, at a 4x4 transform |

Deliberate scope: the B-Rep / geometry kernel slots, PMI (GD&T/datums,
P6-M1-T02), the units + tolerance model (P6-M1-T03), and feature-graph replay
(P6-M3-T03) are later cards; this card reserves only their schema hooks.

### 1.1 Part

| field         | type             | notes |
|---------------|------------------|-------|
| `id`          | string           | unique document-wide |
| `name`        | string           | human-readable |
| `part_number` | string?          | optional PN (BOM/PDM interop); omitted when absent |
| `body_ids`    | string[]         | ids of the bodies this part owns |
| `feature_ids` | string[]         | ids of the features in this part's history |

### 1.2 Body

| field         | type             | notes |
|---------------|------------------|-------|
| `id`          | string           | unique |
| `name`        | string           | |
| `kind`        | string           | one of `solid`, `sheet`, `wire` |
| `bbox`        | object?          | `{min:[x,y,z], max:[x,y,z]}`; omitted when absent |
| `feature_ids` | string[]         | features that produced this body |

The `kind` vocabulary is closed (three values) — a committed enum, not an open
string — so readers can switch on it soundly.

### 1.3 Feature

| field         | type             | notes |
|---------------|------------------|-------|
| `id`          | string           | unique |
| `name`        | string           | |
| `kind`        | string           | open PascalCase vocabulary, e.g. `Extrude`, `Fillet`, `Hole`, `Revolve`, `Pattern`, `Mirror` |
| `parameters`  | object           | key -> JSON value; keys are sorted when serialized (deterministic) |
| `body_ids`    | string[]         | bodies the operation produced or consumed |

`kind` is an open vocabulary on purpose: feature semantics are negotiated
between readers/kernels (and defined by P6-M3-T03 replay), so an unknown kind
must never crash a reader. Tolerance is a documented behavior, not an enum.

### 1.4 Assembly

| field      | type           | notes |
|------------|----------------|-------|
| `id`       | string         | unique |
| `name`     | string         | |
| `instances`| instance[]     | ordered child placements |

### 1.5 Instance

| field       | type             | notes |
|-------------|------------------|-------|
| `id`        | string           | unique |
| `name`      | string           | |
| `source`    | object           | interned tag + ref: `{"kind":"part","id":...}` or `{"kind":"sub-assembly","id":...}` |
| `transform` | number[16]      | row-major 4x4 affine matrix, see 3.3 |
| `visible`   | bool             | view visibility |

An instance sources **exactly one** target (part or sub-assembly) — validated,
never ambiguous.

---

## 2. Ids and the reference model

- Ids are non-empty strings; the document-wide convention is `oep-<class>-<uid>`
  (for example `oep-part-bracket`, `oep-body-bracket`, `oep-inst-fastener-1`).
- One document = one id space: ids must be unique across all five collections
  and all instances.
- **Why flat store + id refs instead of nested ownership:** a body, feature,
  part, or assembly is a first-class object that the PDM layer (P8) evolves as
  its own revision; multiple higher objects reference it (a part can appear in
  many assemblies; the same part can share a body via patterns/mirrors). Id
  refs keep that possible and keep serialization acyclic and cheap.
- References are resolved by `Document::validate()`, never during
  deserialization, so a partially linked file still loads (tolerant-failure
  doctrine, P2-M1-T04).

---

## 3. Serialization format (our own)

### 3.1 Header

Every document opens with:

```json
{
  "format": "oep-geo",
  "version": 1,
  "units": "mm"
}
```

- `format`: fixed tag `oep-geo` (disambiguation from other JSON).
- `version`: schema version, currently `1`. A reader must reject versions it
  does not understand (validation rule `unsupported-version`), because byte
  compatibility is only guaranteed within a version.
- `units`: a plain string (`mm`, `in`, ...). The units/tolerance **model**
  (units conversion, epsilon policy) is P6-M1-T03; this field is the nominal
  document unit today.

### 3.2 Orientation

- The format is **JSON** (RFC 8259), UTF-8.
- Two encodings are valid: canonical pretty (2-space indent, trailing newline)
  and compact JSON. Every writer SHOULD emit canonical pretty; every reader
  MUST accept both.
- Numbers are IEEE-754 doubles. serde_json emits the shortest representation
  that round-trips the exact double, so `serialize -> deserialize` is
  bit-exact for every finite double (NaN/Inf must not appear; validated).

### 3.3 Instance transform

`transform` is a 4x4 **row-major** matrix of 16 doubles, translation in
elements 12/13/14 and the homogeneous row `[15]` fixed to `1.0` for a valid
affine transform:

```
[ 0  1  2  3 ]     identity = [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]
[ 4  5  6  7 ]                translation(x,y,z) sets [12]=x [13]=y [14]=z
[ 8  9 10 11 ]
[12 13 14 15 ]                (you transpose to get the glTF column-major form)
```

Rotation/scale live in the top-left 3x3; validation requires all 16 values
finite but deliberately does not require orthonormality (non-uniform scale is
allowed, so the check stays a sanity check, not a normalizer).

### 3.4 Determinism

The byte output of `Document::to_json_pretty()` is deterministic:

- object key order = Rust struct declaration order (striped through serde);
- every map in the model is a `BTreeMap` (sorted keys) — most importantly the
  feature `parameters` map;
- no `HashMap` anywhere in serialized types.

Consequence: the same logical document always serializes to the same bytes,
which is what makes golden-byte locking meaningful and diff-stable VCS history
possible. This is a hard invariant of the format from day one.

### 3.5 Round-trip guarantee

`serialize -> deserialize` is `==` at the value level (serde `PartialEq`
drops). Optional fields omitted on write deserialize to their default
(`None`/empty/`true` as annotated), so old files stay readable as the schema
adds optional fields (forward compatibility within a version — the exact
versioning policy beyond a single version is decided by the release-train
card P12-T05, not here).

---

## 4. Validation rules (`Document::validate()`)

`validate()` returns every violation as `{code, message}`, never panics, and
rejects nothing at load time. Codes:

| code                    | condition |
|-------------------------|-----------|
| `bad-format`            | `format != "oep-geo"` |
| `unsupported-version`   | `version != 1` |
| `duplicate-id`          | an id appears twice in the document id space |
| `duplicate-ref`         | a reference list contains the same id twice |
| `unresolved-body`       | a part/body/feature references a body id not in `bodies` |
| `unresolved-feature`    | a part/body references a feature id not in `features` |
| `unresolved-source`     | an instance sources a part/assembly id not in `parts`/`assemblies` |
| `invalid-bbox`          | body `bbox` is non-finite or `min[i] > max[i]` |
| `non-finite-transform`  | an instance transform has a NaN/Inf component |
| `assembly-cycle`        | an assembly (transitively) instances itself |

Assembly cycles are detected with a DFS over the `sub-assembly` edges from
every root assembly (an assembly that no other assembly instances).

---

## 5. The canonical example (real material)

`crates/oep-geo/tests/golden/example-bracket.oepgeo.json` is a hand-authored
model of a realistic small product — an 80x40x6 mm bracket (solid body from
Extrude + Fillet + Hole features), a 10x10x20 mm fastener, a fastener-set
sub-assembly holding **four** fastener instances at the bracket corners
`(10,10) (70,10) (10,30) (70,30)`, and a top assembly referencing the bracket
at identity plus the fastener-set as one sub-assembly instance. It exercises
every class, both instance-source kinds, a sub-assembled DAG, and both
`number[]`-of-16 transforms — held to exact bytes by the Rust snapshot test and
independently parsed (stdlib) by the Python acceptance test.

## 6. Extensibility notes

- Unknown feature `kind`s are loadable and only ever warned about.
- New fields are added as `#[serde(default)]`/optional so old files still read.
- New `version` means new parse rules; old readers reject loudly instead of
  misreading (rule `unsupported-version`).

See also: `docs/layout.md` (crate map), P6-M1-T02 (PMI schema), P6-M1-T03
(units/accuracy), P6-M3-T03 (feature graph replay).
