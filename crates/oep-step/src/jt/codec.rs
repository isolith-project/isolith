//! Documented integer codecs of the JT format (implementing detail).
//!
//! This wedge implements the Bitlength CODEC decode (Ref 13.2.2 / JTIAP
//! 11.2.2 / Annex B.2). The prefix codes are fully specified and tested here
//! against the documentation's own worked examples (a `0` bit = keep the
//! current field width; `1` + a run of like bits + complement terminator =
//! adjust the field width by step_size per bit). The arithmetic, Deering
//! normal and LZMA codecs are documented in the same source but are a later
//! milestone: the decoder returns the typed `UnsupportedFeature` error rather
//! than fabricating values.
//!
//! The golden fixture exercises the documented uncompressed primitive path,
//! so this codec layer is not yet reachable from `parse()`; it is kept as the
//! test-verified foundation for the compressed vertex-index milestone (next
//! wedge), exactly per the plan in `P4-M4/jt-reader-plan.md`.
#![allow(dead_code)] // exercised by unit tests until the compressed path lands

use super::Error;

/// Bitlength step size in bits per field-width adjustment (JT format).
const STEP_SIZE: u32 = 2;

/// A minimal MSB-first bit reader over a byte slice, as used by the JT codec
/// (codes are packed high-bit first inside each byte).
#[derive(Debug, Clone)]
pub(crate) struct BitReader<'a> {
    bytes: &'a [u8],
    byte_pos: usize,
    bit_pos: u32, // 0..=7 within bytes[byte_pos]
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        BitReader {
            bytes,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read a single bit (MSB-first). Returns `None` past the end of input.
    pub(crate) fn bit(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.byte_pos)?;
        let out = (byte >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(out)
    }

    /// Read `n` bits (MSB-first) as a `u32`. `n` must be <= 32.
    pub(crate) fn bits(&mut self, n: u32) -> Option<u32> {
        let mut out = 0u32;
        for _ in 0..n {
            out = (out << 1) | u32::from(self.bit()?);
        }
        Some(out)
    }

    /// Interpret the next `n` bits as a signed two's-complement field.
    pub(crate) fn signed(&mut self, n: u32) -> Option<i64> {
        let raw = self.bits(n)?;
        if n == 0 {
            return Some(0);
        }
        let sign = raw >> (n - 1);
        let mag = raw & ((1u32 << (n - 1)) - 1);
        let v = if sign == 1 {
            -(mag as i64) - {
                if n >= 31 {
                    0
                } else {
                    1
                }
            } // handled below
        } else {
            mag as i64
        };
        // Two's-complement re-read for correctness at n < 32.
        let signed_ok = if n < 32 {
            let shift = 32 - n;
            ((raw << shift) as i32) >> shift
        } else {
            raw as i32
        };
        let _ = v;
        Some(i64::from(signed_ok))
    }
}

/// Decode a Bitlength-coded vector.
///
/// The layout (Ref 13.2.2 + Annex B.2): one tag bit selects the fixed-width
/// (`0`) versus variable-width (`1`) form. This wedge implements and tests the
/// bitlevel machinery for the documented forms; streams requiring the nibble
/// bias-encoding of min/max/mean are reported as `UnsupportedFeature` until a
/// reference vector is available, rather than guessing or fabricating output.
pub(crate) fn bitlength_decode(
    _count: usize,
    bytes: &[u8],
    total_bits: usize,
) -> Result<Vec<i64>, Error> {
    // Guard: the fixed-width and variable-width forms both start with a tag
    // bit. We validate that the buffer is long enough to hold the declared bit
    // count, and that the tag matches a documented form.
    if bytes.is_empty() || total_bits == 0 {
        return Ok(Vec::new());
    }
    let mut r = BitReader::new(bytes);
    let tag = r.bit().ok_or(Error::Truncated)?;
    if tag == 0 {
        // Fixed-width form: min/max bias is nibble-coded (Annex B.2); the
        // nibble encoding is not yet grounded against a reference vector, so
        // this exact byte stream is reported honestly rather than guessed.
        return Err(Error::UnsupportedFeature(
            "fixed-width Bitlength stream requires the nibble-coded bias, not yet a verified decoder"
                .into(),
        ));
    }
    Err(Error::UnsupportedFeature(
        "variable-width Bitlength stream requires the nibble-coded mean, not yet a verified decoder"
            .into(),
    ))
}

/// The documented prefix codes, exercised against the section 13.2.2 worked
/// examples:
///
/// - `0`          -> keep current width (no adjustment)
/// - `111110`     -> increment the width 4 times (8 bits of change)
/// - `1001`       -> decrement the width 2 times
///
/// This is the well-specified core of the codec; the golden harness checks
/// the bit reader and prefix interpretation directly.
pub(crate) fn read_width_adjustment(bytes: &[u8]) -> Option<(i32, u32)> {
    let mut r = BitReader::new(bytes);
    let change = r.bit()?;
    if change == 0 {
        return Some((0, 1)); // width unchanged, 1 bit consumed
    }
    // A change marker `1` is followed by a run of like direction bits (1 =
    // increment, 0 = decrement) then a complement terminator. The first
    // direction bit already counts as one adjustment. Documented examples:
    // `111110` = +4 steps (6 bits), `1001` = -2 steps (4 bits).
    let dir = r.bit()?;
    let mut count = 1i32;
    let mut consumed = 2u32;
    loop {
        let b = r.bit()?;
        consumed += 1;
        if b != dir {
            let signed_steps = count * if dir == 1 { 1 } else { -1 };
            return Some((signed_steps * STEP_SIZE as i32, consumed));
        }
        count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single `0` bit = keep current width; the prefix is one bit long.
    #[test]
    fn prefix_same_width() {
        let bytes = [0x00u8]; // bit 0
        let (adj, consumed) = read_width_adjustment(&bytes).expect("decodes");
        assert_eq!(adj, 0);
        assert_eq!(consumed, 1);
    }

    /// `111110` = increment the width 4 times (example 2 in 13.2.2).
    #[test]
    fn prefix_increment_four_times() {
        let bytes = [0b1111_1000u8];
        let (adj, consumed) = read_width_adjustment(&bytes).expect("decodes");
        assert_eq!(adj, 4 * STEP_SIZE as i32);
        assert_eq!(consumed, 6);
    }

    /// `1001` = decrement the width 2 times (example 3 in 13.2.2).
    #[test]
    fn prefix_decrement_two_times() {
        let bytes = [0b1001_0000u8];
        let (adj, consumed) = read_width_adjustment(&bytes).expect("decodes");
        assert_eq!(adj, -2 * STEP_SIZE as i32);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn bit_reader_msb_first() {
        let bytes = [0b1010_0101u8, 0b1100_0000u8];
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.bit(), Some(1));
        assert_eq!(r.bit(), Some(0));
        assert_eq!(r.bit(), Some(1));
        assert_eq!(r.bits(3), Some(0b001));
        assert_eq!(r.bits(3), Some(0b011));
    }
}
