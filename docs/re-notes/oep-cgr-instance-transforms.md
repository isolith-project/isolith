# oep-cgr Transform/Instance Hierarchy (Assembly CGR inside part) + Matrix Stack

P2-M4-T02 · Isolith / OEP P2 CGR Reader
Status: model GREEN + real-material classified-skip with measured reasons
(2026-10-15), per P1-M3-T03 §5 first-parcel boundary
Version: 1.0

## 1. What this card delivers

The transform/instance hierarchy model an **assembly-CGR-inside-a-part**
reader needs, plus the clean-room bridge from raw CGR bytes to it:

- `crates/oep-cgr/src/instance.rs` — `Transform` (row-major 4x4 f32),
  `MatrixStack` (push / pop / multiply-right scene-graph device),
  `Instance` (id, optional name, local transform, children, optional mesh
  ref index), and `SceneGraph::world_instances()` (a depth-first, pre-order
  walk that uses the matrix stack to compose every instance's local
  transform with its ancestors' into world transforms). Format-independent
  and fully deterministic.
- `crates/oep-cgr/src/assembly.rs` — `scan_assembly_structure(bytes)`, a
  tolerant, bounds-checked, classified-outcome scanner:
  `NotCgrPreamble` / `Assembly { instances, transforms }` /
  `NoAssembly(reason)`. Never panics, never fabricates.
- `crates/oep-cgr/tests/golden_instance.rs` — the TDD acceptance test
  (written first, RED against the skeleton crate, then GREEN).

Task signature: when a CGR payload carries a self-consistent transform
stream (>= 2 rigid 4x4 placements), the scanner materializes the placed
instances behind the matrix-stack model; when it does not, it returns a
classified skip naming the measured evidence.

## 2. What the TDD acceptance test locks

1. Transform math: identity; `from_rows` accepts finite / rejects NaN /
   infinities (`TransformError::NonFinite`); `mul` composes (`T(1)*T(2) ==
   T(3)`); `apply_point` maps a point through the homogeneous transform;
   `from_rotation_z(PI/2)` sends +X to +Y; a scaling matrix is not identity.
2. MatrixStack: starts at identity (depth 1); `mult_right` composes onto the
   current frame; push/pop round-trips (depth and current restored); `pop`
   at the base frame returns `MatrixStackError::Empty` and the stack stays
   usable.
3. SceneGraph::world_instances: a 3-level hierarchy (root → A translate
   (10,0,0) → B rotate-Z 90°) yields world transforms with B's world =
   A.world * B.local (origin → (10,0,0); +X → (10,1,0)); pre-order path
   order (root [], A [0], B [0,0]); every instance visited exactly once;
   mesh references carry through.
4. Scanner adversarial: never panics on empty/truncated/garbage/magic-only
   input; each input maps to a classified outcome.
5. Scanner synthetic: CGR preamble + a thin stream of two rigid 4x4
   transforms → `Assembly` with >= 2 instances; truncated stream still
   classifies (no panic).
6. Real material: with `OEP_CGR_SAMPLES` set, every `.cgr` yields a
   classified outcome and zero panics; `VALVE.cgr` must be in the corpus.
   Without it, the leg prints a tolerant SKIP and passes.

## 3. Evidence: transform-record reconnaissance on real material

The scanner's rigidity predicate (upper-left 3x3 orthonormal within 1e-2,
last row [0,0,0,1] within 1e-3, entries bounded) was run over the full
verified corpus plus one modern-era sample on 2026-10-15.

Verified corpus (license-clean, per P1-M1-T01 registry): 10 cfysa DMU
Space-Analysis tutorial parts (R18/R19-era, magic `01 00 00 00`) and the
2007-era 5axes piston sample.

| Sample | bytes | rigid 4x4 | near-id 4x4 | 3x4 rigid | rot-only 3x3 | result |
|---|---|---|---|---|---|---|
| ATOMIZER.cgr | 73728 | 0 | 0 | 0 | 0 | NoAssembly |
| BODY1.cgr | 40960 | 0 | 0 | 0 | 0 | NoAssembly |
| BODY2.cgr | 69632 | 0 | 0 | 0 | 0 | NoAssembly |
| LOCK.cgr | 45056 | 0 | 0 | 0 | 0 | NoAssembly |
| NOZZLE1.cgr | 36864 | 0 | 0 | 0 | 0 | NoAssembly |
| NOZZLE2.cgr | 36864 | 0 | 0 | 0 | 0 | NoAssembly |
| REGULATION_COMMAND.cgr | 36864 | 0 | 0 | 0 | 0 | NoAssembly |
| REGULATOR.cgr | 61440 | 0 | 0 | 0 | 0 | NoAssembly |
| TRIGGER.cgr | 98304 | 0 | 0 | 0 | 0 | NoAssembly |
| VALVE.cgr | 176128 | 0 | 0 | 0 | 0 | NoAssembly |
| piston.cgr | 192512 | 0 | 0 | 0 | 0 | NoAssembly |

Conclusion on the verified old-era corpus: these single-part CGRs carry **no
observable placement/transform records** under any tested hypothesis — not
even an identity placement. That is consistent with the P1 report's first
parcel definition (P1-M3-T03 §6): assembly placement is a later parcel, and
the old-era tutorial samples are authored as single bags of mesh reps with no
instance structure. The model + scanner therefore classify them as
`NoAssembly` with the measured reason, and the acceptance leg records the
transcript below rather than claiming geometry that is not there.

### 3.1 Modern-era lead (NOT yet golden material)

A separately-downloaded `VALVE.cgr` (697 431 bytes, arrived in `~/Downloads`
2026-10-15 13:01 by a prior session) does expose the class strings
`CAT3DBagRep`, `CATSurfacicRep` and **`CAT4x4Matrix`**, i.e. the modern-era
visualization family that does store 4x4 matrices. Its magic is `55 00 00 00`
(u32 LE 0x55), NOT the observed R19-family magic `01 00 00 00`, so the
scanner classifies it `NotCgrPreamble` — correct behavior, not a miss.
`[uncertain]`: provenance of this file is not in the P1-M1-T01 registry and
its license is unverified, so it is recorded here as a lead, not committed or
asserted as golden. When a license-clean modern-era assembly CGR lands, this
same scanner reads its transform stream unchanged.

## 4. Real-material verification transcript (2026-10-15)

Run: `OEP_CGR_SAMPLES=<cfysa>:<piston> cargo test -p oep-cgr --test golden_instance -- --nocapture real_material`

```
CLASSIFIED REGULATOR.cgr: NoAssembly { reason: "scanned 61440 bytes: 0 rigid candidate(s), ..." }
CLASSIFIED BODY2.cgr:    NoAssembly { reason: "scanned 69632 bytes: 0 rigid candidate(s), ..." }
CLASSIFIED NOZZLE2.cgr:  NoAssembly { reason: "scanned 36864 bytes: 0 rigid candidate(s), ..." }
CLASSIFIED REGULATION_COMMAND.cgr: NoAssembly { ... }
CLASSIFIED BODY1.cgr:    NoAssembly { ... }
CLASSIFIED LOCK.cgr:     NoAssembly { ... }
CLASSIFIED TRIGGER.cgr:  NoAssembly { ... }
CLASSIFIED VALVE.cgr:    NoAssembly { "scanned 176128 bytes: 0 rigid candidate(s), ..." } (golden sample)
CLASSIFIED NOZZLE1.cgr:  NoAssembly { ... }
CLASSIFIED ATOMIZER.cgr: NoAssembly { ... }
CLASSIFIED piston.cgr:   NoAssembly { ... }
summary: 11 samples, all classified, zero panics
test result: ok. 1 passed; 0 failed
```

Second run (modern-era lead): `OEP_CGR_SAMPLES=<$HOME/Downloads>` →
`CLASSIFIED VALVE.cgr: NotCgrPreamble (697431 bytes)` — the 0x55 family is
correctly not claimed as a member of the observed R19 spine.

Honesty note: 0 assemblies extracted from the verified corpus today because
no transform stream survives contact with the real bytes — recorded above
rather than papered over. The acceptance test asserts the *classified
outcome* property (Assembly | reasoned NoAssembly | NotCgrPreamble, zero
panics), which holds on all 12 real samples and is the correct,
non-fabricating contract for this stage.

## 5. Trade at the boundary: why model-first

- The matrix stack and world-transform walk are pure math and deterministic;
  they are the durable engineering this card ships, reusable by the
  exporters (P2-M2-T02/T03) and by `oep-geo` (P6-M1-T01 instance schema) the
  moment a transform stream exists.
- The scanner is deliberately strict (rigid predicate + >= 2 stream) to avoid
  false-positive assemblies from float noise; false negatives on exotic
  packing are the acceptable cost at this stage and are surfaced as
  `NoAssembly(reason)` when they happen.

## 6. Open items

- Land P2-M1-T03 (block walker) / P2-M1-T06 (diff harness); feed their
  per-record deltas into `scan_assembly_structure` as instance/transform
  records are isolated, flipping `NoAssembly` → `Assembly` sample by sample.
- Procure a license-clean modern-era (R2015+/3DEXPERIENCE) assembly CGR;
  the 0x55-family `CAT4x4Matrix` observation (§3.1) says such files DO
  contain the 4x4 tables this scanner already reads.
- P6-M1-T01 may wish to share `Instance`/`Transform` types rather than
  duplicate; flagged as a merge seam.

## 7. Clean-room statement

All byte-level facts above were read from the named public sample files on
2026-10-15 by pure-stdlib probes (working probes in the worker scratch) and
the crate's own tests. No Dassault/Spatial library read any CGR; no vendor
binary was executed; no proprietary bytes are committed. Tag/ID semantics
beyond the observed rigid-transform record are `[uncertain]` and asserted
only as hypotheses against real bytes, per program clean-room doctrine.
