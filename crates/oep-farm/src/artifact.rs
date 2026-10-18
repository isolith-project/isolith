//! The streaming artifact a farm job writes its output into.
//!
//! An [Artifact] holds the *output bytes* a long job has produced so far.
//! Resume needs three operations beyond append: the current length, reading a
//! prefix back out (to re-hash and verify the byte-equivalence gate), and
//! truncating back to a checkpoint boundary (to discard the tail of an item a
//! dying worker started but never finished).
//!
//! Two implementations ship:
//!
//! - [VecArtifact]: in-memory bytes, used by unit tests and the in-memory
//!   drill (fast, deterministic).
//! - [FileArtifact]: a real file on disk, used by the durable drill and the
//!   `oep-farm-drill` binary - in production the artifact is where the job's
//!   output refs (P9-M1) eventually point.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// An append-mostly, truncatable byte sink.
pub trait Artifact {
    /// Current number of bytes in the artifact.
    fn len(&self) -> u64;
    /// Whether the artifact is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Append bytes to the end.
    fn append(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    /// Read the whole artifact as bytes.
    fn read_all(&self) -> std::io::Result<Vec<u8>>;
    /// Truncate to exactly `new_len` bytes (no-op if already shorter/equal).
    fn truncate(&mut self, new_len: u64) -> std::io::Result<()>;
}

/// An in-memory artifact (for fast, deterministic tests and drills).
#[derive(Debug, Clone, Default)]
pub struct VecArtifact {
    bytes: Vec<u8>,
}

impl VecArtifact {
    /// An empty artifact.
    pub fn new() -> Self {
        VecArtifact::default()
    }
}

impl Artifact for VecArtifact {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn read_all(&self) -> std::io::Result<Vec<u8>> {
        Ok(self.bytes.clone())
    }

    fn truncate(&mut self, new_len: u64) -> std::io::Result<()> {
        let new_len = new_len as usize;
        if new_len < self.bytes.len() {
            self.bytes.truncate(new_len);
        }
        Ok(())
    }
}

/// A file-backed artifact. Used by the durable eviction drill: the file
/// survives the worker object being dropped, which is what lets a new worker
/// pick up the same artifact after an eviction.
#[derive(Debug)]
pub struct FileArtifact {
    file: std::fs::File,
    len: u64,
}

impl FileArtifact {
    /// Open (creating if needed) `path` as an append artifact.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        // open in read+write, explicitly NOT truncate : we append at the end
        // (and FileArtifact::truncate is the explicit rewind used on resume).
        let mut f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        let len = f.metadata()?.len();
        f.seek(SeekFrom::Start(len))?;
        Ok(FileArtifact { file: f, len })
    }
}

impl Artifact for FileArtifact {
    fn len(&self) -> u64 {
        self.len
    }

    fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.file.write_all(bytes)?;
        self.len += bytes.len() as u64;
        Ok(())
    }

    fn read_all(&self) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.len as usize);
        let mut f = self.file.try_clone()?;
        f.seek(SeekFrom::Start(0))?;
        f.read_to_end(&mut out)?;
        Ok(out)
    }

    fn truncate(&mut self, new_len: u64) -> std::io::Result<()> {
        if new_len < self.len {
            self.file.set_len(new_len)?;
            self.len = new_len;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_artifact_append_read_truncate() {
        let mut a = VecArtifact::new();
        a.append(b"hello ").unwrap();
        a.append(b"world").unwrap();
        assert_eq!(a.len(), 11);
        assert_eq!(a.read_all().unwrap(), b"hello world");
        a.truncate(6).unwrap();
        assert_eq!(a.len(), 6);
        assert_eq!(a.read_all().unwrap(), b"hello ");
        // truncate beyond length is a no-op
        a.truncate(100).unwrap();
        assert_eq!(a.len(), 6);
    }

    #[test]
    fn file_artifact_round_trip_via_new_handle() {
        let dir = std::env::temp_dir().join(format!(
            "oep-farm-artifact-{}-{}",
            std::process::id(),
            crate::splitmix64(0x7777)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.bin");
        {
            let mut a = FileArtifact::open(&path).unwrap();
            a.append(b"abc").unwrap();
            a.append(b"def").unwrap();
        }
        // New handle over the same file sees the durable bytes.
        let mut b = FileArtifact::open(&path).unwrap();
        assert_eq!(b.len(), 6);
        assert_eq!(b.read_all().unwrap(), b"abcdef");
        b.truncate(3).unwrap();
        assert_eq!(b.read_all().unwrap(), b"abc");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
