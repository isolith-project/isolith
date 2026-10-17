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
    CreateItemRequest, CreateRevisionRequest, ErrorEnvelope, FileRef, Health, Item, ItemList,
    ItemSummary, Revision, RevisionList, API_VERSION, SERVICE_NAME,
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
            StoreError::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl StoreError {
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
#[derive(Debug)]
pub struct VaultStore {
    meta: VaultMeta,
    /// sha-256 hex -> file bytes.
    blobs: HashMap<String, Vec<u8>>,
    /// Optional directory for snapshot persistence (store.json + blobs/).
    persist_dir: Option<std::path::PathBuf>,
}

impl VaultStore {
    /// A fresh in-memory vault store (no persistence).
    pub fn new() -> Self {
        VaultStore {
            meta: VaultMeta {
                owner: "keelworks".to_string(),
                next_item_seq: 0,
                items: Vec::new(),
            },
            blobs: HashMap::new(),
            persist_dir: None,
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
        })
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
}

impl Default for VaultStore {
    fn default() -> Self {
        Self::new()
    }
}
