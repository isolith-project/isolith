//! Binary `.3DRep` branch: the native-V5 `V5_CFV3` container header (P3-M3-T01).
//!
//! A `.3DRep` member inside a 3DXML archive (or referenced standalone) is EITHER
//! ASCII `XMLRepresentation` markup OR a native-V5 binary container that starts
//! with the magic `V5_CFV3\0` (format `UVR` in the model doc). This module
//! implements the byte-level header of that binary container, derived purely by
//! observation of 18 real parts from a 2009-era CATIA V5 gallery archive (see
//! `docs/re-notes/3dxml-binary-3drep-format.md` and the P1-M2-T04 hex notebook).
//! There is no public Dassault spec for the container; every offset and constant
//! here was read back from real material on 2026-10-15 (clean-room, lawful
//! observation only).
//!
//! Byte layout observed invariant across all 18 real parts:
//!
//! ```text
//! offset  size  value
//! 0x00    8     magic "V5_CFV3\0"                     (56 35 5F 43 46 56 33 00)
//! 0x08    4     length A, u32 BIG-endian              (main-region length)
//! 0x0C    4     length B, u32 BIG-endian              (tail length)
//!               invariant: length A + length B == file size (exact 18/18)
//! 0x10    8     marker FF x8                          (FF FF FF FF FF FF FF FF)
//! 0x3C    4     prelude `_` CR LF NUL                 (5F 0D 0A 00)
//! 0x40    16    version token "CATIA_V5 CB0001\0"
//! 0x50    1     token follower 'N' (4E)
//! 0x51    2     count, u16 BIG-endian (03E8 == 1000)  [uncertain semantics]
//! file[-8:]     tail terminator "CB__END\0"
//! ```
//!
//! Byte order is big-endian for the two length fields (contrast the CGR header,
//! which is little-endian throughout, P1-M2-T01).
//!
//! Scope boundary (YAGNI): this milestone parses and validates the fixed header
//! prefix `0x00..=0x53` plus the whole-file invariants (length sum, `CB__END`
//! tail) and content-based branch sniffing. It does NOT walk the recursive
//! section/graph payload inside length B (UTF-16LE name tables, GUID-like
//! sequences, `MainDataStream`), nor decode geometry — that is the scoped
//! follow-on milestone after the header/format documentation lands.

/// Binary `.3DRep` magic, bytes `56 35 5F 43 46 56 33 00` == "V5_CFV3" + NUL.
pub const MAGIC: [u8; 8] = [0x56, 0x35, 0x5F, 0x43, 0x46, 0x56, 0x33, 0x00];

/// Byte offset of `MAGIC` (always the file head).
pub const MAGIC_OFF: usize = 0x00;

/// Byte offset of length A, a u32 big-endian (main-region length).
pub const LEN_A_OFF: usize = 0x08;

/// Byte offset of length B, a u32 big-endian (tail length).
pub const LEN_B_OFF: usize = 0x0C;

/// Byte offset of the 8-byte `FF x8` marker.
pub const MARKER_OFF: usize = 0x10;

/// Byte offset of the `_` CR LF NUL prelude (4 bytes at 0x3C..0x40).
pub const PRELUDE_OFF: usize = 0x3C;

/// The 4 prelude bytes observed before the version token: `_ \r \n \0`.
pub const PRELUDE: [u8; 4] = [0x5F, 0x0D, 0x0A, 0x00];

/// Byte offset of the version token "CATIA_V5 CB0001\0" (16 bytes).
pub const TOKEN_OFF: usize = 0x40;

/// The 16-byte version token observed at 0x40..0x50 in all 18 parts.
pub const TOKEN: [u8; 16] = *b"CATIA_V5 CB0001\x00";

/// Byte offset of the token follower, observed as the single byte `'N'` (0x4E).
pub const FOLLOWER_OFF: usize = 0x50;

/// The token follower byte observed at 0x50.
pub const FOLLOWER: u8 = b'N';

/// Byte offset of the count field, a u16 big-endian (observed 1000).
pub const COUNT_OFF: usize = 0x51;

/// Minimum bytes needed for the fixed header prefix (through the count at 0x53).
pub const HEADER_LEN: usize = COUNT_OFF + 2;

/// The 8-byte tail terminator every part ends with.
pub const TAIL: [u8; 8] = *b"CB__END\x00";

/// A parsed `V5_CFV3` binary `.3DRep` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V5Header {
    /// Length A, u32 big-endian at 0x08 (main-region length). `[uncertain semantics]`.
    pub len_a: u32,
    /// Length B, u32 big-endian at 0x0C (tail length). `[uncertain semantics]`.
    pub len_b: u32,
    /// Count, u16 big-endian at 0x51 (observed 1000 in all real parts).
    pub count: u16,
}

/// A header or container that fails the observed `V5_CFV3` shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V5Error {
    /// Not enough bytes to contain the fixed header prefix.
    TooShort {
        /// Bytes required for a complete header (`HEADER_LEN`).
        need: usize,
        /// Bytes actually supplied.
        got: usize,
    },
    /// The eight bytes at 0x00 are not `V5_CFV3\0`.
    BadMagic {
        /// The eight bytes actually read.
        got: [u8; 8],
    },
    /// The bytes at 0x10..0x18 are not all `0xFF`.
    BadMarker {
        /// The eight bytes actually read.
        got: [u8; 8],
    },
    /// The bytes at 0x3C..0x40 are not `_ \r \n \0`.
    BadPrelude {
        /// The four bytes actually read.
        got: [u8; 4],
    },
    /// The bytes at 0x40..0x50 differ from `CATIA_V5 CB0001\0`.
    BadToken {
        /// The sixteen bytes actually read.
        got: [u8; 16],
    },
    /// The single byte at 0x50 is not `'N'`.
    BadFollower {
        /// The byte actually read.
        got: u8,
    },
    /// Whole-file invariant `len_a + len_b == file size` does not hold.
    LengthSumMismatch {
        /// Length A read at 0x08.
        a: u32,
        /// Length B read at 0x0C.
        b: u32,
        /// Actual file/byte length supplied.
        file_len: usize,
    },
    /// Whole-file invariant: the last 8 bytes are not `CB__END\0`.
    BadTail {
        /// The eight bytes actually read at the end.
        got: [u8; 8],
    },
}

/// Parse and validate the fixed `V5_CFV3` header prefix of `bytes`.
///
/// Reads offsets 0x00..=0x53 (magic, length A, length B, marker, prelude,
/// version token, follower, count) and returns a [`V5Header`]. Returns a typed
/// [`V5Error`] on a truncated buffer, bad magic, marker/prelude/token/follower
/// drift. It never panics, so callers can run it directly over untrusted file
/// bytes (fuzz-friendly). Whole-file invariants (length sum and `CB__END` tail)
/// are checked by [`check_container`], which needs the complete buffer.
pub fn parse_header(bytes: &[u8]) -> Result<V5Header, V5Error> {
    if bytes.len() < HEADER_LEN {
        return Err(V5Error::TooShort {
            need: HEADER_LEN,
            got: bytes.len(),
        });
    }
    let magic: [u8; 8] = bytes[MAGIC_OFF..MAGIC_OFF + 8].try_into().unwrap();
    if magic != MAGIC {
        return Err(V5Error::BadMagic { got: magic });
    }
    let marker: [u8; 8] = bytes[MARKER_OFF..MARKER_OFF + 8].try_into().unwrap();
    if marker != [0xFF; 8] {
        return Err(V5Error::BadMarker { got: marker });
    }
    let prelude: [u8; 4] = bytes[PRELUDE_OFF..PRELUDE_OFF + 4].try_into().unwrap();
    if prelude != PRELUDE {
        return Err(V5Error::BadPrelude { got: prelude });
    }
    let token: [u8; 16] = bytes[TOKEN_OFF..TOKEN_OFF + 16].try_into().unwrap();
    if token != TOKEN {
        return Err(V5Error::BadToken { got: token });
    }
    let follower = bytes[FOLLOWER_OFF];
    if follower != FOLLOWER {
        return Err(V5Error::BadFollower { got: follower });
    }
    let len_a = u32::from_be_bytes(bytes[LEN_A_OFF..LEN_A_OFF + 4].try_into().unwrap());
    let len_b = u32::from_be_bytes(bytes[LEN_B_OFF..LEN_B_OFF + 4].try_into().unwrap());
    let count = u16::from_be_bytes(bytes[COUNT_OFF..COUNT_OFF + 2].try_into().unwrap());
    Ok(V5Header {
        len_a,
        len_b,
        count,
    })
}

/// Parse the header prefix AND validate the whole-file invariants.
///
/// Calls [`parse_header`] then additionally requires `len_a + len_b ==
/// bytes.len()` and that the last 8 bytes are `CB__END\0`. Use this on complete
/// `.3DRep` buffers (e.g. a member unpacked from the archive). Never panics.
pub fn check_container(bytes: &[u8]) -> Result<V5Header, V5Error> {
    let h = parse_header(bytes)?;
    let a = h.len_a;
    let b = h.len_b;
    if a.saturating_add(b) != bytes.len() as u32 {
        return Err(V5Error::LengthSumMismatch {
            a,
            b,
            file_len: bytes.len(),
        });
    }
    let n = bytes.len();
    let tail: [u8; 8] = bytes[n - 8..n].try_into().unwrap();
    if tail != TAIL {
        return Err(V5Error::BadTail { got: tail });
    }
    Ok(h)
}

/// Shallow magic sniff: `true` when the buffer starts with `V5_CFV3\0`.
///
/// Cheaper than [`parse_header`] and never errors; use it for content-based
/// branch detection (binary vs ASCII `.3DRep`) before committing to a parse. A
/// `true` result does NOT mean the rest of the header is valid.
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && bytes[..MAGIC.len()] == MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_prefix() -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b[0..8].copy_from_slice(&MAGIC);
        b[8..12].copy_from_slice(&24u32.to_be_bytes());
        b[12..16].copy_from_slice(&6u32.to_be_bytes());
        b[16..24].fill(0xFF);
        b[0x3C..0x40].copy_from_slice(&PRELUDE);
        b[0x40..0x50].copy_from_slice(&TOKEN);
        b[0x50] = FOLLOWER;
        b[0x51..0x53].copy_from_slice(&1000u16.to_be_bytes());
        b
    }

    #[test]
    fn round_trips() {
        let h = parse_header(&valid_prefix()).unwrap();
        assert_eq!(h.len_a, 24);
        assert_eq!(h.len_b, 6);
        assert_eq!(h.count, 1000);
    }

    #[test]
    fn sniffs_magic() {
        assert!(has_magic(b"V5_CFV3\x00junk.."));
        assert!(!has_magic(b""));
        assert!(!has_magic(b"V5_CFV4"));
        assert!(!has_magic(b"<?xml"));
    }

    #[test]
    fn errors_are_typed() {
        assert!(matches!(
            parse_header(b""),
            Err(V5Error::TooShort {
                need: HEADER_LEN,
                got: 0
            })
        ));
    }
}
