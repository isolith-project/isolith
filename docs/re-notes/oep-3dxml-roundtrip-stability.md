# P3-M4-T02: Round-trip stability test (read our own output, byte-class compare)

> Card: t_9426a69a. Branch: task/roundtrip-stability-1015a. Stacked on the P3-M4-T01
> writer PR (ascii crate, committed golden + independent encoder).

## What this proves

The deepest stability property on top of the P3-M4-T01 writer's golden lock:
the **write -> read-back -> write cycle is a strict fixed point at the
whole-archive byte level**.

```
P1 = oep_3dxml::ascii::write_package(mesh)
mesh' = decode(P1)              # our own output read back (test-local mini
                                #   reader mirroring the writer's independent
                                #   helpers; a real rep parser lands in
                                #   P3-M1-T02/M1-T03)
P2 = write_package(mesh')
assert P2 == P1  byte-for-byte  # "byte-class compare"
```

- **Byte-class compare** = strict `Vec<u8>` equality of the full ZIP package,
  the strongest stability claim. Meaningful because the writer is fully
  deterministic (fixed DOS timestamp 1980-01-01, LF endings, shortest-round-trip
  f32 Display).
- **f32 fidelity**: Rust `f32` Display has the shortest-round-trip guarantee, so
  parsed-back positions are bit-identical and re-emission reproduces the same
  shortest text. Verified on exact coordinates AND non-exact values (0.1, 3.14,
  2.5e-3, 1/3, 1e-30/5e30, -0.0).
- **Index fidelity**: strips are one 3-index run per triangle, so out-of-order
  and hole-ridden index arrays decode literally.

## What reads our own output

Two independent readers, neither of which is the writer:

1. **Rust test** (`crates/oep-3dxml/tests/roundtrip_stability.rs`): a test-local
   ZIP central-directory walker + ASCII `.3DRep` text decoder (Positions +
   Face strips), modeled on the writer's own independent helpers. Pending
   P3-M1-T02/M1-T03 (rep parser), this mini reader is the honest read-back.
2. **Python leg** (`tests/test_oep_3dxml_roundtrip_stability.py`): system
   `zipfile` + `xml.etree` only. It:
   - PART A: opens the COMMITTED golden (`box-reference.3dxml`, our own output
     persisted by the writer's GREEN) and proves the unit box is read back
     byte-exactly.
   - PART B: decodes the golden's geometry, re-encodes the DECODED mesh with
     the repo's INDEPENDENT format-facts encoder
     (`crates/oep-3dxml/tools/gen_golden_box_3dxml.py`), and asserts the fresh
     archive is byte-identical to the golden — the byte-class round trip
     verified by code derived from the format facts.
   - PART C (real material, corpus SKIPs when absent): for each real
     ASCII-branch tessellated sample, every Positions token must survive our
     f32 pipeline losslessly (our shortest-round-trip emission reparses to the
     exact f32). NOTE: real samples legitimately format exponents differently
     (`e-016` vs `e-16`), so the claim is "survives OUR pipeline", not "the
     sample text is canonical".

## Files

- `crates/oep-3dxml/tests/roundtrip_stability.rs` (Rust acceptance test, 7 tests)
- `tests/test_oep_3dxml_roundtrip_stability.py` (independent python leg, 6 tests)

## Test inventory (Post-merge GREEN)

- cargo test -p oep-3dxml: writer 11 + round-trip 7 = 18 pass.
- pytest full: 25 pass, 2 skipped (corpus-gated legs; verified 6/6 pass with
  OEP_CORPUS pointing at the real P1 3DXML samples).
- fmt/clippy/workspace/gate: see PR status (8/8 green expected like siblings).

## License posture

Format facts from public MIT/Apache-2.0 samples (docs/re-notes/3dxml-hex-notebook.md)
+ internal-use licensed 3DXML spec (P1-M1-T02). No Dassault XSD text reproduced.
