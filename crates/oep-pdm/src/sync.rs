//! P8-M5-T02: offline working set + sync-back on reconnect.
//!
//! The client-side half of offline mode. While disconnected from the PDM
//! service, edits are recorded into a durable [`OfflineWorkingSet`] -- a plain
//! local folder of content-addressed blobs plus a JSON index -- with zero
//! server contact. On reconnect a [`SyncEngine`] replays the pending edits
//! through `POST /api/v1/sync/push`; the server accepts each one as a clean
//! next revision when its head still equals the edit's recorded base, and
//! returns a pending-merge id when it has diverged (someone else committed
//! meanwhile). Divergent edits are then resolved through the conflict surface
//! (`/api/v1/sync/conflicts/...` and the `/ui/sync/conflicts/...` 3-way page)
//! delivered by the service.
//!
//! Every byte stored locally is content-addressed and hashed at record time;
//! the local hash is what the server is asked to reproduce, so a clean sync or
//! a resolution is verifiable by hash without trusting either side's memory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::api::{FileUpload, SyncPushRequest, SyncPushResult, SyncResolveRequest};
use crate::client::{PdmClient, PdmError};

/// Error surfaced by the local working set (IO / serialization / integrity).
#[derive(Debug)]
pub enum SyncError {
    /// The working-set directory or one of its files could not be read/written.
    Io(String),
    /// A local index or blob failed to (de)serialize.
    Format(String),
    /// The requested edit does not exist in the working set.
    NotFound(String),
    /// The blob bytes on disk do not match the index's recorded hash.
    Integrity {
        /// The edit id whose blob failed verification.
        edit_id: String,
        /// Expected sha-256 (from the index).
        expected: String,
        /// Actual sha-256 of the bytes on disk.
        actual: String,
    },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Io(m) => write!(f, "working-set io: {m}"),
            SyncError::Format(m) => write!(f, "working-set format: {m}"),
            SyncError::NotFound(m) => write!(f, "working-set entry not found: {m}"),
            SyncError::Integrity {
                edit_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "working-set blob integrity for {edit_id}: expected {expected}, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for SyncError {}

/// Persisted lifecycle status of one local edit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EditStatus {
    /// Recorded while offline, not yet pushed to the server.
    Pending,
    /// Pushed cleanly and accepted as a server revision.
    Synced {
        /// The revision number the server assigned.
        revision: String,
    },
    /// Pushed but divergent; a pending merge exists server-side at `sync_id`.
    Conflicted {
        /// Server-minted pending-merge id (resolution surface).
        sync_id: String,
    },
}

/// One edit recorded into the local working set while offline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEdit {
    /// Client-local edit id (opaque, unique within the working set).
    pub edit_id: String,
    /// The vault item the edit targets.
    pub item_id: String,
    /// The revision number this edit was based on (server identity).
    pub base_revision: String,
    /// Content address of base_revision's file (content truth for divergence).
    pub base_blob_sha256: String,
    /// Original file name of the edited document.
    pub filename: String,
    /// Format tag (`step`, `cgr`, ...).
    pub format: String,
    /// Optional media type.
    pub content_type: Option<String>,
    /// Optional lifecycle state for the new revision.
    pub state: Option<String>,
    /// Optional change comment.
    pub comment: Option<String>,
    /// Optional acting principal tag.
    pub created_by: Option<String>,
    /// Content address of the edited bytes (stored in `blobs/`).
    pub ours_blob_sha256: String,
    /// Decoded size of the edited bytes.
    pub ours_size: u64,
    /// Current lifecycle status.
    pub status: EditStatus,
}

/// The on-disk working-set index (matches [`OfflineWorkingSet`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkingSetIndex {
    /// Pending (and historical) local edits, oldest first.
    edits: Vec<LocalEdit>,
}

/// A durable local working set: a folder of content-addressed blobs plus a
/// JSON index of edits, no server required for record/read.
pub struct OfflineWorkingSet {
    dir: PathBuf,
    index: WorkingSetIndex,
}

impl OfflineWorkingSet {
    /// Open (creating if needed) a working set rooted at `dir`.
    pub fn open(dir: &Path) -> Result<Self, SyncError> {
        std::fs::create_dir_all(dir)
            .map_err(|e| SyncError::Io(format!("create {}: {e}", dir.display())))?;
        let index_path = dir.join("index.json");
        let index = if index_path.exists() {
            let text = std::fs::read_to_string(&index_path)
                .map_err(|e| SyncError::Io(format!("read {}: {e}", index_path.display())))?;
            serde_json::from_str(&text)
                .map_err(|e| SyncError::Format(format!("parse index.json: {e}")))?
        } else {
            WorkingSetIndex::default()
        };
        Ok(OfflineWorkingSet {
            dir: dir.to_path_buf(),
            index,
        })
    }

    /// Persist the index to disk.
    pub fn save(&self) -> Result<(), SyncError> {
        let text = serde_json::to_string_pretty(&self.index)
            .map_err(|e| SyncError::Format(format!("serialize index: {e}")))?;
        std::fs::write(self.dir.join("index.json"), text)
            .map_err(|e| SyncError::Io(format!("write index.json: {e}")))
    }

    /// The working-set root directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// All local edits, oldest first (any status).
    pub fn edits(&self) -> &[LocalEdit] {
        &self.index.edits
    }

    /// Edits still waiting to be synced.
    pub fn pending(&self) -> impl Iterator<Item = &LocalEdit> {
        self.index
            .edits
            .iter()
            .filter(|e| e.status == EditStatus::Pending)
    }

    /// Fetch one local edit by id.
    pub fn get(&self, edit_id: &str) -> Result<&LocalEdit, SyncError> {
        self.index
            .edits
            .iter()
            .find(|e| e.edit_id == edit_id)
            .ok_or_else(|| SyncError::NotFound(edit_id.to_string()))
    }

    /// Record a new offline edit (P8-M5-T02 "local working set"). Writes the
    /// edited bytes content-addressed under `blobs/<sha256>`, appends a
    /// [`LocalEdit`] to the index, hashing the bytes at record time (no server).
    /// Returns the generated `edit_id`.
    #[allow(clippy::too_many_arguments)]
    pub fn record_edit(
        &mut self,
        item_id: &str,
        base_revision: &str,
        base_blob_sha256: &str,
        filename: &str,
        format: &str,
        content_type: Option<String>,
        state: Option<String>,
        comment: Option<String>,
        created_by: Option<String>,
        bytes: &[u8],
    ) -> Result<String, SyncError> {
        let sha = hex_sha256(bytes);
        self.write_blob(&sha, bytes)?;
        let edit_id = {
            let n = self.index.edits.len();
            format!("edit-{n}-{}", &sha[..8])
        };
        self.index.edits.push(LocalEdit {
            edit_id: edit_id.clone(),
            item_id: item_id.to_string(),
            base_revision: base_revision.to_string(),
            base_blob_sha256: base_blob_sha256.to_string(),
            filename: filename.to_string(),
            format: format.to_string(),
            content_type,
            state,
            comment,
            created_by,
            ours_blob_sha256: sha,
            ours_size: bytes.len() as u64,
            status: EditStatus::Pending,
        });
        self.save()?;
        Ok(edit_id)
    }

    /// Update one edit's status and persist.
    fn set_status(&mut self, edit_id: &str, status: EditStatus) -> Result<(), SyncError> {
        let edit = self
            .index
            .edits
            .iter_mut()
            .find(|e| e.edit_id == edit_id)
            .ok_or_else(|| SyncError::NotFound(edit_id.to_string()))?;
        edit.status = status;
        self.save()
    }

    /// Mark a pending edit as cleanly synced to `revision`.
    pub fn mark_synced(&mut self, edit_id: &str, revision: &str) -> Result<(), SyncError> {
        self.set_status(
            edit_id,
            EditStatus::Synced {
                revision: revision.to_string(),
            },
        )
    }

    /// Mark a pending edit as conflicted (divergent) at server `sync_id`.
    pub fn mark_conflicted(&mut self, edit_id: &str, sync_id: &str) -> Result<(), SyncError> {
        self.set_status(
            edit_id,
            EditStatus::Conflicted {
                sync_id: sync_id.to_string(),
            },
        )
    }

    /// The edited (ours) bytes for one local edit, verified against the hash
    /// recorded at sync time so a corrupted local blob is surfaced, not synced.
    pub fn ours_bytes(&self, edit_id: &str) -> Result<Vec<u8>, SyncError> {
        let edit = self.get(edit_id)?;
        let bytes = std::fs::read(self.dir.join("blobs").join(&edit.ours_blob_sha256))
            .map_err(|e| SyncError::Io(format!("read blob for {edit_id}: {e}")))?;
        let actual = hex_sha256(&bytes);
        if actual != edit.ours_blob_sha256 {
            return Err(SyncError::Integrity {
                edit_id: edit_id.to_string(),
                expected: edit.ours_blob_sha256.clone(),
                actual,
            });
        }
        Ok(bytes)
    }

    fn write_blob(&self, sha: &str, bytes: &[u8]) -> Result<(), SyncError> {
        let blobs = self.dir.join("blobs");
        std::fs::create_dir_all(&blobs)
            .map_err(|e| SyncError::Io(format!("create {}: {e}", blobs.display())))?;
        // Content-addressed write: only a matching blob is ever overwritten.
        if blobs.join(sha).exists() {
            return Ok(());
        }
        std::fs::write(blobs.join(sha), bytes)
            .map_err(|e| SyncError::Io(format!("write blob {sha}: {e}")))
    }
}

/// Result of replaying the pending edits against the server.
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    /// Local edits accepted as new server revisions (edit_id -> revision).
    pub synced: Vec<(String, String)>,
    /// Local edits that diverged (edit_id -> server sync_id to resolve).
    pub conflicted: Vec<(String, String)>,
    /// True when the server was unreachable and nothing could be pushed
    /// (the working set keeps every edit pending for the next sync).
    pub offline: bool,
}

/// Drives a working set's pending edits through the PDM sync surface.
pub struct SyncEngine<'a> {
    /// Contract client for the reachable-at-reconnect service.
    pub client: &'a PdmClient,
    /// The working set whose pending edits are replayed.
    pub working_set: &'a mut OfflineWorkingSet,
}

impl<'a> SyncEngine<'a> {
    /// Replay every pending local edit through `POST /api/v1/sync/push`.
    ///
    /// On a transport failure (server still unreachable) the loop stops and
    /// reports [`SyncReport::offline`], leaving all un-synced edits pending.
    /// Clean syncs mark the edit [`EditStatus::Synced`]; divergences mark it
    /// [`EditStatus::Conflicted`] with the server's pending-merge id, to be
    /// routed through [`Self::fetch_conflict`] / [`Self::resolve`].
    pub fn sync_pending(&mut self) -> Result<SyncReport, SyncError> {
        let mut report = SyncReport::default();
        let pending_ids: Vec<String> = self
            .working_set
            .pending()
            .map(|e| e.edit_id.clone())
            .collect();
        for edit_id in pending_ids {
            let edit = self.working_set.get(&edit_id)?;
            let bytes = self.working_set.ours_bytes(&edit_id)?;
            let req = SyncPushRequest {
                item_id: edit.item_id.clone(),
                base_revision: edit.base_revision.clone(),
                base_blob_sha256: edit.base_blob_sha256.clone(),
                state: edit.state.clone(),
                comment: edit.comment.clone(),
                created_by: edit.created_by.clone(),
                file: FileUpload {
                    filename: edit.filename.clone(),
                    format: edit.format.clone(),
                    content_type: edit.content_type.clone(),
                    content: {
                        use base64::Engine as _;
                        base64::engine::general_purpose::STANDARD.encode(&bytes)
                    },
                },
            };
            match self.client.sync_push(&req) {
                Ok(SyncPushResult::Synced { revision }) => {
                    let rev = revision.number.clone();
                    self.working_set.mark_synced(&edit_id, &rev)?;
                    report.synced.push((edit_id, rev));
                }
                Ok(SyncPushResult::Conflict { sync_id, .. }) => {
                    self.working_set.mark_conflicted(&edit_id, &sync_id)?;
                    report.conflicted.push((edit_id, sync_id));
                }
                Err(PdmError::Transport(_)) => {
                    report.offline = true;
                    break;
                }
                Err(e) => {
                    // A non-transport protocol/HTTP error is unexpected during
                    // a conflict-free replay; surface it as a local failure.
                    return Err(SyncError::Io(format!("sync push for {edit_id}: {e}")));
                }
            }
        }
        Ok(report)
    }

    /// Fetch the three snapshots of a pending merge (base/ours/theirs).
    pub fn fetch_conflict(&self, sync_id: &str) -> Result<crate::api::SyncConflictView, PdmError> {
        self.client.get_sync_conflict(sync_id)
    }

    /// Resolve a pending merge and return the committed revision. Callers
    /// verify the outcome by comparing `files[0].blob_sha256` with a local
    /// digest of the resolved bytes.
    pub fn resolve(
        &self,
        sync_id: &str,
        req: &SyncResolveRequest,
    ) -> Result<crate::api::Revision, PdmError> {
        self.client.resolve_conflict(sync_id, req)
    }
}

/// sha-256 hex digest (workspace-local helper).
fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("oep-sync-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn sha_hex(bytes: &[u8]) -> String {
        hex_sha256(bytes)
    }

    #[test]
    fn record_edit_round_trips_with_integrity() {
        let dir = tmpdir("roundtrip");
        let mut ws = OfflineWorkingSet::open(&dir).unwrap();
        let bytes = b"step file v1\nsecond line\n";
        let base_hash = sha_hex(b"some base revision bytes");
        let id = ws
            .record_edit(
                "item-1",
                "2",
                &base_hash,
                "part.step",
                "step",
                None,
                None,
                None,
                None,
                bytes,
            )
            .unwrap();
        assert_eq!(ws.edits().len(), 1);
        assert_eq!(ws.ours_bytes(&id).unwrap(), bytes);
        // Reopen from disk: index + blob survive.
        drop(ws);
        let ws2 = OfflineWorkingSet::open(&dir).unwrap();
        assert_eq!(ws2.edits().len(), 1);
        assert_eq!(ws2.ours_bytes(&id).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_local_blob_is_detected() {
        let dir = tmpdir("corrupt");
        let mut ws = OfflineWorkingSet::open(&dir).unwrap();
        let base_hash = sha_hex(b"base");
        let id = ws
            .record_edit(
                "item-1", "1", &base_hash, "f.step", "step", None, None, None, None, b"ok",
            )
            .unwrap();
        // Corrupt the blob on disk.
        let sha = ws.get(&id).unwrap().ours_blob_sha256.clone();
        std::fs::write(dir.join("blobs").join(&sha), b"tampered").unwrap();
        match ws.ours_bytes(&id) {
            Err(SyncError::Integrity { .. }) => {}
            other => panic!("expected Integrity error, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_report_tracks_marks() {
        let dir = tmpdir("marks");
        let mut ws = OfflineWorkingSet::open(&dir).unwrap();
        let base_hash = sha_hex(b"base2");
        let id = ws
            .record_edit(
                "item-1", "1", &base_hash, "a.step", "step", None, None, None, None, b"x",
            )
            .unwrap();
        ws.mark_synced(&id, "3").unwrap();
        assert_eq!(
            ws.get(&id).unwrap().status,
            EditStatus::Synced {
                revision: "3".to_string()
            }
        );
        assert_eq!(ws.pending().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
