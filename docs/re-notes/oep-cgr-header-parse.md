# oep-cgr Header Parse: Golden Sample VALVE.cgr and TDD Contract

P2-M1-T01 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

The first working slice of the `oep-cgr` Cargo crate: a byte-level magic and
header-spine parser with a TDD acceptance test anchored to ONE golden sample
(`VALVE.cgr`). Nothing in this task reads geometry, walks blocks, or guesses
semantics beyond the observed header spine; those are later P2 cards
(P2-M1-T02 version catalog, P2-M1-T03 table-of-blocks, P2-M1-T04 tolerant
failure model, P2-M1-T05 fuzz harness, P2-M2-X mesh extraction).

Deliverables:

- `crates/oep-cgr/src/header.rs` — `parse_header`, `has_magic`, `CgrHeader`,
  `CgrError` (typed, non-panicking), offset/constant docs.
- `crates/oep-cgr/tests/golden_header.rs` — the TDD acceptance test (written
  RED against the skeleton crate, then GREEN).
- This RE note (evidence + provenance + verification transcript).

## 2. The golden sample

| Field | Value |
|---|---|
| File | `VALVE.cgr` (cfysa set, CATIA V5 documentation "DMU Space Analysis" tutorial) |
| Era | CATIA V5 R19-era doc tree (2008-2009) |
| Size | 176128 bytes (0x1000-aligned: 176128 % 4096 == 0) |
| sha256 | `43cc0cb474b5fe10a4d0bf1b8c61f48d952a8bba849aa8b116e12b490d7dcbd6` |
| Registry | P1-M1-T01 `P1-M1/cgr-sample-registry.json` id `dassault-dmu-space-analysis-valve` |
| Observation source | Hex notebook P1-M2-T01 `docs/re-notes/cgr-hex-notebook.md` |

Clean-room rule (board.program.json): the proprietary binary is NEVER
committed to the repo. The repo records observed facts (this note, the
stable constants in `header.rs`) and the acceptance test re-derives them
from real bytes when a private observation directory is supplied.

## 3. Header facts locked by the acceptance test

All byte values read from the real `VALVE.cgr` on 2026-10-15 (and from ten
sibling samples; see the P1-M2-T01 notebook for the full corpus):

| Offset | Value | Reading |
|---|---|---|
| 0x00 | `01 00 00 00` | magic, u32 LE == 1 |
| 0x04..0x63 | 96 zero bytes | fixed zero preamble |
| 0x64 | `00 36 cd 10` | landscape u32 LE == 0x10CD3600 [uncertain semantics; not a length] |
| 0x68..0x77 | `00 07 00 00 00 07 00 00 00 b1 01 0b 01 08 02 4f` | invariant 16-byte TEMPLATE, identical in all 11 corpus samples |
| 0x78.. | family-dependent tail | OUT OF SCOPE here (P2-M1-T02) |

Endianness: little-endian (magic reads 1 as LE, 0x01000000 as BE).

## 4. TDD transcript

RED (before `header.rs` existed):

```
error[E0432]: unresolved import `oep_cgr::header`
  --> crates/oep-cgr/tests/golden_header.rs:23:14
   | import fails because the skeleton crate has no header module
```

GREEN (after `header.rs`, deterministic suite):

```
running 6 tests  (tests/golden_header.rs)
parse_rejects_bad_magic        ... ok
parse_rejects_nonzero_preamble ... ok
parse_rejects_template_drift   ... ok
parse_rejects_truncated_input  ... ok
parse_accepts_golden_header_shape ... ok
golden_sample_real_material    ... ok
0 failed
```

Real-material leg (sample supplied via `OEP_CGR_SAMPLES`):

```
PASS  real golden sample parsed: magic=1 landscape=0x10cd3600 template ok (176128 bytes)
```

CI-safe behavior: with no sample directory the real-material test prints a
tolerant SKIP and passes, so CI stays green without the (never-committed)
proprietary binary. When a sample IS supplied and disagrees with the golden
observation, the test fails loud. Integrity anchor in the real-material leg
is size 176128 + the parsed header fields; the full-file sha256 anchor lives
in the P1-M1-T01 registry and this note (recorded at observation time).

## 5. Boundary statement

- Parses the header spine 0x00..0x78 only. The two observed family tail
  styles (R18-era `CAT3DBagRep` class strings vs R19-era resident f32 at
  0x7c) are version/writer drift for P2-M1-T02, not this spine parse.
- `landscape` semantics remain `[uncertain]` (documented, not guessed).
- Later R2026-era wrapper families (`V5_CFV2`, `__3_NEWPNFCT`) are
  unverified (blocked on a license-clean modern sample) and not targeted
  here; P2-M1-T04 tolerant failure model absorbs them.

## 6. Sources

- Real corpus: ~/.hermes/cache/scratch/cfysa/VALVE.cgr (private observation
  cache; sha256 in the P1-M1-T01 registry).
- Layout facts: docs/re-notes/cgr-hex-notebook.md (P1-M2-T01), P1-M1-T01
  registry. Master plan: /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md.
