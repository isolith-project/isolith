---
title: "Isolith Scope Register: CATIA/SolidWorks Editor Parity Baseline"
short_title: scope-register
version: 1.1.0
decision: editor_spine_parity_baseline
date: 2026-10-22
status: Ratified (decision #13 scope expansion, 2026-10-18; decision #19 editor spine, 2026-10-22)
owner: Isolith program governance (editor spine wave)
applies_to: the whole product surface (editor workstream, geometry core, kernel seam, modeler verticals)
supersedes: the Bucket C exclusion list (master plan scope register, pre-decision-#19 reading)
---

# Isolith Scope Register: CATIA/SolidWorks Editor Parity Baseline

Decision #19 set the product bar: same-or-greater editor functionality than
CATIA/SolidWorks for the working engineer. This register stops being an
exclusion list and becomes the **PARITY BASELINE inventory** an engineer can
read to judge whether Isolith covers their main job. Every named capability
is tracked with its CATIA status, SolidWorks status, cost class, owning
workstream, and a current-state token.

**Why is a row planned?** Each row's `planned` status is a decision about the
competitive landscape, not a preference: `docs/competitive/parity-2026-10.md`
is the feature-for-feature evidence against the 2026-10 clone wave
(SolidCraft, Purrametric, CADCraft, Open CAD Studio, MitCad) and the
incumbent bar (Fusion, SOLIDWORKS, CATIA), capped by this register. Read it
before re-sequencing a row or answering "why is this planned and not
building".

The competitive copy is refreshed quarterly
(`docs/competitive/README.md`), so the matrix stays the standing answer to
that question between refreshes.

The machine-checkable copy is `docs/scope-register.json`; this document is the
human rendering and must stay in sync with it. The drift guard
(`python/oep_tooling/scope_register_check.py`, acceptance
`tests/test_scope_register.py`) fails a row that claims a status higher than a
green accepted test proves (the same honest-claims discipline as the format
matrix, P13-T03).

## Status tokens

- `not-scoped` - name is in the baseline, no commitment to build it yet (the
  register keeps it visible so a missing capability is a decision, not an
  accident).
- `planned` - committed to scope, sequenced, an owner named; evidence is a
  recorded decision/ADR/card, no test yet.
- `building` - actively built; evidence is at least one green accepted test on
  main proving a slice of the capability.
- `shippable` - engineers can do the main job end to end; evidence is a green
  accepted test proving the capability against real material.

**Upgrade rule:** `not-scoped -> planned -> building -> shippable` in order,
and a row may only move up the ladder while a green accepted test proves the
new claim. Moving a row to `building` or `shippable` without citing an existing
green test fails the drift guard. This is how the register keeps honest claims
instead of marketing copy (decision #19: no fake dates, honest sequencing).

## Vendor status tokens (reference)

`base` ships in the standard product; `module` needs an add-on module or
workbench; `limited` is present but partial; `no` is not offered. These cells
are transcribed from the recorded Bucket C rationale (master plan scope
register, decision #13) and the CATIA V5/V6 ecosystem research vault note.
They are reference facts about the incumbent products, not Isolith claims; the
honest-claims rule binds the Isolith `status` column, not these.

## Register

| capability | CATIA | SolidWorks | cost class | owning workstream | status | evidence |
|---|---|---|---|---|---|---|
| 2d-drafting - 2D drafting / drawings (sheets, views, dimensions, title block, export) | base | base | class-1-additive | editor: drafting workbench (card EDITOR-2D-DRAFTING, running) | planned | decision #13/#19; card EDITOR-2D-DRAFTING |
| configurations-design-tables - Configurations / design tables (variant states over the feature tree) | base | base | class-2-model-extend | modeler: oep-geo model-extend (configurations as variant states / design tables; workstream not yet opened) | planned | decision #13 (2027.2 pull-forward) / #19; master plan 2027.2 scope |
| mass-properties - Mass properties (volume, mass, centroid, moments of inertia) | base | base | class-1-additive | editor: mass properties (card EDITOR-MASS-PROP, running) | planned | decision #13/#19; card EDITOR-MASS-PROP |
| interference-clash - Assembly interference / clash detection | base | base | class-1-additive | editor: assembly interference/clash (card EDITOR-MASS-PROP, running) | planned | decision #13/#19; card EDITOR-MASS-PROP |
| exploded-views - Exploded / assembly views | base | base | class-1-additive | editor: exploded views (card EDITOR-MASS-PROP, running) | planned | decision #13/#19; card EDITOR-MASS-PROP |
| sheet-metal - Sheet metal design | base | base | class-3-kernel-gated | kernel seam P6-M3-T04 (oep-engine-occ) then editor vertical; workstream opens after the seam lands | planned | decision #13/#19; cost class 3 (kernel-gated); P6-M3-T01 ADR v1.1.0 |
| weldments - Weldments (welded structures / frames) | module | base | class-3-kernel-gated | kernel seam P6-M3-T04 then editor vertical; workstream opens after the seam lands | planned | decision #13/#19; cost class 2/3 (model + kernel); P6-M3-T01 ADR v1.1.0 |
| mold-tooling - Mold tooling design | base | base | class-3-kernel-gated | kernel seam P6-M3-T04 then editor vertical; workstream opens after the seam lands | planned | decision #13/#19; cost class 3 (kernel-gated); P6-M3-T01 ADR v1.1.0 |
| routing - Piping / tubing / routing | base | base | class-2-model-extend | modeler: routing vertical (opens after the kernel seam; class-2 model-extend) | planned | decision #13/#19; master plan 2027.2 scope |
| electrical-harness - Electrical / harness (3D wire routing, flattening, harness design) | base | base | class-2-model-extend | modeler: electrical/harness vertical (2027.2+ workstream, not yet opened) | planned | decision #13/#19; master plan 2027.2 scope |
| composites - Composites (laminate definition / ply handoff) | base | module | class-2-model-extend | modeler: composites evaluation (2027.2 review gate; no committed workstream) | planned | decision #13 (in scope, evaluated 2027.2+); master plan Bucket C |
| cam - CAM / machining / toolpaths | base | base | class-1-additive | new vertical P15+ (CAM / machining decomposition; not yet opened) | planned | decision #13 (2027.2+ workstream) / #19; master plan 2027.2 scope |
| fea - Simulation / FEA / structural analysis | module | base | class-1-additive | new vertical P15+ (simulation / FEA decomposition; not yet opened) | planned | decision #13 (2027.2+ workstream) / #19; master plan 2027.2 scope |
| class-a-surfacing - Class-A / ICEM-class surface design | base | limited | class-4-different-foundation | SURFACING-CLASS-A program (card running; strategy ADR is the next gate) | planned | decision #19 (moved INTO scope from day one, full-send); card SURFACING-CLASS-A |

Cost classes (from the master plan): `class-1-additive` new crates consuming
`oep-geo`; `class-2-model-extend` extends the versioned `oep-geo` schema (a
version bump + migration); `class-3-kernel-gated` blocked on the kernel seam
P6-M3-T04 (`oep-engine-occ` behind the `oep-geo` Evaluator seam);
`class-4-different-foundation` needs a high-accuracy surface kernel (Class-A).

## Practitioner input contract (LTT practitioner test)

Feature lists do not prove the editor bar (decision #19: same-or-greater
editor functionality than CATIA/SolidWorks). What proves it is the input and
behaviour contract an experienced user carries in their hands - and the LTT
practitioner test of SolidCraft and Purrametric is a free specification of
exactly those expectations (Don directive 2026-10-27). This section tracks one
row per behaviour with an explicit test hook. The status tokens and the
honesty rule are identical to the capability register above: a behaviour row
may only claim `building` or `shippable` while a green accepted test on main
exercises that behaviour, and every row names the seam (`test_hook`) that will
prove it. Machine-checkable copy lives in the
`practitioner_input_contract` block of `docs/scope-register.json`, drift-guarded
by the same tests.

| behaviour | CATIA | SolidWorks | cost class | owning workstream | status | evidence |
|---|---|---|---|---|---|---|
| input-command-search - Command search on a single key (SolidWorks S key, Fusion search) plus a keyboard reachability path to any command | base | base | class-1-additive | shell: oep-shell command surface (P7-M5 command registry, search + generated help index) | building | green test: crates/oep-shell/tests/practitioner_input_contract.rs |
| input-right-click-wheel - Right-click radial menu or equivalent recent/dimension wheel | base | base | class-1-additive | editor bar context surface (no committed context-menu layer yet) | building | green test: crates/oep-app/tests/constraints.rs (context menu applies constraints) |
| input-spacebar-accept - Spacebar accepts/confirms the active tool | base | base | class-1-additive | editor bar active-tool model (no active-tool state in EditorSession yet) | planned | Don directive 2026-10-27; LTT practitioner test of SolidCraft and Purrametric (verified episode findings) |
| input-snap-inference - Two-axis snap inference during a drag: coincident plus midpoint and perpendicular/extension snaps | base | base | class-1-additive | sketch: drag-snap resolver over oep-geo points/segments (not yet opened) | building | green test: crates/oep-app/tests/snap_inference.rs |
| input-escape-semantics - Escape semantics: one press clears the active tool, measured and consistent | base | base | class-1-additive | editor bar active-tool model (no active-tool state in EditorSession yet) | building | green test: crates/oep-app/tests/sketch_draw.rs (escape ladder) + sketch_enter.rs |
| input-cut-region - Cut-region selection from a sketch (choose which enclosed region an extrude consumes, without editing the sketch) | base | base | class-2-model-extend | editor feature vertical: region pick above the sketch profile (not yet opened) | planned | Don directive 2026-10-27; LTT practitioner test of SolidCraft and Purrametric (verified episode findings) |
| input-datum-plane-legibility - Datum plane legibility: labelled without hover, not stacked into a white wash | base | base | class-1-additive | viewer: datum-plane render + label pass (no datum-plane entity in the tree yet) | building | green test: crates/oep-app/tests/sketch_enter.rs |
| input-toolbar-customisation - Toolbar and tab customisation (add a tab or move a command to a missing workspace) | base | base | class-1-additive | shell: workbench layout registry (no toolbar/tab model - commands are a flat registry today) | planned | Don directive 2026-10-27; LTT practitioner test of SolidCraft and Purrametric (verified episode findings) |
| input-undo-redo-parity - Single-press undo/redo parity and a visible undo step boundary per committed dimension | base | base | class-1-additive | shell: oep-shell EditorSession history (undo/redo/journal over canonical document snapshots) | building | green test: crates/oep-shell/tests/practitioner_input_contract.rs |

## Commitment order (decision #19, near term)

1. **First:** drafting / 2D drawings, mass properties (plus interference/clash
   and exploded views), and interactive **assembly mate editing** in the editor
   surface - the last is the editor spine itself (live `oep-assem` solver made
   interactive), not a Bucket C capability row. These three close the "where is
   my print / does it clash / edit the assembly" trio for the working engineer.
2. **Next:** the kernel-gated verticals (sheet metal, weldments, mold tooling,
   routing) - they open only after P6-M3-T04 (`oep-engine-occ`) lands on main
   behind the `oep-geo` Evaluator seam, because they all rest on real B-Rep ops.
   Electrical/harness, CAM, and FEA follow as 2027.2+ workstreams (CAM and FEA
   each get their own P15+ decomposition when opened). Composites carries an
   evaluation gate at 2027.2 and is the weakest commitment in the baseline.
3. **Class-A surfacing** is its own long-horizon kernel-moat program
   (decision #19, full-send): in scope from day one, gated by its own strategy
   ADR before any decomposition (card SURFACING-CLASS-A).

## Sync rule

`docs/scope-register.json` and this file must agree. The drift-guard test
fails if a capability's (or practitioner behaviour's) status in the JSON is
not reflected in its table, or if a row claims
`building`/`shippable` without a cited green accepted test in the tree. Add or
move a capability or behaviour by editing BOTH the JSON source of truth and
this table, then run `tests/test_scope_register.py` and the full gate.

## Change log

- 1.1.0 (2026-10-27): practitioner input contract section added (card
  t_5b00868e, Don directive 2026-10-27; LTT practitioner test of SolidCraft /
  Purrametric). Nine input/behaviour rows, each with a test hook, under the
  same status ceiling as the capabilities. `input-command-search` and
  `input-undo-redo-parity` are `building` with real green tests on main
  (crates/oep-shell/tests/practitioner_input_contract.rs). Escape semantics,
  drag-snap inference, cut-region selection, datum-plane legibility, toolbar
  customisation, right-click wheel and spacebar accept stayed `planned` with
  the reason in each row - none of them is reachable without new editor code,
  so none claims a test it does not have. Scope-register schema bumped to 2
  with a tolerant migration (schema-1 registers still validate).
- 1.0.0 (2026-10-22): parity baseline register committed (card
  FEATURE-BASELINE t_a686d2b9). All 14 Bucket C capabilities tracked with
  CATIA/SolidWorks status, cost class, owning workstream, status token, and
  evidence; near-term commitment order recorded per decision #19. Every row is
  honest at `planned` - none are `building`/`shippable` because no capability
  in this baseline has a green accepted test on main yet.
