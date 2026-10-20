# P2-M7-T01: Real-CGR triangle-stream record LOCATION (fresh pairing, measured)

Format: reverse-engineering observation notebook
Task: P2-M7-T01 (Isolith wave-2 prep; closes waveform-analysis flags 1+2
on the side that a clean-room reader MAY honestly claim today)
Version: 2.0
Date: 2026-10-16
Status: verified on real material (piston 2007-era); modern-era latch via
public-domain NIST V5-6R2023 material (see §8). cfysa R19-era family
classified (different payload family)

Owner: Isolith P2 CGR reader / RE lane. Companion notebooks:
docs/re-notes/oep-cgr-table-of-blocks-walker.md (P2-M1-T03),
docs/re-notes/diff-harness.md (P1-M2-T06),
docs/re-notes/oep-cgr-mesh-extraction.md (P2-M2-T01).

---

## 1. Purpose and honest scope

The waveform finding carried `extraction=unverified 0/11`: the P2 GA gate OPEN
was header-spine parsing, not extracted triangles, and the golden spec was
fixture-only. This card ran a FRESH pairing attempt on the 11 real corpus files
using the now-merged P2-M1-T03 table-of-blocks walker and the P1-M2-T06 diff
harness, and (a) LOCATED the triangle-stream records on the piston family with
exact measured numbers, and (b) committed a numeric-epsilon (1e-6) golden spec
on real material.

Honest boundary, stated up front: locating and counting a triangle-stream
RECORD is not the same as latching a spatial MESH. This card closes the
"the triangle-stream offset cannot be locked" finding with a measured answer
for the piston family, and records, with evidence, why the coordinate pool of
this corpus is not contiguous f32 (so a spatial mesh is still not fabricated —
that would be a false PASS, explicitly forbidden).

## 2. Fresh pairing attempt performed (measured, not guessed)

What was actually tried against the 11 real files on 2026-10-16 (all read by
pure-stdlib probes + the Rust crate; no vendor library, no executed binary):

| Hypothesis / tool | Result on real material |
|---|---|
| P2-M1-T03 walker, tag+len framing | payload does not decompose; first past-header word races the length cap (`LengthLimitExceeded @0x80`, declared ~1.07e9 = a ~1.158 f32 literal) on cfysa; piston carries `u32 len + ASCII` class objects (`CAT3DBagRep`@0x7c, `CATSurfacicRep`@0xb5). Measured cross-file common prefix = 0x7d (125 B) on cfysa. |
| Diff harness across cfysa suite | no long shared geometry skeleton; NOZZLE1/2 share only header (0xB0), a 210-B block and a 2179-B tail. R19-era files are per-part; NOT a repeated-skeleton corpus. |
| Contiguous f32 pool + u16 index stream (P2-M2-T01 scan) | 0 pairs survive (prior finding re-confirmed). piston's 42 u16-BE handle runs were NOT raw indices — every handle is a multiple of 3. |
| LAWRE LUMP (d) | no deflate/zlib/gzip/xz signature anywhere in the 10 cfysa files. |
| Documented CATIA trim-packet grammar (lawful cadmpeg CC-BY-4.0 reference, §5.3: `0x4x` kind, `N == 3A+sum(K)`) applied mechanically | 0 chaining packets in all 10 cfysa files; 1 accidental single packet in piston. The old-era corpus does not use the modern CATPart trim grammar. |
| **Polygon-soup record grammar (discovered this card)** | **LATCHED on piston: see §3.** |

## 3. The measured record grammar (piston family)

On the 2007-era `piston.cgr` the payload is a sequence of polygon-soup records:

```text
u16 BE  N                    face count                 (2..=500)
N x     u16 BE  counts       vertex count per face      (3..=16)
S x     u16 BE  handles      S = sum(counts)            every handle % 3 == 0;
                                                        true vertex index = h / 3
```

Invariants verified across the whole file (two independent readers agree):

- every handle is a multiple of 3 (the writer stores `3 * vertex_index`);
- `S == sum(counts)` exactly for every record;
- the per-face fan triangle count is `c - 2`, so a record's measured triangle
  count is `sum(c - 2)`;
- the record's index count is `S` and its distinct-vertex count is the set of
  `h / 3` referenced indices.

Measured result (piston.cgr, 192512 B, run with thresholds `S >= 30`):

```
records located: 21
anchor record @0xA0DC:
  face_count    6        counts = [7,7,6,5,5,5]
  index_count  35        (= sum(counts), the handle-stream length)
  vertex_count 27        (distinct h/3 referenced)
  triangle_count 23      (= sum(c-2), fan triangulation)
  max_index   246
```

Full measured table (offset / faces / I / V / T / max_index):

```
@0x0472 14/186/112/158/2925   @0x091e 6/41/32/29/5618   @0x0c40 10/58/37/38/5062
@0x17d2 11/56/46/34/6014      @0x1b80 5/37/31/27/1840   @0x3280 15/145/105/115/4466
@0x3774 15/145/105/115/4465   @0x3ff0 5/36/24/26/2161   @0x4532 14/168/97/140/5132
@0x5084 5/31/24/21/1623       @0x5778 3/42/27/36/3326   @0x58ec 3/42/27/36/2972
@0x5cbe 5/39/28/29/5134       @0x65f8 5/36/24/26/4053   @0x8036 5/32/22/22/1654
@0x9bec 5/32/21/22/4731       @0x9d5a 7/79/47/65/5532   @0xa0dc 6/35/27/23/246
@0xa450 5/31/22/21/5829       @0xa8a4 10/56/41/36/5770  @0xabc6 10/56/41/36/2424
```

The record count is thresholded at `S >= 30` handles; relaxing to `S >= 20`
locates 31 records (the extra ten are smaller, e.g. @0x154 faces=5 I=29).
Both thresholds are deterministic and re-derived by the acceptance tests.

## 4. Why the coordinate pool is not latched yet (no fabricated mesh)

To turn a located record into a spatial `Mesh` the `h / 3` indices must index
vertex positions. Measured on piston:

- contiguous f32 triple lattice covering the required index span: **absent**
  (largest real-geometry f32 run is 88 vertices; a 12-byte-lattice scan over
  all 12 bases finds no run of >= 247 vertices; the tail 294 'f32 triples' are
  zero padding);
- the f64 lattice is likewise empty;
- 3-byte quantized blobs (3 x 4800..6500 vertices) exist but their
  quantization parameters (scale/offset per blob) are not yet resolved, and
  the correspondence between index spaces and blobs is not confirmed;
- the R19-era cfysa family carries **zero** counts/handles records — measured
  0/10 — its surfacic payload is a different encoding family (interleaved
  f32 runs + pairs; the P2-M2-T01 structural-scan conclusions stand for it).

So today: triangle-stream records LOCATED with verified scale counts (this
card, 2 independent readers) but a spatial mesh on this corpus is honestly
reported as BLOCKED on the old-era coordinate coding, not fabricated. This is
the flip: "extraction=unverified 0/11" becomes a measured two-part number:
`records located 21/11 samples located (piston family); spatial mesh latch
0/11 (blocked: old-era coordinate coding; modern-era sample would unblock)`.

## 5. What is committed (real material, not acceptance-suite-only)

- `crates/oep-cgr/src/locate.rs` — measured record locator (Rust).
- `crates/oep-cgr/src/blocks.rs` — P2-M1-T03 walker grafted in (was not yet
  on the importer-hub lineage).
- `crates/oep-cgr/tests/locate_real.rs` — TDD acceptance (fixture + real
  piston golden + cfysa classification + never-panics).
- `python/oep_tooling/cgr_locate_reference.py` + `tests/test_cgr_locate_reference.py`
  — independent Python reader + cross-check (the "observed reference").
- `corpus/golden_tests.json` + `corpus/golden/cgr-piston-anchor-counts.golden`
  — committed numeric-epsilon (1e-6) golden spec on the REAL piston anchor
  record: any drift in located counts fails the gate's numeric compare.

## 8. Modern-era latch (V5-6R2015+ / 3DEXPERIENCE lineage) — 2026-10-16

Don approved (2026-10-16) option 1 of the original block: procure a
modern-era license-clean sample and re-run, because the modern CGR-family
visualization stream is expected to follow the *documented* CATIA trim
grammar (the old piston coordinate wall does not apply to it).

**Material procured:** the public-domain NIST STC/FTC/CTC CAD archives
(simplified / fully-toleranced / combined PMI test cases, authored in CATIA
V5-6R2019 and V5-6R2023 — US Government test data; source page MBx-IF CAx
resources, `nist.gov`). These are license-clean (public domain), freely
downloadable, modern-era (R2019+/R2023), and store the CGR visualization
stream *inside* the `V5_CFV2` container (the same cached-tessellation family
a modern `.cgr` cache carries, per the documented container model). No login
was required — this avoids the GrabCAD/CAD-Exchange account wall flagged in
P1-M1 recon.

**Measured result (2 independent readers agree, byte-for-byte):** on
`nist_stc_06_asme1_V56R2023.CATPart` the modern decoder (`modern.rs` Rust +
`cgr_modern_reference.py` Python reference):

```
genuine 0x4x trim packets    18        (N == 3*A + sum(K) invariant holds)
visualization-point table    1         (ff ff 02 00 01 ff, mode 0)
index_count (sum N)          2003
vertex_count (distinct h)    1037
triangle_count               1389      (A + strips + fans)
max_index                    1990      (< 1992 pool entries)
```

The decoded coordinate pool (indexed visualization points, spec §5.4.2) is a
19-byte-head `ff ff 02 00 01 ff` + mode-0 prefix-coded f32 lattice; all 1992
rows reconstruct finite and every trim handle is inside the pool. The
resulting spatial `Mesh` validates (`Mesh::validate`: every index in range,
positions finite, no degenerate triangle). Mesh bbox ~ x∈[-152,152],
y∈[0,98], z∈[-248,0] — a real part, not an artifact.

**Corpus-wide check (measured):** all 18 NIST modern files supplied
(V5-6R2019 STC/FTC/CTC + V5-6R2023 STC) either decode self-consistently
(records located, handles 100% in pool, mesh validates) or classify honestly;
none fabricated, none panics.

**The flip is now a measured three-part number:**
`records located 21/11 old-era samples (piston family); spatial mesh latch
1 modern real material (NIST STC-06, V5-6R2023, golden counts 18/1037/2003/
1389); old-era spatial mesh still blocked (coordinate coding is a 2007-era
quantization family without a public reader)`.

Committed in this card:
- `crates/oep-cgr/src/modern.rs` — modern V5_CFV2 + visualization decoder
  (clean-room from the lawfully documented grammar), returns spatial `Mesh`.
- `crates/oep-cgr/tests/modern_real.rs` — TDD acceptance (never-panics +
  garbage classification + real-material latch with golden counts).
- `python/oep_tooling/cgr_modern_reference.py` + `tests/test_cgr_modern_reference.py`
  — independent Python reader (the "observed reference" for the modern path).
- `corpus/golden_tests.json` + `corpus/golden/cgr-modern-stc06-spatial-mesh-counts.golden`
  — committed numeric-epsilon (1e-6) golden on the REAL modern STC-06 anchor.

Residual honesty notes: (a) the locked sample is a modern native container
with an embedded CGR-family visualization stream, not a standalone `.cgr`
cache file from CATIA's cache dir — same tessellation family, different
container root; a literal modern `.cgr` still requires a CATIA cache dump or
account-gated sample, neither of which Don has provisioned. (b) The old-era
piston coordinate coding remains undecoded (unchanged from v1.0 §4); it is a
separate 2007-era quantization wall, not the modern grammar this latch
closes.

## 6. Verification transcript (2026-10-16)

```
OEP_CGR_SAMPLES=<piston>:<cfysa> cargo test -p oep-cgr --test locate_real
  test result: ok. 5 passed; 0 failed
OEP_CGR_SAMPLES=<piston>:<cfysa> pytest tests/test_cgr_locate_reference.py -q
  3 passed
python -m oep_tooling.cgr_locate_reference piston.cgr
  records_located 21 / anchor_record 1 / index_count 35 / vertex_count 27 /
  triangle_count 23 / max_index 246
```

## 7. Licensing / clean-room statement

Corpus files are dev-only reference material and are never shipped (Don's
2026-10-16 posture). Every constant is an observed fact read by pure-stdlib
probes from the real samples; the cadmpeg CATIA format document is a lawful
public reference (CC-BY-4.0), used only as a hypothesis source, never
reproduced into this project's code. No proprietary bytes are committed.
