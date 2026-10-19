# oep-cgr Fuzz Harness Round 1: cargo-fuzz on the Header Parser

P2-M1-T05 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

The round-1 fuzz harness on the CGR header parser: a libFuzzer (`cargo fuzz`)
target plus a deterministic, stable-toolchain campaign that is the CI-enforced
zero-panic contract. Round 2 (P2-M6-T01) then broadens the adversarial surface
to structure depth, huge files, and truncated tails over the record walker.

Deliverables (all inside `crates/oep-cgr`, branch `task/header-fuzz-1015a`):

- `src/fuzz_round1.rs` — the deterministic round-1 campaign: canonical-header
  builders from observed-public facts, per-class generators, a seeded
  xorshift64* in-process loop (zero new dependencies), and `CampaignReport`.
- `tests/header_fuzz_acceptance.rs` — the TDD acceptance contract (written RED
  first; the initial run failed to compile with `unresolved import
  oep_cgr::fuzz_round1`, then went GREEN once `fuzz_round1` landed).
- `fuzz/` — the cargo-fuzz / libFuzzer target (target `header_parse`) and
  README; workspace-excluded so the pinned STABLE toolchain never builds it
  (needs a nightly for `libfuzzer-sys`).
- `examples/fuzz_campaign.rs` — standalone full-run runner on stable.
- `docs/re-notes/oep-cgr-fuzz-harness-round1.md` — this note.

No new workspace dependencies, no lockfile churn, no nightly requirement for
CI.

## 2. Subject under fuzz

`oep_cgr::header::parse_header` and the magic sniffer `has_magic`. The parser
is slice-based, allocation-free past the tiny header struct, and returns a
typed error for every observed corruption class — it is designed
fuzz-friendly (P2-M1-T01 docstring). The harness proves the invariant on every
byte string, truncated tail, mutation, and real corpus file.

Outcome set (every byte string maps to exactly one): header `Ok`,
`TooShort`, `BadMagic`, `NonZeroPreamble`, `TemplateMismatch`. A panic on any
input is the fail signal; the campaign counts it (`panics`).

## 3. Campaign legs

| Leg | Class | Generates |
|---|---|---|
| Canonical spine + payloads | `canonical` | `Ok` |
| Explicit corruption generators | `tamper` | one of each typed error, deterministically |
| Truncation sweep (+ sharp tail window) | `truncated` | `TooShort` / `Ok` |
| Arbitrary-bytes sweep (0..=4096) | `arbitrary` | any outcome |
| Seeded byte mutation (1..=4 edits) | `mutation` | `BadMagic` / `NonZeroPreamble` / `TemplateMismatch` |
| Real `.cgr` files via `$OEP_CGR_SAMPLES` | `real` | any outcome; never committed |

A fixed seed reproduces a byte-for-byte identical `CampaignReport` on every
leg.

## 4. Verification transcript (real material, 2026-10-15)

Pinned toolchain: rustc 1.98.1 (stable). Corpus dirs in `OEP_CGR_SAMPLES`:
`cfysa` (10 files), `piston` (1), plus `Downloads/VALVE.cgr` — 12 real CGR
files total.

Acceptance (real-material leg, `--nocapture`):

    info  real-material campaign: 12 real file(s) scanned, 3930 total inputs walked
    PASS  real-material campaign clean (12 files, 3930 inputs, 3930 samples)
    test real_material_campaign_skips_without_samples_passes_with_them ... ok
    test result: ok. 1 passed

Standalone runner (full deterministic campaign + real leg):

    deterministic campaign: 10423 inputs, 0 panics,
      outcomes={bad_magic: 4045, nonzero_preamble: 1152, ok: 3489,
                template_mismatch: 265, too_short: 1472}
    real-material leg: 12 file(s) scanned, 10435 total inputs, 0 panics
    PASS  zero panics on real material (12)

Full workspace: `cargo test --workspace` green (incl. 7/7
`header_fuzz_acceptance`), `cargo clippy --workspace --all-targets -D warnings`
clean, `cargo fmt --check` clean.

## 5. Clean-room note

Real CGR binaries are never committed. Every synthesized input derives from
the PUBLIC observed header facts published in
`docs/re-notes/oep-cgr-header-parse.md` (magic `01 00 00 00`, 0x60-byte zero
preamble, landscape u32 at 0x64, 16-byte invariant TEMPLATE at 0x68..0x78).
Real material is only ever read (never stored) through the real-material leg
and the same golden-sample convention as P2-M1-T01.
