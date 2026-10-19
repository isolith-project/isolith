# oep-cgr Fuzz Campaign Round 2: Structure Depth, Huge Files, Truncated Tails

P2-M6-T01 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

Round 2 of the CGR fuzz campaign (round 1 = P2-M1-T05 cargo-fuzz on the
header parser). Round 2 broadens the adversarial surface to the three
structure classes named in the master plan: structure depth, huge files, and
truncated tails.

Deliverables (all inside `crates/oep-cgr`, branch `task/fuzz-round2-1015a`):

- `src/structure.rs` — `walk_probe`: the tolerant, bounds-checked,
  depth-limited record walker that is the SUBJECT under fuzz. It models the
  P1-M2-T01 framing hypothesis (LE `u32 tag + u32 len + payload`,
  depth-first, implicit lengths) as an explicit `[campaign probe]` and
  guarantees structural safety for ANY byte string.
- `src/fuzz.rs` — the deterministic campaign: canonical-input builders from
  observed-public header facts, per-class generators, a seeded xorshift64*
  in-process loop (zero new dependencies), and `CampaignReport`.
- `tests/fuzz_campaign_acceptance.rs` — the TDD acceptance contract (written
  RED first).
- `examples/fuzz_campaign.rs` — standalone full-run runner.
- `fuzz/` — cargo-fuzz/libFuzzer target (round-1 heritage), excluded from
  the workspace so the pinned STABLE toolchain CI never builds it (needs a
  nightly for `libfuzzer-sys`).
- `docs/re-notes/oep-cgr-fuzz-round2.md` — this note.

No new dependencies, no lockfile churn, no nightly requirement for CI.

## 2. Pinned limits (the invariants the campaign proves)

| Limit | Value | Enforced class |
|---|---|---|
| `MAX_DEPTH` | 256 records | structure depth |
| `MAX_BLOCKS` | 1,000,000 records walked | huge files (CPU/memory bound) |
| `MAX_BLOCK_LEN` | 64 MiB declared `len` | huge files (allocation bomb) |

Outcome set (total: every byte string maps to exactly one): `Ok`,
`Truncated`, `LengthLimitExceeded`, `DepthLimitExceeded`,
`BlockBudgetExceeded`.

Structural-safety guarantees verified by the campaign and the acceptance
test:

- Iterative walk (explicit stack): 200,000-level nesting completes with a
  `DepthLimitExceeded`, no call-stack overflow.
- Slice-only, no per-block allocation: a `u32::MAX` declared length is
  refused, never materialized (~4 GiB avoided).
- Node budget: the walker NEVER visits more than `MAX_BLOCKS` records
  (`BlockBudgetExceeded` trips at the cap).
- Truncated tails: any cut point classifies; mid-record cuts are
  `Truncated`, boundary cuts are `Ok` (a sub-8-byte root residue is reported
  as `trailing_residue`, matching the real zero pad observed on CGR tails).

## 3. Key finding (landscape word reads as a length bomb)

Under the round-2 record framing, the OBSERVED landscape u32 at CGR header
offset 0x64 lands, on the natural 8-byte record alignment, as the `len`
field of the record whose tag sits at 0x60. Its observed value `0x10CD3600`
(281,884,160) is a reactive 281 MB block length — far above `MAX_BLOCK_LEN`.
The walker therefore REFUSES the observed header spine with
`LengthLimitExceeded` (offset 0x68, declared 0x10CD3600) instead of
allocating ~281 MB. This is the memory-bomb defence round 2 exists to prove,
and it fired on the first real-adjacent input (`observed_landscape_word_*
test`). It also means every real corpus file classifies as
`LengthLimitExceeded` under the probe framing (a classified, non-panic
outcome) until P2-M1-T03 anchors the true block framing onto measured bytes.

## 4. TDD transcript

RED (acceptance test committed before `structure`/`fuzz` existed):

```
error[E0432]: unresolved import `oep_cgr::fuzz`      --> tests/fuzz_campaign_acceptance.rs
error[E0432]: unresolved import `oep_cgr::structure` --> tests/fuzz_campaign_acceptance.rs
```

GREEN (after `structure.rs` + `fuzz.rs` + lib.rs registration; then the
three real failures surfaced by running the test review, not assumption):

- depth bug: container depth derived from stack length -> every record at
  depth 1. Fixed: store depth on the stack frame.
- budget semantics: trip count could exceed the cap by 1. Fixed: refuse the
  record that would exceed the cap, so `nodes <= MAX_BLOCKS` always.
- canonical header vs probe: the landscape word tripped LengthLimitExceeded.
  Documented as the finding in section 3, not papered over.

Final deterministic suite:

```
test result: ok. 6 passed   (lib unit)
test result: ok. 10 passed  (tests/fuzz_campaign_acceptance.rs)
0 failed
````

Full runner (release, defaults):

```
inputs     : 106411
panics     : 0
first panic: (none)
max nodes  : 1000000 (<= 1000000)
flags      : depth=true length=true truncated=true
outcome histogram: block_budget 1 | depth_limit 4744 | length_limit 55228
                   ok 5847 | truncated 40591
CAMPAIGN: PASS — zero panics across all round-2 input classes.
```

## 5. Real-material verification

Private observation cache (never committed, clean-room): 12 real CGR files —
cfysa set (10), piston.cgr, Downloads/VALVE.cgr. Supplied via
`OEP_CGR_SAMPLES=<dirs>`; the test's real-material leg re-runs the campaign
over every reachable `.cgr`:

```
info  real-material campaign: 12 real file(s) scanned, 2994 total inputs walked
PASS  real-material campaign clean (12 files, 2994 inputs ...)
test real_material_campaign_skips_without_samples_passes_with_them ... ok
```

Zero panics on all real files. Without a sample directory the same test
prints a tolerant SKIP so CI stays green.

## 6. Relationship to other P2 cards

- P2-M1-T03 (table-of-blocks walker): `structure.rs` is the structural
  probe this card needed; T03 should ANCHOR the framing to measured bytes
  (replace the `[campaign probe]` hypothesis with observed framing) rather
  than re-implementing bounds handling.
- P2-M1-T04 (tolerant failure model): the classified-outcome contract
  (`Ok | typed skip/tolerate`) is the tolerant model exercised at the
  walker level.
- P2-M1-T05 (round-1 fuzz): round-2 keeps the round-1 mutation heritage
  (`mutation` leg) and the nightly `fuzz/` cargo-fuzz target.
- P2-M2 richness (mesh/geometry) joins later; the walker is deliberately
  value-agnostic so it stays valid as the parse surface grows.

## 7. Clean-room boundary and how to run

- No proprietary CGR bytes are committed. All synthesized inputs derive from
  observed-public header facts (magic `01 00 00 00`, 0x60 zero preamble,
  landscape 0x10CD3600, 16-byte invariant TEMPLATE) and deterministic
  synthetic payloads. Real files are only read through
  `run_campaign_on_samples` (capped at `REAL_FILE_READ_CAP` = 32 MiB), never
  stored.
- Run:
  - Acceptance + real-material: `cargo test -p oep-cgr`
  - Full campaign: `cargo run --release -p oep-cgr --example fuzz_campaign`
  - Nightly libFuzzer leg: `cd crates/oep-cgr/fuzz && cargo +nightly fuzz run round2`

## 8. Sources

- Master plan: /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md (P2-M6-T01).
- Framing hypothesis + observed header facts: docs/re-notes/cgr-hex-notebook.md (P1-M2-T01) and docs/re-notes/oep-cgr-header-parse.md (P2-M1-T01).
- Real corpus (private observation cache): ~/.hermes/cache/scratch/cfysa, ~/.hermes/cache/scratch/piston, ~/Downloads.
