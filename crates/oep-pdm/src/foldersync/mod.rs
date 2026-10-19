//! P8-M5-T01 -- Sync client: Nextcloud-style folder-sync for CAD files
//! (NAS remote <-> U1/mobile mirror).
//!
//! A working-set folder is **synced, never merged blindly**: both directions
//! run with content-addressed **hash verification** (every transferred byte
//! is re-hashed after the copy and confirmed against its recorded sha-256),
//! and when the same file changed on **both** sides since the last sync
//! point the engine **stages the conflict instead of overwriting** -- the
//! remote variant is parked in the control directory and a durable JSONL
//! ledger entry is written for the P8-M5-T02 conflict-resolution milestone.
//!
//! Layering (each module documents its own contract):
//!
//! - [`manifest`] -- a folder's content-addressed view: relative path ->
//!   [`FileEntry`] (size, sha-256, mtime). The scan never ingests the
//!   `.oep-sync` control directory, so the client's own metadata is not a
//!   payload.
//! - [`plan`] -- the pure, testable three-way reconcile. Given local, remote
//!   and baseline manifests it decides `Upload` / `Download` /
//!   `DeleteLocal` / `DeleteRemote` / `Conflict` with no IO.
//! - [`engine`] -- [`SyncEngine`]: runs a reconcile against two real
//!   directory roots, executes each action with atomic write + hash
//!   verification, stages conflicts, and persists the new baseline so the
//!   next run is incremental. Supports a read-only mode for mobile clients.
//! - [`view`] -- [`ReadOnlyView`], the mobile lens: list + verified fetch
//!   only, no mutating surface.
//!
//! The state/baseline and the conflict ledger live under
//! `<local>/.oep-sync/` (the Nextcloud-style client control dir). The
//! `.oep-sync` relative prefix is reserved for client metadata in every
//! mirror and is never treated as user content.

pub mod engine;
pub mod manifest;
pub mod plan;
pub mod view;

pub use engine::{SyncEngine, SyncError, SyncReport};
pub use manifest::{FileEntry, FolderManifest};
pub use plan::{reconcile, Action, Conflict, SyncPlan};
pub use view::ReadOnlyView;

/// The client control directory name (reserved, never a payload).
pub const CONTROL_DIR: &str = ".oep-sync";
