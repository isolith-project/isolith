---
title: Keelworks oep-step-pmi Semantic PMI Import Approach (P4-M2)
decision: wrap_the_oss_step_io_scene_and_raw_model_for_semantic_pmi_behind_vendor_neutral_wrappers
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks program governance, P4 STEP/QIF/JT lane, P4-M2 semantic-PMI milestone
applies_to: oep-step-pmi (adopting crate), the oep-pmi import destination (P6), the P6 importer hub (oep-import feed)
supersedes: nothing (inherits step-io as the P4 STEP library from workstreams/P4-M1/step-library-adr.md)
---

# Keelworks oep-step-pmi Semantic PMI Import Approach

This record is the binding product decision for the P4-M2-T01 card ("Semantic
PMI import (GD&T, datums, tolerances) into the oep-geo PMI model"). The
P4-M1 ADR (workstreams/P4-M1/step-library-adr.md) explicitly scheduled a revisit at the
P4-M2 gate: "the OCCT binding effort is compared against the AP242 / PMI
fidelity requirement." This is that revisit, resolved.

The decision: keep the pure-Rust OSS STEP library (step-io, pinned =0.2.5)
as the P4 STEP base and build `oep-step-pmi` on top of its scene PMI surface
(dimensions, geometric tolerances, datums) plus a private raw-model walk for
the modifier and tolerance-zone facets, mapping everything onto the internal
`oep_pmi` model (schema P6-M1-T02). OCCT remains the recorded analytic
B-Rep fidelity path for a later milestone; it is NOT needed to import real
AP242 semantic PMI today, and CI still has no C++ toolchain. This keeps the
anti-fabrication doctrine intact: a file with no PMI is honestly flagged
(`pmi_present == false`), never given invented PMI.

## 1. Decision

- `oep-step-pmi` is a new leaf reader crate in the P4 lane. It wraps step-io
  =0.2.5 (the library P4-M1 ratified, same pin) behind a two-layer seam:
  step-io types live ONLY inside `src/import.rs` (the private walk) and never
  appear on the public surface. The public API exposes only `oep_pmi` types
  and coined neutral types (`PmiImport`, `PmiImportReport`, `DroppedItem`,
  `Error`).
- The import maps, from the scene surface: tolerance characteristic (kind),
  tolerance magnitude (the zone value), datum letters, datum references in
  precedence order (primary through tertiary), and dimensional-characteristic
  presence. A private raw-model walk over the SAME ComplexUnit the scene uses
  adds the material-condition modifier (MMC/LMC from
  `GEOMETRIC_TOLERANCE_WITH_MODIFIERS`) and the diameter-zone flag (a
  TOLERANCE_ZONE whose form is cylindrical/circular/spherical and whose target
  is that tolerance's ComplexUnit).
- Authority to remap expectations: this is exactly "semantic PMI import", not
  a presentation/annotation copy. The source of truth is the ASME Y14.5 /
  ISO 1101 semantics endoded in the STEP AP242 entities. Closed-set handling:
  a characteristic that is not in the committed 14-value set (e.g. ISO-only
  coaxiality, or an unrecognized `Other`) is dropped WITH a reason in the
  report, never coerced into a wrong slot.
- Honest absence contract: `import()` returns a valid EMPTY `oep_pmi`
  document with `report.pmi_present == false` when the STEP file has no
  tolerances, datums, or dimensions. `import_strict()` turns that honest
  absence into a typed `Error::NoPmi` for callers that require PMI. Junk /
  non-STEP / unreadable input is a typed `Error::Io` / `Error::NotStep` /
  `Error::Parse`, never a panic.
- Dimensions (size/location/angular) are COUNTED in the report and not mapped
  to `oep_pmi::Note`. The Note class is free text; synthesizing notes from
  dimension values would fabricate text, so dimensional characteristics are
  reported, not invented. This is a documented v1 scope boundary.

## 2. Why this decision

1. The acceptance bar is green CI with no C++ toolchain. The repository has no
   OCCT binding; `rust:1-slim-bookworm` has no C++ toolchain; a C++
   FFI bind is not buildable or gateable at this milestone (this was the
   P4-M1 ADR's operational constraint, and it still holds).
2. step-io's scene PMI surface is a real, published OSS capability. Verified
   against three real NIST MBE AP242 fixtures BEFORE this decision was locked:
   ftc_06 (edition 2) yields 28 geometric tolerances (flatness, perpendicularity,
   position, surface profile), 10 datums, and dimensions; stc_06 (edition 3)
   yields 25 tolerances and 6 datums; ftc_08 (a tessellated-geometry-only part)
   yields zero PMI. MMC modifiers and cylindrical/spherical zone forms read
   correctly through the private raw-model walk. The wedge is real and it is
   reachable without OCCT.
3. The vendor-neutral seam bounds the exposure. If OCCT (or an own parser) is
   adopted later for analytic B-Rep or deeper AP242 facets, only the private
   walk in `src/import.rs` changes; the oep-pmi destination model and the
   public API are untouched.
4. PMI semantics are the CATIA moat the program wants to wedge. Mapping true
   entities (characteristic, value, datums, precedence, modifier, zone) onto a
   committed schema, with honest gaps, is the wedge. Coercing unknown
   characteristics or inventing contrast would burn that trust.

## 3. Context

- The destination model is `oep_pmi::PmiDocument` (P6-M1-T02): format
  `oep-pmi`, version 1, four flat collections (datums, frames, tolerances,
  notes) linked by id and validated structurally at `validate()`. The schema
  deliberately resolves cross-document feature/body references at the importer
  hub (P6-M4-T02), so this reader writes id strings only.
- Real-material fixtures are committed under `crates/oep-step-pmi/fixtures/`
  (NIST MBE PMI Validation and Conformance Testing Project files, published
  test parts): two PMI-bearing and one PMI-free, stable for CI without any
  private corpus.
- step-io reads any mainstream AP through a universal-schema pipeline and
  navigates PMI through `Scene::tolerances()`, `Scene::datums()`,
  `Scene::dimensions()`. Modifiers and zone forms are not on the scene public
  surface, but are reachable through the raw model's ComplexUnit / arena
  structures, which is exactly why the raw-model part of the walk is kept
  private to this crate.
- The P6 importer hub / viewer consumers expect ONE canonical format; this
  crate emits it directly (an `oep_pmi::PmiDocument`), same as oep-step maps
  to `oep_geo::Document`.

## 4. Options considered

1. Wrap step-io's scene PMI surface + a private raw-model walk for modifiers
   and zone forms, mapped to `oep_pmi`. ADOPTED, this record. Zero native
   build, published permissive license, verified against real NIST AP242
   fixtures before this decision was locked, and the whole surface stays
   vendor-neutral behind the oep-pmi contract.
2. Bind Open CASCADE (OCCT) now for PMI. DEFERRED. No binding exists, CI has
   no C++ toolchain, and the P4-M1 ADR already records OCCT (LGPL-2.1 +
   exception) as the long-horizon analytic-fidelity path. step-io delivers
   the semantic wedge today; OCCT remains the recorded path for exact
   analytic B-Rep and any facet step-io drops. Revisit again if a future card
   requires an entity step-io's model does not carry.
3. Own hand-rolled ISO 10303-21 PMI parser. REJECTED, same reason as P4-M1:
   it duplicates years of parser engineering for zero ownership gain and
   violates the established "wrap an existing OSS STEP library" doctrine. It
   remains the escape hatch only if every OSS library becomes untenable.
4. Map dimensions to oep_pmi `Note`. REJECTED for v1: the Note class is free
   text; synthesizing note text from measured values would fabricate surface
   text. Dimensions are counted and reported so nothing is silently dropped.

## 5. Vendor-neutral seam

Same two-layer rule as P4-M1, applied to the PMI lane:

1. `oep-step-pmi`'s public API exposes only oep-pmi types and coined neutral
   types: `import`, `import_path`, `import_strict`, `PmiImport`,
   `PmiImportReport`, `DroppedItem`, `Error`. Signatures never name step-io,
   OCCT, or any engine type.
2. `src/import.rs` is the ONLY module that imports `step_io::...`. Every
   step-io-specific identifier (scene cursor, ComplexUnit, arena, EntityKey
   Debug spelling) is private to that file. The acceptance test asserts the
   OSS identifiers appear nowhere in `lib.rs` / `error.rs`.

A future engine swap (section 9 of the P4-M1 ADR) is then a rewrite of this
single private walk behind an unchanged public API.

## 6. Failure and tolerance posture

- Typed errors only: `Error::Io` (path read), `Error::NotStep` (missing
  `ISO-10303-21` banner), `Error::Parse` (the OSS parse stack failed),
  `Error::NoPmi` (strict import of a PMI-less file). Never a panic.
- `PmiImportReport.pmi_present` is false exactly when the file has no real PMI
  signal (no tolerance, datum, or dimension). The document returned with that
  flag is a valid EMPTY `PmiDocument` (validates clean), so the distinction
  is observable and testable, never papered over.
- Every tolerance the walk cannot map (closed-set miss, absent magnitude,
  non-finite value, unresolved datum) is a `DroppedItem` with a reason, plus
  raw scene warnings kept verbatim in the report. Nothing is invented.

## 7. License

- step-io is MIT OR Apache-2.0 with a single `logos` dependency, unchanged
  from the P4-M1 assessment: fully compatible with the program's Apache-2.0
  applications/libraries posture; `oep-step-pmi` stays Apache-2.0 with no
  LGPL obligation today.
- OCCT (the deferred engine) is LGPL-2.1 + OCCT-exception; that posture is
  already assessed in the P6-M3 boolean-ops ADR and reviewed by P0-M2 before
  any release that links it. No dependency added here changes a crate's
  declared license.

## 8. Clean-room

step-io is a published OSS library; the NIST MBE fixtures are published test
parts (U.S. government source, real PMI). Both are lawful public sources. No
vendor CAD binary is disassembled and no vendor PMI schema is embedded: the
oep-pmi schema is clean-room and documented in `docs/p6-m1-oep-pmi-model-schema.md`.

## 9. Guardrails

- Version pin: `oep-step-pmi` pins `step-io = "=0.2.5"` (same supply-chain
  doctrine as P4-M1). Bumps are deliberate, reviewed changes that re-run the
  golden tests.
- Real-material gate: integration tests run on the three committed NIST AP242
  fixtures (PMI-bearing x2, PMI-free x1) and assert: non-zero mapped
  tolerances/datums/frames with `validate() == []`, MMC and diameter-zone
  captured on the position tolerances, deterministic round trip, `pmi_present
  == false` + typed `NoPmi` on the PMI-free fixture, and typed errors on junk.
- Seam denial in CI: the acceptance test asserts no OSS identifiers on the
  public surface.
- No fabricated PMI: absence is flagged, unknown characteristics are dropped
  with reasons, dimensions are counted not turned into fake notes.
- This decision is revisited only by a version bump with a change-log line.

## 10. Consequences

- A real, CI-green semantic PMI read lane for wave 2: two real AP242 parts
  map into the canonical oep-pmi model and the PMI-free part is honestly
  flagged, feeding the importer hub and future viewers/checkers.
- The platform's own PMI model stays sovereign; no foreign STEP library type
  reaches the public surface.
- OCCT stays on the road as the analytic-fidelity path without blocking this
  milestone, and the escape hatch is documented.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P4-M2-T01. Resolves the
  P4-M2 gate revisit mandated by workstreams/P4-M1/step-library-adr.md: keep step-io
  (pinned =0.2.5) as the P4 STEP base, build oep-step-pmi on its scene PMI
  surface plus a private raw-model walk for MMC/LMC modifiers and zone forms,
  map onto oep_pmi, and honestly flag PMI-less files. OCCT remains the
  deferred analytic-B-Rep fidelity path.
