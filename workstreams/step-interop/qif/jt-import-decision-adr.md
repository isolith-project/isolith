---
title: Keelworks JT Import Decision (Visualization / PMI)
decision: build_own_rust_jt_reader_from_documented_iso_spec_visualization_first_reject_gpl_readers
version: 1.0.0
date: 2026-10-16
status: Ratified
owner: Keelworks program governance, P4-M4 (QIF/JT interop lane)
applies_to: oep-step (adopting crate, P4 lane), a future oep-jt reader module or crate behind the same vendor-neutral seam, the P6 importer hub (oep-import feed), and the P6-M1-T02 PMI model
supersedes: nothing (first JT import decision record)
---

# Keelworks JT Import Decision

This record is the binding product decision for the P4-M4-T02 card: whether
Keelworks adopts an existing open-source JT reader or builds its own, for JT
import covering visualization and PMI. The format in question is Siemens
Jupiter Tessellation (JT), file extensions .jt and .jtext, standardized as
ISO 14306 and commonly carrying tessellated geometry, assembly scene graphs,
visual attributes, levels of detail, and Product and Manufacturing Information
(PMI). It is versioned like the boolean-ops decision (P6-M3-T01), the renderer
decision (P6-M5-T01), and the step-library decision (P4-M1-T01); any material
change bumps the version in the front matter and adds a line to the change log
at the bottom.

The default stated in the P4-M4-T02 card is to weigh adoption against build.
This document records the outcome: Keelworks REJECTS the GPL-2.0-only Open
CASCADE TKJT / JT Assistant reader line as a runtime dependency on license
grounds, rejects the immature permissive candidates on quality grounds, and
BUILDS its own Rust JT reader from the documented ISO 14306 / JTIAP format
description, scoped as its own mini-project with visualization first and PMI
staged second. The evidence file is `workstreams/step-interop/qif/jt-import-sources.md`; every claim
below that drives a decision is checked against a live primary source there.

## 1. Decision

Keelworks BUILDS a Rust JT reader (visualization first, PMI second) in the P4
lane, and does NOT adopt any existing OSS JT reader as a linked dependency as
of 2026-10-16. The reader lives behind the same vendor-neutral seam doctrine as
the STEP work in `oep-step` (P4-M1-T01): JT concepts and any borrowed decode
logic never cross into the oep-geo public surface; the seam is where a future
surprise (a maintained license-clean OSS reader, or a commercial-path license)
could swap in without touching callers.

Concrete shape of the decision:

1. Target JT revisions: JT 8.1, 9.5, 10.0, 10.5 (the versions the free JTIAP
   publication documents; ISO 14306 Part 4 for 10.5 was in final-ballot status
   as of the evidence date). The JTIAP V3.1 format description (PSI 14-1),
   DIN SPEC 91383:2024, and the Siemens JT v10 File Format Reference are the
   operating specification sources.
2. First milestone (visualization): file header + TOC + Logical Scene Graph
   (LSG) segment walk, part/instance/LOD nodes, shape LOD mesh element decode
   (vertex, tri-strip/polygon/polyline/point set), material and transform
   attributes, mapped onto the canonical oep-geo facet model. This is the wedge
   that unblocks a viewable JT and feeds the P6 importer hub.
3. Second milestone (PMI): the Meta Data Segment PMI Manager Meta Data Element
   (generic PMI entities, design groups, associations, user attributes, model
   views) into the P6-M1-T02 native PMI model, mirroring how P4-M2 imports STEP
   PMI. PMI is explicitly deferred past the visualization milestone, not
   promised in it.
4. B-Rep: XT (Parasolid) and STEP B-Rep segments embedded in JT are out of
   scope for this mini-project. They are exact-geometry paths with their own
   heavy dependencies (Parasolid is proprietary; STEP B-Rep is already the P4
   STEP lane's job). Visualization facet extraction is the mini-project's
   deliverable.
5. Nothing from TKJT / JT Assistant / PyOpenJt / Voyager / JTConvert /
   JTReader is copied, linked, or vendored. They are reference material only:
   lawful to read for boundary-case understanding, never a source of code or a
   runtime dependency. This matches the P1-M3 audit's rule for reference
   implementations (GLC_lib for 3DXML being the earlier example).

## 2. Why this decision

Four facts drive it:

1. **The only mature OSS JT reader is GPL-2.0-only, and GPL-2.0 is
   incompatible with the program license posture.** Open Cascade's JT Assistant
   and its TKJT reader library, plus the active community forks (oce-jt,
   PyOpenJt, wwmayer/TKJT), are all GNU GPL v2.0. The program's decided posture
   (P0-M2) is Apache-2.0 for applications and libraries, AGPL-3.0 for the
   server/PDM and P2P orchestration. Linking a GPL-2.0-only library into an
   Apache-2.0 library, or into an AGPL-3.0 program, produces a combined work
   whose distribution terms cannot satisfy both licenses. This is not a
   marginal concern: the FreeCAD project, which is itself LGPL, explicitly
   refuses to take TKJT into its mainline for exactly this reason (the
   maintainer: "Because TKJT is under the GPL it won't be added to FreeCAD");
   PyOpenJt's own README states the GPL extends to all linked work including
   user scripts. Keelworks would have to GPL the whole oep-step crate and
   everything that links it. That is a non-starter for a sovereign, Apache-2.0
   product line.

2. **The permissive-license candidates do not meet the quality bar.** Voyager
   (Java, Apache-2.0) is the only permissively licensed reader found, and it is
   explicitly experimental: created 2025, zero stars, single author, partial
   topology decoding, no B-Rep, no PMI, glTF-export-oriented. JTReader (C#,
   MIT) is minimal and partial; JTConvert (C#) is LGPL-3.0 and 2022-era;
   dxjt_toolkit's parsing dll is closed despite the wrapper being shown. None
   is a maintained, license-clean, production-grade JT reader with PMI. Adopting
   any of them would mean inheriting an unfinished parser and still writing the
   hard parts ourselves, under a foreign language and runtime that contradict
   the Rust-first core decision.

3. **JT is a documented format, so a build is spec-driven, not clean-room
   guesswork.** This is the decisive difference from the CATIA formats (CGR,
   native CATPart) where the program must reverse-engineer from observation
   because no spec exists. JT has a published standard (ISO 14306), a freely
   published Prostep IVIP format description (PSI 14-1 / JTIAP, based on the
   JT 10.5 revision), DIN SPEC 91383:2024, and a Siemens file format reference
   freely viewable. The PMI layout, the LSG nodes, the compression codecs, and
   the conformance classes are all written down. Building a reader to a
   documented spec is lower-risk and higher-certainty than the observation-basis
   readers, and it is squarely inside the clean-room invariant (public specs are
   the operative surface, and we write our own code).

4. **The P4-M1 precedent generalizes cleanly.** For STEP, oep-step adopted a
   pure-Rust OSS library (step-io, MIT OR Apache-2.0) because a license-clean,
   maintained, Rust-usable option existed. For JT, no such option exists (no
   crates.io crate parses standalone JT: cadmpeg-codec-nx only reads embedded
   JT display models inside NX .prt, and document-svg only sniffs the fixed
   header). The coherent extension of the P4-M1 rule is: adopt OSS where a
   license-clean, maintained, Rust-usable library exists; otherwise build behind
   the seam. JT falls in the build branch, and its documented spec makes that
   branch tractable.

## 3. Context

- The adopting crate is oep-step (P4), which owns the STEP AP242 / QIF / JT
  interop lane per the monorepo layout. P4-M1-T01 already established the
  vendor-neutral seam and the step-io wrap; the JT reader is a second tenant of
  the same seam, in the same lane, not a new workstream.
- The P6-M1-T02 PMI model (oep-pmi) is the target for JT PMI when the PMI
  milestone runs; it is the same model P4-M2 populates from STEP.
- The P6-M4-T02 importer hub feeds CGR, 3DXML, and STEP adapters into the one
  canonical oep-geo model; the JT visualization adapter is another such feed.
- The corpus is the honest-gap register: the P1/M4 object-store mirror holds
  CATPart/CATProduct/STEP/3DXML/CGR samples but no JT files (verified 0 JT
  mentions in the corpus manifest and object-store index on 2026-10-16). The
  JT mini-project must acquire or synthesize license-clean JT samples under the
  corpus licensing posture (decision 7: dev-only reference material, never
  shipped) before its golden tests can be real.
- The renderer decision (P6-M5-T01, wgpu/WGSL offline headless) is the
  consuming surface for JT visualization: the JT reader feeds facets into
  oep-geo, the renderer draws them. Section 3 of that record (headless,
  no-display contract) is unaffected by this decision.

## 4. Options considered

1. **Adopt Open CASCADE TKJT / JT Assistant (GPL-2.0-only, C++).** The mature
   OSS JT reader and the only one with real-world mileage (it forked out of
   Open Cascade's JT Assistant). REJECTED: license-incompatible with the
   Apache-2.0 libraries posture and the AGPL-3.0 server posture (a linked
   combined work cannot satisfy GPL-2.0 plus either of our terms); it is C++
   on OCCT, which the CI toolchain (rust:1-slim, no C++ compiler) cannot build,
   the same reason P4-M1 rejected OCCT as a wave-1 STEP dependency; and its OSS
   line is visualization-focused with no PMI story in the free tier. Recorded
   strictly as reference material.
2. **Adopt PyOpenJt (GPL-2.0, Python bindings over TKJT).** Same GPL wall as
   option 1, plus it pulls Python into the Rust core for no gain (it is a
   script-convenience layer over TKJT). REJECTED on license; reference material
   only.
3. **Adopt Voyager (Java, Apache-2.0).** License-clean in isolation, and the
   only permissive candidate. REJECTED on quality/maintenance: experimental,
   2025 single-author, no B-Rep, no PMI, partial topology, and it runs on a JVM
   which the Rust-first core does not host. Its Apache-2.0 code is lawful
   reference reading for scene-graph decode, nothing more.
4. **Adopt a C# / .NET reader (JTReader MIT, JTConvert LGPL-3.0).** Foreign
   runtime for the Rust core, partial coverage, unmaintained or single-
   maintainer, and LGPL-3.0 still disagrees with the Apache-2.0 library copy.
   REJECTED.
5. **Build our own Rust JT reader from the documented spec (ADOPTED).**
   Visualization first (header/TOC/LSG/shape-LOD facets to oep-geo), PMI
   second (PMI Manager Meta Data to oep-pmi), B-Rep out of scope. License-clean
   (our own Apache-2.0 code), Rust-native, CI-buildable with the existing
   toolchain, and grounded in ISO 14306 / JTIAP so failures are decode bugs,
   not reverse-engineering blind spots.
6. **Buy a commercial JT reader (HOOPS Exchange, CAD Exchanger, Datakit,
   Spatial, Okino).** These all read JT up to ~v10.x with tessellation and PMI
   and are the price anchor for the whole interop market. REJECTED as adoption
   sources: the program clean-room invariant bars vendor licensed binaries and
   SDKs from the core, and the sovereignty thesis is precisely to not be behind
   a subscription toll. They remain functional-spec reference (what a reader
   must do) per the P1-M3 commercial-readers audit.

## 5. Vendor-neutral seam

The JT reader exposes, through oep-step (or a future oep-jt crate behind the
same discipline, per the layout.md one-crate-per-workstream rule), a public
surface that deals only in Keelworks types: a JT header summary (version,
toc/segment listing), an LSG tree of part/instance/LOD nodes with names and
bounding data, per-node facet meshes (vertices, triangles/polylines/points,
optional colors and transforms), and later PMI entities. No JT segment offsets,
no compression codec internals, no Prostep/Siemens vocabulary appear on the
public surface. The implementation side owns segment walking, the decompression
(bitlength, arithmetic, Deering normal, LZMA as defined in the JTIAP Annex D/F
and section 11), and the tolerant skip logic for unknown elements. This is the
same seam P4-M1 defined for step-io and the importer hub consumes, so a future
drop-in (a maintained license-clean OSS reader, or a commercial license if the
economics ever change) calls for implementation surgery only, never caller
changes.

## 6. Failure and tolerance posture

JT is a complex, versioned, compressed format and real-world files push beyond
any first-pass parser: unknown element versions, unsupported compression
codecs, truncated files, unhandled LOD variants. The mini-project inherits the
program's tolerant never-crash model (P2-M1-T04): unknown blocks are logged and
skipped, not fatal; decode failure on one part node degrades that node, not the
assembly; the reader returns typed errors (HeaderError, SegmentError,
CodecError, ShapeError) instead of panicking. No fabricated geometry is ever
emitted in place of a failed decode; the honesty budget rule from P4-M1 applies
verbatim. The FreeCAD TKJT integration work (PR #29768) is the practical
evidence this matters: real JT 10.x files trigger unsupported-feature warnings
and unreadable sub-structures, and the correct response is a printed warning
plus an honest empty, never a crash.

## 7. License

- Our JT reader code is authored by Keelworks under Apache-2.0 (the decided
  posture for applications/libraries). No GPL code is linked, copied, or
  adapted.
- The candidate readers are the license constraints: TKJT, JT Assistant, and
  PyOpenJt are GPL-2.0-only (incompatible with Apache-2.0 and AGPL-3.0
  combined works); Voyager is Apache-2.0 (compatible permissively but too
  immature to adopt); JTConvert is LGPL-3.0 (disagrees with the Apache-2.0
  library); dxjt_toolkit's parser dll is closed. The only license-clean
  path to a JT reader today is our own build.
- Specification sources are publications, not code: ISO 14306 (purchase), the
  Prostep IVIP JTIAP / PSI 14-1 free publication, DIN SPEC 91383, and the
  Siemens JT File Format Reference (freely viewable). Reading a public spec and
  writing our own implementation is the allowed clean-room surface; the program
  does not need a Siemens or prostep license to read JT.
- The P0-M2 legal pass and the export-ITAR sweep (P1-M1-T06) documented in the
  P7/legal web apply before any public release; the JT reader is internal until
  then, like every other core crate.

## 8. Clean-room

Building a JT reader from published specifications is compatible with the
program clean-room invariant (doctrine rule 5): the operative surface is
"public specs, lawful observation, license-clean sources", and we write 100% of
our own code. We do not link, vendor, or copy TKJT/JT Assistant/PyOpenJt/
Voyager/JTConvert (all either GPL or partial; none can be safely combined).
Reading their sources to resolve spec ambiguities is lawful observation and is
recorded as such in the evidence file; it is not a contribution to our source
tree. The one residual discipline is the same as the OCCT boolean decision:
"OCCT/TKJT does it, so we can copy the approach" is a forbidden habit; any
borrowed algorithmic idea is re-implemented from the spec, not transcribed from
their code, and the mini-project's CI runs a no-vendored-foreign-source scan.

## 9. Mini-project scope

The JT reader is scoped as its own mini-project so a single follow-on card can
execute it end to end:

- Deliverable: a Rust crate (implemented as `oep-step` JT module first, or a
  dedicated `oep-jt` crate per the one-workstream-one-crate rule) that opens a
  JT file and populates oep-geo facets for a viewable visualization.
- In scope, first milestone: header parse (version string, byte-order marker,
  TOC offset), TOC segment walk, LSG segment traversal (part/instance/LOD/
  switch nodes), shape LOD decode for TriStripSet, PolygonSet, PolylineSet,
  PointSet and vertex representations, material (RGBA + name) and geometric
  transform attributes, LZMA + the documented integer codecs, mapping to
  oep-geo Document.
- In scope, second milestone (gated, not in the first): Meta Data Segment PMI
  Manager decode to the oep-pmi model (generic entities, associations, user
  attributes, model views); GD&T semantics follow the P4-M2 STEP PMI work.
- Explicitly out of scope: XT (Parasolid) and STEP B-Rep segment import,
  JT write, animation/VFM, PMI validation.
- Corpus: acquire or synthesize license-clean JT samples (8.1/9.5/10.x),
  catalogued under corpus/license rules as dev-only; golden tests pin decoded
  facet counts/volumes per sample.
- Acceptance gate: a golden JT sample opens to a known mesh (vertex/triangle
  counts and bounding volume within tolerance), an unsupported/corrupt file
  returns typed errors and never panics, and the vendor-neutral seam stays clean
  (no JT vocabulary on public items). This becomes the P4-M4-T02 follow-on
  card's acceptance contract when the mini-project is cut.

## 10. Guardrails

- Version pin: the reader targets the JTIAP V3.1 (PSI 14-1) / ISO 14306 /
  DIN SPEC 91383:2024 description of JT 8.1, 9.5, 10.0, 10.5; the crate
  records the exact spec revision it is built against, and new JT revisions are
  deliberate, tested PRs, not silent tolerance creep.
- Reasonable-tolerance decode: known CDPAR/Deering/LZMA codecs are implemented
  to the spec; unknown or mis-tagged elements are skipped with a logged warning
  (tolerant never-crash model).
- No vendored foreign source: CI scans the crate for the tell-tale TKJT/JT
  Assistant/PyOpenJt identifier set and fails if copied source appears (extends
  the P0-M3-T04 precommit pattern), keeping the clean-room line checkable.
- Seam denial in CI: an assertion that no JT segment/spec vocabulary appears in
  the crate's public API (same family as the OCCT seam denial gate in
  P6-M3-T01), so the vendor-neutral interface cannot silently leak.
- Test gate: golden JT fixtures (license-clean corpus) assert real facet counts
  and volumes; a corrupt-file corpus asserts typed errors and no panics; the
  acceptance script runs in the precommit gate and CI.
- Escape hatch, documented: the seam means a maintained license-clean OSS JT
  reader can replace the implementation behind an unchanged interface, and the
  record mandates an explicit revisit when (if) such a reader matures or when
  the P5 two-year horizon reaches its go/no-go. Nothing here forbids contributing
  to an Apache/MIT JT reader ecosystem; it only says the program does not ride
  a GPL reader or a toy reader today.

## 11. Enforcement in the plan

- Workspace: the JT reader lands as an oep-step module (or dedicated crate)
  registered per the monorepo layout rules; the P4-M4-T02 follow-on card is the
  mini-project cut with the section 9 acceptance contract.
- CI: the no-vendored-foreign-source scan, the seam-denial scan, and the golden
  JT decode tests join the existing pipeline (P0-M4-T01) and the precommit gate
  (P0-M3-T04).
- Corpus: JT samples are added under the decision-7 corpus licensing posture
  (dev-only reference material, never shipped), the same lane P1-M4-T01 used
  for the object-store mirror.
- Review: this decision is revisited at the next P4 milestone gate. A future
  change of heart (a viable license-clean OSS reader, or an explicit decision
  to pay a commercial SDK) is a new version bump of this record with a change
  log line, not an unrecorded drift.

## 12. Consequences

Building the JT reader instead of adopting one:

- Gives the platform a license-clean, sovereign JT visualization path with no
  GPL contamination and no vendor dependency, matching the program thesis.
- Costs a bounded, spec-grounded build: the documented format turns reverse
  engineering into decode implementation. It is a real cost (a compressed,
  versioned scene graph is not trivial), which is why it is scoped as its own
  mini-project with a visualization-first wedge rather than folded into another
  card.
- Defers full JT readiness to the mini-project's completion; no JT import
  exists today, and the P4-M4-T02 decision record is the gate that releases the
  build (the card's "no build begins without the ADR" acceptance is honored by
  publishing this record before the follow-on card executes).
- Keeps the GPL readers (TKJT/JT Assistant/PyOpenJt) and the commercial SDKs as
  reference material and price anchors only, preserving the clean-room and
  sovereignty invariants.
- Aligns the P4 lane with its own precedent: adopt OSS when a license-clean,
  maintained, Rust-usable library exists (step-io for STEP), build behind the
  seam when it does not (JT).

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P4-M4-T02. Decides to build a
  Rust JT reader (visualization first, PMI second) from the documented
  ISO 14306 / JTIAP spec behind the oep-step vendor-neutral seam, rejecting
  the GPL-2.0-only TKJT/JT Assistant reader line on license grounds, the
  immature permissive candidates on quality grounds, and commercial SDKs on
  clean-room grounds; scopes the build as its own mini-project and records
  guardrails. Evidence: workstreams/step-interop/qif/jt-import-sources.md.
