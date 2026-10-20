# P1 Report: Prioritized Import Targets and the First Parcel

Version: 1.0 (2026-10-15)
Owner: Isolith program, P1 Format Reconnaissance and Corpus (P1.3 baseline
readers audit).
Status: delivered. This is the P1 report that turns the four sibling P1
deliverables into a ranked import plan and a concrete definition of the first
shipped reader. It is a plan document and a contract, written test-first:
`tests/test_p1_report_acceptance.py` is itself the acceptance test this
report must satisfy (TDD), and it was verified on real material on 2026-10-15
(see section 9).

## 1. Executive summary (answer first)

The program builds four import paths over time. In priority order:

1. CGR first (the wedge). Open a real CGR to a viewable triangle mesh. No
   open-source reader for CGR exists today (P1-M3-T01, oss-readers-audit),
   the program has 11 real verified CGR samples and a hex-level observation
   notebook in hand (P1-M1 corpus registry + P1-M2 cgr-hex-notebook), and
   the industry consensus is that CGR is a pure tessellation format
   (P1-M3-T02, commercial-readers-audit). This is the highest-value, lowest
   mystery import path, and the P2 (oep-cgr) workstream is chartered exactly
   for it.
2. 3DXML second. Read the documented ASCII 3DXML branch (mesh + Gregory
   surfaces) for the 3DEXPERIENCE-era interchange path. The format has a
   public spec under a Dassault free-internal-use license and two OSS
   reference implementations exist for the ASCII mesh branch (GLC_lib, a
   FreeCAD macro), but no maintained OSS reader. P3 (oep-3dxml).
3. STEP third. Wrap Open CASCADE Technology as the STEP engine (AP203/214/242
   B-Rep and tessellation) with clean vendor-neutral wrappers. STEP is the
   only CATIA-native escape hatch the existing tools support (FreeCAD opens
   CATIA only via STEP), and OCCT is a strong, license-compatible base. P4
   (oep-step); the differentiating work is AP242 PMI fidelity, which OCCT
   supports only partially and must be budgeted, not hidden.
4. CATPart/CATProduct deferred. Native CATIA V5 containers stay a P5
   two-year-scope effort (program decision 2): undocumented, release-locked,
   no OSS reader, exact B-Rep plus feature history. Nothing in the first
   parcels depends on them.

The first parcel is set in section 5: the first shipped reader is `oep-cgr`
parsing the CGR header and extracting the tessellation mesh, tolerant of any
unknown or malformed block (warn and skip, never crash), exporting to glTF
and STL, and verified against all 11 real corpus CGRs by golden test. It
explicitly does NOT claim B-Rep, PMI, a feature tree, or assembly semantics;
those are later parcels and other workstreams.

## 2. Method

This report does no new primary research. It synthesizes, on 2026-10-15, the
four P1 deliverables that precede it on the P1.3 path:

- P1-M1-T01 corpus registry (workstreams/format-corpus/samples/cgr-sample-registry.json,
  workstreams/format-corpus/samples/cgr-sample-location.md): 11 real, license-verified public CGR
  samples, source and sha256 per file.
- P1-M2-T01 hex notebook (docs/re-notes/cgr-hex-notebook.md and
  python/oep_tooling/cgr_observe.py): byte-level observation of the CGR
  header and block structure from those same files.
- P1-M3-T01 OSS readers audit (workstreams/format-corpus/reader-audit/oss-readers-audit.md and
  workstreams/format-corpus/reader-audit/oss-readers-sources.md): what open-source readers cover and where
  they stop.
- P1-M3-T02 commercial readers audit (workstreams/format-corpus/reader-audit/commercial-readers-audit.md and
  workstreams/format-corpus/reader-audit/commercial-readers-sources.md): what entity classes the industry can
  read, used strictly as a functional spec.

Priorities below are grounded in the recorded gaps and evidence of those
documents, never asserted from the air. Claims that a sibling audit marked
[uncertain] or [unverifiable] are not relied on here.

## 3. Prioritized import targets

The ladder, with the evidence anchor for each rung. Order is decisive: each
import path is ranked by (a) gap size in the OSS field, (b) how much real
material and prior observation we already hold, and (c) how much the path
unlocks the reference vertical (small-shop part to released drawing).

### 3.1 CGR (first, the wedge)

Target: read the CGR container far enough to emit a triangle mesh.

Evidence (P1-M3-T01 oss-readers-audit): no maintained OSS CGR reader exists.
GLC_Player reads COLLADA, 3DXML, OBJ and friends but not CGR and is
unmaintained since 2013. FreeCAD core opens CATIA only via STEP, never CGR.
OCCT has no CGR path. The only public native-reader claim
(jfk-solutions/native-cad-viewer) is unverifiable on live sources and
therefore not a dependency or evidence. The format has no public byte-level
spec (Just Solve lists only generic identity).

Evidence (P1-M1 corpus registry + P1-M2 cgr-hex-notebook): 11 real CGR
samples are located, license-checked and sha256-fingerprinted, and their
bytes were observed: a stable 8-byte magic prefix, a stable header spine
across the two sample families (2007 piston and R19-era doc set), LE scalar
encoding, 0x1000 alignment. That is a lawful-observation base strong enough
to start a reader.

Evidence (P1-M3-T02 commercial-readers-audit): every commercial reader that
touches CGR (Spatial HOOPS, CAD Exchanger, TransMagic) treats it as
tessellation only, never as exact B-Rep. Industry consensus confirms CGR is
the lightweight visualization layer. Matching that scope is sufficient for
the wedge.

Why first: it has the largest OSS gap, the most real material we already
hold, and it is the purest mesh problem, which is exactly the P2 spike-to-
alpha run the master plan calls for. A viewer that opens a .cgr is also the
first credible public proof the program is real (decision 5: public surface
opens after the first working CGR reader).

### 3.2 3DXML (second)

Target: 3DXML ASCII branch (outer ZIP + XML rep, mesh and Gregory-patch
surfaces, instances).

Evidence (P1-M3-T01 oss-readers-audit): GLC_lib contains a working 3DXML
ASCII mesh reader and the FreeCAD community macro imports only the ASCII
3DRep mesh (no binary .3drep, no Gregory decoding, effectively unmaintained,
single reference file). No maintained OSS 3DXML reader exists; Assimp
declined the format citing Dassault licensing, and Okino declined it for the
same reason, which independently confirms the license gate.

Evidence (P1-M3-T02 commercial-readers-audit): commercial readers claim the
V6 path through 3DXML 4.3, i.e. this is how the 3DEXPERIENCE-era data is
consumed in industry.

Why second: the ASCII branch has a published spec under the Dassault free
internal-use license (P1-M1-T02 owns obtaining it), and two reference
implementations of the mesh branch exist to learn from without linking. The
binary .3drep branch is recorded as a later scope (P3-M3) and is not part of
any first parcel (YAGNI until a real consumer materializes).

### 3.3 STEP (third)

Target: AP203/AP214/AP242 import and export by wrapping Open CASCADE
Technology with clean, vendor-neutral wrappers (do not rewrite a STEP engine).

Evidence (P1-M3-T01 oss-readers-audit): OCCT reads AP214 (CC2), AP203 and
parts of AP242, writes AP203/AP214 with AP242 only on an explicit schema
switch; its AP242 PMI is real but partial (GD&T translated only when carried
on shapes; constructive-geometry-referenced GD&T and saved views not
translated). FreeCAD, the program's P7 modeler candidate, gets its STEP path
entirely from OCCT.

Evidence (P1-M3-T02 commercial-readers-audit): STEP AP242 B-Rep plus
tessellation plus semantic PMI is claimed by the commercial SDKs; the
differentiating capability in the industry is AP242 PMI fidelity, not plain
B-Rep STEP.

Why third: the OSS gap is nearly closed (wrap OCCT, respect its limits), so
the priority is lower than the two build-from-scratch mesh paths. But STEP is
essential as the interop escape hatch and the PMI honesty problem
(P4-M5 MBD validator) is a future wedge surface.

### 3.4 CATPart/CATProduct (deferred to P5)

Target: native CATIA V5 containers. Not in scope for the first parcels.

Evidence (P1-M3-T01 oss-readers-audit): no OSS native reader; FreeCAD is
STEP-only for CATIA; native format is undocumented and release-locked; the
only public claim is unverifiable. Program decision 2 (master plan section
Decisions) puts the native reader inside a two-year horizon with a
go/no-go gate at P5-M5. Nothing in the first parcels depends on a native
reader, so it is parked here, not dropped.

## 4. Entity-class value order

Within any single format, the priority order of what to extract (from the
P1-M3-T02 commercial audit synthesis and the P1-M2 observation):

1. Tessellation mesh (vertices, indices, triangles) as the universal
   visualization layer, with multiple LOD where present.
2. Assembly / instance structure and transforms (a part is only useful when
   placed).
3. Color / material tables and per-face attributes.
4. PMI (graphical, then semantic with B-Rep associations).
5. Exact B-Rep and NURBS, only where the format actually carries it (CGR:
   never; 3DXML: only in the surface branch; STEP: always).

For the first parcel only rung 1 matters; the rest are later parcels of the
same workstreams.

## 5. The first parcel (what the first shipped reader must open)

Definition (the contract this report nails down): the first shipped reader is
`oep-cgr`, and the first parcel is reading a real CGR file through to a
viewable, verifiable triangle mesh.

Concretely, the first shipped reader must:

- Recognize and parse the CGR header observed in the P1-M2 hex notebook: the
  stable 8-byte magic prefix (0x01 00 00 00 00 00 00 00), the invariant
  0x68..0x77 template, LE scalar encoding, and size alignment on 0x1000.
  (P2-M1-T01, P2-M1-T02)
- Walk the observed block structure and extract a tessellation mesh: vertex
  positions and triangle indices into an internal mesh type (P2-M2-T01).
  Exact internal block layout is observation-based and still partly
  [uncertain] from the notebook; the reader advances with the hex notebook.
- Never crash. Unknown or malformed blocks produce a warning and are
  skipped; a classified skip is a successful outcome for that block
  (P2-M1-T04 tolerant failure model). Toggle-run the header parser under a
  fuzzer with zero panics on the corpus (P2-M1-T05).
- Export to a standard format a human or downstream tool can open: glTF/glb
  and STL (P2-M2-T02, P2-M2-T03). This is the acceptance surface.
- Pass a golden-file test on all 11 real corpus CGRs from P1-M1: every
  corpus file opens or yields a classified skip, and the extracted mesh
  matches the recorded geometry within fidelity budget (P2-M5-T03, and the
  P2-M2-T04 fidelity note on quantized/lossy geometry).

Doing exactly this, and no more, defines the first parcel. Section 6 lists
what is deliberately NOT in it.

## 6. Explicit non-goals of the first parcel

The first shipped reader does NOT claim, and must not be described as
providing:

- B-Rep (exact boundary representation) geometry. CGR carries no exact
  B-Rep; and the hex notebook confirms only mesh-grade data is observable.
- PMI (GD&T, datums, annotations). None of the P1 evidence puts PMI in a
  standalone CGR tessellation.
- Feature tree / parametric history recovery. That is P5 research (master
  plan P5-M4), and out of scope for a visualization reader by charter.
- Assembly / instance / transform semantics. The first parcel is a single
  CGR bag of mesh reps; assembly placement is a later parcel (P2-M4-T02).

These are listed so the wedge reader is scored only on what the format and
the milestone promise, matching the program's honest-capability doctrine
(P13-T03 format matrix).

## 7. After the first parcel

The next parcels, in the same workstream, follow the master plan P2 ordering:
mesh export CLI with documented exit codes (P2-M5-T01/T02), normal and
vertex-attribute recovery (P2-M3-T01), face/edge topology reconstruction
(P2-M3-T02), color/material tables and instance transforms (P2-M4-T01/T02).
3DXML (P3) and STEP (P4) run alongside as separate workstreams. The first
parcel is deliberately the minimum that produces a viewable artifact.

## 8. Risks and open questions

- CGR block layout is only partly observed. The 0x64 landscape u32 and the
  0x68..0x77 template semantics are [uncertain] per the hex notebook, and
  the location of the actual triangle stream is unconfirmed until the
  P2-M2-T01 extractor and P2-M1-T06 diff harness run. The first parcel
  therefore carries a chance that mesh-extraction effort is displaced within
  CGR; the tolerant model bounds the damage to skips, never crashes.
- The corpus is two families (2007 piston + R19-era doc set) and all samples
  are tutorial-scale (36 KB to 192 KB). Production large-assembly CGR files
  (100k+ parts) are not yet represented; the streaming-perf pass is
  P2-M6-T02 and depends on growing the corpus (P5-M2-T01).
- All 11 samples are license-reference-only: they are observed but never
  redistributed into the repo (P1-M1 policy). The golden tests run against
  locally held copies and skip cleanly when absent, so CI does not depend on
  proprietary bytes.

## 9. Real-material verification (2026-10-15)

This report was checked against its four sibling deliverables in this repo on
2026-10-15, and the first-parcel target (CGR) was verified on real material by
the P1-M2-T01 acceptance test, which re-derives every header fact from the
actual sample files. The acceptance test for THIS report
(`tests/test_p1_report_acceptance.py`) additionally, when real CGR samples
are present (OEP_CGR_SAMPLES), re-hashes each sample and requires the digest
to match the P1-M1 registry entry; on 2026-10-15 all 11 samples matched.
Priorities are grounded in the gap tables and capability matrices of the
cited audits, which were themselves verified against reachable primary
sources on 2026-10-15 (citation lists in the two P1-M3 sources files).

## 10. Version history

- 1.0 (2026-10-15): initial P1 report. Prioritized import ladder, first
  parcel definition, non-goals, verification.
