# Quantized / Lossy CGR Geometry: Vertex Counts vs Native and the Fidelity Budget

P2-M2-T04 · Isolith / OEP P2 CGR reader
Status: measured on real material 2026-10-15 (synthetic round-trip leg GREEN;
real-corpus extraction leg recorded honestly as unverified)
Version: 1.0

## 1. What this note is

CGR (CATIA Graphics Representation) is, by industry consensus and the P1-M3
audits, a **tessellation-only** format: it carries a mesh-grade approximation
of geometry, never an exact B-Rep. That makes every CGR *inherently lossy* —
the fidelity question is not "is there loss" but "how much, and is our
pipeline adding any more". This note records the **fidelity budget**: the
quantization/precision characteristics of the quantized geometry path and a
measured **vertex-count comparison** (native vs what `oep-cgr` emits), per the
master plan P2-M2-T04 and the P2-M5-T03 "matches within fidelity budget"
contract.

This note is re-derivable from real bytes by
`python/oep_tooling/cgr_fidelity.py`; its acceptance test is
`tests/test_cgr_fidelity_budget_acceptance.py` (TDD: written first, RED, then
GREEN). Byte-level values are exact; inferred *meanings* are marked
`[uncertain]` per the program's honest-capability doctrine.

## 2. What "native" means for a tessellation-only format

For CGR the only observable "native" geometry is the tessellated payload in
the file: the vertex pool(s) and the index stream(s) that make up the
surfacic rep. There is no analytic surface, no NURBS, no radius to compare
against — the P1 hex notebook (P1-M2-T01) and shape-record hypothesis
(P1-M2-T05) established that the CGR payload is a depth-first rep-graph
serialization of coarse tessellation data.

So the honest vertex-count comparison is:

    native count   = the tessellation the CGR file stores
                     (vertex-pool sizes / index-stream lengths
                      observed in the bytes)
    emitted count  = the mesh the reader/extractors produce
                     (Mesh.vertices / Mesh.triangles, and onward to
                      STL / glTF)

When a structural pairing latches, **emitted == native by construction**
(the reader copies the pool, it does not re-tessellate) — but only for the
paired leg. For real-corpus files whose mesh the extractor cannot yet latch
(P2-M2-T01: classified skip, 0/11), the honest status is `unverified`, and
this note says so instead of inventing a count-parity pass.

## 3. The measurable round-trip leg (license-clean synthetic, GREEN)

The P2-M2-T02/T03 CLI cards latch a deterministic 5x6 grid payload whose
native tessellation is exact and known by construction:

    native vertices  30   (5 rows x 6 cols, f32 (x,y,0))
    native triangles 40   (each cell split into 2 triangles)

`cgr_fidelity.mesh_counts()` re-runs the same structural pairing the Rust
extractor uses and for this fixture returns (30, 40):

    synthetic native vertex count preserved (30 == 30)  PASS
    synthetic native triangle count preserved (40 == 40) PASS

This is the "compare vertex counts vs native" evidence that is *measurable
today*, and it holds on the same bytes the crate's own tests latch. STL then
writes 3*u16 index references per triangle to an unindexed facet stream (no
vertex loss, format inflation only); glTF keeps the indexed (30, 40) exactly
(P2-M2-T03 golden: box 8/12 in, 8/12 out). So on the measured leg:

    count fidelity          == exact (V and T preserved through extraction)
    coordinate fidelity     == exact (f32 values copied verbatim, no
                                   re-quantization in our pipeline)

## 4. The precision budget (quantized/lossy physics of the format)

Coordinate encoding observed is little-endian **f32** (24-bit significand,
P1-M2-T01 section 3.5 / P2-M2-T01 scan). Two things bound the fidelity of any
number we read:

- **Relative per-coordinate floor ≈ 2^-24 ≈ 5.96e-8** (1 ULP of the mantissa
  step). At part scale S the absolute step is ≈ S x 5.96e-8. Recorded in the
  budget as `precision_floor_relative`.
- **Index width ceiling**: index streams observed are u16-valued
  (`< 0x4000`); u16 caps a single pool at 65,535 vertices, and glTF export
  switches to u32 above 65,536 (P2-M2-T03 finding). Budget field
  `index_width` / `index_ceiling = 65535`.

Lossy-ness the reader cannot recover (documented, not hidden):
- the format's own tessellation error (CATIA's writer sag tolerance) is
  baked in and unrecoverable `[uncertain]` — we do not claim exactness;
- vertex-pool candidates are structural estimates; a pool is not proof of a
  face stream until a pairing latches `[uncertain]`;
- packed/quantized attribute forms (e.g. 3-byte normal-ish bytes) exist in
  real payloads but were shown to be noise-floor, not tables, in the
  P2-M3-T01 note — we deliberately do not treat them as recovered attributes.

Our pipeline adds **no additional loss**: coordinates are f32 in, f32 out
(STL/glTF verbatim; STEP passthrough marked NOT an exact analytic B-Rep per
P2-M5-T02). The fidelity budget is therefore:

    native tessellation  <=  CATIA writer (lossy, unrecoverable)
    reader/exporters     ==  1:1 copy, no added loss (measured on fixture)

## 5. Real-material numbers (2026-10-15, 11-file corpus)

Structural scan over the real corpus (values re-derived by the acceptance
test / tool, not hand-typed):

| File | bytes | vertex-pool candidates | total cand. verts | top pool |
|---|---|---|---|---|
| ATOMIZER.cgr | 73728 | 5 | 861 | 550 |
| BODY1.cgr | 40960 | 6 | 1025 | 512 |
| BODY2.cgr | 69632 | 5 | 944 | 326 |
| LOCK.cgr | 45056 | 4 | 517 | 263 |
| NOZZLE1.cgr | 36864 | 6 | 1063 | 506 |
| NOZZLE2.cgr | 36864 | 4 | 1058 | 505 |
| REGULATION_COMMAND.cgr | 36864 | 7 | 415 | 122 |
| REGULATOR.cgr | 61440 | 8 | 962 | 359 |
| TRIGGER.cgr | 98304 | 10 | 603 | 225 |
| VALVE.cgr | 176128 | 9 | 1410 | 673 |
| piston.cgr | 192512 | 31 | 1419 | 294 |

Every file carries multiple vertex-pool candidates (native tessellation
density bounds) and hundreds+ of u16 index-stream candidates — i.e. the
geometry IS present and dense, but the correct pool<->stream pairing has not
yet latched under the current structural hypotheses. Per program doctrine
this leg is reported `extraction=unverified`, and each file's budget Json
carries `extraction: unverified` — not a fabricated parity claim. The snapshot
`native_vertex_upper_bound` (sum of candidate pool sizes, 415..1419 across
files) is recorded as the *estimate* consumers should not treat as exact.

## 6. Findings for P2-M5-T03 / P1 report

1. The fidelity budget is defined and measurable: relative coordinate floor
   2^-24, u16 index ceiling 65,535, and exact count/coordinate preservation
   on the latched leg.
2. "Matches the recorded geometry within fidelity budget" (P2-M5-T03) can
   therefore be stated concretely: extracted V/T == native V/T when a pairing
   latches, and coordinates equal to f32 round-trip (delta == 0 after
   read/write), with no tolerance slack needed for our own pipeline — the
   slack is entirely the writer's unrecoverable tessellation error.
3. Real-corpus count-parity stays `unverified` until the P2-M1-T03 block
   walker + P2-M1-T06 diff harness flip at least one real sample to `Ok`; the
   fixture leg above proves the measurement harness works end to end.

## 7. Clean-room invariant

All values above were read from the public corpus bytes (P1-M1-T01 registry)
or from the license-clean synthetic fixture by pure-stdlib probes; no vendor
library read any CGR, no proprietary bytes are committed, and no binary was
executed. This note is evidence for the P2 reader, not a license to
redistribute the samples.
