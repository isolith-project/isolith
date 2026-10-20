---
title: Isolith Native CATPart Reader Alpha Target (B-Rep / geometry first, feature tree deferred)
decision: alpha_targets_brep_exact_geometry_first_feature_tree_semantics_deferred
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Isolith P5 native-reader lane (wave 2), P5-M1-T03
applies_to: the native CATPart/CATProduct reader workstream (P5), its feed into the oep-geo canonical model via the P6-M4-T02 importer hub, the P5-M3 B-Rep extraction lane, and the P5-M4 feature-tree recoverability research lane
supersedes: nothing (first native-reader alpha target decision record; the master plan default for P5-M1-T03 is hereby ratified)
---

# Isolith Native CATPart Reader Alpha Target

This record is the binding product decision for the P5-M1-T03 card: what the
alpha target of the native CATPart/CATProduct reader is. The two candidate
targets are, on one side, the reader aiming at B-Rep and exact geometry
(topology plus NURBS/analytic surfaces) as its first delivered capability, and
on the other side the reader aiming at feature-tree semantics (the parametric
modeling history: pads, pockets, fillets, sketches, and their replay). The
master plan states the default as B-Rep/geometry first, with feature-tree
semantics deferred. This document ratifies that default and records the
evidence, options, sequencing, and guardrails that bind it.

The record is versioned like the boolean-ops decision (P6-M3-T01), the
renderer decision (P6-M5-T01), the step-library decision (P4-M1-T01), the PMI
import decision (P4-M2), and the JT import decision (P4-M4-T02); any material
change bumps the version in the front matter and adds a line to the change
log at the bottom.

## 1. Decision

The native CATPart/CATProduct reader alpha targets **B-Rep and exact geometry
first**: the first shipped capability is the extraction of topology (faces,
edges, vertices, shells, solids) and exact surface geometry (NURBS, analytic
surfaces, curves) from the observed CATPart geometry payload into the oep-geo
canonical model. Feature-tree semantics (the parametric/history model: the
feature graph, its parameters, and replay) are **deferred** as a documented
research lane (P5-M4-T01), not delivered in the alpha, and never promised in
the alpha surface.

Concrete shape of the decision:

1. Alpha ground truth is the "Shape 1" representation of a CATPart, observed
   in every part in the wave-1 corpus as the PRDREP default representation
   holding the `_BagRepsList`/`BagRep` geometry bag. The B-Rep extraction
   lane (P5-M3-T01) targets that payload: exact surface geometry and
   topology into `oep-geo`.
2. Feature-tree semantics are scoped to their own research card, P5-M4-T01,
   which produces a written decision memo with an options matrix (full
   recovery vs imported-dumb-solid vs PMI-only). The alpha does not wait on
   that memo, and the memo is not a blocker on the B-Rep lane.
3. Tessellated output remains a byproduct of the geometry lane (a mesh
   derived from the extracted B-Rep for viewer and CGR-parity use), exactly
   as the P2 CGR lane produces meshes. It is not the alpha target.
4. PMI on native files follows geometry, not the reverse: PMI associations
   in the AP242 and QIF lanes bind to B-Rep faces (P4-M2 precedent), so a
   geometry-enabled reader is the prerequisite for native PMI, which stays a
   later milestone.
5. Nothing from any vendor reader, commercial converter, or the observed
   byte stream beyond the sample files themselves is copied, linked, or
   vendored. This follows the clean-room line precisely as the P2/P3/P4
   readers do, and is restated in the Clean-room section.

## 2. Why this decision

Five facts drive it, each grounded in the evidence section.

1. **B-Rep is the entity class the rest of the platform consumes.** Every
   downstream consumer of the native reader works on exact geometry, not on
   history: STEP export/validation (P4-M3), PMI association (P4-M2), the
   geometry core (P6-M1 tolerance model, P6-M3 booleans), the importer hub
   (P6-M4-T02), and the modeler lane (P7). A reader that extracts B-Rep
   unblocks all of those. A reader that extracted feature-tree semantics
   alone would unblock almost none of them: no current consumer replays CATIA
   history.
2. **The industry signal is that CATIA native reading means B-Rep plus
   tessellation, never history.** The independent commercial-reader audit
   (P1-M3-T02) shows HOOPS Exchange, CAD Exchanger, TransMagic, and Okino all
   read CATPart/CATProduct B-Rep, tessellation, assembly structure, and PMI,
   and none advertises feature-tree/history reconstruction as a deliverable.
   HOOPS reads CATIA V5 B-Rep through its CGM kernel; TransMagic explicitly
   distinguishes B-Rep from the CGR Visrep tessellation; CGR is treated by
   every vendor as tessellation-only. Feature-tree recovery is simply not an
   entity class in the commercial reading surface, which is a strong
   functional-spec signal for what is readably observable.
3. **The saved geometry carries the B-Rep, and the feature graph does not
   have a reliable observable footprint in the formats we control.** The
   CATPart hex notebook (P1-M2-T03) shows the part self-labels MechanicalPart
   and carries a default representation ("Shape 1") with a geometry bag; the
   shape-record hypothesis (P1-M2-T05) assigns medium confidence to that
   bag as the exact-geometry carrier and flags every structural meaning
   [uncertain]. What is absent from observation is the authoring history: the
   corpus records feature values and catalog payload but no replayable
   feature graph with parameters. Recovering a proprietary history graph from
   saved bytes is a research problem of unknown yield, which is why the plan
   budgets P5-M4 to assess feasibility rather than promising it in alpha.
4. **Patent posture favors geometry over history.** The P0-M2-T02 patent
   memo flags JP7521894B2, "Extracting feature trees from meshes", as live
   Dassault claim territory, and recommends the feature-tree research be run
   through the same claim screen as the reader. Exact-geometry extraction
   from observable bytes, built on public-art kernels and expired-era
   geometry methods, sits lower in that screen. Choosing geometry first is
   therefore also the lower-risk clean-room position.
5. **Two-year scope is sequenced around it.** The native reader is inside
   the first two years (Don decision 2). The plan already splits P5-M3
   (B-Rep extraction) from P5-M4 (feature-tree recoverability research) and
   run them as separate lanes feeding a single P5-M5 Don gate. Ratifying
   B-Rep first turns that split from a schedule artifact into a decision, so
   the alpha can ship on the observable surface while the feature-tree
   question resolves as evidence, not as a blocker.

## 3. Evidence

All evidence is observation-based, from real corpus material and committed
program artifacts, with confidence marked where structural meaning is not yet
proven. No vendor tool executed, no vendor source read into the tree.

1. **P1-M2-T03, CATPart vs CATProduct container notebook (verified on 121
   real files: 114 CATPart + 7 CATProduct, all magic `V5_CFV2`, all writer
   `V5R29SP0HF0`).** Observed facts: every CATPart registers the class string
   `MechanicalPart` (114/114), every CATProduct registers `CATMechProdCont`
   (7/7), the tokens are mutually exclusive; every file carries one
   `ASMPRODUCT` root; parts carry a default representation token `PRDREP
   'Shape 1'` plus `BagRep`/`_BagRepsList` geometry-bag strings; products are
   composition graphs of logical references to parts' "Shape 1" with no
   embedded child file paths. This fixes the container-level target: the
   reader walks a part to its "Shape 1" bag for geometry.
2. **P1-M2-T05, per-format shape-record hypothesis (verified on the same
   corpus plus scratch samples).** Hypothesis, [uncertain] marked: the CATPart
   shape record is the default representation "Shape 1", i.e. the
   `_BagRepsList`/`BagRep` geometry block(s); `BagRep`/`_BagRepsList` held on
   all observed parts (158/158 across scratch corpora); exact B-Rep and
   tessellated geometry are candidates for what lives in that bag, meaning
   not yet proven at byte-decode confidence. The observation that upgrades
   CATPart geometry from medium to high is the P1-M2-T06 diff harness on the
   `BagRep` region, which the P5 observation tooling (P5-M2-T02) is staged to
   run.
3. **P1-M3-T02, commercial-reader capability audit (verified against vendor
   primary sources 2026-10-15).** Matrix: CATPart/CATProduct B-Rep = Y for
   HOOPS Exchange (CGM), CAD Exchanger, TransMagic, Okino; Tessellation = Y
   for all four; PMI = Y for Exchange/CX/TransMagic; Assembly structure = Y
   for all four; CGR = tessellation-only for all four. Every product reads
   native B-Rep and none reads parametric history. This is a functional-spec
   signal: the industry-read entity class for native CATIA files is exact
   geometry, and the alpha should match that class.
4. **P0-M2-T02, patent posture memo (verified 2026-10-15).** Flags
   JP7521894B2 "Extracting feature trees from meshes" as claimed territory
   related to feature-tree research and recommends claim-screening it;
   records that the native CATPart wall is undocumented schema plus trade
   secret rather than a clean patent map, and that exact-geometry methods are
   largely expired public art. Geometry-first is the lower-risk reading
   position.
5. **Corpus reality.** The wave-1 corpus holds 128 prism + 10 assembly + 6
   surface + 1 dataset entries (145 total in the P1-M4 manifest); the native
   surface sample `boat-hull.CATPart` (MIT, dev-only, sha256
   6d8040f1...) is a real freeform native part in the corpus, usable as a
   B-Rep extraction test target. A geometry-first alpha has real, license-
   clean, observed material to validate against today.

Honesty note: every structural meaning above that is a hypothesis is marked
[uncertain] in the source artifacts; this ADR does not turn an observation
into a claim. What is decided is the target and the order, not that the
geometry decode is already proven.

## 4. Options considered

1. **B-Rep/geometry first (ADOPTED).** The alpha ships exact topology and
   NURBS/analytic surfaces from "Shape 1" into oep-geo. Matches the
   industry-read entity class, unblocks every downstream consumer, and sits
   lowest in the patent/clean-room screen.
2. **Feature-tree/history first (DEFERRED to P5-M4 research).** Building the
   alpha around replaying the parametric history would require a feature
   graph that is not observed to have a reliable saved footprint, sits next
   to a live Dassault feature-tree-from-mesh claim, and would unblock no
   current consumer (nothing replays CATIA history). It is not rejected
   outright: the P5-M4 memo keeps full recovery as an option if the
   observation tooling proves it recoverable. It is deferred past the alpha,
   with the options matrix (full recovery vs imported-dumb-solid vs PMI-only)
   as the P5-M4 deliverable.
3. **Tessellation-first for the native reader (REJECTED).** The P2 CGR lane
   already delivers the fast visualization surface, and it treats CGR as
   tessellation-only everywhere in the industry. Re-targeting the native
   reader at tessellation would duplicate P2 and forfeit the native
   differentiator, which is exact geometry. Mesh output stays a byproduct of
   the geometry lane for viewer parity, not the target.
4. **PMI-first for the native reader (REJECTED for alpha).** The program's
   own precedent is that semantic PMI associates to B-Rep faces (P4-M2 from
   STEP, and the JT lane reuses the same oep-pmi model). PMI without an
   associated B-Rep is annotation floating in space. Geometry first is the
   prerequisite that makes native PMI possible later, matching the P4
   sequencing where PMI followed geometry.

## 5. Sequencing

This decision splits two lanes that the master plan already listed as separate
milestones, and does so explicitly so neither blocks the other:

- **P5-M3-T01 (alpha, on this decision):** B-Rep extraction attempt, topology
  and NURBS surfaces into the oep-geo native model, from the "Shape 1" bag
  observed in the corpus.
- **P5-M3-T02 (on this decision):** geometry accuracy harness vs an external
  converter reference, strictly test-only ground truth per the clean-room
  line (the same rule the P1-M3 audit applied to commercial capability
  claims).
- **P5-M4-T01 (deferred lane):** feature-tree recoverability assessed, with
  the written decision memo and options matrix (full recovery vs
  imported-dumb-solid vs PMI-only). This card consumes the P5-M2 observation
  tooling (save-modify-save diffs, no vendor binary execution).
- **P5-M5-T01:** the two-year Don gate receives the B-Rep alpha status and the
  feature-tree memo together, so the go/no-go is decided on evidence from
  both lanes, not on a promise about history recovery.

The B-Rep lane does not depend on the feature-tree memo; the feature-tree memo
does not block the B-Rep lane. They converge only at the P5-M5 gate.

## 6. Clean-room

The native reader is inside the program's standing clean-room invariant
(board.program.json: no vendor binaries, no copied source, no Dassault/Auto-
desk/Siemens libraries used or linked; lawful observation and public specs
only). Concretely:

- Observation surface: byte-level analysis of lawfully obtained sample files
  (the MIT mohanevs corpus, license-clean dev-only samples, future P5-M2
  user contributions after the P1-M1-T06 sweep). This is the same evidence
  lane P1 used, on which the P1-M2-T03 notebook is verified.
- Allowed derivations: reading and independently implementing from our own
  observations and public art; writing our own parser and geometry code;
  using commercial converter output strictly as test-only ground truth
  (P5-M3-T02), never as a source of format internals.
- Prohibited: copying/adapting/linking vendor source or runtime (no CGM, no
  HOOPS, no Dassault DLLs), decompiling vendor libraries or instrumenting
  vendor processes, using non-public or trade-secret knowledge, and
  reimplementing a live claimed method (e.g. JP7521894B2 feature-tree-from-
  mesh) as claimed. The feature-tree research lane is claim-screened before
  any history-reconstruction code is written.
- Honest marking: any reader capability that is tessellated-byproduct rather
  than exact analytic is marked as such on the surface, exactly as P4-M3
  marks tessellated faceted B-Rep and never claims exact. No fabricated
  geometry is ever emitted in place of a failed decode; the honesty budget
  rule from P4-M1 applies verbatim.

## 7. License

- All reader code authored by Isolith is Apache-2.0 (the decided posture
  for applications/libraries). No vendor or GPL code is linked, copied, or
  adapted.
- The corpus is dev-only reference material under the decided posture
  (Don decision 7): samples that cannot be redistributed are fine to develop
  against; only the ITAR/privacy sweep and the clean-redistribution boundary
  on the released artifact are hard lines. Native CATPart samples in the
  corpus (MIT mohanevs pack, boat-hull dev-only mirror) are clean under that
  posture.
- The P0-M2 legal pass and export-ITAR sweep gates apply before any public
  release; the native reader is internal until then, like every other core
  crate.

## 8. Guardrails

- Test gate: the alpha B-Rep extraction lands behind a golden corpus gate on
  real native samples (topology counts, surface types, bounding volumes
  within tolerance), and the geometry accuracy harness (P5-M3-T02) pins a
  measured epsilon against the test-only converter reference. No claimed
  capability ships without a real-material test.
- Tolerant never-crash: unknown blocks and unreadable sub-structures are
  logged and skipped, not fatal; decode failure degrades that body, not the
  part; typed errors instead of panics (the P2-M1-T04 model applied to P5).
- Honest marking: exact vs tessellated vs [uncertain] is stated on the
  surface and in the docstrings; nothing is labeled CATIA-exact unless the
  independent reference harness measured it.
- No fabricated geometry: a failed decode yields a typed error and an honest
  empty, never substitution geometry.
- Claim screen: before any feature-tree reconstruction code, the P5-M4 lane
  screens against the live Dassault feature-tree claims (P0-M2-T02) and the
  options memo is the release gate, not a promise.
- No em dash, no curly quote in program prose (surface rule), enforced by the
  acceptance test.
- Escape hatch documented: the seam into oep-geo means a future surprise
  (a maintained license-clean native geometry decoder, or an explicit
  commercial-path decision) can swap in behind an unchanged interface; the
  record mandates an explicit revisit at the P5-M5 Don gate.

## 9. Enforcement in the plan

- Workspace: the B-Rep extraction lands in the P5 lane and feeds oep-geo
  through the P6-M4-T02 importer hub, on the same canonical model the P2/P3/P4
  readers populate. The P5-M3-T01 card is the alpha cut with the section 5
  acceptance contract.
- CI: golden CATPart decode tests and the accuracy-harness gate join the
  existing pipeline (P0-M4-T01) and the precommit gate (P0-M3-T04), plus the
  no-vendored-foreign-source scan family.
- Corpus: native samples grow through the P5-M2 contribution pipeline under
  the decision-7 posture (dev-only reference material, never shipped),
  sweep-gated by P1-M1-T06 before entry.
- Review: this decision is revisited at the P5-M5 Don gate, and any material
  change of heart (e.g. feature-tree recovered and promoted to the alpha
  target) is a new version bump of this record with a change log line, not
  an unrecorded drift.

## 10. Consequences

Choosing B-Rep/geometry first and deferring feature-tree semantics:

- Gives the alpha a target that is observable, industry-backed, patent-
  lighter, and consumed by every downstream lane. The first native reader
  milestone is on the strongest evidence and the lowest clean-room risk.
- Defers native PMI and history to their proper lanes: PMI requires the
  associated B-Rep that geometry-first builds; history stays a research
  question that resolves at P5-M4 with an honest options matrix.
- Bounds the alpha risk: the observable surface is exact geometry, and where
  byte-decode confidence is medium, the P5-M2 diff harness and the P5-M3-T02
  reference harness upgrade it before anything is claimed.
- Keeps the two-lane split the plan already drew; this ADR makes it a
  decision instead of a schedule artifact, so neither lane blocks the other
  and the P5-M5 Don gate sees evidence from both.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P5-M1-T03. Decides the native
  CATPart/CATProduct reader alpha targets B-Rep/geometry first (the master
  plan default), defers feature-tree semantics to the P5-M4 research lane
  with its options matrix, weighs B-Rep-first against feature-tree-first,
  tessellation-first, and PMI-first, and records the evidence base
  (P1-M2-T03 container notebook, P1-M2-T05 shape-record hypothesis, P1-M3-T02
  commercial-reader audit, P0-M2-T02 patent/clean-room memo, corpus reality)
  plus guardrails and the two-lane sequencing into the P5-M5 Don gate.
