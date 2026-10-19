# P1-M3-T01 Audit: Source and Verification List

Version: 1.1 (2026-10-15)
Companion: workstreams/P1-M3/oss-readers-audit.md

Every claim enforced by tests/test_oss_readers_audit_acceptance.py was checked against a reachable primary source on 2026-10-15. Items marked [unverifiable] failed live verification and carry no weight in the audit conclusions.

## GLC_Player / GLC_lib

- GLC_Player project site, format list: COLLADA, 3DXML, OBJ, 3DS, STL, OFF, COFF. https://glc-player.net
- GLC-Player on SourceForge, last update 2013-04-05, GPLv2 app license. https://sourceforge.net/projects/glc-player/
- GLC_lib library on SourceForge, LGPLv3+ library license, supported formats. https://sourceforge.net/projects/glc-lib/
- GLC_lib package metadata (Alcance Libre repo): LGPLv3+. http://mirror0.alcancelibre.org/aldos/1.4/source/repoview/GLC_lib.html

## CGR format and CGR loaders

- Spatial glossary: CGR definition, tessellated visualization format, no B-Rep. https://www.spatial.com/glossary/cgr-file
- RAND 3D: extracting geometry from CGR files, tessellation-only statement. https://resources.rand3d.com/insights-from-within/extracting-geometry-from-cgr-files
- Datakit: "Who or what is CGR", triangulated faceted format, CGR read/write capability. https://www.datakit.com/en/news/who-or-what-is-cgr--92.html
- CATIA DMU documentation (catiadoc.free.fr): CGR is the default DMU visualization format, representation of geometry only. http://catiadoc.free.fr/online/cfyugdmu_C2/cfyugdmu0011.htm
- Just Solve the File Format Problem: CATPart identity (V5_CFV2 magic), other CATIA formats incl. cgr. http://justsolve.archiveteam.org/wiki/CATPart
- TransMagic CATIA file converter: commercial readers cover CATPart, CATProduct, Model, XCGM, CGR. https://transmagic.com/catia-file-converter
- 3D-Tool CATIA viewer: commercial viewer opens CATPart, CATProduct, 3DXML, CGR without a CATIA license. https://www.3d-tool.com/cad-files/catia-viewer
- jfk-solutions native-cad-viewer FORMAT_SUPPORT matrix: claims MIT-licensed catia-file-format reader for .catpart/.catproduct/.catshape/.cgr. https://github.com/jfk-solutions/native-cad-viewer/blob/main/FORMAT_SUPPORT.md
- jfk-solutions GitHub org public-repo listing (checked 2026-10-15 via GitHub API): 15 public repos, no catia-file-format present. https://api.github.com/orgs/jfk-solutions/repos?per_page=100
- jfk-solutions/native-cad-viewer LICENSE/readme: "Proprietary and confidential... no license to use, copy, modify". https://github.com/jfk-solutions/native-cad-viewer
- Okino CATIA v5 importer documentation: licensed Dassault runtime DLLs, per-year Spatial license maintenance. https://www.okino.com/convert/_what_is/what_is_catia.htm
- File-extension reference: CATPart container is proprietary, no published byte-level structure. https://file-extensions.com/docs/catpart

## FreeCAD importers

- FreeCAD forum: only way to import a CATIA CatPart is via STEP. https://forum.freecad.org/viewtopic.php?t=104361
- FreeCAD documentation, Import/Export overview (STEP, IGES, OBJ, STL, DXF, SVG, DAE, IFC, OFF, NASTRAN, VRML, FCStd). https://wiki.freecad.org/Import_Export
- FreeCAD Feature list: Open CASCADE Technology kernel, STEP and IGES support. https://wiki.freecad.org/Feature_list
- FreeCAD Part Import command: STEP (with/without colors), IGES (with/without colors), BREP. https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Part_Import.md
- Macro 3DXML import (author heda, 2021, LGPL-2.0-or-later): imports 3DXML-ascii (3DRep) meshes and edges only. https://wiki.freecad.org/Macro_3DXML_import
- FreeCAD forum thread on the 3DXML macro: 3DRep is the mesh part of an unzipped 3DXML; binary blobs unsupported. https://forum.freecad.org/viewtopic.php?t=70543
- CAD Exchanger FreeCAD addon (yorikvanhavre) wraps the commercial CAD Exchanger SDK. https://cadexchanger.com/catproduct-to-freecad

## Open CASCADE STEP

- OCCT STEP Translator user guide: reads AP214 (CC2 CD and DIS), AP203, parts of AP242; writes AP203/AP214, AP242 only on explicit write.step.schema=AP242DIS. https://dev.opencascade.org/doc/overview/html/occt_user_guides__step.html
- OCCT Wiki, STEP page: GD&T processing per AP242 recommended practices; restriction: GD&T imported only for shapes/groups, constructive geometry not translated as referenced shapes; saved views not exported. https://github.com/Open-Cascade-SAS/OCCT/wiki/step
- OCCT license: LGPL-2.1 with Open CASCADE Exception (OCCT_LGPL_EXCEPTION.txt). https://github.com/Open-Cascade-SAS/OCCT/blob/master/OCCT_LGPL_EXCEPTION.txt
- SPDX: Open CASCADE Exception 1.0, applies to OCCT 6.7.0 and later. https://spdx.org/licenses/OCCT-exception-1.0.html
- OCCT license page (dev.opencascade.org): LGPL 2.1 with additional exception. https://dev.opencascade.org/resources/download/occt-public-license
- CAD Assistant (Open Cascade): reads STEP AP203/AP214/AP242 with semantic and tessellated PMI plus dimensions and tolerances. https://www.opencascade.com/products/cad-assistant
- Wikipedia, Open CASCADE Technology: format support and AP242 PMI timeline. https://en.wikipedia.org/wiki/Open_Cascade_Technology

## Bob-the-Builder style tools

- Assimp Fileformats list: importers include STP (STEP), no 3DXML, no CGR. https://github.com/assimp/assimp/blob/master/doc/Fileformats.md
- Assimp issue 3145: 3DXML support declined, citing Dassault restrictive licensing (Okino also declined 3DXML for the same reason). https://github.com/assimp/assimp/issues/3145
- openswx: open C++20 reader for SolidWorks SLDPRT/SLDASM, evidence of the clean-room closed-format reader pattern. https://github.com/schwitters/openswx

## Companion program references

- Keelworks master plan, P1-M3-T01 card and P1.3 milestone. /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md
- Vault research note, CATIA Native Files and 3DEXPERIENCE Deliverables Research (no OSS native reader conclusion, CGR commercial-only). /mnt/vault/Notes/Research/CATIA Native Files and 3DEXPERIENCE Deliverables Research.md
- Vault research note, CATIA V5 and V6 Ecosystem Research (interop landscape, Dassault format guards). /mnt/vault/Notes/Research/CATIA V5 and V6 Ecosystem Research.md
