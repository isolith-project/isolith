# P1-M3-T01 Audit: Open-Source Readers for Keelworks Target Formats

Version: 1.1 (2026-10-15)
Owner: Keelworks program, P1 Format Reconnaissance and Corpus (P1.3 baseline readers audit)
Status: baseline readers audit. Output feeds P1-M3-T03 (prioritized import targets and first parcel), and validates the P2 (CGR) and P3 (3DXML) reader assumptions. The audit records what existing open-source readers can and cannot do for the Keelworks target formats, and the concrete gaps the P2/P3 reader work must fill.

## 1. Purpose and scope

The card asks for an audit of existing open-source (OSS) readers in five named buckets: GLC_Player, any CGR loaders, FreeCAD importers, Open CASCADE STEP, and Bob-the-Builder style tools. The point is not to catalog every OSS CAD tool that exists. The point is to answer, for each Keelworks target format, the question that decides the P2/P3 build plan: is there an existing OSS component we can read, fork, or learn from, and where does it stop (the gap)?

Target formats, from the P1 master plan and the format research notes:

- CGR (CATIA Graphics Representation), tessellated visualization format, standalone .cgr and embedded visualization records in CATPart/CATProduct.
- 3DXML (ASCII and binary .3drep branches), the 3DEXPERIENCE-era interchange format.
- CATPart / CATProduct, native CATIA V5 containers (V5_CFV2 magic), exact B-Rep plus feature history, undocumented and release-locked.
- STEP AP203/AP214/AP242, the neutral interchange format used between OEM and supplier.

Method: each bucket below was checked against reachable primary sources on 2026-10-15 (project homepages, SourceForge, GitHub, official format documentation). The citation list is in workstreams/format-corpus/reader-audit/oss-readers-sources.md. Where a claim could not be verified against a live source it is marked [uncertain] or [unverifiable] and does not drive a decision.

## 2. GLC_Player

GLC_Player is an OpenGL, open-source, cross-platform 3D model viewer, with a companion C++ library GLC_lib. It is a viewer, not a parsing library with an API you can link against for data extraction; GLC_lib is the underlying library and is the reusable piece.

What it reads (verifiable from the project site and SourceForge, 2026-10-15): COLLADA, 3DXML, OBJ, 3DS, STL, OFF, COFF.

Critical fact for Keelworks: GLC_Player does NOT read CGR, CATPart, or CATProduct. Its 3DXML support is limited to the ASCII tessellated representation (the XML surface-mesh branch), the same surface the Dassault 3DXML spec publishes, and it reads it for display only. GLC_Player has been effectively unmaintained: the SourceForge project's last binary update is dated 2013-04-05, project version 2.3.0, and it is built on the Qt4 era. GLC_lib source is the still-useful artifact here: it contains a working 3DXML ASCII tessellation reader and a mesh viewer pipeline written in C++, both of which are lawful reference material for a P3 reader (documented format, Dassault free-internal-use spec) but not a maintained dependency.

License: GLC_lib is LGPL (v3+ per package listings), GLC_Player application is GPLv2. This matters for program license posture: a P3 reader that traces or adapts GLC_lib's 3DXML ASCII mesh path stays clean under Apache-2.0 or LGPL for a library, but copying the GPLv2 viewer application wholesale would not be compatible with Apache-2.0.

Status: stale, last binary release 2013, no active release train.

Gap recorded: GLC_Player reads 3DXML ASCII and common mesh formats only. It does not read CGR, CATPart, or CATProduct, has no parse API, and is not maintained. Its ASCII 3DXML mesh logic is a reference implementation at best, never a runtime dependency.

## 3. CGR loaders

CGR is a tessellated visualization format: a faceted (triangle) approximation of the visible geometry, not exact B-Rep, no editable feature data, no semantic PMI (Spatial glossary, RAND 3D, Datakit, DMU documentation, verified 2026-10-15). It is the format exchanged between OEM and supplier for lightweight review and large-assembly DMU work. A standalone .cgr contains no exact geometry, only the mesh plus limited metadata; a CATPart may embed a CGR visualization record alongside the exact B-Rep.

What exists in OSS (verified 2026-10-15):

- No credible, maintained open-source reader for standalone .cgr files was found. Searches across GitHub, the file-format research communities (Just Solve), and the CAD interop literature surface no OSS project that parses the CGR container. The format has no public specification (it is listed on Just Solve only at the identification level, magic/PRONOM identity, not a parse spec). This matches the companion vault research note, which records that CGR readers are commercial-only.
- GLC_Player (section 2) does not read CGR.
- The only public claim of a native CGR/CATPart/CATProduct browser reader found is in the jfk-solutions/native-cad-viewer FORMAT_SUPPORT matrix, which asserts a "MIT-licensed catia-file-format browser reader" that decodes .catpart/.catproduct/.catshape/.cgr. This claim is [unverifiable]: the catia-file-format repository is not public (it returns 404; the org's 15 public repositories do not include it), the reader is distributed as a minified JS bundle inside a viewer whose own license is "Proprietary and confidential... no license to use, copy, modify", and there is no downloadable source, test corpus, or release to inspect. An unverifiable claim is not an OSS dependency and is not evidence a native reader exists; it is recorded as a lead to re-check at P5 time, nothing more.

Commercial-only CGR readers (recorded here as functional-spec reference only, per P1-M3-T02, not as sources of code): Datakit CrossManager (full CGR read/write), TransMagic, Spatial (HOOPS Exchange / 3D InterOp), CAD Exchanger, TechSoft3D, 3D-Tool, and SOLIDWORKS import/export of CGR.

Status of the OSS field: no maintained OSS CGR reader exists as of 2026-10-15.

Gap recorded: native CGR reader absent in OSS. This is exactly the void P2-M1 through P2-M5 are chartered to fill. Because CGR is a tessellation format (documented as faceted triangles in vendor material, but with no public byte-level spec), P2 must proceed on clean-room observation: corpus samples, hex-level notebooks (P1-M2-T01/T02), and a tolerant, never-crash parse model (P2-M1-T04).

## 4. FreeCAD importers

FreeCAD is the most complete OSS parametric modeler and the program's P7 open-modeler candidate. Its import surface is governed by the underlying Open CASCADE kernel plus FreeCAD's own import/export workbenches.

What FreeCAD reads (verified via FreeCAD documentation and forums, 2026-10-15): STEP (AP203/AP214/AP242 via OCCT), IGES, BREP, OBJ, STL, DXF, SVG, DAE, IFC, OFF, NASTRAN, VRML, plus its native FCStd. The Part Import command additionally covers STEP/IGES/BREP with or without colors.

Critical fact for Keelworks: FreeCAD does NOT open CATPart, CATProduct, or CGR natively. The only path to bring a CATIA part into FreeCAD is via an exported neutral format, in practice STEP (confirmed in the FreeCAD forum: the CATIA-to-FreeCAD path is STEP). There is no CGR loader, no CATPart loader, and no CATProduct assembly loader in FreeCAD core.

3DXML in FreeCAD: there is no core 3DXML importer. A single community macro, Macro_3DXML_import (author heda, 2021, LGPL-2.0-or-later, FreeCAD 0.19 era), imports only the ASCII tessellated .3DRep representation: it reads mesh faces (and optionally edges as polylines) from a single reference file, has no binary .3drep support, no Gregory-patch surface decoding, no assembly/instance/materials richness, and its own author notes applicability beyond the one reference file is unknown. It is a proof of concept, not a maintained importer.

The FreeCAD CAD Exchanger addon wraps the commercial CAD Exchanger SDK; per clean-room invariant (no vendor licensed binaries in our core), it is a commercial-path note, not an OSS reader for us.

License: FreeCAD is LGPL-2.0-or-later; the community macro is LGPL-2.0-or-later.

Status: FreeCAD itself is actively maintained; the 3DXML macro is a 2021 one-off and effectively unmaintained.

Gap recorded: FreeCAD imports CATIA native formats only through STEP (no CGR, no CATPart, no CATProduct, no usable 3DXML importer). Its STEP path is good and is the model for P4. Its lack of a CGR/3DXML path is precisely the P2/P3 void, and it confirms a P7 "open CATIA-lite file" workbench cannot lean on FreeCAD's built-ins for these formats; it must call the oep-cgr/oep-3dxml readers we build.

## 5. Open CASCADE STEP

Open CASCADE Technology (OCCT) is the C++ geometry kernel under FreeCAD and the de facto OSS STEP engine. This bucket is here because the card names "Open CASCADE STEP" separately from FreeCAD: OCCT is the STEP library layer, FreeCAD is an application on top of it.

What OCCT's STEP interface reads (verified from official OCCT docs, 2026-10-15): STEP AP214 (Conformance Class 2, both CD and DIS schema versions) as the primary/full target, AP203, and parts of AP242. On the write side it produces AP203 or AP214 by default; AP242 output requires explicitly setting write.step.schema to AP242DIS.

PMI reality per OCCT documentation: GD&T processing follows the AP242 recommended practices for representation and presentation of product manufacturing information, but with a hard restriction: OCCT STEP Reader imports GD&T only when it is assigned to shapes (faces, edges, vertices) or shape groups; constructive geometry is not translated as referenced shapes. Saved views are not exported. So OCCT's AP242 PMI support is real but partial: semantic GD&T on shape-carried annotations yes, dimension/construction-geometry-carried and full fidelity no.

License: OCCT is LGPL-2.1 with the Open CASCADE Exception (SPDX OCCT-exception-1.0; OCCT 6.7.0 and later. Versions 6.6.0 and earlier use the OCCT-PL license). This is the license posture that makes OCCT usable under the hybrid Apache-2.0 / AGPL plan: LGPL-2.1+exception permits proprietary (Apache) linking, and the exception specifically allows using OCCT header material under terms of your choice.

Status: actively maintained by Open Cascade SAS, current 7.x line, used by FreeCAD and hundreds of commercial products.

Gap recorded: AP242 PMI fidelity partial. OCCT is the strongest OSS STEP base, and P4-M1 (wrap an existing OSS STEP library) should adopt it with clean wrappers, but P4-M2 (semantic PMI import and re-export) must budget for the constructive-geometry-carried GD&T restriction and the "write is AP214 by default, AP242 only on explicit schema switch" trap. This matches the master plan's P4-M1 default: bind OCCT, keep the interface clean and vendor-neutral.

## 6. Bob-the-Builder style tools

The card's fifth bucket, "Bob-the-Builder style tools", reads as the class of lightweight, single-purpose, often one-person viewer/convert utilities that "just build" a reader for one format and stop: quick mesh viewers, browser asset pipelines, and one-off importers. For Keelworks these are survey material: they prove the pattern of thin readers and cheap viewers, and they occasionally hold a parse nugget worth referencing.

Survey as of 2026-10-15:

- Assimp (Open Asset Import Library, BSD/MIT): the mainstream C++ asset-importer library. Its import list includes STEP (STP) alongside dozens of DCC/mesh formats, but it does not read 3DXML, CGR, CATPart, or CATProduct. The Assimp maintainers explicitly declined 3DXML, citing Dassault's restrictive licensing terms (the same reason Okino cites for not implementing 3DXML). This is a useful external check on the licensing reality around 3DXML.
- MeshLab / OBJ-STL-PLY tooling: mesh-only, no CATIA formats.
- view3dscene / VTK / OpenSceneGraph viewers: no CATIA-native readers.
- openswx (SolidWorks SLDPRT/SLDASM reader, C++20, MIT-style): a genuine clean-room, open reader for a closed CAD format, evidence the "one person reads a closed format observationally" pattern works. It is SolidWorks, not CATIA, and it is young, but it is the closest living analog to what P2-M1..M4 aim to do for CGR, and its pragmatic posture (parse what is there, diagnose and skip the rest, never crash) is exactly the P2-M1-T04 tolerant failure model.
- jfk-solutions native-cad-viewer: a proprietary viewer with an unverifiable catia-file-format claim; not usable (section 3).

License landscape in this bucket is heterogeneous (BSD, MIT, GPL, proprietary), and most tools here are mesh-oriented, not B-Rep or PMI aware.

Status: mixed, from actively developed (Assimp) to dormant one-offs.

Gap recorded: the Bob-the-Builder bucket confirms there is no reusable OSS reader for any CATIA-native or CATIA-graphics format hiding in the lightweight-tool world either. Assimp's refusal of 3DXML on license grounds and openswx's success with a different closed format sharpen the program's own conclusion: for CGR/3DXML, we build the reader ourselves, observationally, against our own license-clean corpus, and we keep the tolerant never-crash model.

## 7. Capability matrix (what exists vs what Keelworks needs)

| Reader bucket | CGR | 3DXML ASCII | 3DXML binary .3drep | CATPart/CATProduct | STEP AP242 PMI | License usable? |
| --- | --- | --- | --- | --- | --- | --- |
| GLC_Player / GLC_lib | no | partial (mesh view) | no | no | n/a | GPLv2 app / LGPL lib |
| OSS CGR loader | none exists | - | - | - | n/a | n/a |
| FreeCAD core | no | no | no | no (STEP only) | partial (via OCCT) | LGPL-2.0+ |
| OCCT STEP | no | no | no | no | partial | LGPL-2.1 + exception |
| Bob-the-Builder bucket (Assimp et al.) | no | declined (licensing) | no | no | no | mixed |
| Keelworks target (P2/P3/P4) | build oep-cgr | build oep-3dxml | build/scope .3drep | P5 (two-year) | wrap OCCT, P4-M2 | Apache-2.0 / AGPL |

## 8. Gaps (drives P1-M3-T03 first-parcel prioritization)

The authoritative gap table, one row per target format. "Gap" means the existing OSS field does not cover it, so Keelworks work is required.

| Format | Gap | Evidence (2026-10-15) | P1-M3-T03 consequence |
| --- | --- | --- | --- |
| CGR | native CGR reader absent in OSS | No OSS parser found; jfk claim unverifiable; GLC_Player/FreeCAD/OCCT all lack it; format undocumented, tessellated-only | First parcel: CGR mesh read (P2-M1/M2), clean-room observation on corpus |
| 3DXML | binary .3drep branch read gap | GLC and FreeCAD macro cover only ASCII tessellated; .3drep binary unscoped in OSS | ASCII first (P3-M1..M3), .3drep deferred (P3-M3 YAGNI note) |
| CATPart/CATProduct | no OSS native CATPart reader | FreeCAD STEP-only; OCCT no; native undocumented + release-locked | Out of P2/P3 scope, P5 two-year (program decision 2) |
| STEP AP242 | AP242 PMI fidelity partial | OCCT reads AP214 full, AP242 partial PMI, constructive-geometry-carried GD&T not translated, saved views not exported, AP242 write not default | P4 wraps OCCT; P4-M2 budgets PMI fidelity limits; P4-M5 MBD validator surfaces the honesty gap |

## 9. Recommendations feeding P1-M3-T03

1. The first parcel must open CGR to a viewable mesh (P2-M1 header, P2-M2 mesh extract, P2-M5 CLI). Nothing in OSS does this today; it is the wedge.
2. Build oep-3dxml on the documented ASCII path first (P3-M1 outer ZIP + BOM, P3-M2 Gregory surfaces), using GLC_lib's and the FreeCAD macro's ASCII work as reference implementation material, never as linked dependencies.
3. Adopt OCCT as the P4 STEP base (its license allows Apache linking via the exception), with the AP242 PMI fidelity limits written into the P4-M2 test matrix rather than hidden.
4. Do not plan on any existing OSS tool as a runtime dependency for CGR/CATPart; the R1A/R1B corpus and the P1-M2 hex notebooks are the truth source for the readers.
5. Re-check the jfk-solutions catia-file-format claim at P5 kickoff; if it ever publishes source + license + corpus, it becomes a review candidate, not before.

Verified on real material: this audit's negative claims (no OSS CGR reader, FreeCAD lacks CATIA-native import, OCCT AP242 PMI limits, GLC_Player format list) were checked against live project documentation, SourceForge/GitHub listings, official OCCT docs, and the FreeCAD forum on 2026-10-15. The one claim that could not be verified against live material (jfk catia-file-format) is explicitly marked [unverifiable] and is not treated as fact. Citation list: workstreams/format-corpus/reader-audit/oss-readers-sources.md.
