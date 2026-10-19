//! CGR header parser.
//!
//! Implements the byte-level CGR header layout derived purely by observation
//! of real files (see `docs/re-notes/oep-cgr-header-parse.md` and the
//! P1-M2-T01 hex notebook). There is no public Dassault spec for the CGR
//! container; every offset and constant here was read back from real
//! material on 2026-10-15 (clean-room, lawful observation only).
//!
//! Byte layout observed invariant across all eleven samples in the P1 corpus
//! (V5 R18-era pistol through the R19-era doc set):
//!
//! ```text
//! offset  size  value
//! 0x00    4     magic: u32 LE == 1          (01 00 00 00)
//! 0x04    0x60  zero preamble (96 zero bytes)
//! 0x64    4     per-sample "landscape" u32 LE [uncertain semantics]
//! 0x68    16    invariant TEMPLATE (byte-for-byte identical in all samples)
//! ```
//!
//! Parsing stops at the end of the invariant template (0x78). The two
//! observed family-specific tail styles at 0x78/0x7c (class-name strings on
//! the R18-era sample vs a resident f32 on the R19-era set) are version/
//! writer drift and belong to P2-M1-T02 (version catalog), not this header
//! spine parse. Fields beyond the header spine are not read here.

/// CGR magic, u32 little-endian == 1, at file offset 0x00.
pub const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// Byte offset of the magic field.
pub const MAGIC_OFF: usize = 0x00;

/// Byte offset where the fixed zero preamble begins.
pub const PREAMBLE_OFF: usize = 0x04;

/// Length in bytes of the zero preamble (bytes 0x04..=0x63 are all zero).
pub const PREAMBLE_LEN: usize = 0x60;

/// Byte offset of the per-sample "landscape" u32. Semantics `[uncertain]`.
pub const LANDSCAPE_OFF: usize = 0x64;

/// Byte offset of the 16-byte invariant template.
pub const TEMPLATE_OFF: usize = 0x68;

/// The 16-byte invariant template observed byte-for-byte identical in every
/// real sample: `00 07 00 00 00 07 00 00 00 b1 01 0b 01 08 02 4f`.
pub const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

/// Total decoded header length in bytes (= `TEMPLATE_OFF + TEMPLATE.len()`).
pub const HEADER_LEN: usize = TEMPLATE_OFF + TEMPLATE.len();

/// The parsed CGR header spine (offsets 0x00..0x78).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgrHeader {
    /// Magic value; always the little-endian u32 `1` for a valid header.
    pub magic: u32,
    /// Per-sample u32 at 0x64. Semantics `[uncertain]` (not a length).
    pub landscape: u32,
    /// The 16-byte invariant template at 0x68..0x78.
    pub template: [u8; 16],
}

/// A header that fails the observed CGR spine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CgrError {
    /// Not enough bytes to contain the full header spine.
    TooShort {
        /// Bytes required for a complete header (`HEADER_LEN`).
        need: usize,
        /// Bytes actually supplied.
        got: usize,
    },
    /// The four magic bytes are not `01 00 00 00`.
    BadMagic {
        /// The four bytes actually read.
        got: [u8; 4],
    },
    /// A non-zero byte inside the zero preamble 0x04..0x64.
    NonZeroPreamble {
        /// Absolute file offset of the first unexpected non-zero byte.
        offset: usize,
    },
    /// The bytes at 0x68..0x78 differ from the invariant `TEMPLATE`.
    TemplateMismatch {
        /// The sixteen bytes actually read.
        got: [u8; 16],
    },
}

/// Parse and validate the CGR header spine of `bytes`.
///
/// Returns the parsed header on success. Returns a typed [`CgrError`] for a
/// truncated input, a bad magic, a non-zero preamble byte, or a drifted
/// invariant template. It never panics, so callers can run it directly over
/// untrusted file bytes (fuzz-friendly, P2-M1-T05).
pub fn parse_header(bytes: &[u8]) -> Result<CgrHeader, CgrError> {
    if bytes.len() < HEADER_LEN {
        return Err(CgrError::TooShort {
            need: HEADER_LEN,
            got: bytes.len(),
        });
    }
    let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if magic != MAGIC {
        return Err(CgrError::BadMagic { got: magic });
    }
    let preamble = &bytes[PREAMBLE_OFF..LANDSCAPE_OFF];
    if let Some(i) = preamble.iter().position(|&b| b != 0) {
        return Err(CgrError::NonZeroPreamble {
            offset: PREAMBLE_OFF + i,
        });
    }
    let landscape = u32::from_le_bytes(bytes[LANDSCAPE_OFF..LANDSCAPE_OFF + 4].try_into().unwrap());
    let mut template = [0u8; 16];
    template.copy_from_slice(&bytes[TEMPLATE_OFF..TEMPLATE_OFF + 16]);
    if template != TEMPLATE {
        return Err(CgrError::TemplateMismatch { got: template });
    }
    Ok(CgrHeader {
        magic: u32::from_le_bytes(MAGIC),
        landscape,
        template,
    })
}

/// Shallow magic sniff: `true` when the buffer starts with the CGR magic.
///
/// Cheaper than [`parse_header`] and never errors; use it for file-type
/// detection before committing to a full parse. A `true` result does NOT
/// mean the rest of the header is valid.
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && bytes[..MAGIC.len()] == MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid header spine (magic + zeros + landscape + template).
    fn valid_spine() -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b[0..4].copy_from_slice(&MAGIC);
        b[LANDSCAPE_OFF..LANDSCAPE_OFF + 4].copy_from_slice(&0x10CD_3600u32.to_le_bytes());
        b[TEMPLATE_OFF..TEMPLATE_OFF + 16].copy_from_slice(&TEMPLATE);
        b
    }

    #[test]
    fn round_trips() {
        let h = parse_header(&valid_spine()).unwrap();
        assert_eq!(h.magic, 1);
        assert_eq!(h.landscape, 0x10CD_3600);
        assert_eq!(h.template, TEMPLATE);
    }

    #[test]
    fn sniffs_magic() {
        assert!(has_magic(b"\x01\x00\x00\x00junk.."));
        assert!(!has_magic(b"\x00\x00\x00\x00"));
        assert!(!has_magic(b""));
    }
}
