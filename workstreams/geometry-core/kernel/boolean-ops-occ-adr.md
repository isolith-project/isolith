---
title: Isolith B-Rep Kernel Operations Decision (Open CASCADE)
decision: bind_opencascade_occ_for_b_rep_operations_behind_clean_interface
version: 1.3.0
date: 2026-10-18
status: Ratified
owner: Isolith program governance, P6-M3 (B-Rep ops)
applies_to: oep-geo (adopting crate), the oep-engine-occ FFI crate (P6-M3-T04), the STEP interop lane (P4), and the P6 feature-graph replay (P6-M3-T03)
supersedes: version 1.2.0 of this record (extrude/revolve sweeps + license disposition); 1.3.0 adds the boundary-measurement (axis-aligned bounding box) and rigid-placement (translate, mirror) operations the P6-M3-T03 feature-graph replay evaluator needs to trim slices and place copies of real solids
---

# Isolith B-Rep Kernel Operations Decision

This record is the binding product decision for how Isolith does exact
boundary-representation (B-Rep) operations against a geometry kernel. At
version 1.0.0 the scope was boolean operations (union, difference,
intersection of solids and shells); version 1.1.0 widened the scope to the
kernel operations the modeler actually needs at the seam: the three booleans,
plus fillet, chamfer, and primitive creation (box, cylinder, sphere), and
version 1.2.0 adds the two sweep operations the feature vocabulary names next:
extrude and revolve. Version 1.3.0 adds the boundary-measurement operation
the feature-graph replay and the modeler both call (the axis-aligned bounding
box of a solid) and the rigid-placement operations (translate, mirror about
an axis-aligned plane) the P6-M3-T03 `Trim` / `Offset` / `Mirror` feature
kinds need to slice a slab off a top surface and place copies of a real
solid. It is the answer to the P6-M3-T01 question "adopt/tie
Open CASCADE for B-Rep ops, or build an own kernel-lite", and it is the
decision that P6-M3-T04 implements. It is versioned like the telemetry
decision (P0-M5-T02) and the release train decision (P0-M5-T03); any material
change bumps the version in the front matter and adds a line to the change
log at the bottom.

The default stated in the P6-M3-T01 card is adopted: bind Open CASCADE
Technology (OCCT) for parity of effort, behind a clean interface. This
document records why, what the clean interface is, what the license and
clean-room positions are, and what guardrails keep the decision honest.

## 1. Decision

Isolith ADOPTS Open CASCADE Technology (OCCT) as the geometry kernel behind
the oep-geo clean, documented seam. The oep-geo public surface is the ONLY
thing downstream code (callers, P4 STEP interop, P6-M3-T03 feature replay, P7
modeler shell) sees; OCCT types, handles, tolerances, and idioms never cross
that seam.

Three operation families are in scope at version 1.2.0:

1. Exact B-Rep booleans, mapped to the OCCT BRepAlgoAPI_Fuse /
   BRepAlgoAPI_Cut / BRepAlgoAPI_Common algorithms:
   - union
   - difference
   - intersection
2. Kernel modeling operations the feature tree needs, mapped to their OCCT
   equivalents:
   - fillet (BRepFilletAPI_MakeFillet)
   - chamfer (BRepFilletAPI_MakeChamfer)
   - primitive creation: box, cylinder, sphere (BRepPrimAPI_MakeBox /
     MakeCylinder / MakeSphere)
3. Sweeps of a planar profile, mapped to their OCCT equivalents:
   - extrude (BRepPrimAPI_MakePrism)
   - revolve (BRepPrimAPI_MakeRevol)

The current OCCT release line is 8.0.x (8.0.0 final, May 2026); the binding
implementation (oep-engine-occ, P6-M3-T04) pins a concrete OCCT version per
its own guardrail (section 9) rather than tracking upstream bleeding edge.

Versions 1.0.0/1.1.0 scoped the three booleans, then fillet, chamfer, and
primitives. The widening is recorded because a kernel seam that cannot fillet
a solid is not the seam the P6-M3-T03 feature graph or the P7 modeler
actually call (a feature vocabulary that already names Extrude, Fillet, Hole,
Revolve, Pattern, Mirror needs those operations to be real). Version 1.2.0
extends the seam to extrude and revolve for the same reason: the P7 sketch
lane sketches a 2D profile and the feature tree extrudes or revolves it into
a solid, so the seam those callers hit must make the sweep real. The same
engine, license, seam, and guardrails cover the widened set; nothing about
the adoption rationale changes.

Version 1.3.0 (KERNEL-REPLAY) adds three operations the P6-M3-T03 feature-graph
replay needs to produce real solids once the kernel is wired behind the
evaluator seam:

- `solid_bbox`: the axis-aligned bounding box of a solid (BRepBndLib /
  Bnd_Box behind the seam). The `Trim` feature kind slices a slab off the top
  of a real solid, which requires knowing the solid's measured extent.
- `solid_translate`: rigid translation by a vector (BRepBuilderAPI_Transform
  with a gp_Trsf), realizing the `Offset` feature kind as a real placement.
- `solid_mirror`: reflection about an axis-aligned plane (mirror transform),
  realizing the `Mirror` feature kind as a real placement (a copied solid).

These join booleans/fillet/chamfer/primitives/sweeps behind the same seam with
the same typed-error posture: the binding (`KERNEL-REPLAY`) replays a feature
graph through a real evaluator whose value type carries real solids, and a
kernel failure is a typed `KernelError` / `BooleanError` mapped to a replay
`Evaluator` error, never a panic. Nothing about the adoption rationale
changes; the same engine, license, seam, and guardrails cover the widened set.

Building an own "kernel-lite" engine in wave 1 is the explicitly weighed
alternative and is REJECTED for this milestone (section 4). It remains the
documented escape hatch (section 9), not a parallel build.

## 2. Why this decision

Three facts drive it:

1. Parity of effort is the program doctrine's own arithmetic. An exact B-Rep
   boolean kernel is among the hardest artifacts in CAD software: surface-
   surface intersection, point classification, face splitting, and the
   rebuild/sew step are each research-grade, and they compound. OCCT has
   delivered and maintained BRepAlgoAPI booleans for decades, is battle-tested
   against real engineering geometry, and is the reference open kernel (it
   underlies FreeCAD and most open STEP tooling). Re-deriving that inside a
   wave-1 milestone trades an entire workstream for a feature other milestones
   (P4 STEP, P7 modeler, P8 PDM viewer) depend on today.

2. OCCT is a legitimate, licensed, public dependency, not a clean-room or
   licensing risk. It is open source under LGPL-2.1 with the OCCT Exception
   1.0 (section 7), ARM64 is a first-class target (relevant to the U1/GB-10
   host), and its boolean module is the same engine the rest of the plan
   already leans on (P4-M1-T01 STEP over OCC, P2-M5-T02 STEP passthrough).
   Adopting it once for booleans is consistent with adopting it elsewhere.

3. The clean-interface requirement bounds the downside. The risk that worries
   a sovereign engineering platform is not "linking a mature kernel", it is
   "marrying a foreign kernel so tightly that the platform cannot survive a
   change of heart". The seam in section 5 is what turns an adoption into a
   tie at parity of effort without surrendering the platform's own model.

## 3. Context

- The adopting crate is oep-geo (P6), which owns the native model schema
  (P6-M1-T01): part, body, feature, assembly, instance.
- Boolean operations are listed under P6-M3, with topology undo/redo
  (P6-M3-T02) and the feature graph with replay (P6-M3-T03) as downstream
  consumers of whatever this record decides.
- The math lane (P6-M2) is where NURBS/B-spline integration (P6-M2-T02) was
  already expected to choose "OCC bindings or a Rust NURBS crate"; this
  boolean decision does not force that lane, but it makes OCC availability a
  given on the build, which the math lane may reuse for its own curve and
  surface evaluation.
- The STEP lane (P4-M1-T01) already names OCC as a candidate STEP engine.
  A single, pinned OCC build can serve both lanes.
- Rust is the core language (decision 6 in the master plan). OCCT is C++, so
  the binding is an FFI crate, which is precisely why the clean interface is
  the load-bearing requirement.

## 4. Options considered

1. Bind/tie Open CASCADE (OCCT) for B-Rep boolean ops, keep a clean interface.
   ADOPTED, this record. Parity of effort, mature boolean algorithms, same
   kernel already nominated for STEP, open license compatible with the hybrid
   posture, ARM64 supported. The cost is a C++ dependency, an FFI crate, and
   the discipline of holding the seam (section 5). The seam is enforceable in
   CI (section 9).

2. Own kernel-lite: implement union/difference/intersection on our own B-Rep
   in Rust, no OCC. REJECTED for wave 1. Boolean robustness is a decades-long
   problem, not a sprint. A wave-1 kernel-lite would be exactly accurate on
   boxes, cylinders, and unit tests, and would fail on the real corpus the
   whole program is built to serve. It also cannot be a "lite" scope exercise:
   either it computes real surface-surface intersections (the hard part) or
   it is a polyhedral CSG toy that emits meshes instead of B-Rep, which fails
   the exactness requirement. Deferred as the escape hatch (section 9), to be
   revisited only if OCC binding becomes untenable.

3. Adopt a pure-Rust kernel wholesale (for example truck-shapeops / truck).
   NOT ADOPTED for this milestone. Truck is a genuine, well-engineered pure
   Rust CAD kernel with B-Rep boolean operations, and it is attractive on
   ownership grounds. But it is a complete B-Rep/NURBS world of its own:
   adopting its booleans means adopting its topology, its surface model, and
   its type system, which collides with the clean-interface goal and with the
   P6 native-model intent (our model, our serde schema). Its boolean module is
   newer and less battle-tested than OCCT for production CAD-class geometry.
   Keep it on the radar as a future kernel-lite host; do not adopt today.

4. Mesh-level CSG boolean (triangle-mesh union/difference/intersection).
   REJECTED. Mesh CSG is fast for visualization but produces a tessellation,
   not an exact B-Rep. The program's exactness requirement (P6-M1-T03, "double
   everywhere, documented tolerance model") and the STEP/interop lanes
   (P4-M2, P5) operate on exact geometry. A mesh result would poison every
   downstream consumer. Mesh CSG may appear later as a separate visualization
   aid (P6-M5 renderer), never as the boolean path.

## 5. Clean interface

The requirement this record places on the code is a two-layer rule:

1. oep-geo exposes kernel operations as methods or free functions on the
   oep-geo model types, taking and returning oep-geo types only. The
   signatures are kernel-shaped: for example something like
   `solid_boolean(union|difference|intersection, sys_handle, operands) ->
   Result<Solid, BooleanError>`, `solid_fillet(sys_handle, edge_refs, radius)
   -> Result<Solid, KernelError>`, `solid_chamfer(sys_handle, edge_refs,
   distance) -> Result<Solid, KernelError>`, and primitive constructors
   `make_box` / `make_cylinder` / `make_sphere(...) -> Result<Solid,
   KernelError>`, all defined against oep-geo topology and the P6-M1 schema.
   No TopoDS_Shape, no Handle_*, no BRepFilletAPI, no BRepPrimAPI, no OCCT
   tolerance enums appear in any oep-geo public item.

2. A dedicated FFI crate (working name oep-engine-occ) is where OCCT lives.
   It owns the system OCCT dependency, the cxx.rs bindings, the C++ glue, and
   the translation layer both ways: oep-geo model to TopoDS_Shape before an
   operation, TopoDS_Shape back to oep-geo model after. oep-geo depends on
   nothing from that crate except through the clean interface, which is a
   declared dependency boundary in the workspace layout (docs/layout.md
   convention: no cross-crate path deps until P6-M4-T02; this seam is the
   exception that the importer hub formalizes).

Everything a caller can name, construct, or pattern-match against is oep-geo.
Everything OCCT-shaped is private to the engine crate. A future swap of the
engine (section 9) is then a replacement of one crate behind an unchanged
interface, which is exactly what the escape hatch requires.

## 6. Failure and tolerance posture

Boolean algorithms fail honestly in the real world: coincident faces,
tangent contacts, tiny features at tolerance. The interface therefore returns
a typed result that distinguishes success from a diagnosed failure, never a
panicked process. OCCT reports IsDone()/error status; the engine maps that to
a BooleanError with a message, and oep-geo carries it across the seam
unchanged. The P6-M1-T03 tolerance model (the value used for
intersection/fuzzy tolerances) is passed explicitly by the caller, not baked
in, so accuracy policy lives in oep-geo, not in the engine.

## 7. License

OCCT is distributed under GNU Lesser General Public License version 2.1 with
the "Open CASCADE exception (version 1.0) to GNU LGPL version 2.1". The
exception permits distributing object code (a "work that uses the Library")
that incorporates material from OCCT headers under terms of our choice,
provided the documentation carries prominent notice that the code uses or is
based on facilities provided by Open CASCADE Technology.

Against the program posture (Apache-2.0 applications/libraries, AGPL-3.0 for
the PDM server and P2P orchestration, decided 2026-10-15):

- oep-geo, the oep-engine-occ wrapper, and all Apache-2.0 libraries we author
  remain Apache-2.0. The seam rule (section 5) means our crates compile
  against our own types; the LGPL component stays a separately distributed,
  separately versioned engine dependency. LGPL-2.1 governs that dependency,
  not our crates.
- Object code that literally embeds material from OCCT headers (there will be
  C++ glue inside oep-engine-occ) ships under the exception with the required
  OCCT notice. The P0-M2 legal pass writes the notice text and confirms the
  exact packaging wording before any release (P0-M2-T01), consistent with
  the existing P0-M2 deliverables (export-control memo, trademark memo).
- The server/PDM lane (AGPL-3.0) must not copy the LGPL engine into itself to
  dodge either license's terms; the seam makes that boundary structural.
- No dependency is added that changes a crate's license as declared in the
  P0-M2 legal posture. This is reviewed again at the P0-M2 legal gate.

### 7.1 Confirmed disposition (v1.2.0): separately-linked kernel crate, no GPL-3.0

This version confirms the disposition at the point the engine actually lands
on main behind the seam:

- OCCT is a separately-linked kernel crate (`oep-engine-occ`), exactly as the
  license posture intends: the crates we author (oep-geo and the wrapper)
  stay Apache-2.0 and compile only against our own types; the LGPL-2.1 +
  exception component is a distinct, version-pinned engine dependency that we
  link but do not copy source from. This satisfies the "Apache-2.0-applications
  posture via a separately-linked LGPL kernel crate" reading of the OCCT
  license, matching how the sketch-solver ADR carries its LGPL line.
- No GPL-3.0 enters any build path. The added dependency graph for the engine
  is permissive: `cxx` and `cxx-build` (Apache-2.0/MIT), `cc` (MIT /
  Apache-2.0), `link-cplusplus` (Apache-2.0/MIT), `codespan-reporting`, and
  the macro crates `proc-macro2` / `quote` / `syn` (MIT / Apache-2.0). A
  license gate scans the lock for a GPL/AGPL marker before the engine feature
  may be enabled (see `tests/test_kernel_seam_guardrails.py`).
- No GPL tree is reintroduced. Decision #16 and the solver-adr precedent
  removed the vendored GPL-3.0 SolveSpace resolver from oep-assem because
  GPL-3.0 in a distributed link conflicts with the Apache-2.0-for-applications
  posture. The OCC binding must not become a second GPL tree: OCCT is LGPL-2.1
  + exception (allowed as a separate dependency), and the no-vendored-OCCT
  guardrail (section 9) keeps even that LGPL source out of the repo. Any
  future dependency whose license cannot stay in the permissive + separate-LGPL
  set is reviewed under this same precedent before adoption.

## 8. Clean-room

The program clean-room invariant forbids vendor binaries, copied source, and
any Dassault/Autodesk/Siemens library being used or linked into Isolith.
Binding OCCT does not violate it:

- OCCT is not a vendor CAD binary and not extracted from any vendor product.
  It is a public, open-source, licensed kernel developed by Open Cascade SAS
  and the OCCT community, distributed under a public license (LGPL-2.1 +
  exception). Linking it through its public API is use of licensed open
  source, which the invariant expressly permits (the invariant names "public
  specs, lawful observation, license-clean sources" as the operative surface).
- The boolean path must not be used as a back door to vendor-format reading:
  OCC has its own importer toolbox, and the P5 native CATPart reader lane
  proceeds by our own observation work (corpus + hex notebooks + behavior
  specs) exactly as it already does. OCC's own readers are a different,
  license-scoped surface and are not part of this decision; if they are ever
  evaluated as reference material it is under P1-M3 (readers audit) rules,
  documented separately.
- Residual risk worth recording: none material for booleans. The one habit to
  guard is "OCC does it, so we can copy the approach" in our own geometry
  code; OCCT is LGPL and we do not copy OCCT source into our crates. The
  guardrail in section 9 (no vendored OCCT source) makes that a CI-checkable
  invariant too.

## 9. Guardrails

These keep the decision honest in code, CI, and future reviews:

- Version pin: oep-engine-occ pins a concrete OCCT version (for example a
  7.9.x or 8.0.x release) in its build manifest, with the pinned version and
  the build flags it needs (no unit-visualization toolkits by default). No
  "latest" floating references. Bumps are deliberate, reviewed changes.
- No vendored OCCT source: the build fetches/builds OCCT as a dependency; the
  repo must never contain copied OCCT source files. A CI scan greps the repo
  for the tell-tale OCCT header set to fail if one is added (extends the P0-M4
  precommit-gate pattern).
- Seam denial in CI: a check asserts no OCC type identifiers appear in the
  oep-geo public API surface (grep/ast scan over oep-geo public items for
  TopoDS_, Handle_, BRepAlgo, BRepFilletAPI, BRepPrimAPI, gp_). This is a
  red-on-leak acceptance gate, the same family as the existing acceptance
  tests (a small Python check in tests/, plus the Rust-within-crate `#![deny]`
  convention where the engine's public types are concerned).
- Escape hatch, documented: keeping the seam means an own kernel-lite or a
  pure-Rust kernel can replace oep-engine-occ behind the unchanged oep-geo
  interface. This record mandates an explicit revisit at the P6-M3 review gate
  and again at the P5-M5 go/no-go (two-year horizon), where the effort
  accounting in section 2 is compared against reality. Nothing in this
  decision forbids contributing to a Rust kernel; it only says the P6-M3
  milestone rides OCCT.
- Test gate: kernel acceptance tests run in CI on real material (the
  corpus/golden fixtures) through the seam (oep-geo types in, oep-geo types
  out), which enforces the interface by construction:
  - a union of two intersecting boxes asserts the resulting volume to
    tolerance; a difference and an intersection assert their volumes and face
    counts;
  - a fillet of a box edge asserts the resulting volume and face count;
  - a chamfer of a box edge asserts the resulting volume and face count;
  - primitive creation asserts the box, cylinder, and sphere volumes to
    tolerance;
  - an extrude of a planar profile asserts the resulting volume and face
    count against both the closed form and the committed kernel golden;
  - a revolve of a meridian profile asserts the resulting volume and face
    count against both the closed form and the committed kernel golden;
  - a known-failing geometry returns BooleanError / KernelError, not a panic.
  OCCT has its own growing test suite; we do not rely on it as our gate, we
  use it as upstream confidence only.
- Feature-graph replay gate (v1.3.0): the `KERNEL-REPLAY` binding adds a
  through-the-seam acceptance over the real kernel -- a Sketch + Extrude +
  Offset + Union + Trim + Mirror feature graph replays to real solids whose
  per-step volume and bounding box match committed kernel goldens (JSON) and
  whose canonical replay trace is byte-locked to a committed text golden
  (`.oepkrt`), mirroring the MiniBox byte-lock; an edit to a driven dimension
  re-solves the graph; an unknown feature kind aborts after the completed
  prefix with a typed error (never a panic, never fabricated geometry); and
  the P6-M3-T02 undo/redo stack navigates those real-solid states (time
  navigation re-derives each state's solid by replay). The MiniBox evaluator
  remains only as the documented, test-only reference so structure tests stay
  fast; the kernel evaluator is the production replay path.

## 10. Enforcement in the plan

- Workspace: oep-engine-occ is a Cargo member behind the documented seam
  (section 5). The importer hub (P6-M4-T02) formalizes cross-crate edges.
- CI: the seam-denial scan, the no-vendored-OCCT scan, and the boolean
  acceptance tests join the existing pipeline (P0-M4-T01) and the precommit
  gate (P0-M3-T04).
- Review: this decision is revisited at the P6-M3 gate. A future change of
  heart is a new version bump of this record with a change-log line, not an
  unrecorded drift.

## 11. Consequences

Adopting OCCT for kernel operations:

- Gives wave 1 a working, exact, tolerant kernel path almost immediately,
  unblocking P6-M3-T02/T03, P4 STEP workflows, and P7 modeler work: the
  feature vocabulary (Extrude, Fillet, Hole, Revolve, Pattern, Mirror) can
  finally compute real solids instead of bounding-box proxies.
- Widens the payoff to fillet, chamfer, and primitives, which the P7 modeler
  shell and the P6 feature graph both call directly, at no extra adoption
  cost: it is the same engine behind the same seam.
- Adds a C++/FFI build step and one system-level dependency to the toolchain
  pinning (P0-M3-T02) and CI matrix. This is a deliberate, bounded cost.
- Requires the seam discipline to be maintained by every future contributor;
  the CI denial gate is the moat.
- Keeps the own-kernel option alive without paying for it twice.
- Carries the LGPL-2.1 + exception dependency with the documented notice and
  the P0-M2 legal confirmation before release.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P6-M3-T01. Decides to bind
  Open CASCADE for B-Rep boolean ops behind a clean interface; records options,
  license, clean-room stance, and guardrails.
- 1.1.0 (2026-10-18): Widened scope for P6-M3-T04. Adds fillet, chamfer, and
  primitive creation (box, cylinder, sphere) to the seam alongside the three
  booleans, because the P6-M3-T03 feature graph and the P7 modeler call them
  directly and a boolean-only seam cannot fillet a solid. Same engine, license,
  seam rule, and guardrails; seam-denial scan and test gate extended to the
  fillet/chamfer/primitive identifiers and assertions.
- 1.2.0 (2026-10-22): KERNEL-BASE landing (card KERNEL-BASE). Adds the two
  sweep operations to the seam and the engine: extrude (BRepPrimAPI_MakePrism)
  and revolve (BRepPrimAPI_MakeRevol), so the P7 sketch lane profile becomes a
  real solid. Confirms the license disposition at the point the engine lands on
  main: OCCT as a separately-linked LGPL-2.1 + exception kernel crate, no
  GPL-3.0 in any build path, and no reintroduction of a GPL tree (decision #16
  and the solver-adr precedent). Test gate extended: the extrude/revolve
  assertions join the committed kernel golden matrix.
- 1.3.0 (2026-10-22): KERNEL-REPLAY landing (card KERNEL-REPLAY). Adds the
  boundary-measurement and rigid-placement operations the P6-M3-T03
  feature-graph replay evaluator needs to produce real solids: solid_bbox
  (axis-aligned bounding box, BRepBndLib / Bnd_Box), solid_translate (rigid
  translation), and solid_mirror (reflection about an axis-aligned plane).
  The engine binding exposes occBBox / occTranslate / occMirrorPlane behind
  the same typed seam. Test gate extended: the feature-graph replay acceptance
  (real-solid goldens, byte-locked canonical trace, driven-dimension re-solve,
  tolerant-failure prefix, undo/redo composition) joins the committed kernel
  goldens, and the MiniBox evaluator is explicitly recorded as the
  documented test-only reference rather than a production path.
