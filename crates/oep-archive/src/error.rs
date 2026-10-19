//! Typed failures for the P10 archive lane. Deliberately narrow and explicit:
//! every variant names the layer that failed, so a CLI can present it and a
//! test can map it. No variant ever hides a panic; the archive lane is
//! tolerant per-file (skips), so hard failures here are I/O, schema, and
//! vault-shape problems only.

use std::fmt;

/// A hard failure in the archive lane.
#[derive(Debug)]
pub enum ArchiveError {
    /// Filesystem error (read/write/create).
    Io(String),
    /// The P8 vault store refused an operation.
    Vault(String),
    /// JSON encode/decode failure.
    Serde(String),
    /// A STEP read/write failure.
    Step(String),
    /// A CGR mesh extraction failure.
    Cgr(String),
    /// A geometry writer failure (glTF / STL / 3DXML).
    Geometry(String),
    /// An archive schema/marker/version validation failure.
    Schema(String),
    /// A required archive member is missing.
    NotFound(String),
    /// The archive's own records disagree with its bytes.
    Inconsistent(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "io error: {e}"),
            ArchiveError::Vault(e) => write!(f, "vault error: {e}"),
            ArchiveError::Serde(e) => write!(f, "json error: {e}"),
            ArchiveError::Step(e) => write!(f, "step error: {e}"),
            ArchiveError::Cgr(e) => write!(f, "cgr error: {e}"),
            ArchiveError::Geometry(e) => write!(f, "geometry error: {e}"),
            ArchiveError::Schema(e) => write!(f, "schema error: {e}"),
            ArchiveError::NotFound(e) => write!(f, "not found: {e}"),
            ArchiveError::Inconsistent(e) => write!(f, "inconsistent archive: {e}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for ArchiveError {
    fn from(e: serde_json::Error) -> Self {
        ArchiveError::Serde(e.to_string())
    }
}

impl From<oep_pdm::store::StoreError> for ArchiveError {
    fn from(e: oep_pdm::store::StoreError) -> Self {
        ArchiveError::Vault(e.to_string())
    }
}

impl From<oep_step::Error> for ArchiveError {
    fn from(e: oep_step::Error) -> Self {
        ArchiveError::Step(e.to_string())
    }
}

impl From<oep_cgr::mesh::MeshError> for ArchiveError {
    fn from(e: oep_cgr::mesh::MeshError) -> Self {
        ArchiveError::Cgr(e.to_string())
    }
}
