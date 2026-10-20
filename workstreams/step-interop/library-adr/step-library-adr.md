---
title: Isolith oep-step STEP Library Choice (P4-M1)
decision: wrap_a_pure_rust_oss_step_library_Behind_vendor_neutral_wrappers_now_occt_is_the_p4_m2_fidelity_path
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Isolith program governance, P4 STEP/QIF/JT lane
applies_to: oep-step (adopting crate), the P6 importer hub (oep-import feed), any future engine bindings (oep-engine-occ), and the P4-M2 semantic-PMI lane
supersedes: nothing (first P4-M1 decision record)
---

# Isolith oep-step STEP Library Choice

This record is the binding product decision for the P4-M1-T01 card
"oep-step crate skeleton over an OSS STEP library, vendor-neutral wrappers".
It answers the library-choice question: which existing OSS STEP library does
`oep-step` wrap, and how does it stay vendor-neutral? It is versioned like the
boolean-ops decision (P6-M3-T01) and the telemetry / release-train decisions;
any material change bumps the version in the front matter and adds a change-log
line at the bottom.

The decision: adopt a pure-Rust OSS STEP library (step-io 0.2.5) as the
P4-M1 wrap target behind oep-step's vendor-neutral seam, and record Open
CASCADE Technology (OCCT) as the fidelity path for AP242 semantic PMI in
P4-M2+. This matches the P1-M3 readers audit (Recommendation 3: "Adopt OCCT as
the P4 STEP base") at the fidelity horizon, while keeping today's P4-M1 build
CI-clean without a C++/FFI step that no current crate in the workspace wires.

## 1. Decision

- `oep-step` wraps the pure-Rust OSS STEP library `step-io` (MIT OR
  Apache-2.0, single `logos` lexer dependency, no native build step) behind a
  clean, vendor-neutral public API. step-io types and idioms never appear in
  any `oep-step` public item (section 5).
- `oep-step` parses an ISO 10303-21 STEP physical file (AP203/AP214/AP242
  mainstream by the library's universal-schema pipeline) into vendor-neutral
  facet carriers, then maps them onto the ONE canonical internal model,
  `oep_geo::Document` (the native part/body/feature model of P6-M1-T01).
- Open CASCADE Technology (OCCT) is NOT adopted as a linked dependency today.
  It is the documented fidelity path for the P4-M2 semantic-PMI milestone and
  for exact analytic B-Rep that step-io's facet-focused surface does not
  carry. The seam in section 5 is what lets the platform move to OCCT behind an
  unchanged `oep-step` interface.
- The wrap is a thin reader of the faceted / tessellated branch
  (SHELL_BASED_SURFACE_MODEL, FACE_SURFACE, POLY_LOOP): the exact surface and
  B-Rep branches and PMI are out of P4-M1 scope and are recorded as honest
  gaps, never fabricated geometry.

## 2. Why this decision

Three facts drive it:

1. The acceptance bar is "crate compiles; parses one golden STEP part into the
   oep-geo native model; 1+ TDD test green on real material; precommit gate +
   Gitea CI green". CI runs in a minimal `rust:1-slim-bookworm` container with
   no C++ toolchain and no OCCT; the clean-room reader work in this repo is
   deliberately Rust-first and std-lean (P6-M2 math is "own, std-only, f64").
   A C++/FFI OCCT bind exists nowhere in the workspace today, so a C++ STEP
   wrap is not buildable and not CI-gateable at P4-M1 time.
2. step-io is a genuine existing OSS STEP library (not our own parser): it is
   published, licensed MIT OR Apache-2.0, maintained, and verified against
   this repo's own golden part (the AP214 faceted box reads: one closed shell,
   12 faces, 12 loops, 36 points). Wrapping it is exactly the card's ask.
3. The vendor-neutral seam bounds the downside. The risk that worries a
   sovereign engineering platform is not "linking a parser", it is "marrying a
   foreign library so tightly the platform cannot survive a change of heart".
   The seam turns the wrap into a tie at parity of effort without surrendering
   the platform's own model, and keeps OCCT (or any later engine) a drop-in.

## 3. Context

- The adopting crate is `oep-step` (P4), which is the P4 STEP/QIF/JT read lane
  in the monorepo layout (`docs/layout.md`). It was a reserved skeleton; this
  card makes it real.
- The native model target is `oep_geo::Document` (P6-M1-T01): format
  `oep-geo`, version 1, deterministic serde JSON. The P6 importer hub
  (P6-M4-T02) is the composition root that adapts each reader's output to this
  ONE internal format; `oep-step` feeds that story as a leaf reader that maps
  facets onto the model directly.
- The golden material is the repo's own committed `box-reference.step`: an
  AP214 faceted unit box (SHELL_BASED_SURFACE_MODEL), written by the P6-M4-T01
  exporter. It is REAL material, committed, CI-stable without private corpus.
- The P1-M3 readers audit (2026-10-15) records OCCT as the strongest OSS STEP
  base, with the AP242 PMI fidelity gaps and the "write is AP214 by default"
  trap written into the P4-M2 budget. The P6-M3 boolean-ops ADR names a single
  pinned OCCT build as the future shared engine for booleans and STEP; that
  build does not exist in the workspace yet.
- The R1A/R1B private corpus is not committed; tests must stay green in CI
  without it. Committed golden fixtures are the accepted pattern.

## 4. Options considered

1. Wrap a pure-Rust OSS STEP library (step-io 0.2.5) behind vendor-neutral
   wrappers. ADOPTED, this record. Zero native build, published permissive
   license, verified on the golden part, small footprint. The cost is a
   facet-focused surface now, with exact B-Rep / AP242 PMI deferred.
2. Bind Open CASCADE (OCCT) as a C++ dependency now. DEFERRED to P4-M2 / the
   oep-engine-occ seam. OCCT is the strongest OSS STEP engine and the audit's
   recommendation at the fidelity horizon, and its LGPL-2.1 + exception is
   compatible with the program posture when a pinned FFI crate owns it (the
   P6-M3 boolean ADR already commits to that engine). But no OCCT binding
   exists in the workspace, CI has no C++ toolchain, and the P4-M1 acceptance
   bar must be green today. Deferring keeps the strongest engine on the road,
   not in the parking lot.
3. Own hand-rolled ISO 10303-21 parser. REJECTED. The card explicitly asks to
   WRAP an existing OSS STEP library. A hand-rolled tokenizer and EXPRESS
   entity walk would duplicate years of parser engineering for zero ownership
   gain and would violate the letter of the card. It remains the escape hatch
   only if every OSS library becomes untenable.
4. Link pythonOCC. REJECTED. pythonOCC is a heavy Python binding over OCCT; the
   core is Rust-first (decision 6 of the master plan) and pythonOCC would drag
   a Python runtime and a C++ engine into the core crate. It is useful as a
   validation harness later, never as a core dependency.

## 5. Vendor-neutral seam

The requirement this record places on the code is a two-layer rule:

1. `oep-step`'s public API exposes only oep-step and oep-geo types:
   `oep_step::parse`, `oep_step::to_document`, `oep_step::parse_to_document`,
   `FacetedModel`, `ReadReport`, `ReadOutcome`, `Error`. Signatures never name
   step-io, OCCT, or any engine type. Callers pattern-match against our
   carriers and the canonical `oep_geo::Document`, nothing else.
2. The wrapped library lives behind a private walk in `oep-step`'s `lib.rs`
   (the only module that imports `step_io::...`). Every OSS-specific identifier
   is private to that walk. `facets.rs` holds only coined neutral types.

A future swap (section 9) is then a rewrite of the private walk behind an
unchanged public API: the platform's own model is not surrendered to any
library's type system.

## 6. Failure and tolerance posture

Real STEP files are messy. The reader therefore returns typed
[`oep_step::Error`] variants (`Io`, `NotStep`, `Parse`,
`NoFacetedGeometry`), never a panic, and a [`ReadOutcome`] that can carry a
[`ReadReport`] of what was dropped or skipped with reasons. Nonstandard
references, un-modeled shells, non-faceted loops, and over-sized polygons are
recorded, not invented. Honest naming: `box-reference.step`'s product cluster
is dropped by the underlying library (its universal schema does not model
APPLICATION_PROTOCOL_DEFINITION); the part name falls back to the FILE_NAME
stem and the drop is visible in the report, never papered over.

## 7. License

- step-io is MIT OR Apache-2.0 with a single `logos` dependency. This is fully
  compatible with the program's `Apache-2.0 applications/libraries` posture:
  `oep-step` stays Apache-2.0 with no LGPL obligation today.
- OCCT (the future engine) is LGPL-2.1 with the Open CASCADE exception
  (SPDX OCCT-exception-1.0). That posture is already assessed in the P6-M3
  boolean-ops ADR (section 7): LGPL governs the separately distributed engine
  dependency, not our Apache-2.0 crates, and the required notice is confirmed
  by the P0-M2 legal pass before any release that links OCCT.
- No dependency is added that changes a crate's declared license. The P0-M2
  legal gate reviews this record again before the P4-M2 OCCT binding.

## 8. Clean-room

Wrapping step-io (a published OSS library) and later OCCT (published OSS
kernel, LGPL-2.1 + exception) does not violate the program clean-room
invariant: both are licensed public sources, not vendor CAD binaries and not
extracted from any vendor product. The invariant's operative surface is
"public specs, lawful observation, license-clean sources"; linking licensed
open source through its public API is within it. Our own reader work (CGR,
3DXML) stays observational on our own corpus, exactly as chartered.

## 9. Guardrails

- Version pin: `oep-step` pins `step-io = "=0.2.5"` in its manifest
  (supply-chain doctrine). Bumps are deliberate, reviewed changes, and a bump
  re-runs the golden byte-lock acceptance test.
- Seam denial in CI: the acceptance test asserts oep-step's public surface
  carries no underlying-library identifiers; the golden parse test is the
  seam test (vendor-neutral types in, vendor-neutral types out, real material).
- Escape hatch, documented: keeping the seam means OCCT (via a future
  oep-engine-occ FFI crate) or an own parser can replace step-io behind the
  unchanged oep-step API. This record mandates an explicit revisit at the
  P4-M2 gate, where the OCCT binding effort is compared against the AP242 /
  PMI fidelity requirement.
- Test gate: 1+ TDD acceptance tests run in CI on real material (the committed
  golden `box-reference.step`): parse to facets (shell/face/bbox facts),
  map to a canonical validating `oep_geo::Document`, deterministic round trip,
  and tolerant-failure typed errors on junk input. These glue tests also
  enforce the seam by construction.

## 10. Enforcement in the plan

- Workspace: `oep-step` is an existing Cargo member; this card adds its real
  reader, its oep-geo path dependency (leaf reader), and its golden fixtures.
- CI: the precommit gate (pytest + cargo test) and the Gitea lint/unit/
  integration pipeline run the new acceptance tests; `cargo clippy -D
  warnings` and `cargo fmt --check` keep the crate clean.
- Review: this decision is revisited at the P4-M2 gate. A future change of
  heart is a new version bump of this record with a change-log line, not an
  unrecorded drift.

## 11. Consequences

- Gives wave 1 a real, compiling, CI-green STEP read lane: one golden STEP
  part parses into the oep-geo native model, feeding the importer hub and the
  viewer.
- Keeps the platform's own model sovereign: no foreign STEP library type ever
  reaches the public surface.
- Defers exact analytic B-Rep and AP242 semantic PMI to the P4-M2 lane with
  OCCT, with the fidelity limits already recorded in the P1-M3 audit and the
  P6-M3 ADR.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P4-M1-T01. Decides to wrap a
  pure-Rust OSS STEP library (step-io 0.2.5) behind vendor-neutral wrappers,
  with OCCT recorded as the P4-M2 fidelity path; records options, seam,
  license, clean-room stance, and guardrails.
