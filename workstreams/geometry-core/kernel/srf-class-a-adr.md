---
title: Isolith Class-A Surfacing Strategy Decision (Continuity, Fairing, Surface Reconstruction)
decision: own_permissive_class_a_surface_core_on_the_occt_seam_as_a_strategy_gated_long_program
version: 1.0.0
date: 2026-10-22
status: Ratified
owner: Isolith program governance, editor spine (decision #19), the P15 Class-A surfacing program (new long-horizon workstream)
applies_to: the P15 Class-A surfacing program, the oep-geo surface model (P6), the oep-engine-occ kernel seam (P6-M3-T04), the oep-shell editor surface (P7), the corpus surface-QA lane (P1-M4-T02), and the master plan scope register (amends the Bucket C Class 4 disposition)
supersedes: nothing (first Class-A record; implements the decision #19 in-scope directive that the master plan previously recorded as default-exclude)
---

# Isolith Class-A Surfacing Strategy Decision

This record is the binding product decision for how the Isolith program pursues
Class-A / ICEM-class surfacing: high-accuracy surface construction with G2
continuity, curvature fairing, surface reconstruction, and the diagnostics that
certify a surface as Class-A. It is the kickoff artifact of the P15 program,
the answer the master plan pins on card SURFACING-CLASS-A ("gated by its own
strategy ADR"), and it deliberately contains no geometry and no build. It
selects an approach, fixes the mathematical dependency model, lists the kernel
requirements, registers the long-horizon decomposition, and states the honest
cost in cost-class terms with no calendar promise.

The program's own framing is worth restating at the top: Class-A is the
longest pole in the whole program and the hardest thing in it. Vanilla Open
CASCADE Technology (OCCT) does not deliver Class-A surfaces, and no permissive
open kernel reaches ICEM-class quality today. The commercial moat of the
capability is the surface kernel mathematics and the surface-craft tooling
around it. A fake build is not an option: this document records what reaching
real Class-A requires and what the near-term editor train does NOT get.

## 1. Decision

Isolith pursues Class-A surfacing as a two-track strategy, both tracks honest
and neither one pretending to be the other.

Track 1, near term, is the editor train's surfacing capability: the oep-shell
editor authors and edits surfaces through the existing kernel seam
(oep-engine-occ binding OCCT behind the oep-geo Evaluator interface, P6-M3-T04),
using OCCT's fill, blend, sweep, fillet, and surface-from-curves operations.
This track is explicitly MODELING-GRADE, not Class-A. It is labeled honestly in
the product and in the format matrix. It is what engineers get while the long
program runs, and it is an order of magnitude cheaper than the long program.

Track 2, the long horizon, is the P15 Class-A surfacing program: an OWN,
permissively licensed high-accuracy surface core engineered for Class-A,
composed of the four mathematical modules this record defines (continuity
algebra, fairing, reconstruct-from-triangle-mesh, curvature diagnostics),
built on the existing oep-geo NURBS math as the evaluation base and on OCCT
topology services behind the seam as the B-Rep plumbing. The own surface core
is the kernel moat: nothing open licenses it, and it is what would make
Isolith's surfacing different from every other open CAD.

The hard expectation pin is part of the decision: NO Class-A surface ships
inside the near-term editor train. Class-A certification (highlight-line-clean,
curvature-continuous, reconstructed to tolerance) is gated on the P15 proofs
registered in section 7 and the master plan. The program is strategy-gated: it
spends in tranches behind Go/No-Go gates, and a gate can re-scope it without
calendar damage.

## 2. Why this decision

Three facts drive it.

1. The product bar moved. Decision #19 (2026-10-22, editor spine) made the
   editor the product and same-or-greater editor functionality than
   CATIA/SolidWorks the bar, and moved Class-A into scope from day one as its
   own long-horizon kernel-moat program, accepting the longest pole and the
   biggest team. The alternative (default-exclude, the pre-#19 posture) was
   consistent with a read-only interop product; it is not consistent with an
   editor product whose users style parts.

2. Class-A is the flagship differentiator of the commercial surface that
   Isolith exists to stand in for, and the moat is the math, not the license
   fee. The vault CATIA ecosystem research (Research/CATIA V5 and V6 Ecosystem
   Research, 2026-10-15) records Dassault's own claim that only CATIA combines
   high-accuracy solid modeling with Class-A surface capabilities, and records
   that nothing open attempts Class-A surface modeling. If Isolith wants the
   sovereignty argument to hold for styling and exterior geometry, the surface
   quality machinery has to be ownable, which per strong engineering consensus
   means building it. Reusing a commercial Class-A kernel is rejected on
   license posture alone (section 4); reusing a permissive one is impossible
   because none exists at the required class (section 4, the P7-M1-T02
   finding).

3. The seam discipline that already governs every kernel decision makes the
   risk bounded. The oep-geo Evaluator seam (P6-M3-T04) keeps OCCT off the
   public surface; the own Class-A core lives BEHIND the same seam, so its
   adoption, growth, or replacement is a documented, CI-checkable boundary
   choice, not an invasive one. The near-term editor is not blocked waiting
   for Class-A: Track 1 rides the seam today, Track 2 replaces the surface
   backend incrementally behind it.

## 3. Context

The surface capability the editor needs interacts with several lanes that
already exist, and this ADR sits on them rather than restating them:

- Kernel seam: workstreams/geometry-core/kernel/boolean-ops-occ-adr.md v1.1.0
  ratifies OCCT as the B-Rep operations engine behind the oep-geo seam
  (booleans, fillet, chamfer, primitives), with the implementing FFI crate
  oep-engine-occ as card P6-M3-T04. That crate is the same seam through which
  OCCT surface topology, trimming, and tessellation services are reachable by
  the Class-A core.
- NURBS math: docs/geometry-nurbs.md (P6-M2-T02) delivers own, vendored,
  std-only NURBS basis and surface evaluation (Cox-de Boor, rational NURBS
  curve and tensor-product surface) in crates/oep-geo/src/nurbs.rs with exact
  rational goldens. Non-goals recorded there (second derivatives, knot
  insertion/refinement, degree elevation) are exactly the primitives the
  Class-A continuity and fitting lanes need next.
- Surface input lanes: the Gregory-patch to exact rational biquartic NURBS
  decode (docs/3dxml-gregory-surfaces.md, P3-M2-T01) turns 3DXML LatticeXvl
  surface reps into exact NURBS; the CGR and .3DRep tessellation lanes
  (P2, P1 corpus) supply triangle-mesh input for the reconstruction lane; the
  STEP lane (P4) supplies exact bicubic NURBS fixtures.
- Surface corpus and fidelity bar: workstreams/modeler/shell/blender-bsd-surfacing-memo.md
  (P7-M1-T02) records the fidelity baseline from corpus growth (P1-M4-T02):
  exact class fixtures oep-car-hood.step and oep-impeller-blade.step (bicubic
  B_SPLINE_SURFACE_WITH_KNOTS with pcurves and open shells), open-surface.step,
  the native V5_CFV2 boat-hull.CATPart (P5 RE lane, not yet decodable), and the
  tessellated-class fixtures swept-blade.3dxml and chassis.3DRep (a real CATIA
  V5 Class-A car chassis shell). The fidelity bar: a surfacing path earns the
  "exact" label only if it can take the hood and the blade to and from its
  native representation without a tessellation downgrade.
- The Blender-BSD question is already settled: the P7-M1-T02 memo verdict is
  NO-GO for Blender-BSD as the engineering surfacing path and no BSD
  trimmed-NURBS B-Rep kernel exists near OCCT. The only realistic open,
  license-clean B-Rep surface host is OCCT plus an own permissive core. This
  ADR does not reopen that verdict; it builds on it.
- Master plan: ~/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md
  (the plan) previously listed Class-A as Bucket C Class 4, "different
  foundation", disposition "NOT scoped; defer-or-exclude decision". Decision #19
  superseded that disposition and named this ADR as the gate. Section 7 of this
  record is the decomposition the plan registers (plan v1.16).

## 4. Open paths assessed

This is the review the strategy has to survive. Each path is stated with its
real limit, because the temptation in an open program is to over-credit an
open dependency.

### 4.1 Open CASCADE surfacing limits

OCCT is not a Class-A kernel, and the record says so plainly. What OCCT
provides at the G1/G2 level is real and usable for modeling-grade work:

- Constructive surface operations: fill (surfaces through boundaries),
  blends/fillets, sweeps/revolves, surface-from-curves, offset, and the
  BRepFill / GeomFill families that carry G1/G2 tangency and curvature
  constraints at construction time.
- Point and curve based construction: GeomAPI_Interpolate,
  GeomAPI_PointsToBSpline / PointsToBSplineSurface (least-squares and exact
  fit over point grids), Geom_BezierSurface / Geom_BSplineSurface, knot and
  degree utilities (knot insertion, degree elevation), and the
  BRepLProp / GeomLProp curvature evaluation tools: local continuity checks,
  principal curvatures, and curvature-section probes.

What OCCT does NOT deliver is Class-A quality. Class-A is defined by fairness
across a styled part (continuous, smooth, highlight-line-clean curvature) and
by a modeling discipline of fairing and iterative correction that OCCT neither
implements nor scaffolds. There is no variational fairer, no region-limited
control-point fairing workflow, no surface reconstruction from triangle meshes
with inter-patch G1/G2 enforcement at the class a styling studio demands, and
no curvature-comb / zebra / reflection-line quality tooling. Its
points-to-surface fitting produces acceptable engineering fits, not fair
aesthetic surfaces, and fairness is not something a least-squares fit emits.
The gap is therefore not "OCCT is buggy" but "the fairness and reconstruction
mathematics plus the diagnostics are not part of any open kernel", and
building them is the program.

### 4.2 NURBS surface reconstruction lanes

Reconstruction has a standard ladder of open approaches, all documented in the
NURBS literature (Piegl and Tiller, chapter 9 is the canonical treatment):

- Global approximation by least squares: parameterize the input, place knots
  by averaging, solve the normal equations for control points, optionally with
  a smoothing term (second-difference penalty) to trade fidelity against
  fairness. This is the workhorse open lane and the baseline every
  reconstruction path starts from.
- Global interpolation: exact pass-through of a point grid. Noise-sensitive;
  used to seed fairer results, not as the final answer.
- Parameterization: chord length and centripetal for ordered data;
  conformal/harmonic/least-squares conformal maps for unstructured meshes.
  Parameterization quality is what separates a fair surface from a wrinkled
  one and is a genuine research-grade dependency of the reconstruction lane.
- Quad-dominant reparameterization: the mesh-to-surface path wants a
  quad-dominated layout (Instant Meshes, BSD-3, is the permissive reference)
  so tensor-product patches line up with the part's natural isoparametric
  flow, then per-patch fitting with G1/G2 continuity enforced at shared
  boundaries during stitch.
- Subdivision limits: Catmull-Clark limit surfaces via OpenSubdiv (TOST,
  Pixar license) are smooth and permissive, and are the right reference for
  the panel/skin display class (matching swept-blade.3dxml and chassis.3DRep)
  and for the oep-geo tessellation quality budget. They are not exact trimmed
  NURBS B-Rep and cannot carry the exact-class fixtures (hood, blade) without
  a tessellation downgrade. SubD is a complementary display/quality lane, not
  the engineering surface lane.

The exact-class fixtures (bicubic B_SPLINE_SURFACE_WITH_KNOTS with pcurves
and open shells) define the reconstruction lane's true target: the lane must
produce and consume that NURBS B-Rep class, not a mesh look-alike.

### 4.3 Permissive kernels and approaches that can reach Class-A

The P7-M1-T02 finding stands and is the decisive fact: no BSD-licensed
trimmed-NURBS B-Rep kernel exists anywhere near OCCT. The permissive
components that DO exist are reference-grade building blocks, not a Class-A
kernel:

- OpenSubdiv (TOST, SPDX Pixar): Catmull-Clark evaluator, permissive, used as
  the skin/panel quality reference and the oep-geo tessellation budget
  calibrator, never the exact surface lane.
- Instant Meshes (BSD-3): quad-dominant remeshing, used as the
  reparameterization reference.
- Ayam and libnurbs (BSD): patch NURBS modelers with no trimmed B-Rep
  topology model; useful as evaluator references, not a B-Rep shell surface
  core.
- NURBS++ (LGPL): curve/surface evaluator; an evaluator reference.
- SISL (SINTEF): the strongest open surface-fitting library, but GPL-3.0 and
  therefore EXCLUDED by the program license posture (no GPL-3.0 in delivered
  software, P0-M2).
- OCCT (LGPL-2.1 with the OCCT exception): the only open trimmed-NURBS B-Rep
  kernel broad enough to host a Class-A program topologically (trimming,
  pcurves, shells, booleans), kept behind the seam.

Conclusion, and it is not comfortable: reaching Class-A under the program
license posture means authoring the Class-A mathematics ourselves as an own
permissive core, using OCCT for B-Rep plumbing and own math for the surface
quality. That is the moat the program set out to build, and it is why the
honest expectation (section 8) is a long program, not a calendar item.

### 4.4 What Dassault's CGM moat actually consists of

From the vault CATIA ecosystem research (Research/CATIA V5 and V6 Ecosystem
Research, 2026-10-15):

- The kernel: CGM (Convergence Geometric Modeler), developed by Dassault for
  CATIA V5 and V6 and sold commercially through the Dassault-owned Spatial
  subsidiary. Its stated differentiator is full B-Rep compatibility with
  CATIA V5/V6 plus tolerant modeling from the outset (the ability to model
  with and repair tiny gaps and imperfect input) and hybrid solid+surface in
  one model. Tolerant modeling matters for Class-A reconstruction, which must
  legally heal imperfect scanned or exchanged geometry into a continuous
  surface.
- The Class-A tooling family: Class-A is not a CGM feature drop. It is a
  dedicated workbench lineage: ICEM Shape Design / ICEM Surf (the automotive
  Class-A tool, an acquired ICEM product), Freestyle, Generative Shape Design
  and Wireframe and Surface (GSD), Digitized Shape Editor (DSE), and Quick
  Surface Reconstruction (QSR). ICEM in particular is a separate product
  lineage (ICEM, acquired by Dassault in 2001) with its own fairing solvers,
  its own interaction model, and its own analysis workflow.
- The diagnostics and methodology: Class-A quality is certified with curvature
  combs, zebra striping, reflection lines, and isophotes, applied with a
  discipline of iterative fairing by senior surface practitioners. The
  methodology (knowing which analysis to run, what a clean reflection line
  looks like on an exterior panel, how to fair without disturbing neighbors)
  is institutional and is as much the moat as the math is.
- The surrounding product: the modeler ecosystem (Knowledgeware, DMU, CAA
  ISVs) and on V6 the platform (3DEXPERIENCE, ENOVIA vault, role licensing).
  The platform is where Dassault converts seat revenue into recurring lock;
  the geometry moat is the kernel plus the Class-A tooling plus the craft.

Consequence for Isolith: the CGM moat decomposes into (a) kernel B-Rep
compatibility plus tolerant modeling, which OCCT plus an own core can
credibly answer, (b) the fairness/reconstruction math, which is the thing an
own core must build and which is the commercial moat worth owning, and (c)
the tooling-plus-craft, which is a product-and-methodology program, not a
geometry spike. This ADR scopes (b) and (c)-tooling; (c)-craft is the human
lane the honest-cost writeup accounts for.

## 5. Mathematical dependency model

A Class-A foundation must own certain mathematics outright, because no open
or permissive component supplies them, and must reuse the licensed plumbing
it is granted. The line between own and reuse is the moat boundary.

### 5.1 Own: continuity constraints

Geometric continuity (G1 tangency, G2 curvature continuity, G3) across patch
boundaries is parameterization-independent and is the first thing that makes
a multi-patch surface read as one surface. The foundation must own:

- The continuity algebra of shared edges: how two patches meet with exact G1
  or G2 (the relation of control points and cross-boundary derivatives), and
  the knot-vector compatibility strategy (shared interior knots via knot
  insertion, staying exact, not approximated by a coarser re-fit).
- Twist compatibility at shared corners (the mixed-partial condition a
  corner-trimmed patch arrangement must satisfy), without which G2 cannot be
  certified at corners.
- Watertight seam construction with documented surface tolerance (P6-M1-T03
  discipline: double everywhere, tolerance passed explicitly), the opposite
  of tolerance-smeared stitching.

### 5.2 Own: fairing

Fairness is the difference between a surface that is continuous and one that
is beautiful. The foundation must own:

- Variational fairing: minimize an energy functional over control points and
  weights, classically a linear combination of the stretch term (integral of
  first partials) and the bend term (integral of second partials), giving a
  thin-plate-like Euler-Lagrange system whose smoothing weight trades fidelity
  against fairness.
- Local (region-limited) fairing so a practitioner can fair one patch or one
  span without moving the neighboring surface it must stay continuous with,
  re-imposing the G1/G2 constraints at the region boundary.
- Fairing feedback: curvature plots alongside the model so an operator sees
  the comb while the solver adjusts.

### 5.3 Own: reconstruct-from-triangle-mesh

The reverse-engineering lane that turns triangles (CGR, .3DRep, scanned data,
the tessellated-class corpus) into a surface. The foundation must own:

- Mesh parameterization (chord-length/centripetal baseline for structured
  input; conformal or harmonic maps for unstructured meshes).
- Quad-dominant reparameterization and patch layout so tensor-product patches
  follow the part's natural iso-parametric flow.
- Patch fitting by least squares with a smoothing weight (section 4.2), then
  G1/G2 enforcement when the patch network is stitched, with the exact-class
  NURBS B-Rep as the output contract.
- Tolerant-input handling: legal healing of gaps and imperfect input in the
  spirit of CGM's tolerant modeling, so reconstruction accepts real exchanged
  geometry.

### 5.4 Own: curvature diagnostics

Class-A is certified, not assumed. The foundation must own the diagnostic
tooling that defines the quality bar:

- Curvature comb for curves and the surface analogue.
- Gaussian and mean curvature fields over the surface.
- Reflection lines / highlight lines, zebra striping, and isophotes for
  exterior surfaces (the automotive-class analyses).
- A surface quality report over a part, gated against committed goldens.

These are also a standalone product value: the diagnostics workbench certifies
imported surfaces in the interop product (is this incoming surface fair or
stair-stepped?) and is independently shippable before the kernel matures.

### 5.5 Reuse (licensed, behind the seam)

- OCCT (via oep-engine-occ, P6-M3-T04): B-Rep topology (shells, faces, edges,
  wires), trimming and pcurves, booleans, fillet/chamfer, tessellation,
  split/join, and STEP-grade surface I/O. LGPL-2.1 with the OCCT exception,
  already assessed in the kernel ADR.
- Own NURBS math (P6-M2-T02, crates/oep-geo/src/nurbs.rs): the evaluation and
  basis core the own modules are written against; extended with the
  second-derivative, knot-insertion/refinement, and degree-elevation
  primitives recorded as non-goals there.
- Gregory decode (P3-M2-T01): exact Gregory-to-rational-biquartic-NURBS for
  3DXML surface input into the reconstruction lane.
- Permissive references (OpenSubdiv/TOST, Instant Meshes/BSD-3): quality
  references and reparameterization building blocks behind the seam; never
  GPL; never the exact lane.

### 5.6 Dependency order (what gates what)

Topology seam (P6-M3-T04, already on the roadmap) gates B-Rep plumbing for the
editor and for the Class-A core's trimming/tessellation. Own NURBS math
(P6-M2-T02, delivered) gates evaluation and basis. Continuity algebra gates
multi-patch construction and therefore gates the fairer (which must re-impose
continuity when it fairs) and the reconstructor (which must stitch). The
diagnostics can be built early and against imported surfaces, since they need
only evaluation plus rendering. Reconstructor depends on parameterization,
fitting, and continuity. Editor integration (P7 shell) depends on all of the
above plus the seam. Corpus QA (P1-M4-T02 fixtures plus highlight-line and
curvature goldens) is the continuous acceptance surface for every module and
for the certification claim itself.

## 6. Chosen approach

Two tracks, per section 1, with these specifics:

- Track 1 (modeling-grade, editor train): surfaces via the OCCT seam now,
  honestly labeled modeling-grade. This is the deliverable of record for the
  near-term editor and it is not paused while P15 runs.
- Track 2 (P15, strategy-gated long program): an own permissive Class-A
  surface core comprising continuity algebra, variational fairing,
  reconstruct-from-triangle-mesh, and curvature diagnostics (section 5),
  built on own NURBS math and OCCT topology behind the seam. The core is
  authored Apache-2.0 (application-layer posture), joins the oep-geo surface
  model through the Evaluator seam, and is the recorded kernel moat.
- The kernel itself is not chosen at this step: the ADR fixes the kernel
  REQUIREMENTS (section 6.1) and defers any specific kernel-host build to the
  gated P15-M2 / P15-M4 proofs. The escape hatch stays open; if a permissive
  trimmed-NURBS B-Rep host matures to OCCT-class during the program (none
  exists today), the seam makes it a candidate, and this record is revised.

### 6.1 Kernel requirements

The surface core and any kernel host must satisfy:

1. Trimmed NURBS B-Rep: patches, trimming curves, pcurves, shells; exact f64
   math throughout (P6-M1-T03 double-everywhere).
2. Continuity: G1/G2/G3 geometric-continuity construction across patch
   boundaries, watertight seams within documented surface tolerance, twist
   compatibility at shared corners, exact knot compatibility via insertion.
3. Fairing: variational energy minimization (stretch + bend) with a smoothing
   weight, and region-limited local fairing that re-imposes continuity at the
   region boundary.
4. Reconstruction: mesh parameterization, quad-dominant reparameterization,
   least-squares fitting with smoothing, G1/G2 inter-patch stitching, and
   tolerant-input healing.
5. Diagnostics: curvature comb, Gaussian/mean curvature fields, reflection
   lines / highlight lines, zebra striping, isophotes, and a per-part surface
   quality report.
6. Determinism: byte-deterministic results on a given input and version,
   exercised by committed goldens in CI.
7. License: authored code permissive (Apache-2.0); dependencies only under
   licenses already ratified (LGPL-2.1 + exception for OCCT at the seam);
   no GPL-3.0 (SISL excluded); no commercial kernel licensing (posture).
8. Fidelity: must take oep-car-hood.step and oep-impeller-blade.step to and
   from its native representation without a tessellation downgrade, and must
   reconstruct the tessellated-class fixtures (swept-blade, chassis) to
   continuity within the documented surface tolerance.

### 6.2 What the near-term editor explicitly does NOT get

No Class-A surface construction, no highlight-line certification, no mesh
reconstruction at Class-A tolerance, no fairing solver in the near-term train.
The near-term train gets modeling-grade surfacing through the OCCT seam with
an honest label and an honest format-matrix row. This pin is the acceptance
test's core assertion and is recorded in the master plan.

## 7. Long-horizon decomposition, registered

Registered in the master plan (plan v1.16): P15 is a new long-horizon
workstream, "Class-A / ICEM-class surfacing", cost class 4 (different
foundation), disposition "IN SCOPE via own permissive kernel program,
strategy-gated, see this ADR". The scope register's Class 4 row and the
defer-or-exclude item are amended accordingly. The decomposition:

- P15-M1 Foundation: this ADR, the surface-QA baseline on the six P1-M4-T02
  fixtures plus highlight-line and curvature goldens, and the fidelity bar
  (section 6.1.8). Deliverable: a CI-runnable surface-quality harness and the
  certified-false baseline (the honest statement that nothing in-tree is
  Class-A yet).
- P15-M2 Continuity algebra (own math): G1/G2/G3 patch stitching, watertight
  seams, twist compatibility, exact knot compatibility. Proof gate: two-patch
  and multi-patch arrangements on the bicubic fixtures certified G2 by our own
  diagnostics. GATE 1 (Go/No-Go: foundation proof).
- P15-M3 Fairing (own math): variational fairer with smoothing weight and
  region-limited local fairing, plus the curvature-comb / zebra / reflection /
  isophote diagnostics as the QA gate. Proof gate: a multi-patch surface that
  reads fair on highlight lines. 
- P15-M4 Reconstruct-from-mesh (own math): parameterization,
  quad-dominant reparameterization, least-squares fitting with smoothing,
  G1/G2 stitch, tolerant-input healing. Proof gate: the tessellated-class
  fixtures (swept-blade, chassis shell) reconstructed to continuity within
  tolerance. GATE 2 (Go/No-Go: kernel proof, the honest re-assessment point
  for the whole program).
- P15-M5 Diagnostics workbench: surface quality analysis surfaced in
  oep-viewer and oep-shell; independently shippable and useful on imported
  surfaces immediately. No math dependency beyond evaluation, so it can run
  ahead of M3/M4.
- P15-M6 Editor integration and reference vertical: Class-A surface editing
  in the oep-shell editor on the Track-2 core; P14 reference vertical demo on
  a styled part. GATE 3 (Go/No-Go: editor proof).

Each gate is a Go/No-Go review in the decision-#19 sequence: continue,
re-scope, or stop without calendar damage. The program spends effort in those
tranches, and the master plan notes that P15 is the only workstream whose
gate structure re-opens the whether as well as the how.

## 8. Honest-cost writeup

Decision #17 bars calendar framing; this section uses cost-class and
effort-class language only.

Class-A was already typed in the plan as the only cost class whose moat is the
kernel itself: Class 4, different foundation. The honest magnitude:

Team shape. A credible Class-A program is a kernel program, not a feature
team. Decision #19 accepted "the longest pole and the biggest team". The
disciplines are: computational geometry / CAGD mathematics (continuity and
fairing), numerical linear algebra and optimization (least squares, energy
minimization), CAD surface practice (what makes a reflection line clean),
and a corpus/QA lane that owns the golden suite. That is not one senior
engineer with a notebook; it is a small permanent team in the 3-6 senior
engineer range for the kernel proper, sustained over the program, plus
continuous QA/tooling effort and, for the reconstruction lane specifically,
someone who has actually rebuilt production surfaces.

Effort class. The four own modules on the OCCT seam, in engineering-effort
class terms: a credible G1-plus "good modeling surface" foundation with
diagnostics and honest continuity, low tens of engineer-years. True
automotive-grade G2 Class-A with highlight-line-clean exterior panels and an
in-house variational fairer tuned by practice, high tens of engineer-years
toward the low end of a full kernel-lifetime program. Industry anchors from
the vault research: ICEM began as its own company and its Class-A tooling was
a dedicated multi-year product effort before Dassault acquired it in 2001;
the CGM/Spatial/Dassault kernel work is a decades-long program; SISL, the
closest open analog to the reconstruction core, was a funded national-lab
research effort over years. Nothing in the open ecosystem does Class-A
today; the honest prior is therefore "we would be doing something nobody in
open source has completed", which is precisely why this is the longest pole
and why it is strategy-gated.

Phasing. Effort is spent in tranches behind the three gates, so the program
is honest at every step and cheap to stop: M1 (baseline + goldens) is the
smallest credible slice and is useful immediately because the diagnostics
workbench (M5) precedes full fairing; GATE 1 buys the continuity algebra;
GATE 2 (kernel proof, the first genuinely fair multi-patch surface through
our own pipeline) is the point where the whole program is reassessed against
reality, because nobody in open source has reached it; GATE 3 buys the editor
integration. Track 1 (modeling-grade surfacing on the OCCT seam) is an order
of magnitude below this spend class, at feature scale (engineers-weeks to
engineers-months), and is the editor train's actual commitment.

The honest pin, restated: no Class-A surface inside the near-term editor
train; a modeling-grade surface inside it, labeled as such; Class-A in the
P15 program behind the gates, with effort class (not dates) the only promise
this record makes.

## 9. Guardrails

- No commercial kernel licensing: the program posture excludes rented vendor
  kernels; the moat is ownership. If a design-partner requirement ever forces
  a commercial evaluation, that is a recorded human decision (owner-gate),
  not an engineer's call, and it does not change the own-core trajectory.
- No GPL in the surface lane: SISL is excluded on license; no GPL component
  enters the core; the P0-M2 license gate re-checks the surface crates at
  release.
- Honest labeling: no surface in the product or format matrix is called
  Class-A until it passes the corpus goldens (section 6.1.8) on real
  material, certified by our own diagnostics. Modeling-grade gets the honest
  label.
- Seam discipline: the Class-A core lives behind the oep-geo Evaluator seam
  with the same denial scan the kernel ADR mandates; no OCCT or engine type
  crosses the seam.
- Determinism gate: every fitting/fairing result is golden-tested; a
  non-deterministic (floating-assembly-order-dependent) surface is a bug.
- Gate accountability: the P15 gates are Go/No-Go reviews recorded on the
  board; an attempt to ship Class-A without a passing gate is a process
  violation, not a feature.

## 10. Enforcement in the plan

- Workstream: P15 is registered in the master plan scope register and program
  map (v1.16) with disposition "IN SCOPE via own permissive kernel program,
  strategy-gated, see srf-class-a-adr". Its milestones M1 through M6
  materialize as cards when 2027.1 ships and Don approves the P15 scope, per
  the 2027.2 registration rule.
- Acceptance: tests/test_srf_class_a_adr.py (committed with this record)
  asserts the decision content: the OCC limit, the permissive-reconstruction
  lanes, the CGM moat elements, the own-vs-reuse line, the two-track choice,
  the no-Class-A-in-the-near-term-train pin, and the honest-cost presence.
- Review: this decision is revisited at GATE 1 and GATE 2 and at the P5-M5
  go/no-go. A change of heart is a version bump with a change-log line, not
  unrecorded drift.

## 11. Consequences

Adopting the two-track strategy and registering P15:

- Gives the editor train a real, licensed, modeling-grade surfacing path now
  through the OCCT seam, honestly labeled, unblocked from Class-A.
- Fixes the mathematical ownership line (section 5) so no future worker
  over-credits OCCT, OpenSubdiv, or any permissive library with Class-A
  quality it does not have, and no future worker quietly pulls a GPL or
  commercial component into the surface lane.
- Registers the decomposition and the three gates, so a long-horizon program
  that could otherwise drift or balloon has a measured tranche boundary and a
  point (GATE 2) where the program is honestly re-assessed.
- States the honest expectation in public-surface terms: no Class-A claim
  until the corpus goldens pass. This protects the sovereignty narrative from
  the one claim that would destroy it (claiming a fairing capability that
  does not exist).
- Commits the program, by decision #19, to the longest pole and the biggest
  team, with the cost class recorded so funding and staffing conversations
  happen in honest units.

## Change log

- 1.0.0 (2026-10-22): Initial ratification for SURFACING-CLASS-A (decision #19
  full-send). Two-track strategy: modeling-grade surfacing on the OCCT seam
  in the near-term editor train (honestly labeled), and an own permissive
  Class-A surface core (continuity algebra, variational fairing,
  reconstruct-from-triangle-mesh, curvature diagnostics) as the P15
  strategy-gated long program. Records the OCCT limit, the permissive
  reconstruction lanes, the CGM moat composition, the own-vs-reuse math
  model, kernel requirements, the P15 decomposition with three gates, and
  the honest cost in effort-class terms with no calendar promise.
