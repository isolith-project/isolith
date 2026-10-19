# Commercial Reader Feature List Audit (P1-M3-T02)

Version: 1.0, 2026-10-15

Status: verified against reachable primary sources on 2026-10-15 (see
commercial-readers-sources.md for the citation list and the verification
method at the end of this document).

Purpose, and how this audit is allowed to be used (clean-room invariant)

This audit records WHAT the commercial reader products can read: which file
formats, which entity classes (exact B-Rep geometry, NURBS math, tessellation
meshes, PMI, assembly structure, metadata). It is a functional-spec source for
the Keelworks import roadmap. It is deliberately NOT a source of HOW those
products read data. No vendor implementation detail, technique, data
structure, or code is reproduced or adopted here. Keelworks builds its
readers from lawful observation of public formats and public specs (STEP,
3DXML license, CGR behavior) only, per master plan invariant 5. Treat any
claimed commercial capability below as a target to match or exceed with our
own clean-room implementation, never as a recipe.

The four products named on the card, in order of relevance to the Keelworks
native targets (CGR, 3DXML, CATPart/CATProduct):

1. Spatial HOOPS (HOOPS Exchange SDK, now under Tech Soft 3D)
2. CAD Exchanger SDK
3. TransMagic (SUPERVIEW / PRO / EXPERT)
4. Okino (PolyTrans|CAD, with CAD/Pack, Granite/Pack, DCC/Pack modules)

---

## Spatial HOOPS (HOOPS Exchange SDK)

Product. HOOPS Exchange is the commercial multi-CAD reader SDK from Spatial
Corp, which is now part of Tech Soft 3D (the HOOPS brand continued). Its
documentation, including the "File Formats" tables below, lives at
docs.techsoft3d.com. HOOPS Exchange exposes one standard API over dozens of
native and neutral formats, and reports for each format which entity classes
it provides.

Entity classes this product can read: the HOOPS Exchange file-format tables
carry explicit columns for Tessellation, B-Rep, and PMI per format. The
CATIA V5 reader page states it provides full access to the assembly, B-Rep,
visualization, view, and PMI data stored in a CATIA V5 file, and that B-Rep
support is based on the CGM modeler with evaluators that query the geometric
types directly, plus surface approximation to standard NURBS form.

Native CATIA format claims (functional-spec reading):

- CATIA V4: up to 4.2.5 (MODEL, SESSION, DLV, EXP), Tessellation and B-Rep.
- CATIA V5: up to V5-6R2026, extensions CATDrawing, CATPart, CATProduct,
  CATShape, CGR. Tessellation: supported. B-Rep: supported (CGM based).
  PMI: supported.
- CATIA V6 / 3DEXPERIENCE: 3DXML 4.3.
- CGR specifically: HOOPS Exchange can directly access the cached
  visualization data (called CGR) stored in a CATIA V5 file when the reading
  mode is set to tessellation only, via its cache-activation parameter. So
  CGR is read as a tessellated representation, not as B-Rep.

Neutral and other formats (functional-spec reading): all with per-format
Tess/B-Rep/PMI columns: 3MF, ACIS (SAT/SAB), AutoCAD DWG/DXF (2D and 3D),
3DS, DWF/DWFX, Autodesk Inventor (IPT/IAM), Navisworks (NWC/NWD),
COLLADA, Creo/Pro-E (ASM/NEU/PRT/XAS/XPR), DGN, FBX, glTF/GLB, I-deas,
IFC (up to IFC4.3), IGES, JT (up to v10.11), NX/Unigraphics, Parasolid up to
38.1, PDF, PRC, QIF 3.0, Revit (RVT/RFA), Rhino (3DM), Solid Edge,
SolidWorks, STEP (AP203 Ed1/Ed2, AP214, AP242 Ed1/Ed2/Ed3, plus STEP/XML),
STL, U3D, USD, VRML, OBJ.

PMI claim: PMI is provided in both visual form (reproduce how the model
looked in the authoring system) and semantic form (structured data for
developers). For CATIA V5 the documented PMI types include datum, datum
target, dimension variants, distance, flags, feature control frame,
geometrical tolerance, note object attributes, roughness, and weld.

Commercial posture and status: proprietary commercial SDK. Per
docs.techsoft3d.com the vendor aims to update CATIA V5, SolidWorks, Creo,
and NX readers within 90 days of each authoring-software release, and JT
within 120 days. The 2025.4 and current (hoops) documentation snapshots both
list CATIA V5 up to V5-6R2025/2026, confirming active maintenance.

## CAD Exchanger SDK

Product. CAD Exchanger SDK is a commercial, format-neutral CAD/BIM data
exchange SDK (C++, Python, C#, Java, JavaScript bindings) with a single
unified data model over 30+ supported formats. It models the data it reads
in its own structure with visitors over part-assembly trees, and B-Rep and
mesh geometry, drawings, metadata, and PMI.

Entity classes this product can read: the CAD Exchanger SDK "supported
formats" tables and the marketing scope page both enumerate the entity
classes the SDK keeps: exact geometric data (circles, planes, NURBS curves
and surfaces, etc), topological bodies (solid, sheet, wireframe) and
elements (shells, faces, edges), polygonal representations (meshes) including
multiple level-of-detail (LOD) sets, 2D drawing geometry and annotations
(dimensions, hatches, sheets, views), product structure (assemblies, parts,
instances including external references), metadata (names, user-defined and
validation properties), PMI in graphical and semantic form with associations
to B-Rep faces and edges, and visual attributes (colors, materials, textures,
layers).

Native CATIA format claims (functional-spec reading):

- CATIA V5R16 to V5R34: CATPart, CATProduct, CGR. For import the documented
  scope is B-Rep representations, polygonal representations, assembly
  structure (including via external files), names, user-defined properties,
  colors, and graphical PMI. CAD Exchanger's own format list marks CATIA as
  read-only (no dedicated CATIA writer line).
- CATIA V6 / 3DEXPERIENCE: format list shows CATIA V6 (3D XML) as a
  proprietary import format.

Neutral and other formats (functional-spec reading): among complex CAD
formats STEP, JT (8.0 to 10.9), IGES, PRC, DXF, 3D PDF, IFC; geometric
modeling kernels ACIS, Open CASCADE, Parasolid, Rhino, C3D; mesh formats
3DS, 3MF, COLLADA, FBX, glTF, OBJ, PLY, STL, U3D, USD, VRML, X3D; and the
proprietary CAD formats Inventor, Creo, Revit, Solid Edge, SolidWorks, NX,
DWG.

STEP/PMI claim: for STEP the SDK supports AP203, AP214, AP242 with B-Rep and
polygonal representations, PMI import and export, external references, and a
parallel processing boost to import speed. PMI is handled at three levels:
graphical PMI, semantic PMI, and associations with B-Rep elements.

Commercial posture and status: proprietary commercial SDK. The vendor
publishes rolling documentation (docs324x.cadexchanger.com) and an active
product site (cadexchanger.com); the SDK page promotes recent capabilities
(PMI insights, mesh generation, model editing). Datable to 2026 by the
R2026-era format spans in the docs.

## TransMagic (SUPERVIEW, PRO, EXPERT)

Product. TransMagic is a commercial CAD viewing and translation suite. All
core products (SUPERVIEW, PRO, EXPERT) read the CAD formats; write
capability tiers are PRO (kernel and neutral formats) and EXPERT (also
native formats). TransMagic states it licenses formats directly from their
owners rather than reverse engineering them.

Entity classes this product can read: TransMagic reads hybrid CATIA files
that contain both B-Rep (CAD) and visualization (CGR Visrep) data and lets
the user open them as B-Rep or as visualization geometry. Its CATIA
documentation explicitly distinguishes the CGR as a polygonal (faceted)
visualization representation used for large assemblies, versus the CGM
geometric modeling kernel file which carries full B-Rep compatible with
CATIA V5/V6. It surfaces PMI graphically, in an assembly browser, and in a
per-view PMI browser, and can export PMI into 3D PDF.

Native CATIA format claims (functional-spec reading):

- CATIA V4, V5 and V6: Model, CATPart, CATProduct, CGM (xCGM kernel files)
  and CGR graphics files, read in SUPERVIEW, PRO, and EXPERT.
- CGR and hybrid handling: CGR Visrep models are read as polygonal
  representations; hybrid Brep/Visrep assemblies are auto-detected and the
  user chooses B-Rep or Visrep. TransMagic EXPERT is the only tier that also
  writes CATIA formats (CATPart, CATProduct, CGM, Model, CGR).

Neutral and other formats (functional-spec reading): read of NX/UG (PRT),
Solid Edge (PAR/ASM), SOLIDWORKS, Creo/Pro-E, Inventor, AutoCAD (DWG/DXF),
JT (including JT config files), Parasolid (X_T), ACIS (SAT/SAB/ASAT/ASAB),
STEP, IGES, VDA-FS. The vendor markets conversion to and from 48 different
3D CAD formats, including native, kernel, open-standard (IGES, JT, STEP),
and documentation/graphics (3D PDF, Collada, Web GL), plus Model-Based
Definition (MBD) support.

Commercial posture and status: proprietary commercial product line with a
free 7-day trial. Vendor patently maintains it: an August 2022 release
announced support for CATIA V5 2022 (R32), CATIA V6 2022 (R32), Creo 9.0,
Inventor View 2023, and ACIS 2022 (R32); the format marketing page
(transmagic.com/cad-file-converter and catia-file-converter) is current.

## Okino (PolyTrans|CAD, with CAD/Pack, Granite/Pack, DCC/Pack)

Product. Okino Computer Graphics makes PolyTrans and NuGraf, standalone CAD
to non-CAD conversion programs (with native plug-ins for 3ds Max, Maya,
Softimage). Its CAD input is sold in add-on packs: CAD/Pack (Autodesk
family, SolidWorks, Solid Edge), Granite/Pack (ProE/Creo, IGES, Parasolid,
STEP, ACIS, VDA-FS via PTC Granite), CATIA v4 and CATIA v5 modules, and
Spatial-ACIS and JT Open modules. Okino explicitly states it does not use
reverse-engineered technology and links against the main BREP solid geometry
engines from the respective CAD companies, or embeds the authoring runtime.

Entity classes this product can read: Okino import is B-Rep solids focused
(for CATIA, STEP, IGES, Parasolid, ProE/Creo through license modules), plus
trimmed NURBS, and it converts entire scenes including meshes, hierarchy,
pivot points, vertex normals, UV tangent vectors, vertex colors, texture
coordinates, textures, lights and cameras. The brochure's format matrix
flags each format for Import/Export, Materials and texture maps, Hierarchy,
UV coordinates, Lights and cameras, NURBS, Animation, and Skinning.

Native CATIA format claims (functional-spec reading):

- CATIA v5: import from native CATProduct and CATPart files; the Okino
  brochure's CAD table also lists CGR among the CATIA v5 extensions
  (catproduct, catpart, cgr) and states the CATIA v5 module "utilizes CATIA
  v5 components" / "the actual Dassault CATIA v5 runtime system ensuring
  100% fidelity."
- CATIA v4: import from native MODEL files, via the Spatial-CATIA4 module.

Neutral and other formats (functional-spec reading): a broad DCC/VisSim list
(3ds Max, Maya, Cinema-4D, LightWave, Softimage, COLLADA, FBX, VRML,
Wavefront OBJ with NURBS, U3D from PDF, X3D, OpenFlight) plus CAD inputs by
pack: Inventor (IAM/IPT), DWG/DXF/DWF-3D, Solid Edge, SolidWorks, ProE/Creo
native and neutral, IGES (solids and surfaces), Parasolid, STEP (AP203,
AP214), ACIS SAT, VDA-FS, DGN, IFC, PRC and 3D PDF, Rhino/OpenNURBS,
SketchUp.

Commercial posture and status: proprietary commercial software, perpetual
licenses with optional maintenance, module packs priced separately
(per okino.com/conv/order). Actively maintained; the vendor's format pages
and brochure PDF are current and carry 2026-era references.

---

## Capability matrix: what commercial readers claim for the Keelworks targets

The card asks specifically what entity classes the industry can read. The
matrix records, for the Keelworks-native targets, which entity class each
product claims.

Columns: Products (Exchange = Spatial HOOPS Exchange, CX = CAD Exchanger SDK,
TM = TransMagic, OK = Okino). Cell values: Y = claimed, T = tessellation
only, blank = not claimed for that target.

| Target format | Entity class          | Exchange | CX | TM | OK |
| ---           | ---                   | ---      | ---| ---| ---|
| CATPart/CATProduct | B-Rep           | Y (CGM)  | Y  | Y  | Y (via CATIA runtime) |
| CATPart/CATProduct | Tessellation     | Y        | Y  | Y  | Y |
| CATPart/CATProduct | PMI               | Y (vis+sem) | Y (graph. PMI) | Y (to 3D PDF) | n/a stated |
| CATPart/CATProduct | Assembly structure | Y       | Y (extern refs) | Y  | Y |
| CGR              | Tessellation only   | Y (cache mode) | Y  | Y (Visrep) | listed (.cgr) |
| CGR              | B-Rep               | n/a (tess only) | n/a | n/a (Visrep) | n/a |
| 3DXML (V6)       | (per 3DXML spec)    | Y (3DXML 4.3) | Y (CATIA V6 via 3D XML) | CATIA V6 read | n/a stated |
| STEP (AP242)     | B-Rep + tess + PMI  | Y Ed1-3  | Y AP242 | Y  | Y AP203/214 |

Reading the matrix: every one of the four products reads CATPart/CATProduct
B-Rep and tessellation, and every one treats CGR as a tessellated,
visualization-only representation, never as exact B-Rep. That is the single
strongest functional signal in this audit for our P2 planning: the industry
consensus is that CGR is the fast, large-assembly visualization layer while
exact geometry lives in the B-Rep path.

---

## Synthesis: what the first parcel must cover

This synthesis is the bridge from vendor capability claims to the Keelworks
workstream priorities; P1-M3-T03 (the P1 report) will turn it into the
prioritized import-target list and the first parcel.

Entity classes the industry reads, per this audit. Across all four vendors
the recurring entity classes are: exact B-Rep and NURBS for the native CAD
formats; tessellation meshes (with LOD) as the universal visualization layer;
PMI in graphical and semantic form with B-Rep associations; assembly product
structure including external references; and metadata plus visual attributes.
Our readers should target exactly those classes, in that order of value, and
no proprietary technique is needed to match them.

Implication for P2 (oep-cgr). CGR read as tessellation only is what industry
does everywhere. P2's mesh extraction plan (P2-M2) is therefore correct and
sufficient as the CGR first move; matching the commercial scope here means
assembly/instance handling (P2-M4-T02) and color/material tables (P2-M4-T01)
are required for parity, since all four vendors claim them for CGR.

Implication for P3 (oep-3dxml). Commercial readers claim CATIA V6 via 3DXML
4.3; our P3 ASCII reader plus the binary .3drep branch decision (P3-M3)
aligns with what industry actually consumes for the V6 path.

Implication for P4 (oep-step). STEP AP242 with B-Rep, tessellation, and PMI
in both visual and semantic form is claimed by CAD Exchanger and HOOPS
Exchange, and PMI-to-3D PDF is a TransMagic selling point. P4's semantic PMI
import (P4-M2-T01) and PMI re-export (P4-M2-T02) directly match the
industry-read entity class of semantic PMI with B-Rep associations; the
commercial feature lists corroborate that AP242 PMI fidelity is the
differentiating capability, not plain B-Rep STEP.

First parcel direction. The audit's functional evidence points to a first
parcel of: CATPart/CATProduct B-Rep plus tessellation, with assembly
structure, and CGR as tessellation only, with PMI as the follow-on differentiator.
That matches the existing critical path (P1 corpus to P2/P3 readers to P6
geometry core) and gives P1-M3-T03 concrete vendor-claimed targets to rank.

---

## Verification (real-material check on 2026-10-15)

Every claim above was checked against a reachable primary source on
2026-10-15, listed with URLs in commercial-readers-sources.md. Sources were
the vendors' own documentation and format tables (docs.techsoft3d.com for
HOOPS Exchange, docs324x.cadexchanger.com and cadexchanger.com for CAD
Exchanger, transmagic.com for TransMagic, okino.com for Okino). Where a
claim came from a vendor marketing page rather than a format documentation
table, it is marked as a marketing claim in the sources file. Vendor claims
of capability are recorded as claims, not as verified-by-Keelworks behavior:
we have not executed any of these products, and we do not link or copy any
of their code, per the clean-room invariant. The functional target for
Keelworks is to match or exceed these entity-class claims with clean-room
implementations, verified on our own corpus.
