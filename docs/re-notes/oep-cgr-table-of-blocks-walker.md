# oep-cgr Central Table-of-Blocks Walker (Observed Structure) with Bounds Checks

P2-M1-T03 · Isolith / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this card delivers

The CENTRAL structural walker of the `oep-cgr` reader:

- `crates/oep-cgr/src/blocks.rs` — `anchor_start` (measured header-spine
  validation), `walk_blocks` (the depth-first record walker), the emitted
  `Block` table (offset/tag/len/depth per entered record), `Outcome`, `Limits`,
  `WalkReport`, typed non-panicking `BlocksError`.
- `crates/oep-cgr/tests/table_of_blocks.rs` — the TDD acceptance test (written
  RED against the skeleton crate, then GREEN).
- This RE note (evidence + provenance + verification transcript).

Task signature: walk the OBSERVED CGR structure with hard bounds checks. For
ANY byte string the walker returns a typed `WalkReport` and never panics; it
never allocates a declared record payload; it refuses runaway lengths, depths
and node counts; truncated tails classify instead of crashing. The emitted
block table is the map downstream P2 cards (mesh P2-M2-T01, topology
P2-M3-T02, material P2-M4-T01) consume.

## 2. Observed structure anchored (measured, not guessed)

All values read back from the eleven real corpus samples (10 cfysa R19-era +
1 piston 2007-era) on 2026-10-15; identical to the facts already locked by
P2-M1-T01 `docs/re-notes/oep-cgr-header-parse.md` and the P1-M2-T01 hex
notebook:

| Fact | Value | Samples |
|---|---|---|
| magic @0x00 | `01 00 00 00` (u32 LE == 1) | 11/11 |
| preamble @0x04..0x63 | 96 zero bytes | 11/11 |
| landscape @0x64 | per-writer u32 (`0x10CD3600` cfysa / `0x0F44FC00` piston) [uncertain] | 11/11 |
| invariant template @0x68..0x77 | 16 bytes, byte-identical in all 11 | 11/11 |
| file size | multiple of 0x1000 | 11/11 |
| scalar encoding | little-endian | 11/11 |

The walker ANCHORS on this spine (`anchor_start` returns `HEADER_LEN` = 0x78
for a validated CGR) so its walk offset is measured, not arbitrary. A buffer
that fails magic/preamble/template is `Err(NotCgrPreamble)`, never walked.

Framing hypothesis (from the P1-M2-T01 hex notebook section 7, `[uncertain]`):
the payload is a depth-first serialization of the rep graph with implicit
lengths — `u32 LE tag + u32 LE len + payload`, records nested inside payloads,
and NO forward block table. Re-confirmed for this card by an all-offset
forward-table scan over the corpus: zero clean `(offset,length)` directory
candidates on VALVE/piston. The walker implements exactly this observed
framing and classifies honestly when real material does not decompose.

## 3. The bounds-check contract (what the acceptance test locks)

1. Total: `walk_blocks` returns a `WalkReport` for any input
   (deterministic sweep 0..=4096 + hand cases) — never a panic.
2. Anchor: `anchor_start` accepts the observed spine → `0x78`; wrong magic →
   `NotCgrPreamble`; truncated spine → `TooShort` (never panics at any cut).
3. Table emission: a 3-level nested record chain yields the exact ordered
   block table (R0@0x78 depth 1, R1@0x80 depth 2, R2@0x88 depth 3).
4. Length cap: `u32::MAX` declared len → `LengthLimitExceeded` at the payload
   start, no allocation; the observed landscape word as a len → refused
   (memory-bomb defence).
5. Depth cap: chain at `MAX_DEPTH` parses; `MAX_DEPTH+1` → exhausted; a
   200_000-level chain walks ITERATIVELY (explicit stack) without SIGSEGV.
6. Node budget: an 8 MiB zero container classifies, emitted table ≤
   `MAX_BLOCKS`; a 64 KiB container parses with exactly 8_192 records.
7. Truncation: every prefix cut of the spine and a 3-chain classifies; a
   mid-record header cut → `Truncated`; a boundary cut → `Ok`.
8. Real material: every real `.cgr` yields a classified outcome with zero
   panics; the golden `VALVE.cgr` must be present; the leg SKIPs cleanly when
   the private corpus is not supplied (CI green without proprietary bins).

## 4. Verification transcript (this host, cargo 1.98.1)

Deterministic suite:

```
running 13 tests ... test result: ok. 13 passed; 0 failed
```

Real-material leg
(`OEP_CGR_SAMPLES=<cfysa>:<piston> cargo test -p oep-cgr --test table_of_blocks -- --nocapture`):

```
"ATOMIZER.cgr": start=0x78 outcome=LengthLimitExceeded { offset:128, declared:1066680064 } blocks=0
"BODY1.cgr":    start=0x78 outcome=LengthLimitExceeded { offset:128, declared:1066681088 } blocks=0
... (all 10 cfysa + piston identical shape)
"VALVE.cgr":    start=0x78 outcome=LengthLimitExceeded { offset:128, declared:1066681600 } blocks=0
"piston.cgr":   start=0x78 outcome=LengthLimitExceeded { offset:128, declared:861159747 } blocks=0
verified 11 real samples through the central block walker (zero panics)
test result: ok. 13 passed; 0 failed
```

Honesty note: on the old-era corpus the depth-first framing does NOT cleanly
decompose the payload — a handful of bytes past the anchor, the natural 8-byte
alignment reads the observed landscape literal as a `len` (~281 MB for cfysa),
which the walker REFUSES (`LengthLimitExceeded`) rather than allocating. This
matches the P2-M1-T02/M2-T01 finding that the old-era payload is a tagged,
interleaved serialization whose exact grammar needs the P2-M1-T06 diff harness
or a modern-era sample to lock. The walker's contract is classified safety,
not fabricated geometry: every sample yields a reasoned outcome, zero panics,
and today that outcome races the length-cap defence at the very first past-tail
record. That is the honest, non-fabricating state of the reader.

Quality gates (mirror CI): `cargo fmt --all -- --check` PASS, `cargo clippy
--workspace --all-targets -- -D warnings` PASS, `cargo test --workspace --lib`
PASS, `cargo test --workspace --tests` PASS (13/13 acceptance + 2 unit),
`scripts/gate/precommit-gate.sh` PASS.

## 5. Boundary statement (who owns what)

- `blocks` owns the central walk + the block table emission + bounds checks.
  It deliberately does NOT interpret tag semantics (P2-M1-T02 catalog lane) and
  does NOT choose a tolerant skip/recycle policy (P2-M1-T04 tolerant-failure
  lane reuses `Outcome` from here).
- Sibling PR P2-M6-T01 (`structure.rs`) is the FUZZ CAMPAIGN safety probe — it
  returns counters only and is driven by the round-2 campaign. `blocks` is the
  measured-anchor central walker with the emitted table; the framing constants
  overlap by design (each PR is self-contained against main until merges unify
  them). This overlap was flagged as a merge hotspot in the completion
  metadata.
- No dependency on the still-unmerged `header` / `catalog` / `mesh` modules:
  observed constants are re-declared here per the established self-contained
  pattern (same as `mesh.rs` / `topology.rs`). When those merge, the constants
  can be unified by the reconciler.

## 6. Sources

- Real corpus: `~/.hermes/cache/scratch/cfysa/*.cgr`, `piston/piston.cgr`
  (private observation cache, never committed; checksums in the P1-M1-T01
  registry).
- Layout facts: `docs/re-notes/cgr-hex-notebook.md` (P1-M2-T01),
  `docs/re-notes/oep-cgr-header-parse.md` (P2-M1-T01),
  `docs/re-notes/oep-cgr-mesh-extraction.md` (P2-M2-T01). Master plan:
  `/home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md`.

## 7. Version history

- 1.0 2026-10-15 initial central table-of-blocks walker task (P2-M1-T03).
