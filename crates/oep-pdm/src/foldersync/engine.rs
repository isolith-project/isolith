//! Folder-sync engine (P8-M5-T01): executes a reconcile between two real
//! directory roots with content-addressed verification.
//!
//! # Guarantees
//!
//! - **Hash-verified transfers.** Every [`Action::Upload`] / [`Action::Download`]
//!   reads the source, computes its sha-256, writes to a temp file in the
//!   destination tree, renames it into place, then re-reads the destination
//!   and confirms it hashes to the expected content address. A mismatch is an
//!   error -- corrupted bytes never silently land in a mirror.
//! - **Conflicts are staged, never overwritten.** When the planner reports a
//!   [`Conflict`], neither side is touched; the remote variant is copied into
//!   `<control>/conflicts/<leaf>.<sha16>.conflict` and a JSONL entry is
//!   appended to `<control>/conflicts.json` for the P8-M5-T02 resolution
//!   milestone. Re-running sync does not unstage or resolve it.
//! - **Incremental baseline.** `<control>/state.json` holds the last
//!   successful snapshot; the next run diffs against it, so unchanged files
//!   are never re-transferred.
//! - **Read-only mode.** A read-only client (mobile view) can refresh its
//!   local mirror but can never write to the remote: any plan containing an
//!   upload or remote deletion is refused up front.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::foldersync::manifest::{sha256_hex, FolderManifest};
use crate::foldersync::plan::{reconcile, Action, Conflict};
use crate::foldersync::CONTROL_DIR;

/// Errors surfaced by a sync run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The engine is read-only and the plan would modify the remote.
    ReadOnly {
        /// The relative path whose upload/delete was refused.
        rel: String,
    },
    /// A transferred file's sha-256 did not match its recorded content
    /// address (corruption detected before/after the write landed).
    HashMismatch {
        /// The relative path involved.
        rel: String,
        /// Expected sha-256.
        expected: String,
        /// Actual sha-256 observed on disk.
        actual: String,
    },
    /// A manifest could not be scanned or parsed (surface `ManifestError`).
    Manifest(String),
    /// A JSON state/ledger encode or decode failure.
    Json(String),
    /// A filesystem-level failure.
    Io(io::ErrorKind),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly { rel } => write!(
                f,
                "read-only: sync would modify the remote ({rel}), refused"
            ),
            Self::HashMismatch {
                rel,
                expected,
                actual,
            } => write!(
                f,
                "content mismatch for {rel}: expected {expected}, got {actual}"
            ),
            Self::Manifest(m) => write!(f, "manifest: {m}"),
            Self::Json(e) => write!(f, "sync json: {e}"),
            Self::Io(k) => write!(f, "sync io: {k:?}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<io::Error> for SyncError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind())
    }
}

impl From<crate::foldersync::manifest::ManifestError> for SyncError {
    fn from(e: crate::foldersync::manifest::ManifestError) -> Self {
        Self::Manifest(e.to_string())
    }
}

/// The outcome of one [`SyncEngine::sync`] run.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncReport {
    /// Relative paths uploaded local -> remote.
    pub uploaded: Vec<String>,
    /// Relative paths downloaded remote -> local.
    pub downloaded: Vec<String>,
    /// Relative paths deleted from the local mirror.
    pub deleted_local: Vec<String>,
    /// Relative paths deleted from the remote.
    pub deleted_remote: Vec<String>,
    /// Divergences staged (not transferred, not overwritten).
    pub conflicts: Vec<Conflict>,
    /// Paths that were already in agreement (no action).
    pub in_sync: Vec<String>,
}

/// One durable conflict-ledger line (the P8-M5-T02 handoff seam).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecord {
    /// The conflicting relative path.
    pub rel: String,
    /// sha-256 of the local variant when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_sha256: Option<String>,
    /// sha-256 of the remote variant when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_sha256: Option<String>,
    /// Unix seconds when the conflict was staged.
    pub at_unix: u64,
    /// Path of the staged remote variant, relative to the control dir.
    pub staged_file: String,
}

/// Two real mirror roots plus persisted state. Construct via [`open`].
///
/// The engine never touches the control directory as though it were user
/// content: `.oep-sync/` under the local root holds `state.json` (baseline)
/// and `conflicts/` + `conflicts.json` (staged divergences).
pub struct SyncEngine {
    local: PathBuf,
    remote: PathBuf,
    state_path: PathBuf,
    control_dir: PathBuf,
    read_only: bool,
}

impl SyncEngine {
    /// Open an engine over a local mirror and a remote (NAS) root.
    ///
    /// Both roots are created on demand. The per-client control directory
    /// (`<local>/.oep-sync/`) is created for state and staged conflicts.
    pub fn open(
        local: impl AsRef<Path>,
        remote: impl AsRef<Path>,
        state: impl AsRef<Path>,
    ) -> Result<Self, SyncError> {
        let local = local.as_ref().to_path_buf();
        let remote = remote.as_ref().to_path_buf();
        let state_path = state.as_ref().to_path_buf();
        let control_dir = local.join(CONTROL_DIR);
        fs::create_dir_all(&local).map_err(SyncError::from)?;
        fs::create_dir_all(&remote).map_err(SyncError::from)?;
        fs::create_dir_all(&control_dir).map_err(SyncError::from)?;
        fs::create_dir_all(control_dir.join("conflicts")).map_err(SyncError::from)?;
        Ok(Self {
            local,
            remote,
            state_path,
            control_dir,
            read_only: false,
        })
    }

    /// Toggle read-only mode. When on, any plan that would modify the remote
    /// (upload or remote delete) is refused before execution; local-only
    /// refreshes (downloads) still proceed. This is the mobile/lens mode.
    pub fn set_read_only(&mut self, ro: bool) {
        self.read_only = ro;
    }

    /// True when this engine will not write to the remote.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn old_baseline(&self) -> FolderManifest {
        match fs::read(&self.state_path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| FolderManifest::empty()),
            Err(_) => FolderManifest::empty(),
        }
    }

    fn save_baseline(&self, m: &FolderManifest) -> Result<(), SyncError> {
        let bytes = serde_json::to_vec_pretty(m).map_err(|e| SyncError::Json(e.to_string()))?;
        fs::write(&self.state_path, bytes).map_err(SyncError::from)
    }

    fn append_ledger(&self, rec: &ConflictRecord) -> Result<(), SyncError> {
        let mut line = serde_json::to_vec(rec).map_err(|e| SyncError::Json(e.to_string()))?;
        line.push(b'\n');
        let ledger = self.control_dir.join("conflicts.json");
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ledger)
            .map_err(SyncError::from)?;
        f.write_all(&line).map_err(SyncError::from)
    }

    /// Run one full sync pass and persist the new baseline.
    pub fn sync(&self) -> Result<SyncReport, SyncError> {
        let local_man = FolderManifest::scan(&self.local)?;
        let remote_man = FolderManifest::scan(&self.remote)?;
        let baseline = self.old_baseline();
        let plan = reconcile(&local_man, &remote_man, &baseline);

        // Read-only gate: any remote mutation is refused up front.
        if self.read_only {
            for action in &plan.actions {
                match action {
                    Action::Upload { rel } | Action::DeleteRemote { rel } => {
                        return Err(SyncError::ReadOnly { rel: rel.clone() })
                    }
                    _ => {}
                }
            }
        }

        let mut report = SyncReport {
            in_sync: plan.in_sync.clone(),
            ..Default::default()
        };

        for action in &plan.actions {
            match action {
                Action::Upload { rel } => {
                    self.copy_with_verify(&self.local, &self.remote, rel)?;
                    report.uploaded.push(rel.clone());
                }
                Action::Download { rel } => {
                    self.copy_with_verify(&self.remote, &self.local, rel)?;
                    report.downloaded.push(rel.clone());
                }
                Action::DeleteLocal { rel } => {
                    fs::remove_file(FolderManifest::join_safe(&self.local, rel)?)
                        .map_err(SyncError::from)?;
                    report.deleted_local.push(rel.clone());
                }
                Action::DeleteRemote { rel } => {
                    fs::remove_file(FolderManifest::join_safe(&self.remote, rel)?)
                        .map_err(SyncError::from)?;
                    report.deleted_remote.push(rel.clone());
                }
            }
        }

        for conflict in &plan.conflicts {
            self.stage_conflict(conflict)?;
            report.conflicts.push(conflict.clone());
        }

        // Persist the new baseline. Re-scan both sides POST-transfer so the
        // baseline reflects what is actually on disk now (pre-transfer scans
        // would omit files that were just downloaded/uploaded). For every
        // path where both sides now agree (present+same hash, or absent on
        // both), the consensus entry becomes the new baseline. Conflict paths
        // keep their OLD baseline entry, so the next run still sees the
        // divergence -- nothing is resolved silently.
        let local_after = FolderManifest::scan(&self.local)?;
        let remote_after = FolderManifest::scan(&self.remote)?;
        let mut new_base = baseline.files().clone();
        let mut names: std::collections::BTreeSet<String> = Default::default();
        names.extend(local_after.files().keys().cloned());
        names.extend(remote_after.files().keys().cloned());
        for rel in names {
            if plan.conflicts.iter().any(|c| c.rel == rel) {
                continue; // never promote a conflicted path to "agreed"
            }
            match (local_after.get(&rel), remote_after.get(&rel)) {
                (Some(le), Some(re)) if le.sha256 == re.sha256 => {
                    new_base.insert(rel, le.clone());
                }
                (Some(_), None) | (None, Some(_)) => {
                    // One side only: a delete/change we did not promote is a
                    // conflict the plan deliberately left untouched, so keep
                    // whatever the baseline had.
                }
                (None, None) => {
                    new_base.remove(&rel);
                }
                (Some(_), Some(_)) => {
                    // Divergent content on a non-conflict path should not
                    // happen after a successful run; leave baseline untouched.
                }
            }
        }
        self.save_baseline(&FolderManifest::from_map(new_base))?;

        Ok(report)
    }

    /// Copy one file from `src_root` to `dst_root` with atomic write + hash
    /// verification on both ends.
    fn copy_with_verify(
        &self,
        src_root: &Path,
        dst_root: &Path,
        rel: &str,
    ) -> Result<(), SyncError> {
        let src = FolderManifest::join_safe(src_root, rel)?;
        let dst = FolderManifest::join_safe(dst_root, rel)?;
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(SyncError::from)?;
        }
        let bytes = fs::read(&src).map_err(SyncError::from)?;
        let expected = sha256_hex(&bytes);
        let tmp = dst.with_extension(format!("sync{}.tmp", tmp_tag()));
        fs::write(&tmp, &bytes).map_err(SyncError::from)?;
        // Re-read the temp and confirm it really hashed to the expected
        // address before it is allowed to become the file.
        let written = fs::read(&tmp).map_err(SyncError::from)?;
        let actual = sha256_hex(&written);
        if actual != expected {
            let _ = fs::remove_file(&tmp);
            return Err(SyncError::HashMismatch {
                rel: rel.to_string(),
                expected,
                actual,
            });
        }
        fs::rename(&tmp, &dst).map_err(SyncError::from)?;
        Ok(())
    }

    /// Stage one conflict: copy the remote variant into the control dir and
    /// append a durable ledger line. The live files on BOTH sides are never
    /// touched.
    fn stage_conflict(&self, conflict: &Conflict) -> Result<(), SyncError> {
        // Only a present remote variant is staged (a delete-vs-change keeps
        // only the local side, which is already in place -- nothing to add).
        let (Some(remote_sha), _rel) = (&conflict.remote_sha256, Some(conflict.rel.as_str()))
        else {
            // No remote bytes: record the ledger line with staged_file empty.
            // (Rare: remote deleted while local changed.)
            return self.append_ledger(&ConflictRecord {
                rel: conflict.rel.clone(),
                local_sha256: conflict.local_sha256.clone(),
                remote_sha256: None,
                at_unix: now_unix(),
                staged_file: String::new(),
            });
        };
        let leaf = conflict
            .rel
            .rsplit('/')
            .next()
            .unwrap_or(&conflict.rel)
            .to_string();
        let short = &remote_sha[..remote_sha.len().min(16)];
        let staged_name = format!("{leaf}.{short}.conflict");
        let staged_path = self.control_dir.join("conflicts").join(&staged_name);
        // Idempotent: if we already staged this exact remote variant, do not
        // rewrite it (keeps repeated runs stable).
        if !staged_path.exists() {
            let src = FolderManifest::join_safe(&self.remote, &conflict.rel)?;
            let bytes = fs::read(&src).map_err(SyncError::from)?;
            fs::write(&staged_path, bytes).map_err(SyncError::from)?;
        }
        self.append_ledger(&ConflictRecord {
            rel: conflict.rel.clone(),
            local_sha256: conflict.local_sha256.clone(),
            remote_sha256: Some(remote_sha.clone()),
            at_unix: now_unix(),
            staged_file: format!("conflicts/{staged_name}"),
        })
    }
}

/// A unique-ish tag for temp files (pid + counter + time).
fn tmp_tag() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        now_unix(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    )
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
