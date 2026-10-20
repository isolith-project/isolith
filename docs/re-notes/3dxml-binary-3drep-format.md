# Binary `.3DRep` Format: V5_CFV3 Container Header (Documentation + TDD Contract)

P3-M3-T01 · Isolith / OEP P3 3DXML Reader/Writer
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this milestone delivers

The binary `.3DRep` branch of the `oep-3dxml` crate, started the way the milestone
names it: **header/format documentation first** (this note), backed by a TDD
acceptance test and a minimal parser for the fixed header prefix. **Full parse of
the binary payload is deliberately a FOLLOW-ON milestone** (YAGNI until needed):
this card locks the container identity, the header layout, the whole-file
invariants and the content-based branch discrimination — not the recursive
section/graph data inside length B.

Deliverables:

- `crates/oep-3dxml/src/v5rep.rs` — `parse_header`, `check_container`,
  `has_magic`, `V5Header`, `V5Error` (typed, non-panicking), offset/constant docs.
- `crates/oep-3dxml/tests/golden_v5rep.rs` — the TDD acceptance test (written
  RED against the skeleton crate, then GREEN on 18 real parts).
- This RE note (format documentation + provenance + verification transcript).

Scope boundary (stated, so the follow-on card is unambiguous):

| In scope (this card) | Out of scope (follow-on) |
|---|---|
| Magic `V5_CFV3\0` + branch sniff (`has_magic`) | Decoding the geometry/main-data stream |
| Header prefix 0x00..=0x53 (lengths, marker, prelude, token, count) | Walking the recursive section table in length B |
| Whole-file invariants: `len_a + len_b == size`, `CB__END\0` tail | UTF-16LE name tables, GUID-like sequences, `MainDataStream`, `V4V5_FDT` |
| Content-based `.3DRep` branch classification (binary vs ASCII) | Version/writer drift catalog beyond the observed `CATIA_V5 CB0001` token |

## 2. Real material observed

The binary branch was observed on the Dassault-hosted public gallery sample
`press_rink_axe_fta.3dxml` (609574 B, sha256
`28778061da869c2a1e70a21e527a86c4de7dbed9a18028ea8a100de979ea26b4`), a CATIA V5
(Imagine & Shape lineage) archive created 2009-05-26, schema 4.3, containing 18
binary `.3DRep` members. Full provenance and per-member length table: P1-M2-T04
hex notebook `docs/re-notes/3dxml-hex-notebook.md` section 2.2 / 5.2.

Clean-room rule (board.program.json): the proprietary archive is NEVER committed.
This repo records observed facts (this note, the stable constants in `v5rep.rs`);
the acceptance test re-derives them from real bytes supplied via
`OEP_3DXML_BINARY_REPS` (a dir of extracted `.3DRep`) / `OEP_3DXML_ASCII_REPS`
(dir of ASCII `.3DRep`), and prints a tolerant SKIP when absent so CI stays green.

## 3. Header layout locked by the acceptance test

All byte values read from 18 real parts on 2026-10-15 (big-endian lengths):

| Offset | Size | Value | Reading |
|---|---|---|---|
| 0x00 | 8 | `56 35 5F 43 46 56 33 00` | magic "V5_CFV3" + NUL (exact 18/18) |
| 0x08 | 4 | (u32 BE) | length A — main-region length `[uncertain semantics]` |
| 0x0C | 4 | (u32 BE) | length B — tail length `[uncertain semantics]` |
| -- | -- | `len_a + len_b == file size` | whole-file invariant (exact 18/18) |
| 0x10 | 8 | `FF FF FF FF FF FF FF FF` | marker (exact 18/18) |
| 0x3C | 4 | `5F 0D 0A 00` | prelude `_` CR LF NUL (exact 18/18) |
| 0x40 | 16 | `43 41 54 49 41 5F 56 35 20 43 42 30 30 30 31 00` | version token "CATIA_V5 CB0001\0" (exact 18/18) |
| 0x50 | 1 | `4E` | token follower 'N' (exact 18/18) |
| 0x51 | 2 | `03 E8` (u16 BE) | count == 1000 (exact 18/18) `[uncertain semantics: classic CATIA per-meter unit factor plausible]` |
| file[-8:] | 8 | `43 42 5F 5F 45 4E 44 00` | tail terminator "CB__END" + NUL (exact 18/18) |

Endianness: big-endian for the two length fields (contrast the CGR header, which
is little-endian throughout — P1-M2-T01). A reader must not assume one global
byte order across the format family.

The model-document side: every binary part is declared `format="UVR"`
`version="1.0"` in its `Transmission.3dxml` (18/18), not `format="TESSELLATED"`.
UVR = "Unified Visualization Representation", the native-V5 visualization rep
family. Detection in the model doc is a cross-check; the definitive branch
signature is the first 8 bytes of the member (`V5_CFV3\0` vs `<?xml`).

## 4. TDD contract and transcript

RED (before `v5rep.rs` existed; precommit gate bypassed for the RED-state commit):

```
error[E0432]: unresolved import `oep_3dxml::v5rep`
  --> crates/oep-3dxml/tests/golden_v5rep.rs:37:16
   | could not find `v5rep` in `oep_3dxml`
```

GREEN (after `v5rep.rs`, deterministic + real-material suite):

```
running 15 tests  (tests/golden_v5rep.rs)            + 3 unit tests in v5rep.rs
parse_accepts_synthetic_reference_shape            ... ok
check_container_accepts_whole_reference            ... ok
parse_rejects_bad_magic / truncated / marker / prelude / token / follower ... ok
check_container_rejects_length_sum_mismatch        ... ok
check_container_rejects_missing_tail               ... ok
has_magic_sniffs_only                              ... ok
never_panics_on_garbage                            ... ok
binary_branch_real_material_all_members            ... ok
binary_branch_real_material_golden_member          ... ok
ascii_rep_is_rejected_as_binary                    ... ok
0 failed
```

Real-material legs (wired in via `OEP_3DXML_BINARY_REPS` / `OEP_3DXML_ASCII_REPS`):

```
info  verifying golden member on real material: .../p3m3-binary-reps/R125.3DRep
PASS  golden member parsed: len_a=6268 len_b=746 count=1000 (7014 bytes)
info  verifying 18 binary .3DRep member(s)
PASS  18 binary member(s) verified (magic, length sum, marker, prelude, token, count, tail)
PASS  25 ASCII rep(s) rejected as binary (branch discrimination ok)
```

Golden member anchor: `R125.3DRep`, 7014 B, len_a=6268, len_b=746, count=1000
(notebook section 2.2 row). CI-safe: with no sample dirs the three real-material
legs print SKIP and pass, so CI stays green without the never-committed archive.

Quality gates: `cargo test --workspace` (18 tests), `cargo clippy --workspace
--all-targets -- -D warnings` clean, `cargo fmt --check` clean, precommit gate
PASS on the GREEN commit.

## 5. Clean-room boundary statement

All observations come from the byte content of the license-clean P1 corpus and
the Dassault-hosted gallery sample (reference-only, NOT redistributed). No
Dassault/Spatial library read any file for us; no vendor binary was executed.
Every number in section 3 was read back from a real file. Semantics marked
`[uncertain]` are documented, not guessed — they are exactly the open probes
the follow-on binary-parse milestone and the P1-M2-T06 diff harness should close.

## 6. Follow-on scope (created for the next milestone)

The follow-on card for the binary `.3DRep` full parse should:
1. Re-run this probe on a modern (3DEXPERIENCE-era) binary archive to check
   `V5_CFV3` header drift (hex notebook open question 2).
2. Walk the recursive section table inside length B: shallow counters,
   8-byte container UUID, UTF-16LE name table (`Skeleton`, `SurfacicReps`,
   `Applicative Containers`, `MainDataStream`, `V4V5_FDT`), instance GUID at
   ~0xC0 (hex notebook open questions 1, 3, 5).
3. Investigate `len_a`/`len_b` semantics and the `03 E8` count vs the
   `00 40 00 0C 00 0A 00 0B` block at 0x53+ (unit/axis/scene fields).

This milestone deliberately does NOT do any of that (YAGNI).

## 7. Sources

- Real material: `/home/r1a/.hermes/cache/scratch/3dxml-binary-branch/press_rink_axe_fta.3dxml`
  (reference-only observation; extracted members under
  `/home/r1a/.hermes/cache/scratch/p3m3-binary-reps`).
- Layout facts: `docs/re-notes/3dxml-hex-notebook.md` (P1-M2-T04), P1-M1-T02
  sample catalog. Reader pattern: `docs/re-notes/oep-cgr-header-parse.md`
  (P2-M1-T01). Master plan:
  `/home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md`.
