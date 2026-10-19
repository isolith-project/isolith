//! P2-M1-T05 cargo-fuzz (libFuzzer) target — round 1, the CGR header
//! parser.
//!
//! Feeds every libFuzzer-generated input to the header parser and the magic
//! sniffer. The contract: ANY byte string maps to `Ok` or a typed `CgrError`
//! (TooShort / BadMagic / NonZeroPreamble / TemplateMismatch), never a
//! panic. A libFuzzer crash here is the round-1 fail signal.
//!
//! Building this target needs a NIGHTLY toolchain + `cargo-fuzz`
//! (`cargo +nightly install cargo-fuzz`, or `cargo +nightly fuzz run
//! header_parse`); it is workspace-excluded so the pinned STABLE CI
//! toolchain never compiles it. The stable-toolchain equivalent of this
//! contract — the deterministic `oep_cgr::fuzz_round1` campaign, zero panics
//! on the real corpus — is enforced by `tests/header_fuzz_acceptance.rs` on
//! every push.
#![no_main]

use libfuzzer_sys::fuzz_target;
use oep_cgr::header::{has_magic, parse_header};

fuzz_target!(|data: &[u8]| {
    // The parser is slice-based and never allocates; a panic (or an abort
    // from a stack overflow on a hostile nesting) is the only failure mode
    // left, and the older `has_magic` sniffer is fuzzed along for the cheap
    // file-detection path.
    let _ = has_magic(data);
    let _ = parse_header(data);
});
