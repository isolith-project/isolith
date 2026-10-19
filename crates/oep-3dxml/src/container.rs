//! 3DXML container shell: top-level branch detection and outer ZIP parse.
//!
//! A `.3dxml`/`.3DXML` file is one of two things at the byte level (observed
//! on the 2026-10-15 corpus, see `docs/re-notes/3dxml-hex-notebook.md` §3,
//! §6):
//!
//! 1. **ZIP-archive 3DXML** (dominant; 5 of 6 corpus files) — first four
//!    bytes are `50 4B 03 04` (`PK\x03\x04`, the ZIP local-file header).
//!    Everything else lives inside the ZIP: `Manifest.xml`, one or more
//!    model documents and `.3DRep` geometry members.
//! 2. **Standalone ASCII model document** (observed: quad.3dxml) — plain
//!    XML, first bytes `3C 3F 78 6D` (`<?xm`), single file, no ZIP wrapper.
//!
//! The ZIP container is the public PKWARE APPNOTE format, not Dassault IP;
//! we read it through the audited pure-Rust `zip` crate (deflate-only
//! feature). Every `.3DRep` member in the corpus is DEFLATE (method 8),
//! so decompression is the norm, not the exception.

use std::io::Cursor;

/// First four bytes of a ZIP local-file header: `50 4B 03 04`.
pub const ZIP_LOCAL_HEADER: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// First four bytes of a standalone ASCII model document: `3C 3F 78 6D` (`<?xm`).
pub const STANDALONE_XML_PREFIX: [u8; 4] = [0x3C, 0x3F, 0x78, 0x6D];

/// Canonical name of the container index member inside a ZIP-archive 3DXML.
pub const MANIFEST_NAME: &str = "Manifest.xml";

/// Top-level container branch, detected from the first bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    /// ZIP-archive 3DXML (magic `PK\x03\x04`); parse with [`parse_archive`].
    ZipArchive,
    /// Standalone ASCII model document (starts `<?xm`); no ZIP to parse.
    StandaloneXml,
    /// Neither signature observed; not a known 3DXML container.
    Unknown,
}

impl ContainerKind {
    /// Short human-readable name for the branch (`"zip-archive"`,
    /// `"standalone-xml"`, `"unknown"`), used in diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            ContainerKind::ZipArchive => "zip-archive",
            ContainerKind::StandaloneXml => "standalone-xml",
            ContainerKind::Unknown => "unknown",
        }
    }
}

/// A single member of the ZIP container (one file inside the archive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Decoded member name (UTF-8), e.g. `Manifest.xml`, `NameSetTree.3dxml`.
    pub name: String,
    /// ZIP compression method (observed 3DXML members are
    /// [`zip::CompressionMethod::Deflated`]).
    pub method: zip::CompressionMethod,
    /// Uncompressed size in bytes as recorded by the central directory.
    pub size: u64,
    /// Compressed size in bytes as recorded by the central directory.
    pub compressed_size: u64,
}

/// The parsed outer ZIP container: the ordered member inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    /// Members in central-directory order.
    pub entries: Vec<Entry>,
}

impl Archive {
    /// `true` when a member with exactly `name` is present.
    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }
}

/// Errors from parsing the outer ZIP container. Deliberately non-panicking so
/// callers can run it directly over untrusted file bytes (fuzz-friendly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    /// [`parse_archive`] was called on bytes that do not start with the ZIP
    /// local-file magic.
    NotAZipArchive,
    /// The stream is not a readable ZIP archive (empty, truncated, or corrupt
    /// central directory). The low-level reason is preserved as text.
    Zip {
        /// Description of the underlying ZIP error.
        message: String,
    },
    /// A member's name could not be decoded as UTF-8 (spec-invalid; observed
    /// 3DXML archives use ASCII/UTF-8 names).
    NonUtf8Name {
        /// The raw (undecoded) name bytes.
        raw: Vec<u8>,
    },
}

/// Classify the top-level container branch from the first bytes.
///
/// Never panics and never errors: unknown or too-short input is simply
/// [`ContainerKind::Unknown`]. A ZIP archive whose first member was deleted
/// begins with the empty-archive EOCD `PK\x05\x06` and is NOT matched here
/// (3DXML archives always carry members; treated as `Unknown`).
pub fn detect(bytes: &[u8]) -> ContainerKind {
    if bytes.len() >= 4 {
        let head: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if head == ZIP_LOCAL_HEADER {
            return ContainerKind::ZipArchive;
        }
        if head == STANDALONE_XML_PREFIX {
            return ContainerKind::StandaloneXml;
        }
    }
    ContainerKind::Unknown
}

fn to_zip_err<E: std::fmt::Display>(e: E) -> ContainerError {
    ContainerError::Zip {
        message: e.to_string(),
    }
}

/// Parse the outer ZIP container of `bytes`, returning the member inventory.
///
/// Requires [`detect`]`(bytes) == ZipArchive`; returns
/// [`ContainerError::NotAZipArchive`] otherwise. Members are listed in
/// central-directory order with their method and sizes. Contents are NOT
/// decompressed here — use [`read_member`] for that.
pub fn parse_archive(bytes: &[u8]) -> Result<Archive, ContainerError> {
    if detect(bytes) != ContainerKind::ZipArchive {
        return Err(ContainerError::NotAZipArchive);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(to_zip_err)?;
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(to_zip_err)?;
        let name = match file.name() {
            Ok(cow) => cow.into_owned(),
            Err(_) => {
                // name() fails when the raw member name is not valid UTF-8 —
                // spec-invalid for 3DXML; report the raw bytes.
                return Err(ContainerError::NonUtf8Name {
                    raw: file.name_raw().to_vec(),
                });
            }
        };
        entries.push(Entry {
            name,
            method: file.compression(),
            size: file.size(),
            compressed_size: file.compressed_size(),
        });
    }
    Ok(Archive { entries })
}

/// Read and decompress one member of the ZIP container by exact name.
///
/// Returns the member's full decoded bytes. Errors never panic: a missing
/// member, a non-ZIP input, an undecodable name, or a corrupt member payload
/// all surface as a typed [`ContainerError`].
pub fn read_member(bytes: &[u8], name: &str) -> Result<Vec<u8>, ContainerError> {
    if detect(bytes) != ContainerKind::ZipArchive {
        return Err(ContainerError::NotAZipArchive);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(to_zip_err)?;
    let mut file = archive.by_name(name).map_err(to_zip_err)?;
    // Declared sizes are untrusted input: pre-size at most 64 MiB so a lying
    // header cannot force a giant allocation; read_to_end still handles the
    // real streaming length.
    let cap = (file.size().min(64 * 1024 * 1024)) as usize;
    let mut out = Vec::with_capacity(cap);
    use std::io::Read as _;
    file.read_to_end(&mut out).map_err(to_zip_err)?;
    Ok(out)
}
