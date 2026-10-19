# P7-M1-T02: Blender-BSD path for surfacing - eval memo

Version: 1.0.0 (2026-10-16)
Owner: P7 modeler lane (wave 2), P7-M1-T02 (optional lane)
Status: DECIDED - scoped GO / NO-GO (conditional). Evidence first, then verdict.
Companion: P7-M1-T01 (open modeler shell keep/acquire memo). This memo is the
optional surfacing-path lane the master plan pends on the shell decision; it
does not gate P7-M2 through P7-M5 and is intentionally independent of the
P7-M1-T01 fork.

## 0. Working definition

"Blender-BSD path" is not glossarized in the program; the only "BSD" usage
anywhere in the keelworks tree is the permissive license family (BSD-3/2
Clause, modified BSD, etc.) in the corpus manifests and reader audits. I read
the phrase as: **use Blender as the freeform surfacing environment, with the
surface math carried by permissive (BSD-family) components rather than by
Blender's GPL internals** - i.e. the surfacing capability rides on
permissively licensed libraries (OpenSubdiv/TOST Catmull-Clark, BSD remeshers,
BSD NURBS modelers) so that capability is clean to keep, reuse, or ship under
the OEP Apache-2.0/AGPL-3.0 posture.

Two alternative readings do not change the verdict and are folded in below:
(a) "BSD" = Blender's SubD (subdivision-surface) surfacing workflow - Blender's
actual surfacing method is SubD over a control cage, and its evaluator is the
permissive OpenSubdiv; (b) "BSD" = the BSD OS - irrelevant, this is a
surfacing-capability question, not a platform question.

## 1. Fidelity baseline: what the surface corpus actually proves (P1-M4-T02)

The surface-corpus findings (P1-M4-T02, 6 entries, PR #82) split into two
fidelity classes. Verified by reading the committed fixtures on branch
task/surface-corpus-1016a (not from memory):

Exact / NURBS class (the "engineering surface" bar):
- oep-car-hood.step - AP214, bicubic B_SPLINE_SURFACE_WITH_KNOTS, degree 3x3,
  4x4 control lattice (~1200 x 800 mm crowned hood), OPEN_SHELL with pcurve
  edges, mm units, 1e-7 m uncertainty. Read directly this run: 16 control
  points, knot vectors (0,1),(0,1), multiplicities (4,4),(4,4). CC0.
- oep-impeller-blade.step - AP214, twisted bicubic B_SPLINE_SURFACE_WITH_KNOTS
  blade shell. CC0.
- open-surface.step - AP214 MANIFOLD_SURFACE_SHAPE_REPRESENTATION, one open
  advanced face, planar shell. CC0 (CADProps).
- boat-hull.CATPart - native V5_CFV2 freeform hull, MIT, reserved for the
  P5 RE lane (not yet decodable at P7 fidelity).

Tessellated / shell class (the "display panel" bar):
- swept-blade.3dxml - ZIP 3DXML freeform swept shell, 441 verts / 800 tris;
  this is the one surface entry VERIFIED through the P6-M4-T02 importer hub
  (oep-import -> oep-geo Solid, bbox [-100,-50,0]..[100,50,16.5], validate
  clean). CC0.
- chassis.3DRep - real Dassault CATIA V5 car chassis, Class-A tessellated
  surface shell (.3DRep). Apache-2.0.

Consequence for any surfacing path: the exact class (bicubic NURBS with pcurves
and open shells) is now testable material, and only the tessellated class is
verified end-to-end in oep-geo today. A surfacing path earns the "exact"
label only if it can take oep-car-hood.step and oep-impeller-blade.step to and
from its native representation without a tessellation downgrade.

## 2. Measured Blender reality on U1 (this run)

- Blender 4.0.2 installed at /usr/bin/blender; runs headless via bpy
  (`blender --background --python-expr` verified live).
- Build options (build_options from bpy this run): opensubdiv=True,
  mod_remesh=True, io_wavefront_obj/io_ply/io_stl=True, cycles=True; and
  collada=False, usd=False, alembic=False, openvdb=False. So this build has
  SubD + remesh + mesh I/O, and no neutral CAD data exchange compiled in
  (no Collada/USD/Alembic; no STEP/3DXML/CGR import by default - those are
  third-party addons upstream, not in the mainline build).
- Blender's native NURBS surface support is documented as non-CAD: no trimming,
  no surface-surface intersection, no hole/boundary modeling, no pcurve or
  topology model, no tolerance model, no PMI. This is a long-standing,
  repeatedly-confirmed limitation (2014 GSoC trim branch never landed; current
  GSoC proposals describe the missing trim-contour data structure as a whole
  new surface system; upstream advice is to use FreeCAD/Moi3D/Rhino for that).
  Any NURBS file Blender touches is converted to mesh at display/export time.

## 3. What a "Blender-BSD" stack can and cannot deliver

Can (tessellated / skin / visual shell direction):
- Class-leading smooth-surface skinning of the panel class: SubD Catmull-Clark
  via OpenSubdiv (permissively licensed - TOST, i.e. Apache-2.0 with a
  modified Section 6), remeshing (mod_remesh), shrinkwrap, LOD. This matches
  the fidelity of chassis.3DRep and swept-blade.3dxml and strengthens the
  P6-M5 wgpu preview story.
- BSD/permissive components exist for exactly this direction: Instant Meshes
  (BSD-3 quad remesher), Ayam and libnurbs (BSD NURBS modelers, but NURBS-only
  UI, not B-Rep), OpenSubdiv (TOST). None of them provides a trimmed-B-Rep
  kernel.
- Optional external tool posture: Blender talks neutral formats (glTF/OBJ/PLY/
  STL, this build's exporters), never links into oep-geo.

Cannot (exact / B-Rep NURBS direction):
- No trimmed NURBS B-Rep modeling or editing, so the bicubic corpus fixtures
  cannot pass through a Blender path exactly. A Blender round-trip of
  oep-car-hood.step is, by construction, a tessellation downgrade - precisely
  the fidelity loss the corpus now exists to catch.
- No BSD-licensed trimmed-NURBS B-Rep kernel exists anywhere near OCCT. The
  real B-Rep surfacing options are OCCT (LGPL, already the P4/P6 path) and
  commercial kernels. Attaching BSD NURBS libraries (Ayam/libnurbs) does not
  change this: they are patch-based NURBS, not B-Rep shells with trim
  contours, pcurves, and tolerances.
- License: Blender core is GPL-2.0-or-later (much of the C++ dual Apache-2.0
  since 2021, but that still couples a shipped component to Blender's license
  terms, not the OEP hybrid). Ship-or-embed pulls GPL coupling into an
  Apache/AGPL program; tool-not-component avoids it.

## 4. Decision

NO-GO, Blender-BSD as the engineering surfacing path (exact B-Rep NURBS
surface modeling, Class-A by-the-numbers):
- It cannot reach the exact-fidelity bar the surface corpus defines (bicubic
  B_SPLINE_SURFACE_WITH_KNOTS fixtures with pcurves/open shells). That bar is
  met only by the oep-geo NURBS lane: P6-M2 surface math, P3-M2 Gregory/NURBS
  decode + P3-M2-T02 QC tessellation, P4 STEP import, P5-M3 native B-Rep - all
  already in flight and corpus-tested.

CONDITIONAL GO, Blender-BSD as a scoped optional sub-lane (skin / panel /
visual shell on the tessellated class):
- Use Blender as an OPTIONAL EXTERNAL authoring and visual-QA tool over neutral
  formats (glTF/OBJ/STL/PLY) for the shell class the corpus carries
  (chassis.3DRep, swept-blade families) and for P6-M5 preview/comparison work.
- Never ship Blender or a Blender-derived component inside OEP; never route
  exact surface data through it. Its SubD evaluator (OpenSubdiv, TOST) is a
  useful permissively-licensed REFERENCE for the oep-geo tessellation quality
  budget (P3-M2-T02), not a dependency.
- "Where relevant" = the P7 freeform/panel skinning workflow once the shell and
  viewer paths (P7-M2, P6-M5) exist; it is not on the P7 critical path and
  needs no scheduler card in P7-M2 through P7-M5.

## 5. Recommendation for the board

- Record NO-GO for Blender-BSD as the surfacing/B-Rep path; keep the exact lane
  on oep-geo NURBS (no change to the existing P4/P5/P6 work).
- Record the scoped GO as a sub-lane note only, to be picked up via P7-M2
  (glTF/viewer path) when relevant, with no dedicated P7 card created now.
- Fidelity gate going forward: any future claim of surface support loads the
  six P1-M4-T02 entries through oep-geo and tessellates with QC (P3-M2-T02
  goldens) - the Blender path, being mesh-only, is tested only against the
  tessellated class, never asserted against the NURBS fixtures.

## 6. Sources

- P1-M4-T02 surface-corpus growth memo + importer-verification log (board
  attachments, t_49733626, PR #82).
- corpus/surface-samples.json + corpus/data/surface/oep-car-hood.step read
  directly from gitea branch task/surface-corpus-1016a (this run).
- Blender 4.0.2 on U1: `blender --background --python-expr` live probe of
  bpy.app.build_options and headless bpy (this run).
- Blender license: project - GPL-2.0-or-later (with Apache-2.0 dual licensing
  on the C++ library surface introduced 2021/2022).
- Blender NURBS limitation: Blender developer wiki NURBS reports and devtalk
  "Better curve surface support" threads (trim/boolean-on-NURBS absent;
  upstream guidance points to FreeCAD/Moi3D/Rhino).
- Permissive surfacing components: OpenSubdiv - Pixar TOST (Apache-2.0 modified
  Sec.6, SPDX Pixar); Instant Meshes - BSD-3-Clause; Ayam - BSD; libnurbs -
  modified BSD; SINTEF SISL - GPL (not BSD).

Change log:
- 1.0.0 (2026-10-16) initial memo, scoped GO / NO-GO decision recorded.
