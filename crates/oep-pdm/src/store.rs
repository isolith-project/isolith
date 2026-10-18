//! In-memory vault store implementing the P8-M1-T02 vault object model subset:
//! Item -> Revision -> File, with content-addressed blob storage (each file's
//! bytes stored once under their sha-256).
//!
//! This is the data layer of the stub. It is deliberately small and honest
//! about its limits: no RBAC/audit/permissions (P8-M3), no revision state
//! machine beyond accepting the contract's four states (P8-M2), no relationship
//! graph (P8-M2-T03). When P8-M6-T01 comes online those concerns live there;
//! the API surface this store serves does not change.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::api::{
    ConflictSide, CreateItemRequest, CreateRevisionRequest, ErrorEnvelope, FileRef, Health, Item,
    ItemList, ItemSummary, Revision, RevisionList, SyncConflictView, SyncPushRequest,
    SyncPushResult, SyncResolveRequest, API_VERSION, SERVICE_NAME,
};

/// Domain-level store failure, mapped to the contract error envelope by the
/// server layer.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreError {
    /// An item with the same name already exists (409).
    DuplicateName(String),
    /// An item or revision does not exist (404).
    NotFound(&'static str, String),
    /// The item has no revisions yet, so no file can be opened (404).
    NoFile,
    /// The requested revision state is not one of the contract's four (400).
    InvalidState(String),
    /// The request violated a contract validation rule (400).
    BadRequest(String),
    /// A working-set lock could not be acquired (409, P8-M6-T03 gate).
    LockConflict {
        /// Who currently holds the live lock.
        holder: String,
        /// When the conflicting lease expires (ms since epoch).
        expires: u64,
    },
    /// A lock token check failed: not held / wrong / expired (409).
    LockToken(String),
    /// A defect or store-level failure (500).
    Internal(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::DuplicateName(n) => write!(f, "duplicate item name: {n}"),
            StoreError::NotFound(kind, id) => write!(f, "{kind} not found: {id}"),
            StoreError::NoFile => write!(f, "item has no file (no revisions)"),
            StoreError::InvalidState(s) => write!(f, "invalid revision state: {s}"),
            StoreError::BadRequest(m) => write!(f, "bad request: {m}"),
            StoreError::LockConflict { holder, .. } => {
                write!(f, "lock conflict: already held by {holder}")
            }
            StoreError::LockToken(m) => write!(f, "lock token rejected: {m}"),
            StoreError::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl StoreError {
    /// Map the working-set lock error to a gate-surface store error.
    pub fn from_lock(e: crate::working_set::LockError) -> StoreError {
        use crate::working_set::LockError as LE;
        match e {
            LE::ConflictingLock { holder, expires } => StoreError::LockConflict { holder, expires },
            LE::NotHeld => StoreError::LockToken("lock not held by this token".into()),
            LE::TokenMismatch => StoreError::LockToken("wrong or stale token".into()),
        }
    }

    /// Map a publish-gate refusal as a store error.
    pub fn from_publish(e: crate::working_set::PublishError) -> StoreError {
        use crate::working_set::PublishError as PE;
        match e {
            PE::NotHeld { object } => {
                StoreError::LockToken(format!("no live lock on {object}; acquire first"))
            }
            PE::LockingExpired { object } => {
                StoreError::LockToken(format!("lock on {object} expired or stolen; re-acquire"))
            }
        }
    }

    /// Map a state-machine transition refusal to a contract error.
    pub fn from_transition(
        from_wire: &str,
        to_wire: &str,
        e: crate::state::TransitionError,
    ) -> StoreError {
        use crate::state::TransitionError as TE;
        let detail = match e {
            TE::IllegalTransition { from, to } => {
                format!("illegal lifecycle hop: {from} -> {to}")
            }
            TE::Terminal(s) => format!("terminal state {s}: no further transitions"),
            TE::MaturityNotFinal { from, to, .. } => {
                format!("cannot {from} -> {to}: maturity must be final to release")
            }
            TE::Immutable { op, state } => {
                format!("cannot mutate {op} of {state}: revision frozen")
            }
        };
        StoreError::InvalidState(format!(
            "transition {from_wire} -> {to_wire} refused: {detail}"
        ))
    }

    /// Map this failure to the HTTP status + contract error envelope.
    pub fn envelope(&self) -> (u16, ErrorEnvelope) {
        use crate::api::error_code as ec;
        match self {
            StoreError::NotFound(what, id) => (
                404,
                ErrorEnvelope::new(ec::NOT_FOUND, format!("{what} not found: {id}")),
            ),
            StoreError::DuplicateName(name) => (
                409,
                ErrorEnvelope::new(
                    ec::DUPLICATE_NAME,
                    format!("an item named '{name}' already exists"),
                ),
            ),
            StoreError::NoFile => (
                404,
                ErrorEnvelope::new(ec::NO_FILE, "item has no file to open (no revisions)"),
            ),
            StoreError::InvalidState(state) => (
                400,
                ErrorEnvelope::new(
                    ec::INVALID_STATE,
                    format!("invalid revision state: {state}"),
                ),
            ),
            StoreError::BadRequest(msg) => (400, ErrorEnvelope::new(ec::BAD_REQUEST, msg.clone())),
            StoreError::LockConflict { holder, expires } => (
                409,
                ErrorEnvelope::new(
                    ec::LOCK_CONFLICT,
                    format!("lock already held by {holder} (expires {expires}ms)"),
                ),
            ),
            StoreError::LockToken(msg) => (
                409,
                ErrorEnvelope::new(ec::LOCK_TOKEN, format!("lock token rejected: {msg}")),
            ),
            StoreError::Internal(msg) => (500, ErrorEnvelope::new(ec::INTERNAL, msg.clone())),
        }
    }
}

/// Persistable part of the store (blobs live separately, content-addressed).
#[derive(Serialize, Deserialize, Debug, Clone)]
struct VaultMeta {
    owner: String,
    next_item_seq: u64,
    items: Vec<StoredItem>,
    /// P8-M5-T02: pending sync merges awaiting resolution. Persisted so an
    /// unresolved conflict survives a restart (the three snapshots are
    /// content-addressed; base/theirs revisions stay in the vault).
    pending_conflicts: Vec<SyncConflictRecord>,
}

/// A recorded divergent sync push (P8-M5-T02 pending merge). `ours` is the
/// offline edit's file (its blob lives in the content-addressed store); base
/// and theirs are existing vault revisions referenced by number + hash.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SyncConflictRecord {
    sync_id: String,
    item_id: String,
    base_revision: String,
    base_blob_sha256: String,
    ours: FileRef,
    theirs_revision: String,
    theirs_blob_sha256: String,
    created_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StoredItem {
    item: Item,
    next_rev: u64,
    revisions: Vec<Revision>,
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// RFC 3339 UTC timestamp from unix epoch seconds (stdlib-only, deterministic).
fn utc_rfc3339(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let day_secs = epoch_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        day_secs / 3600,
        (day_secs % 3600) / 60,
        day_secs % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm (public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The vault store: items + revisions + content-addressed blobs.
///
/// P8-M6-T03 adds the real lifecycle layers behind the same mutex:
/// * [`LockManager`](crate::working_set::LockManager) -- the single-writer
///   serialization point for publish (lock / edit / publish protocol);
/// * a [`crate::audit::AuditLog`] -- append-only, tamper-evident record of
///   reads / changes / releases, enabled when a persist dir is configured.
#[derive(Debug)]
pub struct VaultStore {
    meta: VaultMeta,
    /// sha-256 hex -> file bytes.
    blobs: HashMap<String, Vec<u8>>,
    /// Optional directory for snapshot persistence (store.json + blobs/).
    persist_dir: Option<std::path::PathBuf>,
    /// Working-set lock manager (P8-M1-T03); always present.
    locks: crate::working_set::LockManager,
    /// Tamper-evident audit log (P8-M3-T03); enabled when configured.
    audit: Option<crate::audit::AuditLog>,
}

impl VaultStore {
    /// A fresh in-memory vault store (no persistence).
    pub fn new() -> Self {
        VaultStore {
            meta: VaultMeta {
                owner: "keelworks".to_string(),
                next_item_seq: 0,
                items: Vec::new(),
                pending_conflicts: Vec::new(),
            },
            blobs: HashMap::new(),
            persist_dir: None,
            locks: crate::working_set::LockManager::new(),
            audit: None,
        }
    }

    /// Point the store at a directory for snapshot persistence. Existing
    /// content (if any) is loaded.
    pub fn set_persist_dir(&mut self, dir: &Path) -> Result<(), StoreError> {
        if dir.join("store.json").exists() {
            *self = VaultStore::load(dir)?;
            self.persist_dir = Some(dir.to_path_buf());
        } else {
            self.persist_dir = Some(dir.to_path_buf());
            self.save()?;
        }
        Ok(())
    }

    /// Load a persisted snapshot (metadata + any blobs present on disk).
    pub fn load(dir: &Path) -> Result<Self, StoreError> {
        let meta_path = dir.join("store.json");
        let text = std::fs::read_to_string(&meta_path).map_err(|e| {
            StoreError::Internal(format!("cannot read {}: {e}", meta_path.display()))
        })?;
        let meta: VaultMeta = serde_json::from_str(&text)
            .map_err(|e| StoreError::Internal(format!("corrupt {}: {e}", meta_path.display())))?;
        let mut blobs = HashMap::new();
        let blobs_dir = dir.join("blobs");
        if blobs_dir.is_dir() {
            for entry in std::fs::read_dir(&blobs_dir).map_err(|e| {
                StoreError::Internal(format!("cannot list {}: {e}", blobs_dir.display()))
            })? {
                let entry = entry.map_err(|e| StoreError::Internal(e.to_string()))?;
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.len() == 64 {
                    if let Ok(bytes) = std::fs::read(entry.path()) {
                        blobs.insert(fname, bytes);
                    }
                }
            }
        }
        Ok(VaultStore {
            meta,
            blobs,
            persist_dir: Some(dir.to_path_buf()),
            locks: crate::working_set::LockManager::new(),
            audit: None,
        })
    }

    /// Enable the append-only, tamper-evident audit log (P8-M3-T03) under
    /// `<persist_dir>/audit`. Refuses (but does not fail hard) when the store
    /// has no persist dir, since the audit log is a file-backed store.
    pub fn enable_audit(&mut self) -> Result<(), StoreError> {
        let Some(dir) = &self.persist_dir else {
            return Err(StoreError::Internal(
                "audit log requires a persist dir (set_persist_dir first)".into(),
            ));
        };
        let audit_root = dir.join("audit");
        let log = crate::audit::AuditLog::open(&audit_root).map_err(|e| {
            StoreError::Internal(format!(
                "cannot open audit log at {}: {e}",
                audit_root.display()
            ))
        })?;
        self.audit = Some(log);
        Ok(())
    }

    /// Record an audit event (best-effort: audited stores append; stores
    /// without an enabled log are not audible). Never fails the caller.
    pub(crate) fn record_audit(&mut self, actor: &str, action: crate::audit::Action, target: &str) {
        if let Some(log) = &mut self.audit {
            let _ = log.append(actor, action, target, None);
        }
    }

    /// The current audit log view (P8-M6-T03 surface).
    pub fn audit_view(&self) -> Result<crate::api::AuditView, StoreError> {
        match &self.audit {
            Some(log) => Ok(crate::api::AuditView {
                entries: log.entries().to_vec(),
                count: log.len(),
                append_only: log.append_only(),
            }),
            None => Err(StoreError::Internal(
                "audit log not enabled (start with --persist-dir + enable_audit)".into(),
            )),
        }
    }

    /// Re-derive the full hash chain from the on-disk audit log and confirm it
    /// is intact (P8-M3-T03 guarantee, exposed over the gate surface).
    pub fn audit_verify(&self) -> Result<crate::api::AuditVerifyReply, StoreError> {
        match &self.audit {
            Some(log) => match log.verify() {
                Ok(entries) => Ok(crate::api::AuditVerifyReply {
                    ok: true,
                    count: entries.len(),
                    head_hash: log.head_hash().to_string(),
                    detail: None,
                }),
                Err(e) => Ok(crate::api::AuditVerifyReply {
                    ok: false,
                    count: 0,
                    head_hash: String::new(),
                    detail: Some(e.to_string()),
                }),
            },
            None => Err(StoreError::Internal(
                "audit log not enabled (start with --persist-dir + enable_audit)".into(),
            )),
        }
    }

    /// Acquire a working-set lock on an item (P8-M6-T03 gate surface).
    pub fn acquire_lock(
        &mut self,
        item_id: &str,
        holder: &str,
    ) -> Result<crate::api::LockView, StoreError> {
        self.find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let token = self
            .locks
            .acquire(&item_id.to_string(), &holder.to_string())
            .map_err(StoreError::from_lock)?;
        let expires = self.locks.expires_at(&item_id.to_string()).unwrap_or(0);
        self.record_audit(
            holder,
            crate::audit::Action::Change,
            &format!("{item_id}:lock:acquire"),
        );
        Ok(crate::api::LockView {
            object: item_id.to_string(),
            holder: holder.to_string(),
            token: token.wire().to_string(),
            expires_ms: expires,
        })
    }

    /// Release a working-set lock previously granted by its token.
    pub fn release_lock(
        &mut self,
        item_id: &str,
        token: &str,
    ) -> Result<crate::api::LockView, StoreError> {
        let parsed = crate::working_set::LockToken::from_wire(token)
            .ok_or_else(|| StoreError::from_lock(crate::working_set::LockError::TokenMismatch))?;
        self.locks
            .release(&item_id.to_string(), &parsed)
            .map_err(StoreError::from_lock)?;
        let holder = crate::api::LockView {
            object: item_id.to_string(),
            holder: "unknown".to_string(),
            token: token.to_string(),
            expires_ms: 0,
        };
        self.record_audit(
            "pdm",
            crate::audit::Action::Change,
            &format!("{item_id}:lock:release"),
        );
        Ok(holder)
    }

    /// Apply a lifecycle transition to a revision. The transition is
    /// lock-gated: only the live lock token holder may publish a release (or
    /// any other hop), which is the working-set protocol's serialization point.
    pub fn transition(
        &mut self,
        item_id: &str,
        rev: &str,
        to: &str,
        token: &str,
    ) -> Result<crate::api::Revision, StoreError> {
        let target = crate::state::RevisionState::from_wire(to)
            .ok_or_else(|| StoreError::InvalidState(to.to_string()))?;
        // The lock is the publish gate (P8-M1-T03): a stale / missing token
        // means the caller is not the current writer and may not transition.
        let parsed = crate::working_set::LockToken::from_wire(token)
            .ok_or_else(|| StoreError::from_lock(crate::working_set::LockError::TokenMismatch))?;
        self.locks
            .authorize_publish(&item_id.to_string(), &parsed)
            .map_err(StoreError::from_publish)?;

        // Load the revision and run the P8-M2-T01 state machine on it.
        let stored = self
            .find_item_mut(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let revision = stored
            .revisions
            .iter_mut()
            .find(|r| r.number == rev)
            .ok_or_else(|| StoreError::NotFound("revision", format!("{item_id}/{rev}")))?;
        let from = crate::state::RevisionState::from_wire(&revision.state)
            .unwrap_or(crate::state::RevisionState::Working);
        let mut lc = crate::state::Lifecycle::from_parts(
            from,
            revision
                .maturity
                .as_deref()
                .and_then(crate::state::Maturity::from_wire),
        );
        // Releasing requires final maturity (the machine's gate).
        if target == crate::state::RevisionState::Released && !lc.is_released() {
            let _ = lc.set_maturity(crate::state::Maturity::Final);
        }
        lc.transition_to(target)
            .map_err(|e| StoreError::from_transition(&revision.state, to, e))?;
        revision.state = lc.state().wire().to_string();
        revision.maturity = Some(
            lc.maturity()
                .unwrap_or(crate::state::Maturity::Final)
                .wire()
                .to_string(),
        );
        let out = revision.clone();
        let action = if lc.is_released() {
            crate::audit::Action::Release
        } else {
            crate::audit::Action::Change
        };
        self.record_audit("pdm", action, &format!("{item_id}/rev:{rev}:{to}"));
        self.save()?;
        Ok(out)
    }

    /// Persist the metadata snapshot and every blob (content-addressed) if a
    /// persist directory is configured.
    pub fn save(&self) -> Result<(), StoreError> {
        let Some(dir) = &self.persist_dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir).map_err(|e| StoreError::Internal(e.to_string()))?;
        let blobs_dir = dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir).map_err(|e| StoreError::Internal(e.to_string()))?;
        let text = serde_json::to_string_pretty(&self.meta)
            .map_err(|e| StoreError::Internal(e.to_string()))?;
        std::fs::write(dir.join("store.json"), text)
            .map_err(|e| StoreError::Internal(e.to_string()))?;
        for (sha, bytes) in &self.blobs {
            std::fs::write(blobs_dir.join(sha), bytes)
                .map_err(|e| StoreError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    fn now() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        utc_rfc3339(secs)
    }

    fn find_item_mut(&mut self, id: &str) -> Option<&mut StoredItem> {
        self.meta.items.iter_mut().find(|s| s.item.id == id)
    }

    fn find_item(&self, id: &str) -> Option<&StoredItem> {
        self.meta.items.iter().find(|s| s.item.id == id)
    }

    /// The health payload reported at `GET /api/v1/health`.
    pub fn health(&self) -> Health {
        Health {
            status: "ok".to_string(),
            service: SERVICE_NAME.to_string(),
            version: API_VERSION.to_string(),
        }
    }

    /// Create an item; rejects duplicate names and unknown kinds.
    pub fn create_item(&mut self, req: &CreateItemRequest) -> Result<Item, StoreError> {
        if req.name.trim().is_empty() {
            return Err(StoreError::BadRequest("name must not be empty".into()));
        }
        if !crate::api::ALLOWED_KINDS.contains(&req.kind.as_str()) {
            return Err(StoreError::BadRequest(format!(
                "kind must be one of {:?}",
                crate::api::ALLOWED_KINDS
            )));
        }
        if self.meta.items.iter().any(|s| s.item.name == req.name) {
            return Err(StoreError::DuplicateName(req.name.clone()));
        }

        let seq = self.meta.next_item_seq;
        self.meta.next_item_seq += 1;
        let tag = &hex_sha256(&format!("{}-{}", req.name, seq).into_bytes())[..12];
        let id = format!("item-{seq:x}-{tag}");
        let item = Item {
            id: id.clone(),
            name: req.name.clone(),
            kind: req.kind.clone(),
            description: req.description.clone(),
            owner: req.owner.clone(),
            context: req.context.clone(),
            created_at: Self::now(),
            latest_revision: None,
            revision_count: 0,
        };
        self.meta.items.push(StoredItem {
            item: item.clone(),
            next_rev: 1,
            revisions: Vec::new(),
        });
        self.record_audit(
            req.owner.as_deref().unwrap_or("pdm"),
            crate::audit::Action::Change,
            &format!("item:{id}:create"),
        );
        self.save()?;
        Ok(item)
    }

    /// Catalogue list, newest first, with prefix filter and pagination.
    pub fn list_items(&self, prefix: &str, limit: usize, offset: usize) -> ItemList {
        let mut all: Vec<&Item> = self.meta.items.iter().rev().map(|s| &s.item).collect();
        if !prefix.is_empty() {
            all.retain(|i| i.name.starts_with(prefix));
        }
        let total = all.len();
        let page = all
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|i| ItemSummary {
                id: i.id.clone(),
                name: i.name.clone(),
                kind: i.kind.clone(),
                latest_revision: i.latest_revision.clone(),
                revision_count: i.revision_count,
            })
            .collect();
        ItemList { items: page, total }
    }

    /// Full item metadata (includes latest revision number).
    pub fn get_item(&self, id: &str) -> Result<Item, StoreError> {
        self.find_item(id)
            .map(|s| s.item.clone())
            .ok_or_else(|| StoreError::NotFound("item", id.to_string()))
    }

    /// All revisions of an item, newest first.
    pub fn list_revisions(&self, item_id: &str) -> Result<RevisionList, StoreError> {
        let stored = self
            .find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let mut revisions = stored.revisions.clone();
        revisions.reverse(); // newest first
        Ok(RevisionList {
            item_id: item_id.to_string(),
            latest: stored.item.latest_revision.clone(),
            revisions,
        })
    }

    /// Metadata for one revision.
    pub fn get_revision(&self, item_id: &str, rev: &str) -> Result<Revision, StoreError> {
        let stored = self
            .find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        stored
            .revisions
            .iter()
            .find(|r| r.number == rev)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("revision", format!("{item_id}/{rev}")))
    }

    /// Append the next revision of an item, storing its file content-addressed.
    pub fn create_revision(
        &mut self,
        item_id: &str,
        req: &CreateRevisionRequest,
    ) -> Result<Revision, StoreError> {
        let state = req.state.clone().unwrap_or_else(|| "working".to_string());
        if !crate::api::ALLOWED_STATES.contains(&state.as_str()) {
            return Err(StoreError::InvalidState(state));
        }
        if req.file.filename.trim().is_empty() || req.file.format.trim().is_empty() {
            return Err(StoreError::BadRequest(
                "file.filename and file.format must not be empty".into(),
            ));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(req.file.content.as_bytes())
            .map_err(|e| {
                StoreError::BadRequest(format!("file.content is not valid base64: {e}"))
            })?;
        let size = bytes.len() as u64;
        let sha = hex_sha256(&bytes);
        self.blobs.entry(sha.clone()).or_insert(bytes);

        let stored = self
            .find_item_mut(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let number = stored.next_rev.to_string();
        stored.next_rev += 1;
        let file = FileRef {
            id: format!("file-{item_id}-{number}"),
            filename: req.file.filename.clone(),
            format: req.file.format.clone(),
            content_type: req
                .file
                .content_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            size,
            blob_sha256: sha,
        };
        let revision = Revision {
            number: number.clone(),
            state,
            maturity: None,
            comment: req.comment.clone(),
            created_at: Self::now(),
            created_by: req.created_by.clone().unwrap_or_else(|| "pdm".to_string()),
            files: vec![file],
        };
        stored.revisions.push(revision.clone());
        stored.item.latest_revision = Some(number);
        stored.item.revision_count += 1;
        self.record_audit(
            revision.created_by.as_str(),
            if revision.state == "released" {
                crate::audit::Action::Release
            } else {
                crate::audit::Action::Change
            },
            &format!("{item_id}/rev:{}", revision.number),
        );
        self.save()?;
        Ok(revision)
    }

    /// Latest-revision file payload: (item_id, rev, FileRef clone, bytes).
    pub fn open_latest(&self, item_id: &str) -> Result<(String, FileRef, Vec<u8>), StoreError> {
        let stored = self
            .find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let Some(latest) = &stored.item.latest_revision else {
            return Err(StoreError::NoFile);
        };
        let rev = stored
            .revisions
            .iter()
            .find(|r| &r.number == latest)
            .ok_or_else(|| StoreError::Internal("latest revision missing from store".into()))?;
        self.file_payload(rev)
            .ok_or_else(|| StoreError::Internal("blob missing from store".into()))
    }

    /// Specific-revision file payload.
    pub fn open_revision(
        &self,
        item_id: &str,
        rev: &str,
    ) -> Result<(String, FileRef, Vec<u8>), StoreError> {
        let stored = self
            .find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let revision = stored
            .revisions
            .iter()
            .find(|r| r.number == rev)
            .ok_or_else(|| StoreError::NotFound("revision", format!("{item_id}/{rev}")))?;
        self.file_payload(revision)
            .ok_or_else(|| StoreError::Internal("blob missing from store".into()))
    }

    fn file_payload(&self, revision: &Revision) -> Option<(String, FileRef, Vec<u8>)> {
        let file = revision.files.first()?;
        let bytes = self.blobs.get(&file.blob_sha256)?.clone();
        Some((revision.number.clone(), file.clone(), bytes))
    }

    /// The file metadata attached to one revision (its single file).
    fn revision_file(&self, item_id: &str, rev: &str) -> Result<FileRef, StoreError> {
        let stored = self
            .find_item(item_id)
            .ok_or_else(|| StoreError::NotFound("item", item_id.to_string()))?;
        let revision = stored
            .revisions
            .iter()
            .find(|r| r.number == rev)
            .ok_or_else(|| StoreError::NotFound("revision", format!("{item_id}/{rev}")))?;
        revision
            .files
            .first()
            .cloned()
            .ok_or_else(|| StoreError::Internal("revision has no file".into()))
    }

    /// Bytes of one vault revision's file.
    fn revision_file_bytes(&self, item_id: &str, rev: &str) -> Result<Vec<u8>, StoreError> {
        let file = self.revision_file(item_id, rev)?;
        self.blobs
            .get(&file.blob_sha256)
            .cloned()
            .ok_or_else(|| StoreError::Internal("blob missing from store".into()))
    }

    fn conflict_record(&self, sync_id: &str) -> Result<&SyncConflictRecord, StoreError> {
        self.meta
            .pending_conflicts
            .iter()
            .find(|c| c.sync_id == sync_id)
            .ok_or_else(|| StoreError::NotFound("sync conflict", sync_id.to_string()))
    }
}

impl VaultStore {
    /// P8-M5-T02: push one offline edit with its base identity.
    ///
    /// Content-true divergence check: the edit is accepted as the item's next
    /// revision only when the current head equals the stated `base_revision`
    /// *and* `base_blob_sha256`. On match a revision is created from `ours`
    /// (201-family [`SyncPushResult::Synced`]). On mismatch a pending merge is
    /// recorded -- `ours` stored content-addressed -- and
    /// [`SyncPushResult::Conflict`] is returned (no revision created), to be
    /// resolved through the conflict surface.
    pub fn sync_push(&mut self, req: &SyncPushRequest) -> Result<SyncPushResult, StoreError> {
        // The item must exist and already carry at least one revision to be a
        // base; an item with no revisions has no content to diverge from.
        self.find_item(&req.item_id)
            .ok_or_else(|| StoreError::NotFound("item", req.item_id.clone()))?;
        let (head_rev, head_file) = match self.open_latest(&req.item_id) {
            Ok((rev, file, _)) => (rev, file.blob_sha256),
            Err(_) => return Err(StoreError::NoFile),
        };

        if head_rev == req.base_revision && head_file == req.base_blob_sha256 {
            // Clean sync: ours becomes the next revision.
            let rev = self.create_revision(
                &req.item_id,
                &CreateRevisionRequest {
                    state: req.state.clone(),
                    comment: req.comment.clone(),
                    created_by: req.created_by.clone(),
                    file: req.file.clone(),
                },
            )?;
            return Ok(SyncPushResult::Synced { revision: rev });
        }

        // Divergent: store ours content-addressed, record the pending merge.
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(req.file.content.as_bytes())
            .map_err(|e| StoreError::BadRequest(format!("file.content invalid base64: {e}")))?;
        let sha = hex_sha256(&bytes);
        let ours_size = bytes.len() as u64;
        self.blobs.entry(sha.clone()).or_insert(bytes);
        let ours = FileRef {
            id: format!("file-sync-{}-{}", req.item_id, req.base_revision),
            filename: req.file.filename.clone(),
            format: req.file.format.clone(),
            content_type: req
                .file
                .content_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            size: ours_size,
            blob_sha256: sha,
        };
        let sync_id = next_sync_id(&req.item_id, &req.base_revision, &ours.blob_sha256);
        self.meta.pending_conflicts.push(SyncConflictRecord {
            sync_id: sync_id.clone(),
            item_id: req.item_id.clone(),
            base_revision: req.base_revision.clone(),
            base_blob_sha256: req.base_blob_sha256.clone(),
            ours,
            theirs_revision: head_rev.clone(),
            theirs_blob_sha256: head_file.clone(),
            created_at: Self::now(),
        });
        self.save()?;
        Ok(SyncPushResult::Conflict {
            sync_id,
            base_revision: req.base_revision.clone(),
            base_blob_sha256: req.base_blob_sha256.clone(),
            theirs_revision: head_rev,
            theirs_blob_sha256: head_file,
        })
    }

    /// The three immutable snapshots of a pending merge for the resolution UI.
    pub fn get_sync_conflict(&self, sync_id: &str) -> Result<SyncConflictView, StoreError> {
        let record = self.conflict_record(sync_id)?;
        let base = self.revision_file(&record.item_id, &record.base_revision)?;
        let theirs = self.revision_file(&record.item_id, &record.theirs_revision)?;
        Ok(SyncConflictView {
            sync_id: record.sync_id.clone(),
            item_id: record.item_id.clone(),
            base: ConflictSide {
                revision: Some(record.base_revision.clone()),
                filename: base.filename,
                format: base.format,
                content_type: base.content_type,
                size: base.size,
                blob_sha256: base.blob_sha256,
            },
            ours: ConflictSide {
                revision: None,
                filename: record.ours.filename.clone(),
                format: record.ours.format.clone(),
                content_type: record.ours.content_type.clone(),
                size: record.ours.size,
                blob_sha256: record.ours.blob_sha256.clone(),
            },
            theirs: ConflictSide {
                revision: Some(record.theirs_revision.clone()),
                filename: theirs.filename,
                format: theirs.format,
                content_type: theirs.content_type,
                size: theirs.size,
                blob_sha256: theirs.blob_sha256,
            },
        })
    }

    /// Raw bytes of one side of a pending merge, for byte-exact verification.
    pub fn open_conflict_side(
        &self,
        sync_id: &str,
        side: &str,
    ) -> Result<(ConflictSide, Vec<u8>), StoreError> {
        let view = self.get_sync_conflict(sync_id)?;
        let bytes = match side {
            "base" => {
                let rev = view
                    .base
                    .revision
                    .as_deref()
                    .ok_or_else(|| StoreError::Internal("base has no revision".into()))?;
                self.revision_file_bytes(&view.item_id, rev)?
            }
            "ours" => self
                .blobs
                .get(&view.ours.blob_sha256)
                .cloned()
                .ok_or_else(|| StoreError::Internal("ours blob missing from store".into()))?,
            "theirs" => {
                let rev = view
                    .theirs
                    .revision
                    .as_deref()
                    .ok_or_else(|| StoreError::Internal("theirs has no revision".into()))?;
                self.revision_file_bytes(&view.item_id, rev)?
            }
            other => {
                return Err(StoreError::BadRequest(format!(
                    "side must be base/ours/theirs, got {other}"
                )))
            }
        };
        let meta = match side {
            "base" => view.base,
            "ours" => view.ours,
            "theirs" => view.theirs,
            _ => unreachable!(),
        };
        Ok((meta, bytes))
    }

    /// P8-M5-T02: resolve a pending merge by committing a new revision.
    ///
    /// `ours`/`theirs` reuse the recorded snapshot content (content-verified);
    /// `merged` uses the client-provided document. The committed revision's
    /// blob hash is the hash of the resolved bytes -- clients verify the
    /// outcome by comparing the returned `files[0].blob_sha256` against a
    /// locally computed digest of the same bytes.
    pub fn resolve_conflict(
        &mut self,
        sync_id: &str,
        req: &SyncResolveRequest,
    ) -> Result<Revision, StoreError> {
        let record = self.conflict_record(sync_id)?.clone();
        let (filename, format, content_type, bytes) = match req.choice.as_str() {
            "ours" => {
                let b = self
                    .blobs
                    .get(&record.ours.blob_sha256)
                    .cloned()
                    .ok_or_else(|| StoreError::Internal("ours blob missing from store".into()))?;
                (
                    record.ours.filename.clone(),
                    record.ours.format.clone(),
                    None,
                    b,
                )
            }
            "theirs" => {
                let meta = self.revision_file(&record.item_id, &record.theirs_revision)?;
                let b = self.revision_file_bytes(&record.item_id, &record.theirs_revision)?;
                (
                    meta.filename.clone(),
                    meta.format.clone(),
                    Some(meta.content_type),
                    b,
                )
            }
            "merged" => {
                let upload = req.merged.as_ref().ok_or_else(|| {
                    StoreError::BadRequest("merged resolution requires a file".into())
                })?;
                let b = base64::engine::general_purpose::STANDARD
                    .decode(upload.content.as_bytes())
                    .map_err(|e| {
                        StoreError::BadRequest(format!("merged.content invalid base64: {e}"))
                    })?;
                (
                    upload.filename.clone(),
                    upload.format.clone(),
                    upload.content_type.clone(),
                    b,
                )
            }
            other => {
                return Err(StoreError::BadRequest(format!(
                    "choice must be one of ours/theirs/merged, got {other}"
                )))
            }
        };
        // Commit a new revision from the resolved bytes (content-addressed).
        let content = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let revision = self.create_revision(
            &record.item_id,
            &CreateRevisionRequest {
                state: None,
                comment: Some(format!("sync resolution of {sync_id} ({})", req.choice)),
                created_by: None,
                file: crate::api::FileUpload {
                    filename,
                    format,
                    content_type,
                    content,
                },
            },
        )?;
        self.meta.pending_conflicts.retain(|c| c.sync_id != sync_id);
        self.save()?;
        Ok(revision)
    }
}

/// Deterministic server-minted sync-conflict id (item + base + ours hash).
fn next_sync_id(item_id: &str, base_revision: &str, ours_sha: &str) -> String {
    let mut h = Sha256::new();
    h.update(format!("{item_id}|{base_revision}|{ours_sha}").as_bytes());
    format!("sync-{:x}", h.finalize())[..40].to_string()
}

impl Default for VaultStore {
    fn default() -> Self {
        Self::new()
    }
}
