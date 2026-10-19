# Public 3DXML Sample Catalog (P1-M1-T02)

Version: 1.0, 2026-10-15
Author: Keelworks default worker
**Task:** Locate public 3DXML samples (Dassault published samples, open collections) + obtain 3DXML spec under Dassault free internal-use license. This catalog covers the sample-location half; the specification request process is documented separately in [3dxml-spec-process.md](3dxml-spec-process.md).

## Summary

Eight public 3DXML samples were located and verified on real material this
session. Six are redistributable under permissive open licenses and are
committed to `corpus/data/3dxml/`. Two are reference-only: the official
Dassault gallery sample (hosted by 3ds.com without a stated redistribution
license) and a DMG-Lib academic sample (private non-commercial terms). No
sample here is proprietary Dassault product data; all are demonstration or
test models authored for exchange-format examples.

## Catalog

| Name | Source collection | URL | License | Redistributable | Size (B) | sha256 (first 32) |
|---|---|---|---|---|---|---|
| cube1.3dxml | trimesh models (mikedh/trimesh) | https://github.com/mikedh/trimesh/blob/main/models/cube1.3dxml | MIT | yes | 4834 | 8a4d4daef69baf11f674f41d9e022724 |
| blocks.3dxml | trimesh models (mikedh/trimesh) | https://github.com/mikedh/trimesh/blob/main/models/blocks.3dxml | MIT | yes | 2583 | 14ec76309f176fd5f70428d9f8b76664 |
| cycloidal.3DXML | trimesh models (mikedh/trimesh) | https://github.com/mikedh/trimesh/blob/main/models/cycloidal.3DXML | MIT | yes | 742428 | 007751ab8639097ca146431b0d0c605a |
| FM1_satellite.3DXML | trimesh models (mikedh/trimesh) | https://github.com/mikedh/trimesh/blob/main/models/FM1_satellite.3DXML | MIT | yes | 1763813 | 7ff05256666d63199ac16aa041b2869a |
| machinist.3DXML | trimesh models (mikedh/trimesh) | https://github.com/mikedh/trimesh/blob/main/models/machinist.3DXML | MIT | yes | 38849 | 2203d90f73d47cbb088a9e3b5f4e6178 |
| quad.3dxml | i2e-haw-hamburg/cad-in-unity | https://github.com/i2e-haw-hamburg/cad-in-unity/blob/master/3D%20XML%20Example/quad.3dxml | Apache-2.0 | yes | 14317 | 24459eb1e786451d614c21f6a5cbca3b |
| press_rink_axe_fta.3dxml | Dassault 3D XML Gallery (official) | https://www.3ds.com/fileadmin/PRODUCTS/3DVIA/3DXML/gallery/3dxml_models/press_rink_axe_fta.3dxml | Dassault-hosted, no stated redistribution license | reference-only | 609574 | 28778061da869c2a1e70a21e527a86c4 |
| landinggear.3dxml | DMG-Lib (Digital Mechanism and Gear Library) | https://www.dmg-lib.org/dmglib/handler?caxfi=5563053 | private, non-commercial, single download (§53 German CA) | reference-only | n/a | n/a |

## Collection notes

### Dassault published samples (official 3D XML Gallery)
- Location: https://www.3ds.com/fileadmin/PRODUCTS/3DVIA/3DXML/gallery/press.htm
  ("3D XML Gallery" page, maintained on 3ds.com).
- Sample verified this session: press_rink_axe_fta.3dxml, 609,574 bytes, a ZIP
  3DXML containing a transmission assembly (Press Rink Axe) with `.3DRep`
  tessellated parts, FTA (3D annotations) representation, Manifest.xml and
  .IntegrityCertificat, authored 2009-05-26 in CATIA V5 Imagine & Shape.
- The page authoring line confirms "Authoring : CATIA V5 Imagine & Shape".
- License verdict: 3ds.com makes these files freely downloadable but does not
  publish a redistribution license, so they are catalogued here (URL + sha256)
  and NOT committed to the shared corpus (conservative, license-clean stance).
- The 3DXML Player itself is also downloadable from the same portal family.

### Open collections
- trimesh model fixtures (https://github.com/mikedh/trimesh): the project is
  MIT-licensed and ships 3DXML files under models/ that its own threedxml
  loader tests against; authoritative examples of the ZIP-archive 3DXML
  variant (Manifest.xml + Abaqus_Model.3dxml + .3DRep geometry).
- i2e-haw-hamburg/cad-in-unity (https://github.com/i2e-haw-hamburg/cad-in-unity),
  Apache-2.0, ships a `3D XML Example/` folder that includes pure-ASCII 3DXML
  (quad.3dxml) and a companion binary `.3DRep` (Chassis.3DRep) used by its Unity
  CAD importer. The ASCII variant is the branch the GLC_Player open-source viewer
  can already read (glc-player.net lists "3DXML V3 & V4 (Ascii)" support).
- DMG-Lib (dmg-lib.org): scholarly CAx repository with 3DXML files (e.g. "aircraft
  landing gear", ~508 KB). Access is free but the download terms (single,
  non-exclusive, non-transferable, private non-commercial use) make the files
  reference-only for our corpus. Not committed.
- GrabCAD Community Library tag "3dxml" (https://grabcad.com/library/tag/3dxml):
  hosts user-uploaded 3DXML models; each model has its own uploader-set terms, so
  they would need per-file license verification before corpus entry. Listed here
  as a source to mine, not yet ingested.

## Format observations (real material, 2026-10-15)

Two structural branches confirmed on the verified files:

1. ZIP-archive 3DXML (most common): a `.3dxml`/`.3DXML` file is a ZIP container
   (magic `PK`). Contents observed:
   - Manifest.xml (tiny, ~194-259 B)
   - A `<name>.3dxml` model document with `<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML">`
     root, `<Header><SchemaVersion>4.3</SchemaVersion>`, ProductStructure
     (Reference3D/Instance3D), and `<ReferenceRep ... associatedFile="urn:3DXML:*.3DRep">`
   - Zero or more binary `.3DRep` files (tessellated geometry: TessPart_N.3DRep,
     Abaqus_Geometry.3DRep, ...) and optional material/rendering reps
   - Optional CATMaterialRef.3dxml and Preview2D.png, plus .IntegrityCertificat
2. Pure-ASCII 3DXML (rare, preferred by open tooling): plain XML file, no ZIP
   wrapper. Confirmed on quad.3dxml (14,317 B), schema 4.3, CATIA V5 2009
   authoring, self-contained XML geometry.

Full schema-version matrix and hex-level notes will be built in P1-M2-T04
(3DXML ASCII vs binary .3drep branches) on top of this corpus.

## Parity with sibling P1-M1-T01 (CGR samples)

CGR and 3DXML are both transfer-file families without stated open specs in the
same way STEP is; both are being catalogued identically (URL + license +
redistributability + hash), and both keep vendor-hosted demo files out of the
repo when no redistribution license is stated. 3DXML's difference: the ASCII
branch is already parseable by open tooling (trimesh threedxml loader, GLC_Player),
which de-risks the P3 reader milestone ahead of any spec licence.

## Sources

See [sources.md](sources.md) for the full source register and
[verification.md](verification.md) for the real-material verification record.
