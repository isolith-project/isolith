# P6-M4-T02 — Importer hub: P2/P3/P4 adapters feed `oep-geo`; one canonical format

Milestone: P6-M4-T02 (Keelworks wave 1 / P6). Program: Open Engineering
Platform. 2026-10-16. Integration card: this delivery defines the first
cross-crate `path` deps in the workspace (docs/layout.md), so the ONE canonical
internal format is consumed by every reader.

## What shipped

New crate `crates/oep-import` — the importer hub. It path-depends on
`oep-geo` (the dependency-free canonical model), `oep-cgr`, `oep-3dxml` and
`oep-step` (leaf reader crates), and maps each source into an
`oep_geo::Document` (`format="oep-geo"`, `version=1`):

- `SourceKind { Cgr, ThreeDXml, Step }` + `SourceKind::detect` — observed
  signatures (CGR magic + template, 3DXML ZIP/standalone, STEP ISO-10303-21).
- `ImportOutcome { Imported{document, report}, Skipped{source, reason}, Reserved{source, reason} }`
  and typed `ImportError` — non-panicking by construction, tolerant-failure
  doctrine on every input class (junk, truncated, unknown).
- P2 CGR adapter (`adapters::cgr`): deterministic `from_mesh` → one part + one
  Solid body whose bbox is the exact vertex bbox; `import_bytes` runs
  `oep_cgr::mesh::extract` and maps a confirmed mesh to the canonical document
  or a classified `Skipped` (never fabricates).
- P3 3DXML adapter (`adapters::xml3d`): `bom` container parse → manifest root
  model doc → `rep::parse_model` → nodes→parts, mesh `ReferenceRep` members →
  Solid bodies (bbox from vertices), owned via `InstanceRep` wiring; root
  product structure → Assembly + Instances with `RelativeMatrix` mapped to the
  canonical row-major 4x4 transform.
- P4 STEP adapter (`adapters::step`): registered seam returning a typed
  `Reserved` — `oep-step` is an empty skeleton in wave 1 (P4-M1-T01 not
  started); honest, never fabricated output.

## Verification (real material)

- Committed golden `crates/oep-import/tests/golden/cube1-import.oepgeo.json`
  generated from the real license-clean Abaqus **cube1.3dxml** corpus container
  (8/12 box, known-good geometry from two independent decoders):
  - 2 parts (`Abaqus_Model`, `Abaqus_Geometry`), 1 solid body with bbox
    spanning the ~10 cube, 1 assembly with 1 instance of Abaqus_Geometry at a
    canonical transform, `validate()==[]`.
  - Byte-locked two ways: Rust `crates/oep-import/tests/importer_hub.rs`
    (live import must reproduce the golden byte-for-byte + static structural
    lock) and stdlib-only Python
    `tests/test_oep_import_hub_acceptance.py` (independent re-parse of the
    committed golden). Real-material legs SKIP cleanly in CI when the private
    corpus is absent (OEP_3DXML_SAMPLES), matching repo convention.
- `cargo test -p oep-import`: 8/8 green with OEP_3DXML_SAMPLES set (live
  golden reproduction exercised on the real cube1.3dxml).

## [uncertain] / honesty notes

- 3DXML `RelativeMatrix` convention: parsed as 12 doubles row-major 4x3 with
  translation in the bottom row, mapped to the canonical row-major 4x4
  ([16], translation at elements 12/13/14, w=1) per P6-M1-T01. Pinned choice,
  cross-checked on cube1; a wider corpus pass may refine it.
- CGR adapter maps the reader's confirmed mesh to part/body. Many real CGRs
  yield a classified `Skipped` (the extractor's self-consistency scan finds no
  triangle stream under the tested clean-room hypotheses) — that is honest
  behavior, never a claim of geometry.
- STEP returns `Reserved` until P4-M1-T01 starts; no geometry is fabricated.

## Hotspots (flag for orchestrator)

- `crates/oep-3dxml/src/lib.rs` and `crates/oep-cgr/src/lib.rs` were merged
  from three/two sibling PR branches (additive `pub mod` unions) to build the
  hub; further additive-only edits wanted.
- `docs/layout.md` edited by this card AND sibling P6-M1-T02 (pmi schema):
  keep delta small.
- `Cargo.lock` regenerated after each dep merge (regenerate, never hand-merge).
