//! JT container spine: file header, block directory and element stream walk.
//!
//! This module is private implementation. The public surface is the caller's
//! vendor-neutral types in `super::mod`; nothing here references JT container
//! vocabulary on public items.

use super::{Cursor, Error};

/// Parsed file header facts (Siemens JT v10 Ref 5.1.1 / JTIAP 5.1.1).
pub(crate) struct HeaderInfo {
    pub version: String,
    pub big_endian: bool,
    pub directory_offset: usize,
    pub root_block: [u8; 16],
}

/// Parse the 125-byte file header.
pub(crate) fn parse_header(bytes: &[u8]) -> Result<HeaderInfo, Error> {
    if bytes.len() < 80 {
        // Too short even for the fixed version field or empty/junk input.
        return Err(Error::NotJt);
    }
    let version = String::from_utf8_lossy(&bytes[..80]).to_string();
    if !looks_like_version(&version) {
        return Err(Error::NotJt);
    }
    // The full fixed header is 125 bytes; truncated-but-versioned input must
    // fall to a typed truncation error, never a panic.
    if bytes.len() < 125 {
        return Err(Error::Truncated);
    }
    let order = bytes[80];
    let big_endian = match order {
        0 => false,
        1 => true,
        other => {
            return Err(Error::BadHeader(format!(
                "invalid byte order marker {other} (documented values 0=LSB, 1=MSB)"
            )))
        }
    };
    let c = &mut Cursor::new(&bytes[81..], big_endian);
    let _empty = c.i32()?;
    let directory_offset = c.u64()? as usize;
    let mut root_block = [0u8; 16];
    root_block.copy_from_slice(c.take(16)?);
    // 16-byte empty GUID tail is not structurally validated (tolerant).
    Ok(HeaderInfo {
        version,
        big_endian,
        directory_offset,
        root_block,
    })
}

/// A block directory entry (Siemens JT v10 Ref 5.1.2).
#[derive(Debug, Clone)]
pub(crate) struct BlockEntry {
    pub guid: [u8; 16],
    pub offset: usize,
    pub length: usize,
    pub kind: u32,
}

/// Walk the block directory at `offset`, honoring `big_endian`.
pub(crate) fn walk_directory(
    bytes: &[u8],
    offset: usize,
    big_endian: bool,
) -> Result<Vec<BlockEntry>, Error> {
    let mut c = Cursor::new(bytes, big_endian);
    c.pos = offset;
    if c.remaining() < 4 {
        return Err(Error::BadDirectory(
            "directory offset points past end of file".into(),
        ));
    }
    let count = c.i32()?;
    if !(0..=4096).contains(&count) {
        return Err(Error::BadDirectory(format!(
            "implausible directory entry count {count}"
        )));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut guid = [0u8; 16];
        guid.copy_from_slice(c.take(16)?);
        let seg_offset = c.u64()? as usize;
        let seg_length = c.u32()? as usize;
        let attrs = c.u32()?;
        // Bits 24-31 of attributes are the block type (Ref Table 5 / Table 6).
        let kind = attrs >> 24;
        entries.push(BlockEntry {
            guid,
            offset: seg_offset,
            length: seg_length,
            kind,
        });
    }
    Ok(entries)
}

/// Locate the block with the exact GUID `want` (used for LSG + shape refs).
#[allow(dead_code)]
pub(crate) fn find_block<'a>(entries: &'a [BlockEntry], want: &[u8; 16]) -> Option<&'a BlockEntry> {
    entries.iter().find(|e| &e.guid == want)
}

/// Read the block header (Ref 5.1.3.1) at `offset`; returns (kind, length).
#[allow(dead_code)]
pub(crate) fn block_header(
    bytes: &[u8],
    offset: usize,
    big_endian: bool,
) -> Result<(u32, usize), Error> {
    let mut c = Cursor::new(bytes, big_endian);
    c.pos = offset;
    c.take(16)?; // guid
    let kind = c.i32()? as u32;
    let length = c.i32()? as usize;
    if length < 24 || offset + length > bytes.len() {
        return Err(Error::BadStructure(format!(
            "block length {length} at offset {offset} overruns the file"
        )));
    }
    Ok((kind, length))
}

/// The 16-byte object type id read from an element header.
pub(crate) fn read_guid(c: &mut Cursor) -> Result<[u8; 16], Error> {
    let mut g = [0u8; 16];
    g.copy_from_slice(c.take(16)?);
    Ok(g)
}

/// Byte-order-independent test of the version prefix: a valid JT file starts
/// with the string `Version ` (Siemens/JTIAP 5.1.1) or an ASCII `V` for older
/// writers; this wedge accepts files whose header text contains `Version ` or
/// begins with `V` followed by digits.
pub(crate) fn looks_like_version(v: &str) -> bool {
    let t = v.trim_end_matches([' ', '\n', '\r']);
    t.starts_with("Version ")
        || (t.starts_with('V') && t[1..].starts_with(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_sniff() {
        assert!(looks_like_version("Version 10.0 JT DM 8.0.5.0"));
        assert!(looks_like_version("V8.1"));
        assert!(!looks_like_version("not a jt file at all"));
        assert!(!looks_like_version(""));
    }
}
