# oep-cgr fuzz round 1: cargo-fuzz harness on the header parser (P2-M1-T05)

A libFuzzer harness over `oep_cgr::header::parse_header` (+ `has_magic`).
The contract: any byte string maps to `Ok` or a typed `CgrError`, never a
panic. Zero panics on the corpus is the invariant; the deterministic
stable-toolchain proof lives in `crates/oep-cgr/tests/header_fuzz_acceptance.rs`
and runs on every push via the `oep_cgr::fuzz_round1` campaign.

## Requirements

- Nightly Rust (the pinned repo toolchain is STABLE 1.98.1 — this `fuzz/`
  crate is intentionally excluded from the workspace so stable CI never
  builds it).
- `cargo-fuzz`: `rustup toolchain install nightly && cargo +nightly install
  cargo-fuzz`.

## Run

```sh
cd crates/oep-cgr
# seed corpus (drop the canonical observed spine in as a starting point):
mkdir -p fuzz/corpus/header_parse
python3 - <<'EOF'
import struct
b = bytearray(0x78)
b[0:4] = bytes([0x01,0,0,0])
b[0x64:0x68] = struct.pack('<I', 0x10CD3600)
b[0x68:0x78] = bytes([0x00,0x07,0x00,0x00,0x00,0x07,0x00,0x00,0x00,0xB1,0x01,0x0B,0x01,0x08,0x02,0x4F])
open('fuzz/corpus/header_parse/000-spine','wb').write(b)
EOF
cargo +nightly fuzz run header_parse
```

## Legs

- `header_parse` — the libFuzzer target (this dir).
- `oep_cgr::fuzz_round1` + `tests/header_fuzz_acceptance.rs` — the
  deterministic stable campaign that CI enforces: canonical spine,
  truncation sweep, arbitrary-bytes sweep, seeded mutation, explicit
  corruption classes, and the real-material leg over `$OEP_CGR_SAMPLES`
  (zero panics on every real `.cgr`).

Clean-room: real CGR binaries are never committed. The seed corpus above is
synthesized from the observed public header facts (magic `01 00 00 00`,
0x60-byte zero preamble, landscape u32 at 0x64, 16-byte invariant TEMPLATE at
0x68..0x78) per `docs/re-notes/oep-cgr-header-parse.md`.
