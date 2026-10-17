//! P8-M6-T02: integration adapters -- the seam the OEP shell (P7 workbench)
//! and any adopted tools use to talk to the PDM API.
//!
//! The thin layer sits between the shell/tool and the raw REST verbs of the
//! P8 API v1 contract. It owns the mapping between the shell's item identity
//! space (human, e.g. `BOX-01`) and the server's opaque item ids, turns the
//! two workbench verbs ("open from PDM", "save revisions to PDM") into typed,
//! non-panicking calls against the live service, and keeps a unit-level
//! [`AdapterInstrument`] so integration behavior is observable and assertable.
//!
//! Clean-room stance (program invariant): `PdmAdapter` is our own typed
//! projection of the P8 REST API v1 contract. No vendor PDM client
//! (ENOVIA/3DEXPERIENCE, Teamcenter, or any other) API surface, object model,
//! or semantics are copied, re-exported, or referenced anywhere in this module
//! or its tests.
//!
//! This replaces the P7-M2-T02 stub path: the adapter is constructed against
//! the live PDM base URL (the P8-M1-T01 axum `PdmService`), and the
//! acceptance tests in `tests/adapter_live_api.rs` drive it over real sockets.

use std::fmt;

use crate::api::{ItemSummary, Revision};
use crate::client::{FilePayload, PdmClient, SaveRevisionInput};

/// Unit-level instrumentation for the adapter layer. Every verb bumps the
/// matching counter and byte totals, so a test (or a future metrics sink) can
/// observe exactly what the integration layer did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterInstrument {
    /// `GET /health` probes made through the adapter.
    pub health_checks: u64,
    /// Save-revisions-to-PDM calls that successfully appended a revision.
    pub saves: u64,
    /// Items auto-created by the adapter on first save (missing-server-item).
    pub items_created: u64,
    /// Open-from-PDM calls (latest or at-revision) that returned a file.
    pub opens: u64,
    /// Catalogue queries (`GET /items`).
    pub catalogue_queries: u64,
    /// Revision-list queries (`GET /items/{id}/revisions`).
    pub revision_queries: u64,
    /// Total bytes uploaded (requested file bytes for saved revisions).
    pub bytes_uploaded: u64,
    /// Total bytes downloaded (opened file bytes).
    pub bytes_downloaded: u64,
    /// Failures of any kind routed through the adapter.
    pub errors: u64,
}

/// Typed, non-panicking adapter failures. Never a panic on any input path
/// (tolerant-failure doctrine).
#[derive(Debug)]
pub enum AdapterError {
    /// The shell named an item the adapter could not resolve and was not asked
    /// to create.
    ItemNotFound(String),
    /// The underlying PDM client/contract failed (mapped verbatim).
    Pdm(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::ItemNotFound(item) => {
                write!(f, "PDM item not found in the vault: {item}")
            }
            AdapterError::Pdm(m) => write!(f, "PDM adapter error: {m}"),
        }
    }
}

impl std::error::Error for AdapterError {}

impl From<crate::client::PdmError> for AdapterError {
    fn from(e: crate::client::PdmError) -> Self {
        AdapterError::Pdm(e.to_string())
    }
}

/// One file opened from the PDM vault through the adapter (latest or at a
/// specific revision). This is what the workbench routes by `format` into the
/// importer hub (the geometry half lives on the oep-import/oep-geo lanes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedFile {
    /// The shell-facing item atom that was opened.
    pub item_id: String,
    /// Revision number that was served.
    pub rev: String,
    /// Original file name as stored in the vault.
    pub filename: String,
    /// Format tag for client-side routing (`step`, `cgr`, `3dxml`, ...).
    pub format: String,
    /// Media type from the download response.
    pub content_type: String,
    /// Content address (sha-256 hex) of the returned bytes.
    pub blob_sha256: String,
    /// The file bytes, byte-exact.
    pub bytes: Vec<u8>,
}

/// The record returned by a successful save-revisions-to-PDM through the
/// adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRevision {
    /// The shell-facing item atom the revision was appended to.
    pub item_id: String,
    /// Server-assigned revision number (string, e.g. `"1"`, `"2"`).
    pub revision: String,
    /// Lifecycle state recorded on the revision.
    pub state: String,
    /// Content address (sha-256 hex) of the stored bytes.
    pub blob_sha256: String,
}

/// A catalogue row the shell/tool browses (name, kind, latest revision).
///
/// Re-exported from the contract types so callers need only this module.
pub type CatalogueItem = ItemSummary;

impl OpenedFile {
    fn from_payload(payload: FilePayload) -> Self {
        OpenedFile {
            item_id: payload.item_id,
            rev: payload.rev,
            filename: payload.filename,
            format: payload.format,
            content_type: payload.content_type,
            blob_sha256: payload.blob_sha256,
            bytes: payload.bytes,
        }
    }
}

/// The integration adapter. Wraps the REST client bound to a live PDM service
/// with the shell-facing verb set plus unit instrumentation.
#[derive(Debug, Clone)]
pub struct PdmAdapter {
    client: PdmClient,
    instr: AdapterInstrument,
}

impl PdmAdapter {
    /// Bind the adapter to a live PDM service base URL
    /// (`http://host:port/api/v1`, or the host root -- the client anchors it).
    pub fn new(base_url: &str) -> Result<Self, AdapterError> {
        let client = PdmClient::new(base_url)?;
        Ok(PdmAdapter {
            client,
            instr: AdapterInstrument::default(),
        })
    }

    /// The adapter's unit instrumentation snapshot.
    pub fn instrument(&self) -> &AdapterInstrument {
        &self.instr
    }

    /// Probe service liveness + contract identity through the adapter.
    pub fn health(&mut self) -> Result<crate::api::Health, AdapterError> {
        self.instr.health_checks += 1;
        match self.client.health() {
            Ok(h) => Ok(h),
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// Browser the vault catalogue (`GET /api/v1/items`), newest first.
    pub fn catalogue(&mut self, prefix: Option<&str>) -> Result<Vec<CatalogueItem>, AdapterError> {
        self.instr.catalogue_queries += 1;
        match self.client.list_items(prefix) {
            Ok(list) => Ok(list.items),
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// All revisions of an item, newest first (metadata only, no bytes).
    pub fn revisions(&mut self, item: &str) -> Result<Vec<Revision>, AdapterError> {
        self.instr.revision_queries += 1;
        let id = match self.resolve(item) {
            Ok(id) => id,
            Err(e) => {
                self.instr.errors += 1;
                return Err(e);
            }
        };
        match self.client.list_revisions(&id) {
            Ok(list) => Ok(list.revisions),
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// Open-from-PDM: fetch the latest revision's file bytes for `item`.
    ///
    /// `item` may be a human item atom (resolved against the catalogue) or a
    /// raw server item id.
    pub fn open_from_pdm(&mut self, item: &str) -> Result<OpenedFile, AdapterError> {
        let id = match self.resolve(item) {
            Ok(id) => id,
            Err(e) => {
                self.instr.errors += 1;
                return Err(e);
            }
        };
        match self.client.open_latest_file(&id) {
            Ok(payload) => {
                self.instr.opens += 1;
                self.instr.bytes_downloaded += payload.bytes.len() as u64;
                let mut out = OpenedFile::from_payload(payload);
                // Present the shell-facing item atom, not the server id.
                out.item_id = item.to_string();
                Ok(out)
            }
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// Open-from-PDM at a specific revision number.
    pub fn open_from_pdm_at(&mut self, item: &str, rev: &str) -> Result<OpenedFile, AdapterError> {
        let id = match self.resolve(item) {
            Ok(id) => id,
            Err(e) => {
                self.instr.errors += 1;
                return Err(e);
            }
        };
        match self.client.open_revision_file(&id, rev) {
            Ok(payload) => {
                self.instr.opens += 1;
                self.instr.bytes_downloaded += payload.bytes.len() as u64;
                let mut out = OpenedFile::from_payload(payload);
                out.item_id = item.to_string();
                Ok(out)
            }
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// Save-revisions-to-PDM: publish `bytes` as `item`'s next revision.
    ///
    /// If the item does not exist in the vault yet it is created (kind
    /// `part`) -- the shell publishes a new part without a pre-created item.
    /// `summary` becomes the revision change comment.
    pub fn save_revisions_to_pdm(
        &mut self,
        item: &str,
        filename: &str,
        format: &str,
        bytes: Vec<u8>,
        summary: &str,
    ) -> Result<SavedRevision, AdapterError> {
        let id = match self.resolve(item) {
            Ok(id) => id,
            Err(AdapterError::ItemNotFound(_)) => self.ensure_item(item)?,
            Err(e) => return Err(e),
        };
        let input = SaveRevisionInput {
            filename: filename.to_string(),
            format: format.to_string(),
            content_type: Some("application/octet-stream".to_string()),
            bytes,
            state: Some("working".to_string()),
            comment: Some(summary.to_string()),
            created_by: Some("oep-adapter".to_string()),
        };
        match self.client.save_revision(&id, input) {
            Ok(rev) => {
                self.instr.saves += 1;
                self.instr.bytes_uploaded += rev.files.iter().map(|f| f.size).sum::<u64>();
                Ok(SavedRevision {
                    item_id: item.to_string(),
                    revision: rev.number,
                    state: rev.state,
                    blob_sha256: rev
                        .files
                        .first()
                        .map(|f| f.blob_sha256.clone())
                        .unwrap_or_default(),
                })
            }
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }

    /// Resolve a shell item atom to a server item id.
    ///
    /// Resolution order: if the atom is itself a known server id use it
    /// verbatim; otherwise look it up as an item name in the catalogue.
    fn resolve(&mut self, item: &str) -> Result<String, AdapterError> {
        if item.trim().is_empty() {
            return Err(AdapterError::ItemNotFound(item.to_string()));
        }
        let list = self.client.list_items(Some("")).map_err(|e| {
            self.instr.errors += 1;
            AdapterError::from(e)
        })?;
        // Direct server-id hit first.
        if let Some(r) = list.items.iter().find(|i| i.id == item) {
            return Ok(r.id.clone());
        }
        // Name hit.
        match list.items.iter().find(|i| i.name == item) {
            Some(r) => Ok(r.id.clone()),
            None => Err(AdapterError::ItemNotFound(item.to_string())),
        }
    }

    /// Create the item the shell named (auto-create on first save).
    fn ensure_item(&mut self, item: &str) -> Result<String, AdapterError> {
        let req = crate::api::CreateItemRequest {
            name: item.to_string(),
            kind: "part".to_string(),
            description: Some("created by the OEP integration adapter".to_string()),
            owner: Some("oep-adapter".to_string()),
            context: None,
        };
        match self.client.create_item(&req) {
            Ok(created) => {
                self.instr.items_created += 1;
                Ok(created.id)
            }
            Err(e) => {
                self.instr.errors += 1;
                Err(e.into())
            }
        }
    }
}
