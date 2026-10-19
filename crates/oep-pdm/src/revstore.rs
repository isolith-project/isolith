//! P8-M2-T01 -- Persisted, queryable revision store.
//!
//! The [`state`](crate::state) module is a pure machine with no IO; this
//! module makes a set of revisions durable and queryable.
//!
//! Layout under the store root:
//!
//! ```text
//! <root>/blobs/<sha256>                  content-addressed file payloads
//! <root>/revisions/<item_id>/<rev>.json  one snapshot per revision (authoritative)
//! <root>/items.json                      item index (revision numbers, latest, created_at)
//! ```
//!
//! Every mutation goes through the state machine first; only a validated
//! result is written to disk, so the on-disk state is always consistent with
//! the machine. Queries read the latest snapshots. File payloads are
//! content-addressed by their sha-256 and verified on read. `item_id` is
//! restricted to `[A-Za-z0-9._-]` so identifiers can never escape the store
//! root, and released/obsolete revisions reject every mutation (immutability
//! is enforced here too, not just in the machine).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::state::{Lifecycle, Maturity, RevisionState, TransitionError};

/// A revision snapshot as persisted. The machine carries the logic; this
/// carries the facts (state + maturity plus identity and audit fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredRevision {
    /// Owning item identifier (sanitized `[A-Za-z0-9._-]`).
    pub item_id: String,
    /// Positive, server-assigned revision number, starting at 1.
    pub revision: u32,
    /// Lifecycle state; always consistent with the machine.
    pub state: RevisionState,
    /// Maturity, or `None` when not assigned yet.
    pub maturity: Option<Maturity>,
    /// Unix seconds when the revision was created.
    pub created_at_unix: u64,
    /// Unix seconds of the last validated mutation.
    pub changed_at_unix: u64,
    /// Who created the revision (identifier only, never a secret).
    pub created_by: String,
    /// Free-form comment attached to the latest mutation, if any.
    pub comment: Option<String>,
    /// File name of the payload attached, if any.
    pub filename: Option<String>,
    /// sha-256 (hex) of the attached payload; content-addressed handle.
    pub blob_sha256: Option<String>,
}

impl StoredRevision {
    /// The revision's current lifecycle (state + maturity) for the machine.
    pub fn lifecycle(&self) -> Lifecycle {
        Lifecycle::from_parts(self.state, self.maturity)
    }
}

/// Item-level index entry, persisted in `items.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRecord {
    /// The item identifier.
    pub item_id: String,
    /// Unix seconds the item's first revision was created.
    pub created_at_unix: u64,
    /// Revision numbers in ascending order.
    pub revision_numbers: Vec<u32>,
}

/// Why a store operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The state machine refused the hop or mutation.
    State(TransitionError),
    /// The named revision does not exist in this store.
    NoSuchRevision {
        /// Item searched in.
        item_id: String,
        /// Revision number searched for.
        revision: u32,
    },
    /// The item_id does not meet the allowed alphabet `[A-Za-z0-9._-]`.
    InvalidItemId(String),
    /// A persisted payload failed to hash back to its content address.
    HashMismatch {
        /// The expected sha-256 hex recorded on the revision.
        expected: String,
        /// The sha-256 hex actually computed from the blob.
        actual: String,
    },
    /// A stored revision file could not be parsed (corrupt on disk).
    CorruptRevision {
        /// The file path that failed to parse.
        path: String,
    },
    /// A store-level JSON encode/decode failure.
    Json(String),
    /// A filesystem-level failure.
    Io(io::ErrorKind),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(e) => write!(f, "state machine: {e}"),
            Self::NoSuchRevision { item_id, revision } => {
                write!(f, "no such revision: {item_id} revision {revision}")
            }
            Self::InvalidItemId(id) => {
                write!(
                    f,
                    "invalid item id {id:?}: allowed alphabet is [A-Za-z0-9._-]"
                )
            }
            Self::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "content hash mismatch: expected {expected}, got {actual}"
                )
            }
            Self::CorruptRevision { path } => write!(f, "corrupt revision file: {path}"),
            Self::Json(e) => write!(f, "store json: {e}"),
            Self::Io(k) => write!(f, "store io: {k:?}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<TransitionError> for StoreError {
    fn from(e: TransitionError) -> Self {
        Self::State(e)
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind())
    }
}

/// A directory-backed, content-addressed revision store (see module docs).
#[derive(Debug, Clone)]
pub struct RevisionStore {
    root: PathBuf,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Return `Ok(id)` when the item id meets `[A-Za-z0-9._-]` and is non-empty.
pub fn validate_item_id(id: &str) -> Result<(), StoreError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(StoreError::InvalidItemId(id.to_string()));
    }
    Ok(())
}

impl RevisionStore {
    /// Open (creating on demand) a store rooted at `root`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("revisions")).map_err(StoreError::from)?;
        fs::create_dir_all(root.join("blobs")).map_err(StoreError::from)?;
        Ok(Self { root })
    }

    /// The store root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn revision_path(&self, item_id: &str, revision: u32) -> PathBuf {
        self.root
            .join("revisions")
            .join(item_id)
            .join(format!("{revision}.json"))
    }

    fn item_index_path(&self) -> PathBuf {
        self.root.join("items.json")
    }

    fn read_index(&self) -> BTreeMap<String, ItemRecord> {
        let raw = fs::read(self.item_index_path());
        match raw {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| BTreeMap::new()),
            Err(_) => BTreeMap::new(),
        }
    }

    fn write_index(&self, index: &BTreeMap<String, ItemRecord>) -> Result<(), StoreError> {
        let bytes =
            serde_json::to_vec_pretty(index).map_err(|e| StoreError::Json(e.to_string()))?;
        fs::write(self.item_index_path(), bytes).map_err(StoreError::from)
    }

    fn load_revision(&self, item_id: &str, revision: u32) -> Result<StoredRevision, StoreError> {
        validate_item_id(item_id)?;
        let path = self.revision_path(item_id, revision);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                return Err(StoreError::NoSuchRevision {
                    item_id: item_id.to_string(),
                    revision,
                })
            }
        };
        serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision {
            path: path.display().to_string(),
        })
    }

    fn save_revision(&self, rev: &StoredRevision) -> Result<(), StoreError> {
        validate_item_id(&rev.item_id)?;
        let dir = self.root.join("revisions").join(&rev.item_id);
        fs::create_dir_all(&dir).map_err(StoreError::from)?;
        let bytes = serde_json::to_vec_pretty(rev).map_err(|e| StoreError::Json(e.to_string()))?;
        fs::write(self.revision_path(&rev.item_id, rev.revision), bytes).map_err(StoreError::from)
    }

    fn ensure_item_indexed(
        &self,
        item_id: &str,
        created_at: u64,
    ) -> Result<BTreeMap<String, ItemRecord>, StoreError> {
        let mut index = self.read_index();
        match index.get_mut(item_id) {
            Some(rec) => {
                // Normalize any drift (sorted, no dupes).
                rec.revision_numbers.sort_unstable();
                rec.revision_numbers.dedup();
            }
            None => {
                index.insert(
                    item_id.to_string(),
                    ItemRecord {
                        item_id: item_id.to_string(),
                        created_at_unix: created_at,
                        revision_numbers: Vec::new(),
                    },
                );
            }
        }
        self.write_index(&index)?;
        Ok(index)
    }

    /// Create the next revision of an item as `working` with no maturity.
    ///
    /// Revision numbers are assigned per item, starting at 1 and strictly
    /// increasing. The returned snapshot is persisted immediately.
    pub fn create_revision(
        &mut self,
        item_id: &str,
        created_by: &str,
    ) -> Result<StoredRevision, StoreError> {
        validate_item_id(item_id)?;
        let index = self.ensure_item_indexed(item_id, now_unix())?;
        let last = index
            .get(item_id)
            .map(|r| r.revision_numbers.last().copied().unwrap_or(0))
            .unwrap_or(0);
        let revision = last + 1;
        let now = now_unix();
        let rev = StoredRevision {
            item_id: item_id.to_string(),
            revision,
            state: RevisionState::Working,
            maturity: None,
            created_at_unix: now,
            changed_at_unix: now,
            created_by: created_by.to_string(),
            comment: None,
            filename: None,
            blob_sha256: None,
        };
        self.save_revision(&rev)?;

        let mut idx = self.read_index();
        let rec = idx.get_mut(item_id).expect("indexed just above");
        rec.revision_numbers.push(revision);
        rec.revision_numbers.sort_unstable();
        self.write_index(&idx)?;
        Ok(rev)
    }

    fn mutate(
        &mut self,
        item_id: &str,
        revision: u32,
        op: impl FnOnce(&mut StoredRevision) -> Result<(), StoreError>,
    ) -> Result<StoredRevision, StoreError> {
        let mut rev = self.load_revision(item_id, revision)?;
        op(&mut rev)?;
        rev.changed_at_unix = now_unix();
        self.save_revision(&rev)?;
        Ok(rev)
    }

    /// Apply one state transition and persist the validated result.
    pub fn transition(
        &mut self,
        item_id: &str,
        revision: u32,
        to: RevisionState,
    ) -> Result<StoredRevision, StoreError> {
        self.mutate(item_id, revision, |rev| {
            let mut lc = rev.lifecycle();
            lc.transition_to(to)?;
            // The machine guarantees the new (state, maturity) is legal.
            rev.state = lc.state();
            rev.maturity = lc.maturity();
            Ok(())
        })
    }

    /// Set maturity and persist, enforcing the machine's transient rules.
    pub fn set_maturity(
        &mut self,
        item_id: &str,
        revision: u32,
        m: Maturity,
    ) -> Result<StoredRevision, StoreError> {
        self.mutate(item_id, revision, |rev| {
            let mut lc = rev.lifecycle();
            lc.set_maturity(m)?;
            rev.maturity = lc.maturity();
            Ok(())
        })
    }

    /// Advance maturity one rung and persist.
    pub fn advance_maturity(
        &mut self,
        item_id: &str,
        revision: u32,
    ) -> Result<StoredRevision, StoreError> {
        self.mutate(item_id, revision, |rev| {
            let mut lc = rev.lifecycle();
            lc.advance_maturity()?;
            rev.maturity = lc.maturity();
            Ok(())
        })
    }

    /// Attach a content-addressed payload to a non-frozen revision.
    ///
    /// The payload is stored under `<root>/blobs/<sha256>`, the revision is
    /// linked by hash, and the hash is what subsequent reads verify. Refused
    /// for released or obsolete revisions (immutability).
    pub fn attach_content(
        &mut self,
        item_id: &str,
        revision: u32,
        filename: &str,
        bytes: &[u8],
        comment: Option<&str>,
    ) -> Result<StoredRevision, StoreError> {
        // Refuse frozen revisions before touching the store: a rejected attach
        // must not leave a stray content-addressed blob behind.
        let existing = self.load_revision(item_id, revision)?;
        if existing.state.is_released() || existing.state.is_terminal() {
            return Err(StoreError::State(TransitionError::Immutable {
                op: "content",
                state: existing.state,
            }));
        }
        let digest = sha256_hex(bytes);
        let blob_path = self.root.join("blobs").join(&digest);
        if !blob_path.exists() {
            fs::create_dir_all(self.root.join("blobs")).map_err(StoreError::from)?;
            fs::write(&blob_path, bytes).map_err(StoreError::from)?;
        }
        self.mutate(item_id, revision, |rev| {
            rev.filename = Some(filename.to_string());
            rev.blob_sha256 = Some(digest.clone());
            if comment.is_some() {
                rev.comment = comment.map(|s| s.to_string());
            }
            Ok(())
        })
    }

    /// Re-compute the content hash of an attached blob and confirm it matches
    /// the recorded content address. Returns `false` on any missing/mismatch
    /// (not an error) so integrity checks can be batched.
    pub fn verify_content(&self, item_id: &str, revision: u32) -> Result<bool, StoreError> {
        let rev = self.load_revision(item_id, revision)?;
        let Some(expected) = rev.blob_sha256 else {
            return Ok(false);
        };
        let blob_path = self.root.join("blobs").join(&expected);
        match fs::read(&blob_path) {
            Ok(bytes) => Ok(sha256_hex(&bytes) == expected),
            Err(_) => Ok(false),
        }
    }

    /// Fetch one revision's snapshot.
    pub fn get_revision(&self, item_id: &str, revision: u32) -> Result<StoredRevision, StoreError> {
        self.load_revision(item_id, revision)
    }

    /// List an item's revisions, newest first.
    pub fn list_revisions(&self, item_id: &str) -> Result<Vec<StoredRevision>, StoreError> {
        validate_item_id(item_id)?;
        let dir = self.root.join("revisions").join(item_id);
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };
        let mut revs = Vec::new();
        for ent in entries.flatten() {
            if let Some(stem) = ent.path().file_stem().and_then(|s| s.to_str()) {
                if let Ok(n) = stem.parse::<u32>() {
                    if let Ok(rev) = self.load_revision(item_id, n) {
                        revs.push(rev);
                    }
                }
            }
        }
        revs.sort_by_key(|r| std::cmp::Reverse(r.revision));
        Ok(revs)
    }

    /// The highest-numbered revision of an item, if any.
    pub fn latest_revision(&self, item_id: &str) -> Result<Option<StoredRevision>, StoreError> {
        Ok(self.list_revisions(item_id)?.into_iter().next())
    }

    /// All items with their index records, sorted by item id.
    pub fn all_items(&self) -> Result<Vec<ItemRecord>, StoreError> {
        Ok(self.read_index().into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A unique, throwaway temp directory that cleans itself up.
    struct TempRoot {
        dir: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "oep-pdm-revsm-test-{}-{tag}-{n}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp root");
            Self { dir }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn invalid_item_ids_rejected() {
        assert!(validate_item_id("ok_id.1-2").is_ok());
        for bad in ["", "../etc", "a/b", "a b", "a\\b", "a:b"] {
            assert!(validate_item_id(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn create_assigns_sequential_revisions() {
        let t = TempRoot::new("seq");
        let mut store = RevisionStore::open(&t.dir).unwrap();
        let a = store.create_revision("part-001", "alice").unwrap();
        assert_eq!((a.item_id.as_str(), a.revision), ("part-001", 1));
        assert_eq!(a.state, RevisionState::Working);
        assert_eq!(a.maturity, None);
        let b = store.create_revision("part-001", "alice").unwrap();
        assert_eq!(b.revision, 2);
        let c = store.create_revision("other", "bob").unwrap();
        assert_eq!((c.item_id.as_str(), c.revision), ("other", 1));
    }

    #[test]
    fn list_latest_and_get_are_consistent() {
        let t = TempRoot::new("query");
        let mut store = RevisionStore::open(&t.dir).unwrap();
        for _ in 0..3 {
            store.create_revision("asm-9", "carol").unwrap();
        }
        let list = store.list_revisions("asm-9").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(
            list.iter().map(|r| r.revision).collect::<Vec<_>>(),
            vec![3, 2, 1],
            "newest first"
        );
        let latest = store.latest_revision("asm-9").unwrap().unwrap();
        assert_eq!(latest.revision, 3);
        let got = store.get_revision("asm-9", 2).unwrap();
        assert_eq!(got.revision, 2);
        assert!(store.get_revision("asm-9", 99).is_err());
        assert!(store.latest_revision("missing-item").unwrap().is_none());
    }

    #[test]
    fn released_immutability_holds_in_store() {
        let t = TempRoot::new("immut");
        let mut store = RevisionStore::open(&t.dir).unwrap();
        let rev = store.create_revision("plate", "dan").unwrap();
        let n = rev.revision;
        store.advance_maturity("plate", n).unwrap();
        store.advance_maturity("plate", n).unwrap();
        store.advance_maturity("plate", n).unwrap();
        store
            .transition("plate", n, RevisionState::Released)
            .unwrap();

        // State hops back to working / under_review refused.
        assert!(store
            .transition("plate", n, RevisionState::Working)
            .is_err());
        assert!(store
            .transition("plate", n, RevisionState::UnderReview)
            .is_err());
        // Maturity mutations refused.
        assert!(store
            .set_maturity("plate", n, Maturity::Preliminary)
            .is_err());
        assert!(store.advance_maturity("plate", n).is_err());
        // Content mutation refused.
        let err = store
            .attach_content("plate", n, "p.step", b"stuff", None)
            .unwrap_err();
        assert!(matches!(
            err,
            StoreError::State(TransitionError::Immutable { .. })
        ));
        // But released -> obsolete is still allowed.
        store
            .transition("plate", n, RevisionState::Obsolete)
            .unwrap();
    }

    #[test]
    fn obsolete_revision_is_fully_immutable() {
        let t = TempRoot::new("obs");
        let mut store = RevisionStore::open(&t.dir).unwrap();
        let rev = store.create_revision("doc-7", "erin").unwrap();
        let n = rev.revision;
        store
            .transition("doc-7", n, RevisionState::Obsolete)
            .unwrap();
        for to in RevisionState::ALL {
            assert!(store.transition("doc-7", n, to).is_err());
        }
        assert!(store.attach_content("doc-7", n, "f", b"x", None).is_err());
    }

    #[test]
    fn content_is_content_addressed_and_verified() {
        let t = TempRoot::new("blob");
        let mut store = RevisionStore::open(&t.dir).unwrap();
        let rev = store.create_revision("geom", "frank").unwrap();
        let n = rev.revision;
        let payload = b"geometry bytes 123";
        store
            .attach_content("geom", n, "g.step", payload, Some("geometry commit"))
            .unwrap();
        let rev = store.get_revision("geom", n).unwrap();
        let digest = rev.blob_sha256.clone().unwrap();
        assert_eq!(digest.len(), 64);
        // Blob exists under its own hash.
        let blob_path = store.root().join("blobs").join(&digest);
        assert!(blob_path.exists());
        let on_disk = fs::read(&blob_path).unwrap();
        assert_eq!(on_disk, payload);
        // Verification passes on the pristine blob.
        assert!(store.verify_content("geom", n).unwrap());
        // Tampering breaks the content address.
        fs::write(&blob_path, b"tampered!").unwrap();
        assert!(!store.verify_content("geom", n).unwrap());
    }

    #[test]
    fn state_and_immutability_survive_reopen() {
        let t = TempRoot::new("reopen");
        {
            let mut store = RevisionStore::open(&t.dir).unwrap();
            let rev = store.create_revision("bracket", "grace").unwrap();
            let n = rev.revision;
            for _ in 0..3 {
                store.advance_maturity("bracket", n).unwrap();
            }
            store
                .transition("bracket", n, RevisionState::UnderReview)
                .unwrap();
            store
                .transition("bracket", n, RevisionState::Released)
                .unwrap();
        }
        // Reopen: everything is still there and still enforced.
        let mut store = RevisionStore::open(&t.dir).unwrap();
        let rev = store.get_revision("bracket", 1).unwrap();
        assert_eq!(rev.state, RevisionState::Released);
        assert_eq!(rev.maturity, Some(Maturity::Final));
        assert_eq!(store.all_items().unwrap().len(), 1);
        assert!(store
            .transition("bracket", 1, RevisionState::Working)
            .is_err());
        assert!(store
            .set_maturity("bracket", 1, Maturity::Detailed)
            .is_err());
        // And a second item's revisions persisted too.
        store.create_revision("bracket", "grace").unwrap();
        assert_eq!(
            store.latest_revision("bracket").unwrap().unwrap().revision,
            2
        );
    }
}
