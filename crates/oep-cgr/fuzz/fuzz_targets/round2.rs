//! P2-M6-T01 cargo-fuzz target (round 2, nightly only).
//!
//! Feeds every libFuzzer-generated input to the tolerant record walker with
//! the pinned invariant limits. The campaign contract: any input classifies,
//! never panics. A libFuzzer crash here is the round-2 fail signal.
#![no_main]

use libfuzzer_sys::fuzz_target;
use oep_cgr::structure::{walk_probe, Limits};

fuzz_target!(|data: &[u8]| {
    // The walker is slice-based and non-allocating per block; a panic or an
    // abort (stack overflow would be an abort) is the only failure mode left.
    let _ = walk_probe(data, &Limits::default());
});
