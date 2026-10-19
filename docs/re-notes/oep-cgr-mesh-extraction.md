# oep-cgr Tessellation Mesh Extraction: Internal Mesh Type + Structural Scan

P2-M2-T01 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15 (mesh type GREEN; real-material
payload extraction classified-skip with reasons, per P1-M3-T03 section 8)
Version: 1.0

## 1. What this card delivers

The internal mesh carrier of the `oep-cgr` reader and the clean-room bridge
from raw CGR bytes to that carrier:

- `crates/oep-cgr/src/mesh.rs` — `oep_cgr::mesh::Mesh`
  (vertices: `Vec<[f32;3]>`, triangles/index-triples: `Vec<[u32;3]>`), with
  build-time index-bounds enforcement, whole-mesh invariant validation
  (in-range indices, finite coordinates, non-degenerate triangles), and a
  classified-by-reason extractor `extract(bytes)`.
- `crates/oep-cgr/tests/golden_mesh.rs` — the TDD acceptance test (written
  first, RED against the skeleton crate, then GREEN).

Task signature: a tessellated CGR payload, when decryptable under the tested
structural hypotheses, is returned as `Ok(Mesh)`; when the payload does not
contain a self-consistent indexed triangle stream under those hypotheses, it
is returned as `Err(MeshError::MeshNotFound(reason))` — a *classified skip*
with the exact record/layout detail, per P2-M1-T04 tolerant-failure doctrine.
The extractor never panics on any input (deterministic adversarial tests +
all real corpus samples).

## 2. What the TDD acceptance test locks

Part 1 (mesh type, deterministic — no real bytes needed):

- vertices and triangle indices are held and returned correctly;
- out-of-range triangle indices are rejected at build time
  (`MeshError::IndexOutOfRange`);
- `validate()` catches out-of-range raw pushes and non-finite coordinates
  (`MeshError::NonFiniteVertex`) and accepts a valid mesh;
- degenerate (zero-area) triangles are rejected by `validate()`.

Part 2 (adversarial, deterministic): `extract` never panics on empty,
truncated, garbage, and magic-only inputs.

Part 3 (real material, skip-clean): when a private corpus is supplied via
`OEP_CGR_SAMPLES`, every `.cgr` sample yields a *classified* outcome — a
structurally-valid `Mesh` or a reasoned `MeshNotFound` skip — and the golden
sample `VALVE.cgr` must be among them. Zero panics across the corpus.

## 3. Evidence: byte-level reconnaissance actually performed (clean-room)

The extractor's structural scan is not a guess — it encodes the observed
layout hypotheses and checks them against real bytes. This card's worker ran
the following hypotheses against all eleven real corpus samples and recorded
fact/no-fact for each (probe scripts in the worker's scratch, results below):

| Hypothesis | Test | Result on real material |
|---|---|---|
| Bounded-f32 runs = vertex positions | contiguous LE f32 triples, \|v\|<1e5, stride-4/12 | present, but no clean single pool per record; many small runs interleaved with tags |
| "count × N*12B coords" vertex record | u32 count then count×3 plausible floats | 0 clean hits (VALVE), 10 gappy hits (piston) at 0x3231/0x3725/0x44dd/... — not self-consistent |
| "count × N*4B indices" index record | u32 count then count×u32 <2M | 0 clean hits |
| u16 BE/LE index streams | contiguous u16 <0x4000 | dozens of streams per file, but none pair with a preceding vertex pool with high fractional usage |
| vertex-pool ↔ index-stream structural pairing | every index < pool size; >½ pool used; <10% degenerate tris | 0 pairs survive on all 11 samples (except one zero-area degenerate artifact on piston) |
| stride/tag packing (`05 00` tags + coords) | stride-6/10/14/16 tag+f32 density | flat ~71-76% plausible — no periodic packing signal |
| CAT3DBagRep polygon soup (`count, count×indices`) | u16 count 3..40 then C×u16 | 0 contiguous face runs (piston any width/mode) |
| `ff 4x` trim-packet grammar (BE handles) | kind=0x40|mask, counts, N×BE-u16 handles | 17 accidental hits with non-index handle values — grammar does not match old-era bytes |

Conclusion on real material: the old-era corpus payload is a tagged,
interleaved serialization whose triangle-stream offset cannot yet be locked by
structural scan alone. This confirms — rather than refutes — the P1 report's
recorded risk (P1-M3-T03 §8: "the location of the actual triangle stream is
unconfirmed until the P2-M2-T01 extractor and P2-M1-T06 diff harness run").
The two tools that unblock real extraction are the next cards on the same
lane: the P2-M1-T03 table-of-blocks walker (observed structure + bounds
checks) and the P2-M1-T06 save/load-modify-bisect diff harness. This card
builds the mesh carrier and the classified-skip scaffold they plug into, and
it honestly returns `MeshNotFound(reason)` today instead of inventing
geometry.

## 4. Real-material verification transcript (2026-10-15)

Run: `OEP_CGR_SAMPLES=<cfysa>:<piston> cargo test -p oep-cgr --test golden_mesh -- --nocapture`

```
SKIP  "ATOMIZER.cgr":  scan 73728 bytes: 5 vertex-pool candidates, no self-consistent index pairing
SKIP  "BODY1.cgr":     scan 40960 bytes: 6 vertex-pool candidates, ...
SKIP  "BODY2.cgr":     scan 69632 bytes: 5 vertex-pool candidates, ...
SKIP  "LOCK.cgr":      scan 45056 bytes: 4 vertex-pool candidates, ...
SKIP  "NOZZLE1.cgr":   scan 36864 bytes: 6 vertex-pool candidates, ...
SKIP  "NOZZLE2.cgr":   scan 36864 bytes: 4 vertex-pool candidates, ...
SKIP  "REGULATION_COMMAND.cgr": scan 36864 bytes: 7 vertex-pool candidates, ...
SKIP  "REGULATOR.cgr": scan 61440 bytes: 8 vertex-pool candidates, ...
SKIP  "TRIGGER.cgr":   scan 98304 bytes: 10 vertex-pool candidates, ...
SKIP  "VALVE.cgr":     scan 176128 bytes: 9 vertex-pool candidates, ...  (golden sample)
SKIP  "piston.cgr":    scan 192512 bytes: 31 vertex-pool candidates, ...
summary: 0 mesh, 11 classified skip of 11 samples
test result: ok. 6 passed; 0 failed
```

Honesty note: 0 meshes extracted today because no structural hypothesis
survived contact with the real bytes — recorded above rather than papered
over. The acceptance test asserts the *classified outcome* property (mesh or
reasoned skip, zero panics), which holds on all 11 samples and is the correct,
non-fabricating contract for this stage of the reader.

## 5. Deterministic fixture for downstream cards

Downstream cards (P2-M2-T02 STL / P2-M2-T03 glTF exporters, P2-M5 CLI) can
test against a synthetic mesh with zero proprietary bytes:

```rust
let mut m = Mesh::new();
let a = m.push_vertex([0.,0.,0.]);  // triangle fan corners
let b = m.push_vertex([1.,0.,0.]);
let c = m.push_vertex([0.,1.,0.]);
m.push_triangle(a, b, c).unwrap();
assert!(m.validate().is_ok());
```

## 6. Open items — updated by P2-M7-T01 (2026-10-16, fresh pairing + modern latch)

The P2-M7-T01 fresh pairing attempt (docs/re-notes/oep-cgr-real-material-locate.md)
resolved the prior *"triangle-stream offset unknown"* state into a measured
three-part number:

- **Piston (2007-era) family: triangle-stream RECORDS are now LOCATED.**
  The payload is a polygon-soup of `[u16BE N][N x u16BE counts][S x u16BE
  handles]` records where every handle is `3 * vertex_index`; 21 records
  (31 at `S>=20`) with exact offsets and scale counts are committed, and a
  numeric-epsilon (1e-6) golden anchors the @0xA0DC record
  (face 6 / index 35 / vertex 27 / triangle 23 / max-index 246).
- **Coordinate pool of this corpus is NOT contiguous f32** (measured: no f32
  lattice of >=247 vertices exists in piston; quantized 3-byte blobs exist but
  their scale/offset coding is unresolved), so a spatial `Mesh` on the
  old-era corpus is honestly still NOT fabricated.
- **R19-era cfysa family carries zero counts/handles records** — a different
  payload family; its probes stay as recorded in §3.
- **Modern-era (V5-6R2015+/3DEXPERIENCE) latch: DONE on real material.**
  Via the public-domain NIST STC/FTC/CTC archives (V5-6R2019 + V5-6R2023,
  US Government test data, no login), `modern.rs` + the Python reference
  decode the embedded CGR-family visualization stream by the documented
  container + trim grammar (cadmpeg §5.3/§5.4.2): genuine `0x4x` packets
  satisfying `N == 3*A + sum(K)`, a `ff ff 02 00 01 ff` indexed
  visualization-point f32 pool, and — on the STC-06 anchor — a spatial mesh
  with golden counts records=18 / triangle=1389 / index=2003 / vertex=1037 /
  max_index=1990, committed numeric-epsilon (1e-6) and verified by two
  independent readers. This flips this card's `MeshNotFound` to `Ok(mesh)`
  for real modern material.

Remaining honestly-open corners:
- old-era (piston) coordinate coding (quantized 3-byte blobs) — a 2007-era
  quantization family with no public reader; unchanged.
- a literal standalone modern `.cgr` cache file (as opposed to the modern
  container's embedded CGR-family stream) requires a CATIA cache dump or an
  account-gated sample; neither provisioned.

## 7. Clean-room statement

All byte-level facts above were read from the named public sample files on
2026-10-15 by pure-stdlib probes. No Dassault/Spatial library read any CGR;
no vendor binary was executed; no proprietary bytes are committed. The `ff 4x`
grammar tested in §3 is a lawful public reference hypothesis (cadmpeg CC-BY-4.0
CATIA format documentation), used strictly as a hypothesis to check against
real bytes, not as reproduced source.
