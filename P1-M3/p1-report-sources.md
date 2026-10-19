# P1-M3-T03 P1 Report: Sources and Verification List

Version: 1.0 (2026-10-15)
Companion: P1-M3/p1-report.md

This report is a synthesis of the four sibling P1 deliverables committed to
this repo. It performs no new primary research. Every claim in the report
traces to one of the listed sibling documents, which were themselves verified
against reachable primary sources on 2026-10-15. The two audit source lists
below carry the full URL-level citations.

## Sibling deliverables synthesized (verified in this repo on 2026-10-15)

- P1-M1-T01 corpus: P1-M1/cgr-sample-location.md,
  P1-M1/cgr-sample-registry.json, tests/test_cgr_registry_acceptance.py
  (11 real CGR samples located, license-verified, sha256 per file).
- P1-M2-T01 hex notebook: docs/re-notes/cgr-hex-notebook.md,
  python/oep_tooling/cgr_observe.py,
  tests/test_cgr_hex_notebook_acceptance.py (byte-level CGR header/block
  observation from the real samples).
- P1-M3-T01 OSS readers audit: P1-M3/oss-readers-audit.md,
  P1-M3/oss-readers-sources.md, tests/test_oss_readers_audit_acceptance.py.
- P1-M3-T02 commercial readers audit:
  P1-M3/commercial-readers-audit.md, P1-M3/commercial-readers-sources.md,
  tests/test_commercial_readers_audit_acceptance.py.

The OSS audit's citation list (URL level) covers: Spatial glossary CGR
definition (https://www.spatial.com/glossary/cgr-file), RAND 3D on CGR
tessellation (https://resources.rand3d.com/insights-from-within/extracting-geometry-from-cgr-files),
Datakit CGR note (https://www.datakit.com/en/news/who-or-what-is-cgr--92.html),
Just Solve CATPart/CGR identity (http://justsolve.archiveteam.org/wiki/CATPart),
FreeCAD forum CATIA-via-STEP (https://forum.freecad.org/viewtopic.php?t=104361),
FreeCAD Import/Export docs (https://wiki.freecad.org/Import_Export),
FreeCAD Macro 3DXML import (https://wiki.freecad.org/Macro_3DXML_import),
OCCT STEP Translator guide (https://dev.opencascade.org/doc/overview/html/occt_user_guides__step.html),
OCCT STEP wiki with PMI restriction
(https://github.com/Open-Cascade-SAS/OCCT/wiki/step),
OCCT SPDX LGPL-2.1-with-exception
(https://spdx.org/licenses/OCCT-exception-1.0.html),
Assimp Fileformats (no 3DXML/CGR)
(https://github.com/assimp/assimp/blob/master/doc/Fileformats.md),
Assimp issue 3145 declining 3DXML on licensing
(https://github.com/assimp/assimp/issues/3145),
openswx clean-room closed-format reader pattern
(https://github.com/schwitters/openswx).

The commercial audit's citation list (URL level) covers: Spatial HOOPS
Exchange supported formats and CATIA V5 reader (CGR via cache-activation,
tessellation-only)
(https://docs.techsoft3d.com/hoops/exchange/start/supported-formats.html,
https://docs.techsoft3d.com/exchange/latest/start/format/catia_v5_reader.html),
CAD Exchanger SDK formats (CATIA V5 CATPart/CATProduct/CGR scope)
(https://docs324x.cadexchanger.com/sdk/sdk_supported_formats.html),
TransMagic CATIA converter page
(https://transmagic.com/catia-file-converter), Okino CATIA v5 importer docs
(https://www.okino.com/convert/_what_is/what_is_catia.htm).

## Program references (decisions and master plan, not primary format sources)

- Master plan with P1-M3-T03 card, P2/P3/P4/P5 workstreams and decision 2
  (native CATPart reader in two-year scope):
  /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md
- Vault research notes: "CATIA Native Files and 3DEXPERIENCE Deliverables
  Research", "CATIA V5 and V6 Ecosystem Research" (base landscape the
  audits built on).

## Verification method

- All sibling deliverables were read and cross-checked in this repo at HEAD
  of their task branches on 2026-10-15.
- The report's first-parcel target (CGR) is verified on real material by the
  P1-M2-T01 acceptance test (re-derives header facts from real samples) and
  by this report's own acceptance test, which re-hashes every present sample
  and matches sha256 against the P1-M1 registry (11/11 matched on
  2026-10-15).
- Nothing in this report depends on claims the sibling audits marked
  [unverifiable] or [uncertain]; those carry no decision weight here.
