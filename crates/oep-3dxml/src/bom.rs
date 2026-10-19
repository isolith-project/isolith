//! P3-M1-T01 entry point: parse the **outer ZIP** + the container **BOM**.
//!
//! [`parse`] is the single milestone behavior: given the bytes of a 3DXML
//! file it classifies the container branch, and for a ZIP-archive 3DXML it
//! returns the member inventory (outer ZIP parse) AND the decoded
//! `Manifest.xml` (the BOM: primary model document + container metadata),
//! verifying that the declared `<Root>` names a member that actually exists
//! in the archive ("BOM coherence").
//!
//! Scope boundary: a standalone ASCII model document is classified and
//! returned without a BOM (its `<Model_3dxml>` parse is P3-M1-T02); the
//! binary V5_CFV3 `.3DRep` branch (P3-M3) is not touched here.
//!
//! Everything is typed-error and non-panicking, so callers can run it
//! directly over untrusted file bytes (fuzz-friendly).

use crate::container::{self, Archive, ContainerKind, MANIFEST_NAME};
use crate::manifest::{self, Manifest};

/// Result of parsing a 3DXML container's outer ZIP + BOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bom {
    /// Top-level container branch detected from the first bytes.
    pub kind: ContainerKind,
    /// Member inventory, present for [`ContainerKind::ZipArchive`].
    pub archive: Option<Archive>,
    /// Decoded `Manifest.xml` (the container BOM), present for
    /// [`ContainerKind::ZipArchive`].
    pub manifest: Option<Manifest>,
}

/// Typed, non-panicking errors from [`parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BomError {
    /// Fewer than 4 bytes supplied — too short to classify.
    TooShort {
        /// Bytes actually supplied.
        got: usize,
    },
    /// The first four bytes match neither `PK\x03\x04` nor `<?xm`.
    UnknownContainer {
        /// The four bytes actually read.
        head: [u8; 4],
    },
    /// Failure while reading the ZIP container (see [`container::ContainerError`]).
    Zip(container::ContainerError),
    /// Failure while decoding the `Manifest.xml` BOM (see [`manifest::ManifestError`]).
    Manifest(manifest::ManifestError),
    /// The archive has no `Manifest.xml` member (a ZIP, but not a 3DXML).
    MissingManifest,
    /// The manifest's `<Root>` names a member that is not in the archive.
    RootNotInArchive {
        /// The declared root member name.
        root: String,
    },
}

/// Parse a 3DXML file's bytes: classify the container, parse the outer ZIP,
/// and read the `Manifest.xml` BOM.
///
/// - [`ContainerKind::ZipArchive`]: returns the archive inventory + manifest,
///   and fails with [`BomError::RootNotInArchive`] if the declared `<Root>`
///   is not an actual member.
/// - [`ContainerKind::StandaloneXml`]: returns a `Bom` with that kind and no
///   archive/BOM (model-doc XML parse is P3-M1-T02).
/// - [`ContainerKind::Unknown`] / too-short input: typed error, never a panic.
pub fn parse(bytes: &[u8]) -> Result<Bom, BomError> {
    if bytes.len() < 4 {
        return Err(BomError::TooShort { got: bytes.len() });
    }
    let kind = container::detect(bytes);
    match kind {
        ContainerKind::StandaloneXml => Ok(Bom {
            kind,
            archive: None,
            manifest: None,
        }),
        ContainerKind::Unknown => Err(BomError::UnknownContainer {
            head: [bytes[0], bytes[1], bytes[2], bytes[3]],
        }),
        ContainerKind::ZipArchive => {
            let archive = container::parse_archive(bytes).map_err(BomError::Zip)?;
            let manifest_bytes =
                container::read_member(bytes, MANIFEST_NAME).map_err(BomError::Zip)?;
            let manifest = manifest::parse_manifest(&manifest_bytes).map_err(BomError::Manifest)?;
            if !archive.has(&manifest.root) {
                return Err(BomError::RootNotInArchive {
                    root: manifest.root,
                });
            }
            Ok(Bom {
                kind,
                archive: Some(archive),
                manifest: Some(manifest),
            })
        }
    }
}
