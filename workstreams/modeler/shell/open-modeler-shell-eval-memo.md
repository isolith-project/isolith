---
title: Isolith Open Modeler Shell Decision (keep a thin own shell around Open CASCADE; do not acquire FreeCAD or the CADCraft lineage)
decision: keep_thin_own_modeler_shell_around_opencascade_reject_acquiring_freecad_or_cadcraft
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Isolith program governance, P7 modeler lane (P7-M1-T01)
applies_to: the P7 modeler lane (P7-M1 through P7-M5), the oep-geo canonical model that the shell hosts, the oep-engine-occ boundary, the wgpu renderer surface (P6-M5), the downstream cards P7-M2-T01/T02, P7-M3-T01/T02, P7-M4-T01/T02, and P7-M5-T01/T02, and the P8 PDM and P9 compute integrations
supersedes: nothing (first open modeler shell decision record; the master-plan working assumption that P7 builds FreeCAD workbenches is hereby replaced)
---

# Isolith Open Modeler Shell Decision (keep/acquire)

This record is the binding product decision for the P7-M1-T01 card: which
"open modeler shell" the platform models inside. The question is narrowly
drawn in the card: FreeCAD as the open modeler shell, the CADCraft lineage,
or a thin own shell around Open CASCADE Technology (OCCT). The answer shapes
everything downstream in the P7 lane, from the interchange workbench
(P7-M2-T01) to the power-user velocity and accessibility passes (P7-M5), and
it interacts with already-ratified program decisions (the oep-geo canonical
model, the wgpu renderer, Rust-first core, the hybrid license posture, and
the clean-room invariant).

The decision is versioned like the boolean-ops record (P6-M3-T01), the
renderer record (P6-M5-T01), the native-reader record (P5-M1-T03), and the JT
record (P4-M4-T02); any material change bumps the version in the front matter
and adds a line to the change log at the bottom.

## 1. Decision

Isolith KEEPS a thin own modeler shell around Open CASCADE Technology. That
shell is built in Rust as an extension of the P6 modeler host (the wgpu-based
oep-geo preview/app shell started in P6-M5-T02), displays and edits the
oep-geo canonical model directly, and reaches OCCT only through the
oep-engine-occ boundary ratified in the boolean-ops record. Isolith does NOT
acquire FreeCAD as the P7 modeler shell, and does NOT acquire the CADCraft
lineage, today.

Concrete shape of the decision:

1. The shell is our own application surface. It hosts oep-geo models (part,
   body, feature, assembly, instance), its viewport is the wgpu renderer, and
   its interactive surface is a Rust immediate-mode UI stack (for example egui,
   the permissive Rust UI used by the CADCraft lineage), with OCCT type
   identifiers and handles never crossing the oep-geo public surface (the
   existing seam rule, section 9 of the P6-M3 boolean record, applies to the
   shell verbatim).
2. The authoring truth stays oep-geo. Every P7-M2..P7-M5 capability is built
   on oep-geo objects: the "OEP workbench" of P7-M2-T01 is the shell's own
   panel/command surface that calls the P2/P3/P4/P6 crates directly; the
   assembly workbench of P7-M3 maps the oep-geo assembly model; the sketcher
   of P7-M4 stores its sketch and variable sets as oep-geo features; the
   drafting and export paths are the P6-M4 exporters (STEP, STL, glTF, OBJ,
   SVG, PDF).
3. Geometric constraint solving is ADOPTED as OSS libraries, not rebuilt.
   P7-M3-T01 already pre-decides this ("adopt an OSS solver lib, do not
   rebuild"). Verified fact: FreeCAD's 2D geometric constraint solver (the
   PlaneGCS line under src/Mod/Sketcher/App/planegcs, LGPL-2.1-or-later) has
   been extracted as a standalone LGPL library usable outside FreeCAD, and
   permissive Rust solver and UI crates exist. The concrete solver choice is a
   P7-M3-T01 / P7-M4-T01 deliverable with its own ADR; this record only binds
   the "adopt, do not rebuild" rule and the "no FreeCAD-module source copied"
   clean-room line.
4. FreeCAD stays as the documented acquire-escape hatch and as our behavioral
   and UX reference. We may read FreeCAD (its LGPL source and its public
   docs) to understand sketcher semantics, variable-set behavior, and power-user
   UX; we do not link, copy, or fork it into the shell, and we do not let its
   document model take over the authoring path. P7-M4-T01 keeps its wording
   "like FreeCAD v1 variable sets" as a semantics target, not as an import.
5. The CADCraft lineage is on watch, not in the build. If it ships a real 3D
   B-Rep parametric core (its own roadmap places parametrics and the 3D kernel
   well after the drafting alpha), this record revisits whether a pure-Rust
   host is worth adopting. Its run is also treated as evidence that the
   agent-driven, clean-room, decomposition-first build doctrine scales to CAD
   (section 4.4).

## 2. Why this decision

Five facts drive it, each grounded in the evidence section.

1. A single canonical model is a ratified program invariant, and acquiring
   FreeCAD breaks it at the worst seam. The program decided (P6-M1-T01, and
   restated in the boolean record: "our model, our serde schema") that oep-geo
   is THE model. FreeCAD is itself a feature-based parametric CAD application
   with its own document model, its own feature tree, and its own
   TopoShape-wrapped OCCT shapes. Using FreeCAD as the shell means the live
   authoring model is FreeCAD's, and oep-geo becomes an interchange format
   that must round-trip through a bidirectional seam. The seam is the problem:
   a native CATPart B-Rep (P5 alpha) arrives in FreeCAD as exactly the
   imported "dumb solid" that the P5-M4 options matrix flags, FreeCAD cannot
   edit it as a feature, and the oep-geo feature graph (P6-M3-T03) lives
   where FreeCAD cannot see it. The more the platform feeds oep-geo (readers,
   PMI, booleans, feature replay), the worse FreeCAD expresses it. An own
   shell has exactly one model, so the seam disappears by construction.
2. The renderer is already decided, and FreeCAD's viewport contradicts it.
   P6-M5-T01 ratified wgpu/WGSL, headless by construction, with a committed
   real-material spike, so that preview output and P2P render-job output are
   the same code path ("preview equals job output" as a property). FreeCAD's
   GUI viewport is Qt plus Coin3D (Open Inventor), a separate, windowed,
   non-headless render stack. Acquiring FreeCAD means running two renderers
   for authoring and preview, duplicating the P6-M5 viewer work already
   committed, and losing the single-codebase fidelity property that the
   renderer record exists to guarantee. An own shell's viewport IS the wgpu
   renderer (section 7).
3. Rust-first core and the clean-room / supply-chain posture cut both ways,
   and both favor the own shell. The own shell keeps the entire authoring path
   in Rust and Apache-2.0, with OCCT the only C++/LGPL component and already
   isolated behind oep-engine-occ. Acquiring FreeCAD adds a Qt plus Python plus
   Coin3D stack (hundreds of MB of third-party code), Python in the primary
   authoring path (against the Rust-first core posture; C++/Python is why the
   master plan puts the shell outside the Rust core), and a second pinned OCCT
   build in the workspace (FreeCAD pins its own OCCT release line, not ours),
   plus an external release cadence and governance we do not control.
4. Both acquirable shells are capability-mismatched today. FreeCAD is mature
   and CATIA-class, but it is the mismatch of section 2.1 and 2.2: the wrong
   model and the wrong renderer, regardless of how good its sketcher is.
   CADCraft is a pre-alpha 2D drafting tool: its own roadmap reports roughly
   38% ready for real work, roughly 50% feature breadth versus AutoCAD
   (245 of 491 menu items, with 2D menus about 65%), a 3D kernel estimated at
   300-450 agent-hours away, parametrics scheduled after 3D, and beta roughly
   600-930 hours out. It models AutoCAD (a 2D drafting mindset), not CATIA
   (3D parametric, freeform surface, assembly, MBD), which is precisely the
   class the program exists to serve. It cannot host P7 today on any timeline
   that keeps the P7 cards alive, and its DWG/DXF focus is off-program.
5. Build feasibility is evidenced, not asserted. The program has already
   proven the own-shell ingredients on real material: the boolean record
   ratified a working OCCT binding behind a clean seam (P6-M3-T01), and the
   renderer record committed a running headless wgpu spike that draws real
   pixels with no display (P6-M5-T01). The CADCraft run shows a small team
   driving multiple agents reached half of AutoCAD's command surface in about
   a week measured on its own parity checklist. The own shell does not
   re-derive geometry or constraint solving; it adopts OSS solver libs
   (section 1.3) and hosts the already-adopted OCCT kernel behind an open
   interface. What it builds fresh is the interactive sketcher/drafting UX,
   which is the honest cost this record budgets as decomposed, milestone-gated
   work rather than a promise of parity.

## 3. Context

- The P7 lane in the master plan is titled "Open Modeler Integration
  (FreeCAD-class core)". The downstream cards are written shell-agnostic: the
  interchange workbench is "in the adopted shell (post P7-M1-T01)", the
  sketcher is "adopt FreeCAD sketcher semantics IF FreeCAD is the chosen
  shell", and P7-M3-T01 already pre-decides "constraint solver decision
  (adopt an OSS solver lib, do not rebuild)". The fork was always open; this
  record closes it.
- The boolean record (P6-M3-T01) names the "P7 modeler shell" as a downstream
  caller of oep-geo through the clean interface, and the renderer record
  (P6-M5-T01) already starts an own desktop app shell (P6-M5-T02 preview
  viewer) that shows oep-geo models on wgpu. The own-shell decision makes
  those two program artifacts directly load-bearing and turns P6-M5-T02 into
  the seed of the modeler host.
- The importer hub (P6-M4-T02) feeds every reader (P2 CGR, P3 3DXML, P4 STEP,
  P5 native) into oep-geo. The shell is the consuming surface of that hub. An
  own shell consumes it natively; a FreeCAD shell would consume it only after
  a second translation into FreeCAD's document.
- P8 (PDM) and P9 (P2P compute/render workers) operate on oep-geo objects and
  the wgpu renderer. An own shell saves revisions and farms renders against
  oep-geo directly; a FreeCAD shell would serialize into FreeCAD documents
  for every PDM save and would need a FreeCAD-compatible viewer for P9 jobs.

## 4. Evidence

All facts below are checked against live primary sources on 2026-10-16; the
evidence file with URLs is workstreams/modeler/shell/open-modeler-shell-sources.md. Stars and
issue counts are point-in-time and labeled as such. FreeCAD and CADCraft are
open-source projects read lawfully as reference material, never as source of
copied code.

1. FreeCAD (primary: Wikipedia article, FreeCAD GitHub releases, FreeCAD
   source). FreeCAD is "a general-purpose parametric 3D computer-aided design
   (CAD) modeler", CATIA-class, written in C++/Python, LGPL (project sources
   LGPL-2.0-or-later; current distribution metadata lists LGPL-2.1-or-later),
   built on OCCT plus Coin3D (Open Inventor) plus Qt plus Python, and usable
   as a library by other programs. Stable release 1.1 (2026-03-24); preview
   26.3-RC1 (2026-10-08), with a fixed calendar cadence of three releases per
   year (next 27.1, 27.2, 27.3). Its Sketcher geometric constraint solver
   (PlaneGCS) lives under src/Mod/Sketcher/App/planegcs and is LGPL-2.1-or-
   later, and has been extracted as a standalone LGPL-2.1-or-later library
   ("planegcs") with Python bindings, usable outside FreeCAD. Relevance:
   FreeCAD is the strongest open modeler; its sketcher solver is adoptable
   without adopting FreeCAD; its document model and Coin3D viewport are why an
   own shell is preferred (section 2).
2. OCCT (primary: dev.opencascade.org release page). Latest release is OCCT
   8.0.1 (8.x family), LGPL-2.1 with the OCCT exception, and it is already the
   pinned boolean engine behind oep-engine-occ (P6-M3-T01). The own shell's
   kernel is therefore a dependency already ratified, versioned, and isolated;
   the shell adds no new system dependency beyond it.
3. CADCraft lineage (primary: storytold/cadcraft GitHub, ROADMAP.md, ArtCraft
   site, licenseware.io writeup). Released 2026-10-07; Apache-2.0; pure Rust
   with egui (no Electron); AutoCAD-class 2D drafting (command line, object
   snaps, layers, dimensions, blocks, DXF read/write R12-2018, DWG R13-2018
   via the MPL-2.0 acadrust library, SVG/PNG export); agent-first via MCP,
   JSON control channel, and CLI. Self-reported roadmap (2026-10-10): pre-
   alpha, ready for real work about 38%, feature breadth about 50% (245/491
   AutoCAD menu items) with 2D menus about 65%, 3D kernel estimated 300-450
   agent-hours out, parametrics listed after 3D, beta about 600-930 hours out;
   target is AutoCAD 2027 parity. Point-in-time GitHub: 5,254 stars, 631 open
   issues, 9 days old. Relevance: the right Rust/agent-first DNA but the wrong
   product class (2D drafting, no 3D kernel, no parametrics, no assembly, no
   B-Rep) and a pre-alpha maturity; acquisition is rejected today and kept on
   watch (section 5.3).
4. Program-ratified decisions (primary: Isolith master plan decomposition
   2026-10-15, P6-M3 boolean ADR, P6-M5 renderer ADR, board program record).
   (a) oep-geo is the canonical model ("our model, our serde schema"); (b) the
   renderer is wgpu/WGSL, headless, with a committed real-material spike; (c)
   core is Rust-first (decision 6); (d) license posture is Apache-2.0 for
   applications/libraries and AGPL-3.0 for the server/PDM and P2P; (e) the
   clean-room invariant forbids vendor binaries and copied source; (f)
   P5-M1-T03 ratified B-Rep/geometry first for the native reader, so exact
   geometry (not feature history) is what will stream into the shell; (g)
   P7-M3-T01 pre-decides solver adoption. These are the load-bearing context
   for sections 2 and 5.

Honesty note: this record does not understate the own-shell build cost. The
sketcher and drafting UX are genuinely large interactive surfaces; adopting a
solver library (section 1.3) removes the solver algorithm, not the sketch-UI
ergonomics. The claim being made is that the own shell is the only option
consistent with the already-ratified architecture, that its cost is bounded by
the decomposed milestone gates in section 10, and that either acquirable shell
today conflicts with a ratified decision or lacks the required capability
class, which is a worse and more permanent cost than a longer P7 schedule
sectioned into tested milestones.

## 5. Options considered

1. ACQUIRE FreeCAD as the P7 modeler shell - REJECTED for the shell role, and
   KEPT as the documented acquire-escape hatch and behavioral reference. The
   shell role installs a second authoring document/feature model next to
   oep-geo (the two-truth seam, "dumb solids" for our B-Rep), a Qt/Coin3D
   viewport that contradicts the ratified wgpu renderer, a Qt plus Python plus
   Coin3D dependency tower with a second pinned OCCT build, and a product
   identity as a FreeCAD distribution. FreeCAD remains valuable to the
   program as (a) the highest-fidelity open sketcher/UX reference to read
   lawfully, (b) the source of the adoptable PlaneGCS solver semantics, and
   (c) the escape hatch if the own shell misses the section 11 triggers.
2. ACQUIRE the CADCraft lineage as the P7 modeler shell - REJECTED today,
   KEPT on watch. Pure Rust, egui, Apache-2.0, and agent-first (MCP/JSON/CLI)
   are exactly the program's DNA, but the product is a pre-alpha 2D drafting
   tool with no 3D B-Rep kernel, no parametrics, no sketcher, no assembly, no
   surface support, and a self-reported 3D kernel 300-450 agent-hours away
   with parametrics scheduled after it. It hosts none of P7-M2..P7-M5 as
   written. Watch triggers (section 11): a real 3D parametric core or a
   decision to lean the product toward 2D drafting, at which point this record
   revisits.
3. KEEP a thin own shell around Open CASCADE (oep-geo + oep-engine-occ + wgpu)
   - ADOPTED. Single canonical model, single renderer, Rust-first, the
   cleanest clean-room and supply-chain posture, native PDM/P2P/agent
   integration, and our own release and UX timeline. Cost is honest: the
   sketcher/drafting/assembly interactive UX is built in-house over the
   adopted OCC kernel and adopted OSS solver libraries, gated milestone by
   milestone (section 10), with the documented acquire-escape hatch if a gate
   slips (section 11).
4. (Weighed and folded into 1.) A "hybrid" where FreeCAD is the shell and
   oep-geo is only the interchange model - REJECTED as a variant of 1. The
   hybrid names oep-geo as the durable truth but makes FreeCAD the authoring
   truth, so it keeps both the two-truth seam and the renderer split while
   adding the fiction that the interchange role is the model role. It does not
   escape the costs of 1; it only documents them.

## 6. Shell contract

The own shell must satisfy, as a contract for the downstream cards:

- oep-geo types only on the shell surface: the shell talks to oep-geo, which
  talks to oep-engine-occ. No OCCT, TopoDS_, Handle_* or OCC tolerance tokens
  in shell code or in the oep-geo public API (the P6-M3 seam denial gate
  extends to the shell crate).
- The viewport is the wgpu renderer (P6-M5), so the shell's display pipeline,
  camera, selection, and clipping reuse oep-render; preview fidelity equals
  P9 render-job fidelity by construction.
- The "workbench" language of P7-M2-T01 means the shell's panel and command
  surface: buttons that call the P2/P3/P4/P5 readers and the P6 exporters
  through the importer hub, plus PDM load/save against the P8 API contract.
- Assembly (P7-M3) operates on the oep-geo assembly model (instances,
  transforms, mates) with an adopted OSS constraint solver behind an ADR;
  kinematics/DMU-like semantics are our own on oep-geo.
- Sketcher and parametric variable sets (P7-M4) store constraints and var sets
  as oep-geo features; the adopted solver solves them; behaviorally they match
  FreeCAD v1 variable-set semantics without importing FreeCAD.
- Command-line parity (P7-M5-T01) is native: every shell command is a machine-
  callable CLI/JSON command, which also gives the agent-first workflow that
  the CADCraft lineage proves users and agents expect.

## 7. Renderer

The shell's interactive surface is the wgpu renderer decided in P6-M5: the
same device, pipeline, and WGSL shaders feed the shell's windowed surface and
the headless P9 render job. There is exactly one render stack in the product.
The Qt/Coin3D viewport that a FreeCAD acquisition would require is not adopted
anywhere, which keeps the "preview equals job output" invariant structural.

## 8. License

- The own shell and all shell-authored crates are Apache-2.0, matching the
  program posture for applications/libraries.
- OCCT is LGPL-2.1 with the OCCT exception and stays isolated inside
  oep-engine-occ exactly as the boolean record already governs (section 7 of
  that record: the LGPL component is a separately distributed, separately
  versioned engine dependency; our crates compile against our own types).
- Adopted OSS solver libraries (for example the PlaneGCS-style standalone
  LGPL-2.1-or-later line, or a permissive Rust solver chosen at P7-M3-T01/M4-
  T01) are linked behind the same seam discipline with the P0-M2-confirmed
  notice text, the same way OCCT is.
- Nothing in the AGPL-3.0 server/PDM/P2P lane copies an LGPL library into
  itself; the seam is structural.
- If the CADCraft lineage is ever adopted (watch trigger), it is Apache-2.0
  / MIT OR Apache-2.0, compatible with the posture; its DWG path (acadrust,
  MPL-2.0) is off-program regardless. The MPL-2.0 and GPL-only reader
  candidates are handled per the P4-M4 JT record's rule (reject GPL-2.0-only,
  permissive-but-immature stays reference-only).
- P0-M2 legal pass confirms the notice and packaging text before any release
  that embeds OCCT or an LGPL solver, consistent with the boolean record.

## 9. Clean-room

The program clean-room invariant (no vendor binaries, no copied source, no
Dassault/Autodesk/Siemens libraries used or linked) applies to the shell
unchanged:

- OCCT is a licensed public kernel, already inside the invariant per the
  boolean record.
- FreeCAD and CADCraft are open-source projects read lawfully as reference
  material: we may read their source and public docs to understand sketcher
  semantics, var-set behavior, command-line UX, and parity checklists. We do
  not copy, adapt, or link FreeCAD source or the CADCraft source into our
  crates, and we reimplement our own shell code. Adopting an available OSS
  solver library (PlaneGCS-style) is linking a public library, not copying a
  modeling-module implementation; our sketcher writes its own code against
  that library.
- The no-vendored-foreign-source CI scan family (from P0-M4 and the boolean
  record) extends to the shell: no FreeCAD module tree and no CADCraft tree
  enter the repo, and the seam-denial gate asserts no OCC type identifiers in
  shell/oep-geo public surfaces.

## 10. Enforcement in the plan

- The downstream cards proceed on the own shell as written, because their
  wording is already shell-agnostic: "workbench" in P7-M2-T01 means the own
  shell's panel/command surface; P7-M3-T01 keeps its solver ADR deliverable
  (adopted OSS solver, verified on a real two-part fixture); P7-M4-T01 keeps
  its "like FreeCAD v1 variable sets" semantics target against the own-shell
  sketcher; P7-M5-T01/T02 are the own shell's UX passes.
- P7-M2-T01 depends on the real-CGR mesh latch card (t_899eb193) per its own
  gating; this record does not change that edge.
- New decomposition cards for the sketcher, drafting/2D generation, and
  assembly UX spawn off the P7-M2 and P7-M3 gates (assignee: default), each
  as one-testable-behavior leaves per program doctrine.
- Acceptance tests for those cards run on real material and the corpus; the
  shell's own acceptance test (tests/test_p7m1t01_modeler_shell_adr.py)
  enforces this record's shape and the em-dash/curly-quote surface rule.
- Review: this record is revisited at each P7 milestone gate (P7-M2, P7-M3,
  P7-M4, P7-M5). A material change of heart is a version bump with a change
  log line, not an unrecorded drift.

## 11. Guardrails

- Milestone gates as heartbeat: the own shell is built behind gates, not
  promised. The named gates are P7-M2-T01 (shell loads a latched real CGR
  mesh and a 3DXML through the workbench surface), P7-M3-T01 (assembly of a
  real two-part fixture with an adopted solver ADR), and P7-M4-T01 (a sketch
  parametrically drives a model dimension against the adopted solver).
- Escape hatch, documented and trigger-bound: if the own shell misses a named
  gate at its milestone review (P7-M2 or P7-M3), the acquire-FreeCAD escape
  hatch is revisited with an honest effort comparison, exactly as the
  boolean record's escape hatch works for the kernel. This is a version bump
  of this record, not a silent pivot. The oep-geo interchange role survives
  either way, so the escape hatch is cheap.
- Revisit triggers, explicit: (a) the own shell misses a named gate; (b) the
  CADCraft lineage ships a real 3D B-Rep parametric core (or the program
  pivots the shell toward 2D drafting); (c) a shell becomes embeddable as a
  pure library without its document model taking over authoring. The record is
  re-evaluated on any of these, and at the P5-M5 two-year Don gate along with
  the other ADRs.
- Version pin and build hygiene: the shell reuses the pinned oep-engine-occ
  OCCT only; no second OCCT/Qt/Python/Coin3D build enters the workspace. The
  adopted solver and UI crates are version-pinned per P0-M3-T02 toolchain
  discipline.
- Test gate: every shell milestone merges only behind its acceptance test on
  real material (corpus fixtures), CI green on the pinned toolchain, precommit
  gate pass, and the em-dash/curly-quote ban in all prose.
- Evidence currency: the sources file records the 2026-10-16 check; every
  version bump re-checks the numbered facts (FreeCAD/CADCraft/OCCT releases,
  parity numbers, solver licensing) before reuse.

## 12. Consequences

Keeping the own shell:

- Makes the sovereign story literal: our model, our renderer, our Rust, our
  release train. oep-geo stays the single truth; readers (P2/P3/P4/P5), PDM
  (P8), and compute (P9) all talk to it with no foreign document in the
  authoring path, and P14 single-binary integration is on-path.
- Makes P7-M2..P7-M5 proceed on the shell-agnostic cards as written, with
  P7-M2-T01 still gated on the real-CGR mesh latch and workstreams/modeler/assembly/P7-M4 carrying
  their own solver ADRs.
- Makes the honest cost visible: the sketcher and drafting UX are the longest
  lanes in P7, built over the adopted kernel and solver, gated milestone by
  milestone instead of promised as parity. This is the price of not installing
  a second model and a second renderer in the product.
- Keeps both doors open without double spend: FreeCAD remains the documented
  acquire-escape hatch (waiting behind the oep-geo interchange role), and the
  CADCraft lineage remains the watched pure-Rust host whose run continues to
  validate the program's own agent-driven, clean-room build doctrine.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P7-M1-T01. Decides the open
  modeler shell fork: keep a thin own Rust shell around Open CASCADE on the
  oep-geo canonical model and the wgpu renderer, reject acquiring FreeCAD as
  the shell (kept as behavioral reference and acquire-escape hatch), reject
  the CADCraft lineage today (kept on watch), adopt OSS solver libraries
  rather than rebuilding, and re-state the consequence for P7-M2..P7-M5
  plus the milestone gates and revisit triggers. Evidence file:
  workstreams/modeler/shell/open-modeler-shell-sources.md (live sources checked 2026-10-16).
