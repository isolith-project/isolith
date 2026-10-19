# Verification record, P1-M1-T02 (real-material check on 2026-10-15)

This task's claims were verified against real material on 2026-10-15
(UTC-07:00). Nothing here rests on un-checked memory: every committed sample
was downloaded from its source URL, its file identity was checked, and its
sha256 was computed and locked into both the catalog and the acceptance test's
COMMITTED_SAMPLES map.

## Method

- web_search + web_extract to locate candidate public 3DXML sources.
- curl download of each sample file from its source URL (raw.githubusercontent /
  3ds.com fileadmin).
- `file` identity check + `unzip -l` (for ZIP-archive 3DXML) or raw XML head
  (for pure-ASCII 3DXML) on each downloaded file.
- sha256 hashing of every file; the hash is what the repo commits and what the
  acceptance test recomputes and compares (integrity source of truth, per the
  P0-M4-T02 corpus-manifest convention).

## Files verified

Committed samples (real bytes in corpus/data/3dxml, sha256 recomputed at commit
time and matched by tests/test_3dxml_samples_catalog.py):

| File | Identity (file) | ZIP contents observed | sha256 (full) |
|---|---|---|---|
| cube1.3dxml | Zip archive (deflate) | Manifest.xml, Abaqus_Geometry.3DRep, Abaqus_Model.3dxml, CATMaterialRef.3dxml, CATRepImage.3dxml, material_Rendering.3DRep, image.bmp | 8a4d4daef69baf11f674f41d9e022724f0b5a510bf36f26847479e6df6f04e6b |
| blocks.3dxml | Zip archive | Manifest.xml, Abaqus_Geometry.3DRep, Abaqus_Model.3dxml | 14ec76309f176fd5f70428d9f8b766640446b61cab8530c8708069dad4c1ead9 |
| cycloidal.3DXML | Zip archive | TessPart_N.3DRep (many), Manifest.xml, MaterialDef_N.3DRep | 007751ab8639097ca146431b0d0c605a2523a269a8dbdc81facd966ddbc0a2de |
| FM1_satellite.3DXML | Zip archive | TessPart_N.3DRep (many), Manifest.xml | 7ff05256666d63199ac16aa041b2869acde06df3ee03d839b5c0436eee2663ee |
| machinist.3DXML | Zip archive | (3DXML ZIP) | 2203d90f73d47cbb088a9e3b5f4e61780c0f65e575f0f27182c10be3f57c6753 |
| quad.3dxml | XML 1.0 document, ASCII text | (pure ASCII, no ZIP) | 24459eb1e786451d614c21f6a5cbca3b88eb226479e931b51673481613319d79 |

Reference-only file (downloaded and identity-checked, NOT committed):

| File | Identity | sha256 (full) | Why not committed |
|---|---|---|---|
| press_rink_axe_fta.3dxml | Zip archive (3DXML), 609574 B | 28778061da869c2a1e70a21e527a86c4de7dbed9a18028ea8a100de979ea26b4 | Dassault-hosted; no redistribution licence stated on 3ds.com gallery |
| landinggear.3dxml | download returned HTML (portal session required) | n/a | DMG-Lib private/non-commercial terms; interactive portal needed |

## Identity / structural checks performed

- ZIP branch: each "Zip archive" file starts with PK\x03\x04 and unzips to
  Manifest.xml + model .3dxml + .3DRep geometry (TessPart_, Abaqus_Geometry_,
  or named part reps), plus optional material/rendering reps. Cycloidal and
  FM1_satellite contain 17+ tessellated part reps, i.e. real assembly-scale
  3DXML, useful for the P2/P3 regression corpus.
- ASCII branch: quad.3dxml is a 14,317-byte plain XML 3DXML with
  `<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML">`, `<Header><SchemaVersion>4.3`,
  ProductStructure (Reference3D/Instance3D/ReferenceRep format=TESSELLATED),
  DefaultView/Viewpoint. Confirmed readable in principle by open tooling
  (trimesh threedxml loader, GLC_Player ASCII 3DXML support).
- Dassault gallery sample: 609,574 B ZIP 3DXML; 22 entries incl. 19 .3DRep part
  files, Transmission.3dxml, FTA (3D annotations) reps, Preview2D.png,
  .IntegrityCertificat; authoring line "CATIA V5 Imagine & Shape" (gallery page).

## Source-UX verification

- trimesh raw URLs return HTTP 200 with the file bytes (downloaded, hashed).
- quad.3dxml raw URL returns HTTP 200 (downloaded, hashed).
- 3ds.com gallery page HTTP 200, contains the direct relative link to
  3dxml_models/press_rink_axe_fta.3dxml; the file download returns HTTP 200.
- dmg-lib page HTTP 200 but the file handler requires an interactive portal
  session (returned HTML, not bytes) - recorded as reference-only.

## Acceptance-test linkage

tests/test_3dxml_samples_catalog.py recomputes each committed sample's sha256
and requires an exact match to the values above; it also requires the catalog to
carry url/license/redistributability/size columns and the spec-process document to
name the request portal and the internal-use limitation. All checks pass on this
real material (see test run output in the task record).

## Status

All checks passed 2026-10-15. Real-material conclusion: the catalog's file types,
licences and hashes are correct as committed. Caveats: the two reference-only
files are catalogued but not in the shared corpus because their hosting terms do
not grant redistribution; the official XSD/licence acquisition remains PENDING
(a human 3DS Passport signup step, see 3dxml-spec-process.md).
