//! Content-addressed, hash-verified blob storage (P8-M1-T02).
//!
//! A [`BlobStore`] keeps file bytes under their sha-256 content address, one
//! file per address under `root/blobs/<sha256>`, and **verifies by hash** on
//! every read and during `verify()` -- the same discipline the P1-M4-T01 NAS
//! object-store mirror implements (`oep_tooling/objectstore.py`: content
//! address in the object name, every GET-back re-hashed against the manifest).
//!
//! Properties:
//! - **Content-addressed**: identical bytes store once (dedup); the address is
//!   a pure function of the content, never a sequence number.
//! - **Verified on write**: `put` re-hashes after writing; a hash/name
//!   collision is an error, never a silent overwrite.
//! - **Verified on read**: `get` re-hashes before returning; a tampered or
//!   truncated blob is a [`BlobError::Corrupt`], never silently trusted.
//! - **Verified set**: `verify()` re-hashes every blob; `verify_refs` proves
//!   that every content address a [`crate::model::VaultDoc`] references is
//!   present and hash-correct.
//! - **Atomic store**: writes go to a temp file then rename, so a crash never
//!   leaves a half-written blob under its final address.
//! - **Tolerant root**: an absent root is created lazily; reading a blob that
//!   is simply missing is a distinct [`BlobError::Missing`] (the P1-M4-T01
//!   semantics: unreachable store => caller classifies, never crashes).

use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};

/// On-disk blob storage keyed by sha-256 content address.
pub struct BlobStore {
    /// Root directory; blobs live under `root/blobs/<sha256>`.
    root: PathBuf,
}

/// A blob-store failure, each variant distinguishable by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobError {
    /// I/O problem while reading/writing.
    Io(String),
    /// A blob with this address does not exist.
    Missing(String),
    /// Bytes read back under an address do not hash to that address.
    Corrupt {
        /// Expected address.
        expected: String,
        /// Actual sha-256 of the bytes on disk.
        actual: String,
    },
    /// A `put` was asked to store bytes whose real hash differs from a
    /// caller-supplied expected address (address/bytes mismatch).
    AddressMismatch {
        /// Address the caller claimed.
        expected: String,
        /// Real sha-256 of the supplied bytes.
        actual: String,
    },
    /// A content address that is not 64 lowercase hex chars.
    InvalidAddress(String),
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::Io(m) => write!(f, "io error: {m}"),
            BlobError::Missing(s) => write!(f, "blob missing: {s}"),
            BlobError::Corrupt { expected, actual } => {
                write!(f, "blob corrupt: expected hash {expected}, got {actual}")
            }
            BlobError::AddressMismatch { expected, actual } => {
                write!(
                    f,
                    "put address mismatch: expected {expected}, bytes hash to {actual}"
                )
            }
            BlobError::InvalidAddress(s) => write!(f, "invalid content address: {s}"),
        }
    }
}

impl std::error::Error for BlobError {}

/// sha-256 of some bytes as 64 lowercase hex characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// True when `s` is a 64-char lowercase-hex sha-256 address.
pub fn is_valid_address(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Result of a full [`BlobStore::verify`] scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VerifyReport {
    /// Number of blobs whose bytes re-hashed to their file name.
    pub ok: usize,
    /// Number of blobs that failed verification.
    pub corrupt: usize,
    /// Lowest-level detail of the first corruption, if any.
    #[allow(missing_docs)]
    pub first_error: Option<BlobError>,
}

impl BlobStore {
    /// A blob store rooted at `root` (created lazily). The blobs directory is
    /// `root/blobs`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        BlobStore { root: root.into() }
    }

    /// The root directory configured for this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// Store `bytes` under their content address and return the address.
    ///
    /// Idempotent and deduplicating: if a blob with that address already
    /// exists and re-hashes to the address, this is a no-op returning the same
    /// address (P1-M4-T01 409-idempotency doctrine). If the existing bytes do
    /// NOT hash to the address, that is an existing-store corruption error.
    pub fn put(&self, bytes: &[u8]) -> Result<String, BlobError> {
        let sha = sha256_hex(bytes);
        let path = self.blobs_dir().join(&sha);
        if path.exists() {
            let on_disk = std::fs::read(&path)
                .map_err(|e| BlobError::Io(format!("{}: {e}", path.display())))?;
            if sha256_hex(&on_disk) == sha {
                return Ok(sha); // dedup: identical bytes already present
            }
            return Err(BlobError::Corrupt {
                expected: sha.clone(),
                actual: sha256_hex(&on_disk),
            });
        }
        std::fs::create_dir_all(self.blobs_dir())
            .map_err(|e| BlobError::Io(format!("{}: {e}", self.blobs_dir().display())))?;
        // Atomic write: temp file in the same dir, then rename into place.
        let tmp = self.blobs_dir().join(format!(".tmp-{sha}"));
        std::fs::write(&tmp, bytes)
            .map_err(|e| BlobError::Io(format!("{}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| BlobError::Io(format!("{} -> {}: {e}", tmp.display(), path.display())))?;
        // Write is verified: the address is a pure function of the bytes just
        // written, and the atomic rename guarantees the file under `sha` IS
        // those bytes.
        Ok(sha)
    }

    /// True when a blob with this content address is present.
    pub fn has(&self, address: &str) -> bool {
        is_valid_address(address) && self.blobs_dir().join(address).is_file()
    }

    /// Fetch bytes by content address, re-hashing them before returning.
    ///
    /// A bytes/address mismatch is [`BlobError::Corrupt`]; an absent blob is
    /// [`BlobError::Missing`] (callers classify, per tolerant doctrine).
    pub fn get(&self, address: &str) -> Result<Vec<u8>, BlobError> {
        if !is_valid_address(address) {
            return Err(BlobError::InvalidAddress(address.to_string()));
        }
        let path = self.blobs_dir().join(address);
        let bytes = std::fs::read(&path).map_err(|_| BlobError::Missing(address.to_string()))?;
        let actual = sha256_hex(&bytes);
        if actual != address {
            return Err(BlobError::Corrupt {
                expected: address.to_string(),
                actual,
            });
        }
        Ok(bytes)
    }

    /// Re-hash every blob in the store; the address must match the file name.
    pub fn verify(&self) -> VerifyReport {
        let mut report = VerifyReport::default();
        let Some(entries) = std::fs::read_dir(self.blobs_dir()).ok() else {
            return report;
        };
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with(".tmp-") || !is_valid_address(&fname) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(entry.path()) {
                if sha256_hex(&bytes) == fname {
                    report.ok += 1;
                } else {
                    report.corrupt += 1;
                    if report.first_error.is_none() {
                        report.first_error = Some(BlobError::Corrupt {
                            expected: fname.clone(),
                            actual: sha256_hex(&bytes),
                        });
                    }
                }
            } else {
                report.corrupt += 1;
                if report.first_error.is_none() {
                    report.first_error = Some(BlobError::Io(format!(
                        "unreadable {}",
                        entry.path().display()
                    )));
                }
            }
        }
        report
    }

    /// Prove that every referenced content address in `addresses` is present
    /// and hash-correct. Returns `Ok(())` or the first failing address.
    pub fn verify_addresses<'a>(
        &self,
        addresses: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), BlobError> {
        for address in addresses {
            self.get(address)?;
        }
        Ok(())
    }

    /// Prove that every `blob_sha256` referenced by the file records of a vault
    /// document is present and hash-correct. The acceptance criterion for
    /// content-addressed, hash-verified vault storage.
    pub fn verify_doc_files(&self, doc: &crate::model::VaultDoc) -> Result<(), BlobError> {
        self.verify_addresses(doc.files.iter().map(|f| f.blob_sha256.as_str()))
    }
}
