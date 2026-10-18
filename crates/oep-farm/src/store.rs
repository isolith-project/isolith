//! Where checkpoints live.
//!
//! The farm runner writes a [Checkpoint] to a [CheckpointStore] periodically
//! while it runs and reads the latest one when it (re)starts. Two backends
//! ship:
//!
//! - [MemoryStore]: a plain in-memory table for unit tests and the fast path.
//! - [DirStore]: a durable directory store used by the *worker-eviction
//!   drill* - the point of checkpoints is that they survive the worker's
//!   process going away. A new worker opens the same directory, reads the
//!   latest checkpoint file and resumes from it. Writes go to a temp file
//!   then rename, so a crash mid-write never leaves a torn checkpoint.
//!
//! The [CheckpointStore] trait (keyed by job id, `load_latest`/`save`) is the
//! seam a NATS JetStream KV or P8 vault-backed store can plug into later
//! without touching the runner.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::record::{Checkpoint, CheckpointError};

/// Errors from the checkpoint store.
#[derive(Debug)]
pub enum StoreError {
    /// Underlying filesystem error.
    Io(std::io::Error),
    /// A stored checkpoint could not be decoded (file is foreign/corrupt).
    BadRecord(CheckpointError),
    /// The requested job id is not valid for this store (e.g. path unsafe).
    InvalidJobId(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "store io error: {e}"),
            StoreError::BadRecord(e) => write!(f, "store bad record: {e}"),
            StoreError::InvalidJobId(s) => write!(f, "store invalid job id: {s}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// Blanket impl so callers can pass `&mut S` (a borrowed store) wherever an
/// owned `S` would do: the runner only needs a single in-flight account.
impl<S: CheckpointStore + ?Sized> CheckpointStore for &mut S {
    fn load_latest(&self, job_id: &str) -> Result<Option<Checkpoint>, StoreError> {
        (**self).load_latest(job_id)
    }

    fn save(&mut self, ckpt: &Checkpoint) -> Result<(), StoreError> {
        (**self).save(ckpt)
    }
}

/// A key/value-ish persistence seam for checkpoints.
pub trait CheckpointStore {
    /// The most recent checkpoint for `job_id`, or `None` if none was saved.
    fn load_latest(&self, job_id: &str) -> Result<Option<Checkpoint>, StoreError>;
    /// Persist `ckpt` (atomic in the durable backend).
    fn save(&mut self, ckpt: &Checkpoint) -> Result<(), StoreError>;
}

/// Checkpoint file suffix used by [DirStore].
pub const SUFFIX: &str = ".ckpt";

// ---------------------------------------------------------------------------

/// An in-memory checkpoint store (deterministic, no IO).
#[derive(Debug, Default, Clone)]
pub struct MemoryStore {
    records: HashMap<String, Checkpoint>,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        MemoryStore::default()
    }
}

impl CheckpointStore for MemoryStore {
    fn load_latest(&self, job_id: &str) -> Result<Option<Checkpoint>, StoreError> {
        Ok(self.records.get(job_id).cloned())
    }

    fn save(&mut self, ckpt: &Checkpoint) -> Result<(), StoreError> {
        self.records.insert(ckpt.job_id.clone(), ckpt.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// A durable directory-backed checkpoint store.
///
/// Files live at `<dir>/<job_id>.ckpt`. `save` writes to a unique temp file
/// in the same directory and renames over the destination, so a crash between
/// the two leaves the previous checkpoint intact (never a torn record).
#[derive(Debug, Clone)]
pub struct DirStore {
    dir: PathBuf,
}

impl DirStore {
    /// Open (creating if needed) `dir` as a checkpoint store.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(DirStore { dir })
    }

    /// A path to store a job's checkpoint at.
    fn path_for(&self, job_id: &str) -> Result<PathBuf, StoreError> {
        let base = job_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        if base.is_empty() {
            return Err(StoreError::InvalidJobId(job_id.to_string()));
        }
        Ok(self.dir.join(format!("{base}{SUFFIX}")))
    }
}

impl CheckpointStore for DirStore {
    fn load_latest(&self, job_id: &str) -> Result<Option<Checkpoint>, StoreError> {
        let path = self.path_for(job_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        Checkpoint::decode(&bytes)
            .map(Some)
            .map_err(StoreError::BadRecord)
    }

    fn save(&mut self, ckpt: &Checkpoint) -> Result<(), StoreError> {
        let path = self.path_for(&ckpt.job_id)?;
        let bytes = ckpt.encode();
        let tmp = self.dir.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            ckpt.job_id
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect::<String>()
        ));
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Checkpoint;

    fn sample(job: &str, next: u64) -> Checkpoint {
        Checkpoint::new(job, next, next * 8, next * 1024)
    }

    #[test]
    fn memory_store_round_trips() {
        let mut s = MemoryStore::new();
        assert_eq!(s.load_latest("a").unwrap(), None);
        s.save(&sample("a", 3)).unwrap();
        assert_eq!(s.load_latest("a").unwrap(), Some(sample("a", 3)));
        // A later checkpoint overwrites for the same job.
        s.save(&sample("a", 9)).unwrap();
        assert_eq!(s.load_latest("a").unwrap(), Some(sample("a", 9)));
        assert_eq!(s.load_latest("b").unwrap(), None);
    }

    #[test]
    fn dir_store_survives_new_handle() {
        let dir = std::env::temp_dir().join(format!(
            "oep-farm-dirstore-{}-{}",
            std::process::id(),
            crate::splitmix64(0x5eed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut s = DirStore::open(&dir).unwrap();
            s.save(&sample("render-job", 42)).unwrap();
        }
        // A brand-new store handle reading the same directory sees the record:
        // this is the "worker evicted, new worker, same disk" shape.
        let fresh = DirStore::open(&dir).unwrap();
        assert_eq!(
            fresh.load_latest("render-job").unwrap(),
            Some(sample("render-job", 42))
        );
        assert_eq!(fresh.load_latest("other").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dir_store_path_sanitizes_jobs() {
        let dir = std::env::temp_dir().join(format!(
            "oep-farm-dirstore2-{}-{}",
            std::process::id(),
            crate::splitmix64(0xbeef)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = DirStore::open(&dir).unwrap();
        // Spaces and slashes in the id must not escape the store dir.
        s.save(&sample("j b/x", 1)).unwrap();
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            found.push(entry.unwrap().file_name().to_string_lossy().to_string());
        }
        assert_ne!(found, vec!["x.ckpt".to_string()]);
        assert!(found.iter().all(|f| !f.contains('/')));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
