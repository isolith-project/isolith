//! Pure three-way reconcile: local vs remote vs baseline (P8-M5-T01).
//!
//! The planner answers "what must happen to make local and remote converge,
//! given a baseline snapshot taken at the last successful sync?" with no IO.
//! The rules are deliberately conservative -- **a conflict is never an
//! overwrite** and **a change on one side never silently cancels a change on
//! the other**:
//!
//! | local | remote | baseline | decision |
//! |-------|--------|----------|----------|
//! | A     | A      | A        | in sync |
//! | A     | A      | B        | in sync (converged to the same new bytes) |
//! | A     | B      | A        | remote changed -> [`Action::Download`] |
//! | B     | A      | A        | local changed  -> [`Action::Upload`] |
//! | A     | B      | C        | both changed   -> [`Conflict`] |
//! | _     | A      | _        | local deleted, remote unchanged -> [`Action::DeleteLocal`] |
//! | _     | B      | A        | local deleted, remote changed   -> [`Conflict`] |
//! | A     | _      | _        | remote deleted, local unchanged -> [`Action::DeleteRemote`] |
//! | B     | _      | A        | remote deleted, local changed   -> [`Conflict`] |
//!
//! For a path with no baseline entry at all: present on exactly one side is a
//! one-way add ([`Action::Upload`]/[`Action::Download`]); present on both
//! sides is a conflict only when the content differs, otherwise it is in
//! sync.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::foldersync::manifest::FolderManifest;

/// A single transfer or deletion the engine must perform to converge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Copy local -> remote (`rel` is the normalized relative path).
    Upload {
        /// The relative path to upload.
        rel: String,
    },
    /// Copy remote -> local (`rel` is the normalized relative path).
    Download {
        /// The relative path to download.
        rel: String,
    },
    /// Delete the file from the local mirror (remote deleted it).
    DeleteLocal {
        /// The relative path to remove locally.
        rel: String,
    },
    /// Delete the file from the remote (local deleted it).
    DeleteRemote {
        /// The relative path to remove remotely.
        rel: String,
    },
}

/// A detected divergence that the planner refuses to resolve silently.
///
/// Both sides changed the same path relative to the baseline; no transfer
/// happens. The engine stages this for the P8-M5-T02 conflict-resolution
/// milestone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// The conflicting relative path.
    pub rel: String,
    /// sha-256 of the local variant, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_sha256: Option<String>,
    /// sha-256 of the remote variant, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_sha256: Option<String>,
}

/// The full output of [`reconcile`]: actions to run plus conflicts to stage.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncPlan {
    /// Concrete actions, in relative-path order (deterministic).
    pub actions: Vec<Action>,
    /// Divergences the planner refused to overwrite.
    pub conflicts: Vec<Conflict>,
    /// Paths already in agreement that needed no action.
    pub in_sync: Vec<String>,
}

/// Three-way reconcile (pure): `local`/`remote` are the two live mirrors,
/// `baseline` is the snapshot from the last successful sync.
pub fn reconcile(
    local: &FolderManifest,
    remote: &FolderManifest,
    baseline: &FolderManifest,
) -> SyncPlan {
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(local.files().keys().cloned());
    names.extend(remote.files().keys().cloned());
    names.extend(baseline.files().keys().cloned());

    let mut plan = SyncPlan::default();
    for rel in names {
        let l = local.get(&rel);
        let r = remote.get(&rel);
        let b = baseline.get(&rel);

        match (l, r, b) {
            // Both sides agree (whether or not the baseline matches): no work.
            (Some(le), Some(re), _) if le.sha256 == re.sha256 => {
                plan.in_sync.push(rel);
            }
            // Only remote exists (local deleted or never had it).
            (None, Some(re), b) => match b {
                // Baseline has the same bytes remote still has: local deleted
                // it -> the deletion propagates to the remote mirror.
                Some(be) if be.sha256 == re.sha256 => {
                    plan.actions.push(Action::DeleteRemote { rel });
                }
                // No baseline, or baseline differs: either a remote add (no
                // baseline) or remote changed while local deleted (baseline
                // differs). With no baseline, a remote-only file is a fresh
                // add on that side -> download. With a baseline that differs,
                // it is a delete-vs-change conflict.
                None => plan.actions.push(Action::Download { rel }),
                Some(_) => plan.conflicts.push(Conflict {
                    rel: rel.clone(),
                    local_sha256: None,
                    remote_sha256: Some(re.sha256.clone()),
                }),
            },
            // Only local exists (remote deleted or never had it).
            (Some(le), None, b) => match b {
                Some(be) if be.sha256 == le.sha256 => {
                    plan.actions.push(Action::DeleteLocal { rel });
                }
                None => plan.actions.push(Action::Upload { rel }),
                Some(_) => plan.conflicts.push(Conflict {
                    rel: rel.clone(),
                    local_sha256: Some(le.sha256.clone()),
                    remote_sha256: None,
                }),
            },
            // Both sides have the file, with different content.
            (Some(le), Some(re), b) => {
                match b {
                    // Remote changed, local matches baseline -> download.
                    Some(be) if be.sha256 == le.sha256 => {
                        plan.actions.push(Action::Download { rel });
                    }
                    // Local changed, remote matches baseline -> upload.
                    Some(be) if be.sha256 == re.sha256 => {
                        plan.actions.push(Action::Upload { rel });
                    }
                    // Both different from baseline (or no baseline) -> the
                    // planner will not pick a winner.
                    _ => plan.conflicts.push(Conflict {
                        rel: rel.clone(),
                        local_sha256: Some(le.sha256.clone()),
                        remote_sha256: Some(re.sha256.clone()),
                    }),
                }
            }
            // (None, None, Some(_)): both sides deleted it -> converge on the
            // deletion, no action needed.
            (None, None, Some(_)) => {}
            (None, None, None) => {}
        }
    }
    plan
}
