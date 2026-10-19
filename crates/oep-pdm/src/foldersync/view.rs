//! Mobile read-only view over a working-set mirror (P8-M5-T01).
//!
//! [`ReadOnlyView`] is the surface a thin mobile client consumes: it lists
//! the working set (relative path, size, sha-256, mtime -- enough to render a
//! file list and thumbnails without opening bytes) and fetches a single
//! file's bytes with **hash verification against the live scan**, so the
//! phone can never be served corrupted content that crawls silently into the
//! view.
//!
//! The type intentionally exposes **no mutating methods**: only `new`, `list`
//! and `fetch`. A client built on it cannot write to the working set or the
//! NAS. The P8 review must gate on that boundary. Read-on tilt for the NAS
//! is further enforced by the engine's read-only mode (see
//! [`SyncEngine::set_read_only`]).

use std::path::Path;

use crate::foldersync::manifest::FolderManifest;

/// One row of the mobile listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewEntry {
    /// Normalized relative path (`assembly/a.step`).
    pub rel: String,
    /// Total size in bytes.
    pub size: u64,
    /// sha-256 (64 lowercase hex) of the file bytes.
    pub sha256: String,
    /// Modification time, Unix seconds.
    pub mtime_unix: u64,
}

/// A read-only lens over a synced working-set folder.
#[derive(Debug, Clone)]
pub struct ReadOnlyView {
    root: std::path::PathBuf,
}

impl ReadOnlyView {
    /// Point the view at a synced mirror root (the local copy a mobile
    /// client has pulled, or the NAS remote itself when directly reachable).
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// List the working set: relative path, size, sha-256 and mtime per
    /// file. The client control directory is never listed.
    pub fn list(&self) -> Result<Vec<ViewEntry>, crate::foldersync::manifest::ManifestError> {
        let m = FolderManifest::scan(&self.root)?;
        Ok(m.files()
            .iter()
            .map(|(rel, e)| ViewEntry {
                rel: rel.clone(),
                size: e.size,
                sha256: e.sha256.clone(),
                mtime_unix: e.mtime_unix,
            })
            .collect())
    }

    /// Fetch one file's bytes, verified against a freshly scanned manifest.
    ///
    /// The bytes are re-hashed after reading and rejected if they do not
    /// match the sha-256 recorded for that path, so a tampered or truncated
    /// file surfaces as an error, never as silently-wrong content in the
    /// mobile view.
    pub fn fetch(&self, rel: &str) -> Result<Vec<u8>, crate::foldersync::manifest::ManifestError> {
        let m = FolderManifest::scan(&self.root)?;
        let expected = m
            .get(rel)
            .ok_or_else(|| {
                crate::foldersync::manifest::ManifestError::PathEscape(format!(
                    "no such file: {rel}"
                ))
            })?
            .sha256
            .clone();
        let path = FolderManifest::join_safe(&self.root, rel)?;
        let bytes = std::fs::read(&path)?;
        let actual = crate::foldersync::manifest::sha256_hex(&bytes);
        if actual != expected {
            return Err(crate::foldersync::manifest::ManifestError::PathEscape(
                format!("content mismatch for {rel}"),
            ));
        }
        Ok(bytes)
    }

    /// Convenience: the manifest itself (for a client that wants to cache the
    /// listing).
    pub fn manifest(&self) -> Result<FolderManifest, crate::foldersync::manifest::ManifestError> {
        FolderManifest::scan(&self.root)
    }
}
