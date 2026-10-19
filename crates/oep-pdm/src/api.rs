//! P8 REST API v1 contract types -- the one source of truth shared by the stub
//! server, the `PdmClient`, and (after P8-M6-T01) the real P8 service.
//!
//! The wire contract is the OpenAPI spec at `P8-M6/openapi-v1.yaml`; these
//! structs are its Rust projection, field-for-field. A client or server that
//! uses only these types is guaranteed to speak the documented contract.

use serde::{Deserialize, Serialize};

/// Contract version reported by `GET /api/v1/health` and used for handshakes.
pub const API_VERSION: &str = "1.0.0";
/// Contract root. All resources live under this prefix regardless of host/port.
pub const API_BASE: &str = "/api/v1";
/// The health probe path.
pub const HEALTH_PATH: &str = "/api/v1/health";
/// The item catalogue path (browser browse root).
pub const ITEMS_PATH: &str = "/api/v1/items";

/// Name the service identifies itself with (health payload).
pub const SERVICE_NAME: &str = "oep-pdm";

/// Item kinds accepted by the v1 contract (P8-M1-T02 vault model subset).
pub const ALLOWED_KINDS: [&str; 3] = ["part", "assembly", "document"];
/// Revision states accepted by v1 (carried verbatim; the state machine itself
/// is P8-M2-T01's work, the stub stores what it is given within these values).
pub const ALLOWED_STATES: [&str; 4] = ["working", "under_review", "released", "obsolete"];

/// The sync-push path (offline working set -> server).
pub const SYNC_PUSH_PATH: &str = "/api/v1/sync/push";
/// The sync-conflict collection/root path.
pub const SYNC_CONFLICTS_PATH: &str = "/api/v1/sync/conflicts";

/// Literal error codes used in the `{"error":{code,message}}` envelope.
// Values are part of the wire contract (see the OpenAPI spec); this module
// documents them as strings so server and client agree by name.
#[allow(missing_docs)]
pub mod error_code {
    pub const BAD_REQUEST: &str = "bad_request";
    pub const NOT_FOUND: &str = "not_found";
    pub const NO_FILE: &str = "no_file";
    pub const DUPLICATE_NAME: &str = "duplicate_name";
    pub const INVALID_STATE: &str = "invalid_state";
    pub const METHOD_NOT_ALLOWED: &str = "method_not_allowed";
    pub const UNSUPPORTED_MEDIA: &str = "unsupported_media";
    pub const INTERNAL: &str = "internal_error";
    /// P11-T04: a vault blob failed its content-hash verification on read.
    pub const INTEGRITY: &str = "integrity_error";
    // ---- P8-M6-T03 additive gate surface (never used by the v1 verbs) ----
    /// A working-set lock cannot be acquired (another principal holds it).
    pub const LOCK_CONFLICT: &str = "lock_conflict";
    /// A lock-token check failed: not held, wrong token, or lease expired.
    pub const LOCK_TOKEN: &str = "lock_token";
    /// The audit log is not available (no persist dir / audit root).
    pub const AUDIT_UNAVAILABLE: &str = "audit_unavailable";
    /// P8-M5-T02: a conflict id is unknown or already resolved.
    pub const SYNC_CONFLICT: &str = "sync_conflict";
}

// ---------------------------------------------------------------------------
// P8-M6-T03 additive gate surface (lock / transition / audit). These are NEW
// paths and types, additive to the v1 contract -- the existing verbs and their
// byte-exact behavior do not change (P8-M6/openapi-v1.yaml stays the same for
// them). The gate surface makes the real P8-M1-T03 / P8-M2-T01 / P8-M3-T03
// lifecycle layers observable and testable over the same base URL.
// ---------------------------------------------------------------------------

/// `POST /api/v1/items/{item_id}/lock` request: who wants the lock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockAcquireRequest {
    /// Principal (user / workbench / agent) acquiring the lock.
    pub holder: String,
}

/// A granted working-set lock (response payload).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockView {
    /// The locked vault object (item id).
    pub object: String,
    /// Who holds the lock.
    pub holder: String,
    /// Server-minted one-time token proving ownership (opaque).
    pub token: String,
    /// Lease expiry in unix milliseconds.
    pub expires_ms: u64,
}

/// `DELETE /api/v1/items/{item_id}/lock` request: the token to release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockReleaseRequest {
    /// The one-time token granted at acquire (or steal).
    pub token: String,
}

/// `POST /api/v1/items/{item_id}/revisions/{rev}/transition` request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionRequest {
    /// The destination lifecycle state (working/under_review/released/obsolete).
    pub to: String,
    /// The working-set lock token. Required for every hop (the lock is the
    /// publish serialization point, P8-M1-T03).
    pub token: String,
}

/// The audit view returned by `GET /api/v1/audit`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditView {
    /// All entries, oldest first (seq asc).
    pub entries: Vec<crate::audit::Entry>,
    /// Number of entries.
    pub count: usize,
    /// Storage is append-only by construction (P8-M3-T03).
    pub append_only: bool,
}

/// Result of `GET /api/v1/audit/verify`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditVerifyReply {
    /// True when the whole chain re-verifies (no tamper detected).
    pub ok: bool,
    /// Number of entries in the verified chain.
    pub count: usize,
    /// Chained hash of the last entry (head checkpoint).
    pub head_hash: String,
    /// Human detail when `ok` is false (tamper evidence).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// `GET /api/v1/health` response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Health {
    /// Liveness flag; always `"ok"` when the response is 200.
    pub status: String,
    /// Server identity, e.g. `oep-pdm`.
    pub service: String,
    /// Contract version this server speaks (must equal [`API_VERSION`]).
    pub version: String,
}

/// A file attached to a revision. Content-addressed: `blob_sha256` is the
/// hex sha-256 of the decoded file bytes (P8-M1-T02 content-addressed blobs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRef {
    /// Per-revision file id (opaque).
    pub id: String,
    /// Original file name as uploaded.
    pub filename: String,
    /// Format tag the client uses to route the file (`step`, `cgr`, ...).
    pub format: String,
    /// Stored media type (falls back to `application/octet-stream`).
    pub content_type: String,
    /// Decoded file size in bytes.
    pub size: u64,
    /// Content address: lowercase hex sha-256 of the decoded bytes.
    pub blob_sha256: String,
}

/// One revision of a vault item (metadata; file bytes served separately).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Revision {
    /// Revision number as a string (`"1"`, `"2"`, ...), assigned by the server.
    pub number: String,
    /// Lifecycle state: one of [`ALLOWED_STATES`].
    pub state: String,
    /// Optional maturity tag (P8-M2-T01 surface; the stub carries what it gets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maturity: Option<String>,
    /// Optional change comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Acting principal that created the revision.
    pub created_by: String,
    /// Files in this revision (a revision currently carries exactly one).
    pub files: Vec<FileRef>,
}

/// A vault item (the thing the browser shows in its catalogue).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    /// Opaque url-safe item id.
    pub id: String,
    /// Human item name; unique within a store.
    pub name: String,
    /// Item kind: one of [`ALLOWED_KINDS`].
    pub kind: String,
    /// Optional free-text description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional owner/principal tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Optional classification/context tag (P8-M1-T02 context surface).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Latest revision number, or `None` before the first save-revision.
    pub latest_revision: Option<String>,
    /// Number of revisions saved so far.
    pub revision_count: u64,
}

/// Lightweight item row for the catalogue list (no optional metadata).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemSummary {
    /// Opaque url-safe item id.
    pub id: String,
    /// Human item name.
    pub name: String,
    /// Item kind.
    pub kind: String,
    /// Latest revision number, or `None` before the first save-revision.
    pub latest_revision: Option<String>,
    /// Number of revisions saved so far.
    pub revision_count: u64,
}

/// `GET /api/v1/items` response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemList {
    /// Catalogue page (respects limit/offset).
    pub items: Vec<ItemSummary>,
    /// Total matching items across all pages.
    pub total: usize,
}

/// Request body for `POST /api/v1/items`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateItemRequest {
    /// Required, non-empty, store-unique item name.
    pub name: String,
    /// Required item kind: one of [`ALLOWED_KINDS`].
    pub kind: String,
    /// Optional free-text description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional owner/principal tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Optional classification/context tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl CreateItemRequest {
    /// Convenience for constructing an item from name + kind only (test
    /// ergonomics; the optional metadata stays `None`).
    pub fn default_min() -> Self {
        CreateItemRequest {
            name: String::new(),
            kind: String::new(),
            description: None,
            owner: None,
            context: None,
        }
    }
}

/// The upload half of a save-revision: file content is base64 inside JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileUpload {
    /// Required, non-empty original file name.
    pub filename: String,
    /// Required, non-empty format tag for client-side routing.
    pub format: String,
    /// Optional media type (defaults to `application/octet-stream`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// base64-encoded file bytes (standard base64 with padding).
    pub content: String,
}

/// Request body for `POST /api/v1/items/{item_id}/revisions`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateRevisionRequest {
    /// Optional lifecycle state; defaults to `working` in [`ALLOWED_STATES`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Optional change comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Optional acting principal tag (defaults to `pdm`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// The file to attach to the new revision.
    pub file: FileUpload,
}

/// `GET /api/v1/items/{item_id}/revisions` response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RevisionList {
    /// Item the revisions belong to.
    pub item_id: String,
    /// Latest revision number, or `None` before the first save-revision.
    pub latest: Option<String>,
    /// Revisions, newest first.
    pub revisions: Vec<Revision>,
}

/// A single error detail in the contract envelope
/// `{"error": {"code": ..., "message": ...}}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetail {
    /// Machine-readable error code (see [`error_code`]).
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

/// Full error envelope serialized as the response body on non-2xx.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorEnvelope {
    /// The error detail.
    pub error: ErrorDetail,
}

impl ErrorEnvelope {
    /// Build an envelope from a code + message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        ErrorEnvelope {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// P8-M5-T02: offline sync surface (sync push + conflict resolution)
// ---------------------------------------------------------------------------

/// Request body for `POST /api/v1/sync/push`.
///
/// Carries one offline-made edit plus the identity of the revision it was
/// based on (["item_id"]). The server accepts it as a clean next revision only
/// when the item's current head still equals the stated `base_revision` and
/// `base_blob_sha256`; otherwise it records a pending merge ([`SyncConflictView`]
/// via [`SyncPushResult::Conflict`]) and no revision is created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncPushRequest {
    /// The item the offline edit targets.
    pub item_id: String,
    /// The revision number the offline edit was based on ("1", "2", ...).
    pub base_revision: String,
    /// Content address of base_revision's file (content truth for divergence).
    pub base_blob_sha256: String,
    /// Optional lifecycle state for the new revision (default `working`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Optional change comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Optional acting principal tag (defaults to `pdm`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// The offline edit (ours). Content-addressed by the server.
    pub file: FileUpload,
}

/// Result of `POST /api/v1/sync/push`.
///
/// Tagged on `status`: `synced` (201) means the offline edit became the next
/// revision; `conflict` (409) means divergence was detected and a pending merge
/// was recorded at `sync_id` (no revision created).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum SyncPushResult {
    /// Clean sync: the offline edit became the item's next revision (201).
    Synced {
        /// The revision that was created from the offline edit.
        revision: Revision,
    },
    /// Divergent: the item's head moved since `base_revision` (409). No
    /// revision was created; route the conflict id to the view/resolve surface.
    Conflict {
        /// Server-minted id of the recorded pending merge.
        sync_id: String,
        /// The base revision echoed back (unchanged from the request).
        base_revision: String,
        /// The base content hash echoed back.
        base_blob_sha256: String,
        /// The item's current head revision (the "theirs" snapshot).
        theirs_revision: String,
        /// The content hash of the item's current head ("theirs" content).
        theirs_blob_sha256: String,
    },
}

/// One immutable snapshot of a three-way merge (base / ours / theirs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictSide {
    /// Server revision number for base/theirs; `None` for ``ours`` because the
    /// offline edit has not (yet) become a server revision.
    pub revision: Option<String>,
    /// Original file name.
    pub filename: String,
    /// Format tag (`step`, `cgr`, ...).
    pub format: String,
    /// Stored media type.
    pub content_type: String,
    /// Decoded file size in bytes.
    pub size: u64,
    /// Content address of this snapshot's bytes.
    pub blob_sha256: String,
}

/// `GET /api/v1/sync/conflicts/{sync_id}`: the three snapshots of a pending
/// merge, so a resolution UI can render base/ours/theirs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncConflictView {
    /// The pending-merge id.
    pub sync_id: String,
    /// The item the conflict belongs to.
    pub item_id: String,
    /// The revision the offline edit was based on.
    pub base: ConflictSide,
    /// The offline edit that was pushed but not committed.
    pub ours: ConflictSide,
    /// The item's head at conflict time.
    pub theirs: ConflictSide,
}

/// Request body for `POST /api/v1/sync/conflicts/{sync_id}/resolve`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncResolveRequest {
    /// Which snapshot wins: `ours`, `theirs`, or `merged` (client-provided).
    pub choice: String,
    /// Required when `choice == "merged"`; the client-merged document to commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged: Option<FileUpload>,
}

// ===========================================================================
// P7-M2-T01 workbench client contract (ported from task/p7m5t01-velocity-1016a)
//
// The typed seam the OEP interchange workbench's PDM load/save surface calls.
// Additive to the REST v1 contract above: the same `state::RevisionState` is
// reused (not a second enum), so the workbench stub and the REST server agree
// on revision states.
// ===========================================================================

/// Metadata record for one stored revision (P7-M2-T01 workbench client).
/// The content itself is addressed separately by [`Self::blob_digest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionSummary {
    /// The item this revision belongs to (e.g. a part number or document id).
    pub item_id: String,
    /// The revision identifier (e.g. "A" or "C.2"). Flat, ordered per item.
    pub revision: String,
    /// Current maturity state.
    pub state: crate::state::RevisionState,
    /// Content address of the stored blob (`digest::content_digest`), when a
    /// blob was supplied.
    pub blob_digest: Option<String>,
    /// Free-form human summary attached at save time.
    pub summary: Option<String>,
    /// Unix-seconds timestamp recorded when the revision was saved.
    pub created_at: u64,
}

/// A save request: the content to store plus the item/revision identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdmWriteRequest {
    /// Item identifier (part number / document id).
    pub item_id: String,
    /// Revision identifier.
    pub revision: String,
    /// Optional free-form summary.
    pub summary: Option<String>,
    /// The blob bytes to store, content-addressed by the vault.
    pub content: Vec<u8>,
    /// State the new revision should start in (almost always `Working`).
    pub state: crate::state::RevisionState,
}

impl PdmWriteRequest {
    /// Builder-ish convenience: a Working-state write request.
    pub fn new(item_id: impl Into<String>, revision: impl Into<String>, content: Vec<u8>) -> Self {
        Self {
            item_id: item_id.into(),
            revision: revision.into(),
            summary: None,
            content,
            state: crate::state::RevisionState::Working,
        }
    }

    /// Set the summary on this request (builder style).
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }
}

/// Typed, non-panicking PDM errors. Every failure is a variant with a
/// documented meaning; the stub and the future server share this surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdmError {
    /// The item does not exist in the vault.
    ItemNotFound {
        /// The missing item id.
        item_id: String,
    },
    /// The item exists but the named revision does not.
    RevisionNotFound {
        /// The item id.
        item_id: String,
        /// The requested revision.
        revision: String,
    },
    /// The revision record references a blob the vault cannot locate.
    BlobMissing {
        /// The missing content address.
        digest: String,
    },
    /// The vault reached an internally inconsistent state (should not happen).
    InconsistentState {
        /// Human-readable detail.
        message: String,
    },
    /// An underlying filesystem operation failed (persisted stub).
    Io {
        /// Human-readable detail.
        message: String,
    },
}

impl std::fmt::Display for PdmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdmError::ItemNotFound { item_id } => write!(f, "PDM item not found: {item_id}"),
            PdmError::RevisionNotFound { item_id, revision } => {
                write!(f, "PDM revision not found: {item_id}@{revision}")
            }
            PdmError::BlobMissing { digest } => write!(f, "PDM blob missing: {digest}"),
            PdmError::InconsistentState { message } => write!(f, "PDM inconsistent: {message}"),
            PdmError::Io { message } => write!(f, "PDM I/O error: {message}"),
        }
    }
}

impl std::error::Error for PdmError {}

/// The P8 API contract: every PDM operation the OEP workbench (and later the
/// P7-M2-T02 browser integration) performs goes through this trait.
///
/// Implementations must be non-panicking and typed-error on every input path
/// (tolerant-failure doctrine, P2-M1-T04).
pub trait PdmClient {
    /// Store a new revision's content and return its record. Overwrites an
    /// identical item+revision (idempotent re-save keeps the same blob digest).
    fn save_revision(&mut self, request: &PdmWriteRequest) -> Result<RevisionSummary, PdmError>;

    /// Read back a revision's stored blob bytes.
    fn open_revision(&mut self, item_id: &str, revision: &str) -> Result<Vec<u8>, PdmError>;

    /// List all revisions of an item, newest first. `ItemNotFound` when the
    /// item has never been saved.
    fn list_revisions(&mut self, item_id: &str) -> Result<Vec<RevisionSummary>, PdmError>;

    /// The latest (head) revision of an item. `ItemNotFound` when absent.
    fn head_revision(&mut self, item_id: &str) -> Result<RevisionSummary, PdmError>;

    /// The content digest the vault recorded for a stored revision
    /// (`crate::digest::content_digest` of the blob), when any.
    fn digest_of(&self, item_id: &str, revision: &str) -> Option<String>;
}
