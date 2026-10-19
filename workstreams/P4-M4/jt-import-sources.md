# P4-M4-T02 JT Import Decision: Primary Source Evidence

This file records the verifiable primary sources behind the JT import decision
ADR (workstreams/P4-M4/jt-import-decision-adr.md). Each row is a claim the ADR rests on,
the primary source it was checked against, and the verification date.

## Format status: JT is documented and standardized

| Claim | Primary source (URL) | Verified |
| --- | --- | --- |
| JT is an ISO standard: ISO 14306 (3rd edition in progress; Part 3 for JT 9.5 published June 2025, Part 4 for JT 10.5 in FDIS ballot Nov 2025) | https://www.prostep.org/en/projects/jt-project-groups-jt-wf-jt-if-jt-bm | 2026-10-16 |
| The free Prostep IVIP publication PSI 14-1 / JTIAP (JT Industrial Application Package, based on JT 10.5 spec) is the openly published file-format description; DIN SPEC 91383:2024 is the JTIAP standardized as a DIN SPEC | https://www.prostep.org/fileadmin/prod-pay-download-8c1d/JTIAP_V3_Format_Description_and_annexs.pdf and https://webstore.ansi.org/standards/din/dinspec913832024 | 2026-10-16 |
| Siemens publishes the JT File Format Reference (v10 Rev-B / v10.5) freely to view | https://www.plm.automation.siemens.com/en_us/Images/JT-v10-file-format-reference-rev-B_tcm1023-233786.pdf | 2026-10-16 |
| JT carries PMI in the Meta Data Segment (PMI Manager Meta Data Element, section 9.2 of the spec) plus assembly/LSG, LODs, attributes, XT/STEP B-Rep segments | JTIAP V3.1 format description (TOC sections 6, 7, 8, 9, 11) | 2026-10-16 |

## OSS JT readers (adoption candidates)

| Claim | Primary source (URL) | Verified |
| --- | --- | --- |
| OCCT JT Assistant / TKJT is the mature OSS JT reader, published by Open Cascade / community forks, GPL-2.0-only | https://github.com/Open-Cascade-SAS/JT-Assistant (License: GNU GPL v2.0), https://github.com/cbsghost/oce-jt (GPL-2.0), https://git.dev.opencascade.org/gitweb/?p=jt.git | 2026-10-16 |
| FreeCAD has an optional JT reader built on TKJT but excludes TKJT from its own mainline specifically because TKJT is GPL ("Because TKJT is under the GPL it won't be added to FreeCAD"); FreeCAD's own JtReader in tree is LGPL-2.1-or-later glue around the external toolkit | https://github.com/FreeCAD/FreeCAD/pull/11323 (wwmayer comment); https://github.com/Connor9220/FreeCAD/commit/cdb990e810c3ee0835df121db563f6afb308922a | 2026-10-16 |
| PyOpenJt (OpenCascade TKJT + Python binding) is GPL-2.0; its README states GPL applies to all linked work | https://github.com/jriegel/PyOpenJt | 2026-10-16 |
| Voyager (Java, JT v10 experimental reader) is Apache-2.0 but WIP/experimental: incomplete topology decoding, no B-Rep/PMI, created 2025, 0 stars, single author | https://github.com/TobiasMusin/Voyager | 2026-10-16 |
| svr2kos2/JTReader (C#/.NET) is MIT but minimal and partial | https://github.com/svr2kos2/JTReader | 2026-10-16 |
| JTConvert (C#) is LGPL-3.0, a read/convert CLI; dxjt_toolkit's parsing dll is NOT free (only usage/demo code open) | https://github.com/space928/JTConvert ; https://github.com/bitbybit3d/dxjt_toolkit | 2026-10-16 |
| No standalone, maintained, license-clean pure-Rust JT reader exists on crates.io (queries: "jt reader", "jupiter tessellation", "jt viewer" return nothing relevant; cadmpeg-codec-nx reads embedded JT display models inside NX .prt, scoped to Parasolid/NX; document-svg jt.rs only sniffs the fixed header) | https://crates.io/api/v1/crates?q=jt%20reader ; https://docs.rs/cadmpeg-codec-nx ; https://docs.rs/document-svg | 2026-10-16 |

## Commercial / proprietary (functional spec reference only, per P1-M3 convention)

HOOPS Exchange reads JT up to v10.11 (tessellation + B-Rep + PMI);
CAD Exchanger lists JT 8.0 - 10.9; Datakit, Okino (JT Open module), 3D-Tool,
TransMagic also market JT. These are price anchors and functional-spec
reference, NOT adoption sources (program clean-room invariant: no vendor
binaries or licensed SDKs in core). Source: workstreams/P1-M3/commercial-readers-audit.md
and https://docs.techsoft3d.com/hoops/exchange/start/format/jt_reader.html

## Program context

- Keelworks license posture (P0-M2): Apache-2.0 applications/libraries,
  AGPL-3.0 server/PDM and P2P orchestration components. GPL-2.0-only is
  incompatible with Apache-2.0 for linked libraries and with AGPL-3.0 for the
  server (a combined work cannot carry both).
- Clean-room invariant (doctrine rule 5): no vendor binaries, no copied
  source, no Dassault/Autodesk/Siemens libraries; public specs + lawful
  observation only.
- P4-M1 precedent (workstreams/P4-M1/step-library-adr.md): oep-step chose a pure-Rust OSS
  STEP library (step-io, MIT OR Apache-2.0) behind a vendor-neutral seam, and
  rejected OCCT as a linked dependency for wave 1 because CI runs in a minimal
  rust:1-slim container with no C++ toolchain.
- P1-M3 readers audit (workstreams/P1-M3/oss-readers-audit.md): for undocumented CATIA
  formats (CGR, 3DXML binary) the program builds its own readers; documented
  interoperable formats (STEP) are wrapped from OSS. JT is documented, so it
  falls in the documented/wrap-or-build-on-spec family.

## P4-M4-T03 wedge delivery facts (2026-10-16)

- The JT reader visualization-first wedge is delivered on branch
  task/jt-reader-1016a as an oep-step `jt` module behind the same
  vendor-neutral seam as the STEP lane (oep_step::jt::parse -> JtModel ->
  to_document -> oep_geo Document).
- Implementation is clean-room, spec-driven: every layout is cited to the
  Siemens JT v10 reference Rev-B / JTIAP PSI 14-1 V3 sections in
  docs/re-notes/oep-jt-container-and-lsg.md and workstreams/P4-M4/jt-reader-plan.md.
  The decode notes are the living spec-to-code map for this workstream.
- In scope and GREEN: file header (version, byte order, block-directory
  offset, root reference), block directory walk, block headers, element
  stream walk (compressed form + plain form), LSG traversal (partition,
  part nodes, string + late-loaded property atoms, property table), shape
  decode for the documented uncompressed Primitive Set Box path, the
  Bitlength CODEC prefix-code core, and mapping onto the canonical
  oep-geo Document (one part / one Solid body / ImportJt feature).
- Golden: corpus/jt/golden-box-a.jt is a license-clean synthesized fixture
  written from the documented byte layout by
  python/oep_tooling/jt_fixtures.py (no third-party JT file is
  redistributed; decision-7 dev-only posture). crates/oep-step/tests/
  jt_golden.rs pins 8 vertices / 12 triangles and bounding volume 6.0
  within tolerance through both the vendor-neutral model and the canonical
  document.
- Corrupt corpus (7 variants: empty, junk, truncated header, bad byte
  order, out-of-range directory offset, block overrun, non-LSG root)
  returns typed errors and never panics, under std::panic::catch_unwind.
- Gate scans (tests/test_p4m4_jt_reader.py, run in CI): (a) vendor-neutral
  seam - no JT container/codec vocabulary on public items; (b) no-vendored
  foreign source - TKJT / JT Assistant / PyOpenJt / Voyager / JTConvert
  identifiers fail the build if present in code.
- Documented-but-deferred (typed UnsupportedFeature, never fabricated):
  LZMA, arithmetic and Deering codec payloads, the bitlength nibble bias,
  topologically compressed shapes (v9/v10 TriStrip/Polygon), lossy
  quantized primitives, non-identity orientations, and PMI/Meta Data
  (ADR's gated second milestone).
