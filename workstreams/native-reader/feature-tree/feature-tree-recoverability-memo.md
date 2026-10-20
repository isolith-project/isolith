---
title: Keelworks Feature-Tree Recoverability Decision Memo (full recovery vs imported-dumb-solid vs PMI-only)
decision: import_b_rep_as_dumb_solid_extract_feature_history_as_honestly_marked_metadata_do_not_promise_full_replay
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks P5 native-reader lane (wave 2), P5-M4-T01
applies_to: the feature-tree recoverability question in the native CATPart/CATProduct reader lane (P5-M4), the B-Rep extraction lane (P5-M3) it sits beside, the oep-geo feature-graph model (P6-M3-T03) that would consume any recovered history, and the P6-M4-T02 importer hub
supersedes: nothing (first feature-tree recoverability decision memo in the P5 native-reader lane)
---

# Keelworks Feature-Tree Recoverability Decision Memo

This document is the written deliverable for P5-M4-T01: assess whether the
parametric feature tree of a native CATPart is recoverable from the saved
bytes, and decide among the three named outcomes with an options matrix. It is
a decision-support memo that feeds the P5-M5-T01 Don gate; it does not gate
the P5-M3 B-Rep lane. It is versioned like the alpha-target ADR (P5-M1-T03),
the boolean-ops decision (P6-M3-T01), the renderer decision (P6-M5-T01), the
step-library decision (P4-M1-T01), and the JT import decision (P4-M4-T02);
any material change bumps the version in the front matter and adds a line to
the change log at the bottom.

Every claim below is grounded in observation of real corpus material and
committed program artifacts. No vendor binary was executed, no vendor source
was read into the tree (the program clean-room invariant). Where byte
evidence does not prove meaning, the claim is marked [uncertain].
Recoverability is assessed on evidence, never on a promise.

## 1. Assessment

Question: can the parametric feature tree (the modeling history: feature
instances, their parameters, and their replay) be recovered from a native
CATPart byte stream, on the corpus the program actually holds?

### 1.1 What is observable: feature-tree structure

The feature-graph structure is present in the saved bytes and readable. A
fresh census on the resolved native corpus (123 real CATPart files from the
P1-M4 shared corpus mirror, P5-M4 verification run, 2026-10-16) found
length-prefixed standalone feature-instance records with an enumerable type
vocabulary. Record counts per type (feature instances sitting on a
length-prefix boundary, prefix byte = strlen + 1):

  Sketch            123   (every part)
  _FtrList          123   (every part)
  MechanicalPart    123   (every part)
  CurrentFeature    123   (every part)
  Pad               106
  Pocket             81
  Hole               69
  Mirror             39
  Shaft              26
  Chamfer            13
  Groove              7
  Slot                4
  Draft               2
  Rib                 1
  Shell               1
  Total length-prefixed feature records: 841

Feature and attribute-name tokens appear as readable ASCII dictionaries tied
to those records. Attribute-key census (all-parts aggregate):

  Value           30287   FirstLimit      123
  Layer            3198   SecondLimit     123
  Angle            2269   SolveManager    123
  Length           1887   Pocket_EndLimit 82
  Radius           1778   Hole_Depth      70
  Offset            795   Hole_BottomType 70
  Depth             456   GSMPlaneValues  41
  Hole_Diameter     140   GSMPlaneOffset  41
  MergeThin         123   EdgeFillet_Radius 27
  ResultOUT         123

The P5-M1-T02 observability report reached the same conclusion at high
confidence: the parameter graph (feature instances, names, attribute keys) is
observable in 114/114 parts, serialized in creation/file order with an
enumerable type vocabulary. So the tree's shape, its feature names, and the
attribute-key schema are recoverable as byte-grounded facts. This is not in
doubt on this corpus.

### 1.2 What is uncertain: parameter values and replay semantics

The numeric parameter VALUES are not plain ASCII. A corpus-wide scan found
binary IEEE-754 double values within a few bytes of the value-keyword family
(Value/Offset/Depth/Layer/Length/Radius/Angle) in all 123 parts (19774
plausible double hits total), and the key-to-value binding runs through a
"Value" indirection idiom rather than an ASCII "key = number" table. The
P5-M1-T02 report marks this [uncertain]: exact numeric parameter recovery
needs a binary value-record decode whose layout is structurally anticipated
but not yet derived (C9 medium). The dependency edges (which feature consumes
which) and the replay semantics (how a feature's output is derived) are NOT
readable as strings: parent/ref structure is not name-tagged the way the CGR
rep-graph is. The p5-m2-t02 byte-bisect tooling confirms that when a feature
name or attribute changes, the changed bytes sit in named feature-stream
regions with 16-hex stream identifiers, but back-mapping a changed range to a
parsed entity is exactly the reader work that P5-M3 owns. Cross-writer and
cross-era stability is untested: the whole corpus is one writer generation
(V5R29SP0HF0, build 10-11-2018.20.00), and the R2026 drift leg is honestly
reported blocked_on_sample by P5-M1-T01.

### 1.3 Industry and legal signals

The independent commercial-reader audit (P1-M3-T02) shows every vendor that
reads CATPart/CATProduct (HOOPS Exchange via CGM, CAD Exchanger, TransMagic,
Okino) reads B-Rep, tessellation, assembly structure, and PMI, and NONE
advertises feature-tree / history reconstruction as a deliverable. That is a
functional-spec signal: the industry-read entity class for native CATIA files
is exact geometry plus annotations, not a replayable history. The patent
posture memo (P0-M2-T02) flags JP7521894B2, "Extracting feature trees from
meshes," as live Dassault claim territory adjacent to feature-tree recovery,
and requires a claim screen before any history-reconstruction code is
written. The alpha-target ADR (P5-M1-T03) already ratified B-Rep/geometry
first and scoped feature-tree semantics to this memo as a research lane whose
outcome is evidence, not a commitment made at alpha time.

### 1.4 Bottom line of the assessment

Feature-tree STRUCTURE (names, types, order, attribute schema) is recoverable
at high confidence on this corpus. Feature-tree VALUES need a binary
value-record decode that is plausible and structurally anticipated but
[uncertain]. Full CATIA-identical replay semantics (dependency closure,
solver state, knowledgeware behavior) are not observable and would require
reimplementing proprietary modeler semantics that the program does not target
(the oep-geo feature graph P6-M3-T03 is explicitly NOT CATIA semantics). Any
recovered history can therefore be a faithful feature-history METADATA record
and an approximate, honestly-marked replay, never a claim of CATIA parity.

## 2. Options matrix

Scores are qualitative ratings on the evidence above, not fabricated numbers:
High / Medium / Low. "Observability" is how much of the feature tree the byte
evidence proves recoverable. "Legal / clean-room" is exposure on the patent
posture and the clean-room invariant (Higher score = safer).

| Option | Observability (evidence) | Effort / risk | Legal / clean-room | Downstream value | Verdict |
| --- | --- | --- | --- | --- | --- |
| Full recovery | Medium: structure High, values [uncertain], replay semantics not observable | High: binary value-record decode + dependency inference + own replay engine | Lower: adjacent to JP7521894B2; claim screen required | High: editable native-feature fidelity for the modeler lane (P7) | Defer: only if values/replay prove recoverable; currently a research question, not a deliverable |
| Imported-dumb-solid | High for geometry (P5-M3 B-Rep target); feature tree not recovered, part arrives as a non-parametric solid | Low: reuses the B-Rep extraction lane, matches every commercial reader | Higher: exact-geometry extraction from observed bytes, industry-normal | High: unblocks STEP export, PMI association, booleans, importer hub; the alpha differentiator stays exact geometry | Adopt as the shipped native-reader surface |
| PMI-only | Not observed as a native feature in this corpus; PMI arrives via geometry association (P4-M2 precedent) | Low: annotation-only reader | Higher | Medium: MBD wedge value, but forfeits the native geometry differentiator | Reject for the native lane; PMI is already served by P4 fences against geometry |

Reading the matrix: full recovery is the only option that preserves
edit-history fidelity, and it is exactly the option the evidence does not yet
support as a deliverable (values [uncertain], replay semantics not
observable, live claim adjacency). Imported-dumb-solid is the dumb-solid
import (geometry read as a non-parametric solid, industry-normal) that the
ratified alpha already targets. PMI-only is the narrowest surface and
duplicates the P4 annotation path.

## 3. Recommendation

Recommend imported-dumb-solid as the shipped native-reader surface, with a
best-effort feature-history metadata extraction added, honestly marked. Full
recovery is deferred to a claim-screened research spike, not promised; PMI-
only is rejected for the native lane.

Concrete shape:

1. The native reader ships the B-Rep / exact geometry of a CATPart as a
   non-parametric solid (the P5-M1-T03 alpha target), verified against real
   material by the P5-M3-T02 accuracy harness. This is the entity class every
   commercial reader supports and every downstream consumer (STEP export,
   PMI association, booleans, importer hub) consumes.
2. Add a feature-history METADATA extraction on the same pass: the observable
   feature tree (names, types, order, attribute-key schema) mapped onto the
   oep-geo feature graph (P6-M3-T03) as a read-only history record. Values
   that decode cleanly are included and marked recovered; the rest are marked
   [uncertain] or omitted. This costs little (the structure is already
   readable at high confidence) and gives PDM/metadata and the modeler lane a
   truthful history surface without pretending replay parity.
3. Full replay recovery (feature values fully decoded and replayed through
   an own feature-graph engine) stays behind a claim-screen and a scoped
   spike, gated on the P5-M3-T02 geometry accuracy harness proving the B-Rep
   ground truth first. It is a decision for the P5-M5-T01 Don gate, made on
   this memo's evidence, not a blocker on the B-Rep lane.
4. PMI on native files follows geometry, matching the P4-M2 precedent: after
   the geometry lane lands, native PMI association becomes a later milestone.
   It is not a substitute for the native reader, it is a follow-on.

Recommended because: the evidence supports a geometry-first, honest-metadata
native reader today, unblocks every downstream consumer, sits lowest in the
patent/clean-room screen, and reserves the full-recovery question for the
P5-M5 gate where it can be decided on spike results instead of on a promise.

## 4. Evidence

All evidence is observation-based, from real corpus material and committed
program artifacts.

1. P5-M1-T02 observability report (docs/re-notes/p5-m1-t02-native-catpart-
   observability.md): parameter graph observable at high confidence; numeric
   values binary; B-Rep payload present, coordinate-consistent; record-level
   semantics [uncertain].
2. P5-M4 verification run (this memo, 2026-10-16): committed observer
   python/oep_tooling/feature_tree_observe.py and evidence
   corpus/obs/feature-tree-observation.json on 123 resolved native
   CATPart files: 841 length-prefixed feature records across 15 types;
   attribute-key census above; 19774 plausible IEEE-754 value hits near the
   value-keyword family in 123/123 parts; window dumps of
   Pad/Pocket/Hole/Sketch/_FtrList records showing the readable ASCII schema
   dictionaries.
3. P5-M2-T02 observation / byte-bisect tooling (python/oep_tooling/obsdiff.py,
   corpus/obs/): labeled in-place edits recovered byte-exactly; sibling-pair
   bisects locate feature-stream string regions (Prism_ThickThin2,
   EdgeFillet_SharpEdgeRemoval, 16-hex identifiers) [uncertain back-map].
4. P1-M3-T02 commercial-reader capability audit: every vendor reads CATPart
   B-Rep / tessellation / assembly / PMI; none advertises feature-tree or
   history reconstruction.
5. P0-M2-T02 patent posture memo: JP7521894B2 "Extracting feature trees from
   meshes" is live Dassault claim territory; feature-tree research must be
   claim-screened; exact-geometry methods are largely expired public art.
6. P6-M3-T03 feature-graph model (oep-geo feature_graph.rs + doc): our own
   open mini feature tree with deterministic replay, explicitly NOT CATIA
   semantics, with a kernel-agnostic Evaluator seam; the natural target for
   any honestly-marked recovered history.
7. P5-M1-T03 alpha-target ADR v1.0.0: B-Rep/geometry first ratified;
   feature-tree semantics deferred to this memo with the three-option matrix
   as the P5-M4 deliverable; two lanes converge only at the P5-M5 Don gate.
8. P5-M1-T01 drift report: all 131 native rows are one writer generation
   (V5R29SP0HF0, build 10-11-2018.20.00); R2026 leg blocked_on_sample.

Honesty note: where a claim above is a hypothesis rather than a byte decoded
fact, it is marked [uncertain] in the source artifacts; this memo does not
turn an observation into a promise.

## 5. Guardrails

- Claim screen: before any feature-history reconstruction code, the P5-M4
  lane screens against the live Dassault feature-tree claims (P0-M2-T02); the
  memo is the release gate, not a promise.
- Honest marking: recovered history is labeled recovered vs [uncertain] vs
  omitted on the surface and in the model; nothing is labeled CATIA exact
  unless the independent reference harness measured it; tessellated-byproduct
  and non-replayable metadata are marked as such.
- Tolerant never-crash: unknown feature kinds and unreadable value records
  are skipped or marked, never fatal; decode failure degrades that record,
  not the part (P2-M1-T04 model applied to P5).
- No fabricated geometry: a failed decode yields a typed error and an honest
  empty, never substitution geometry or invented parameter values.
- Test gate: the metadata extractor lands behind a golden corpus gate on real
  native samples (feature-name lists, ordering, key schema), and the B-Rep
  accuracy harness (P5-M3-T02) pins a measured epsilon before anything is
  claimed.
- Surface rule: no em dash, no curly quote in program prose, enforced by the
  acceptance test.
- Revisit: this memo is revisited at the P5-M5-T01 Don gate; any change of
  heart on full recovery is a version bump with a change-log line, not an
  unrecorded drift.

## 6. Sequencing

- P5-M3-T01 (alpha, on the ratified ADR): B-Rep extraction; this memo does
  not gate it.
- P5-M3-T02: geometry accuracy harness; the honest pre-condition for trusting
  the geometry, and for any later full-recovery spike.
- P5-M4-T01 (this memo): recoverability assessed on evidence; options matrix
  and recommendation as delivered.
- P5-M5-T01: the Don gate receives the B-Rep alpha status and this memo
  together, so the two-year go/no-go (continue / pivot to PMI-annotation-only
  / reprioritize) is decided on evidence from both lanes.

## 7. Change log

- 1.0.0 (2026-10-16): Initial ratification for P5-M4-T01. Assesses
  feature-tree recoverability on real corpus observation (structure
  recoverable at high confidence, values [uncertain], replay semantics not
  observable), delivers the options matrix (full recovery vs imported-dumb-
  solid vs PMI-only) with a scored table, and recommends imported-dumb-solid
  as the shipped surface plus best-effort feature-history metadata, with full
  recovery deferred to a claim-screened spike decided at the P5-M5 Don gate.
