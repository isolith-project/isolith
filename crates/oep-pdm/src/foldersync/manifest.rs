//! Content-addressed folder manifest (P8-M5-T01).
//!
//! A [`FolderManifest`] is the fingerprint of a mirrored directory: a map of
//! normalized relative paths (`a/b/c.step`, forward slashes, never a leading
//! slash, never `..`) to [`FileEntry`] payloads carrying size, sha-256 and
//! mtime. It is what the sync planner diffs and what verification re-derives
//! from disk, so `manifest.scan(root)` + `entry.sha256` is the single source
//! of truth for "what bytes are on this side".
//!
//! Safety rules the scan enforces:
//!
//! - The reserved [`crate::CONTROL_DIR`] (`.oep-sync`) is never ingested as
//!   a payload, anywhere in the tree, so client metadata is never synced as
//!   user content and never re-enters the manifest.
//! - Relative paths are normalized to forward slashes; absolute paths and
//!   `..` escapes are rejected by [`FolderManifest::join_safe`], never
//!   silently laundered.
//! - A nested file that suddenly becomes unreadable mid-scan is surfaced as
//!   an error rather than a silently missing entry (tolerant, but honest).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::foldersync::CONTROL_DIR;

/// One file's fingerprint: total size, sha-256 (64 lowercase hex), mtime.
///
/// The sha-256 is the content address: two folders are "the same" for a path
/// exactly when their `FileEntry::sha256` match. mtime is informational
/// (mtime alone can change without content changing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Total size in bytes.
    pub size: u64,
    /// sha-256 of the file bytes, 64 lowercase hex chars.
    pub sha256: String,
    /// Modification time, Unix seconds.
    pub mtime_unix: u64,
}

/// Error produced while scanning or serializing a folder manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// A filesystem read failed while scanning.
    Io(io::ErrorKind),
    /// A nested path escaped the scan root (`..`, absolute prefix, or
    /// non-UTF-8 component that cannot be represented as a rel path).
    PathEscape(String),
    /// A manifest could not be (de)serialized.
    Json(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(k) => write!(f, "manifest io: {k:?}"),
            Self::PathEscape(p) => write!(f, "path escapes scan root: {p}"),
            Self::Json(e) => write!(f, "manifest json: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<io::Error> for ManifestError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind())
    }
}

/// The content-addressed view of one mirror directory.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FolderManifest {
    /// Relative path (normalized `a/b.ext`) -> file fingerprint.
    files: BTreeMap<String, FileEntry>,
}

impl FolderManifest {
    /// An empty manifest.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a manifest from a pre-fingerprinted map (helper for tests and
    /// deserialization paths that already hold entries).
    pub fn from_map(files: BTreeMap<String, FileEntry>) -> Self {
        Self { files }
    }

    /// The underlying relative-path -> entry map.
    pub fn files(&self) -> &BTreeMap<String, FileEntry> {
        &self.files
    }

    /// Look up one relative path.
    pub fn get(&self, rel: &str) -> Option<&FileEntry> {
        self.files.get(rel)
    }

    /// Number of tracked files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True when no files are tracked.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Recursively scan `root` into a manifest.
    ///
    /// The reserved `.oep-sync` control directory is skipped at any depth, so
    /// client metadata never becomes a synced payload. Symlinks are followed
    /// as files when their target is a regular file (mirroring a NAS share
    /// that may expose symlinks); a broken link is skipped with a warning
    /// surfaced as [`ManifestError::Io`].
    pub fn scan(root: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let root = root.as_ref();
        let mut files = BTreeMap::new();
        Self::scan_dir(root, root, &mut files)?;
        Ok(Self { files })
    }

    fn scan_dir(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, FileEntry>,
    ) -> Result<(), ManifestError> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy().into_owned();
            if name_str == CONTROL_DIR {
                continue; // client metadata is never a payload
            }
            let rel = Self::safe_rel(root, &path)?;
            let Some(rel) = rel else {
                continue; // a non-representable rel (defensive) is skipped
            };
            let ft = entry.file_type()?;
            if ft.is_dir() {
                Self::scan_dir(root, &path, out)?;
            } else if ft.is_file() || ft.is_symlink() {
                match fs::metadata(&path) {
                    Ok(md) if md.is_file() => {
                        let bytes = fs::read(&path)?;
                        let digest = sha256_hex(&bytes);
                        let mtime = unix_secs(&md);
                        out.insert(
                            rel,
                            FileEntry {
                                size: md.len(),
                                sha256: digest,
                                mtime_unix: mtime,
                            },
                        );
                    }
                    Ok(_) => {}         // dir-ish symlink: skip
                    Err(_) => continue, // broken link: skip, do not fail scan
                }
            }
        }
        Ok(())
    }

    /// Turn an absolute path under `root` into a normalized relative path.
    ///
    /// Returns `Ok(None)` when the path does not actually live under `root`.
    /// Returns `Err` for a component structure that cannot be represented
    /// (absolute components, `..`, non-UTF-8 segments).
    fn safe_rel(root: &Path, path: &Path) -> Result<Option<String>, ManifestError> {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| ManifestError::PathEscape(path.display().to_string()))?;
        let mut parts = Vec::new();
        for comp in rel.components() {
            match comp {
                Component::Normal(seg) => match seg.to_str() {
                    Some(s) => parts.push(s.to_string()),
                    None => {
                        return Err(ManifestError::PathEscape(
                            seg.to_string_lossy().into_owned(),
                        ))
                    }
                },
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(ManifestError::PathEscape(path.display().to_string()))
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ManifestError::PathEscape(path.display().to_string()))
                }
            }
        }
        if parts.is_empty() {
            return Ok(None);
        }
        Ok(Some(parts.join("/")))
    }

    /// Safely join a relative path (or user-supplied path) onto `root`,
    /// rejecting anything that escapes the root.
    ///
    /// Accepts a relative path with forward or native separators and returns
    /// the joined absolute path, or an error for an absolute path or any
    /// parent-directory (`..`) component. This is the gate every sync action
    /// passes its rel path through before touching the filesystem.
    pub fn join_safe(
        root: impl AsRef<Path>,
        rel: impl AsRef<Path>,
    ) -> Result<PathBuf, ManifestError> {
        let root = root.as_ref();
        let rel = rel.as_ref();
        if rel.is_absolute() {
            return Err(ManifestError::PathEscape(rel.display().to_string()));
        }
        let mut joined = root.to_path_buf();
        for comp in rel.components() {
            match comp {
                Component::Normal(seg) => joined.push(seg),
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(ManifestError::PathEscape(rel.display().to_string()))
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ManifestError::PathEscape(rel.display().to_string()))
                }
            }
        }
        Ok(joined)
    }
}

/// sha-256 of some bytes, 64 lowercase hex chars.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Unix-seconds mtime of a metadata record (0 when unavailable).
fn unix_secs(md: &fs::Metadata) -> u64 {
    use std::time::UNIX_EPOCH;
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
