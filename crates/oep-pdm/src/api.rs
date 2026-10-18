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
    // ---- P8-M6-T03 additive gate surface (never used by the v1 verbs) ----
    /// A working-set lock cannot be acquired (another principal holds it).
    pub const LOCK_CONFLICT: &str = "lock_conflict";
    /// A lock-token check failed: not held, wrong token, or lease expired.
    pub const LOCK_TOKEN: &str = "lock_token";
    /// The audit log is not available (no persist dir / audit root).
    pub const AUDIT_UNAVAILABLE: &str = "audit_unavailable";
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
