//! Documented wire schemas for the P10 escape-hatch archive.
//!
//! Every JSON payload an archive emits carries an explicit `format` marker
//! plus a `version`, so a consumer (or a future OEP release) can type-check
//! what it is reading before trusting it. These structs ARE the schema
//! documentation: the field-by-field comments here are the contract a
//! third-party tool can rely on (P10-M3-T02 "documented archive").

use serde::{Deserialize, Serialize};

/// Wire format marker of the archive root [`ArchiveManifest`].
pub const MANIFEST_FORMAT: &str = "oep-archive";
/// Wire format marker of [`VaultExport`] (the JSON vault metadata dump).
pub const VAULT_EXPORT_FORMAT: &str = "oep-vault-export";
/// Wire format marker of the retention policy payload ([`crate::retention`]).
pub const RETENTION_FORMAT: &str = "oep-retention-policy";
/// Wire format marker of the [`ImportReport`] written by `verify-import`.
pub const IMPORT_REPORT_FORMAT: &str = "oep-import-report";
/// The schema version every P10 archive payload currently emits.
pub const ARCHIVE_SCHEMA_VERSION: u64 = 1;

/// Role a single archive artifact plays in the escape-hatch contract.
///
/// - `geometry`: neutral geometry in an interchange format (the portability
///   guarantee; `step|glb|stl|3dxml`).
/// - `model`: oep-geo own-format document JSON (open + documented, provided
///   for fidelity; NOT the portability guarantee).
/// - `metadata`: the machine JSON dumps (manifest / vault export / policy).
/// - `raw`: the original vault blob, preserved byte-for-byte under
///   `raw/`. A raw blob may be a proprietary format (e.g. CGR); the archive's
///   portability does not depend on it, which is exactly why it is kept apart
///   from `geometry`.
/// - `checksums`: the SHA-256 integrity manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactRole {
    /// Neutral interchange geometry (the escape-hatch guarantee).
    Geometry,
    /// oep-geo own-format JSON document.
    Model,
    /// Machine JSON metadata (manifest / vault export / policy).
    Metadata,
    /// Byte-for-byte original vault blob passthrough.
    Raw,
    /// The archive integrity manifest.
    Checksums,
}

impl ArtifactRole {
    /// The stable lowercase wire token.
    pub const fn as_str(&self) -> &'static str {
        match self {
            ArtifactRole::Geometry => "geometry",
            ArtifactRole::Model => "model",
            ArtifactRole::Metadata => "metadata",
            ArtifactRole::Raw => "raw",
            ArtifactRole::Checksums => "checksums",
        }
    }
}

impl std::fmt::Display for ArtifactRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tool identity stamped into every payload (who wrote this file).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    /// Tool/binary name.
    pub name: String,
    /// Tool version (from `CARGO_PKG_VERSION`).
    pub version: String,
}

impl ToolInfo {
    /// Identity of the running archive tool.
    pub fn this() -> Self {
        Self {
            name: "oep-archive".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Identity of the vault that produced an archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultIdentity {
    /// Vault owner/instance tag (the `VaultMeta.owner` value).
    pub owner: String,
    /// Human-friendly vault name (usually the owner again; kept explicit so a
    /// renamed vault still carries its origin label).
    pub name: String,
}

/// Counts that let a consumer sanity-check an archive without scanning it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveSummary {
    /// Number of vault items exported.
    pub items: u64,
    /// Number of revisions exported.
    pub revisions: u64,
    /// Number of revision files exported.
    pub files: u64,
    /// Number of neutral geometry artifacts written (the guarantee).
    pub geometry_artifacts: u64,
    /// Number of oep-geo model JSON documents written.
    pub model_artifacts: u64,
    /// Number of raw byte-for-byte passthrough blobs written.
    pub raw_artifacts: u64,
    /// Number of per-file derivation attempts that were classified as a skip
    /// (tolerant-failure: recorded, never hidden, never fatal).
    pub skipped: u64,
}

/// One emitted artifact (or one deliberately skipped derivation).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRecord {
    /// Path relative to the archive root, `/`-separated.
    pub path: String,
    /// Lowercase hex sha-256 of the file bytes.
    pub sha256: String,
    /// One of [`ArtifactRole::as_str`] tokens.
    pub role: String,
    /// Format tag of the artifact (`step`, `glb`, `stl`, `3dxml`, `json`, ...).
    pub format: String,
    /// Source vault item id (empty for root-level metadata artifacts).
    pub item_id: String,
    /// Source vault item name (empty for root-level metadata artifacts).
    pub item_name: String,
    /// Source revision number (empty for root-level metadata artifacts).
    pub revision: String,
    /// Original vault filename the artifact derives from.
    pub source_filename: String,
    /// True when the artifact is a recorded skip (no file written).
    pub skipped: bool,
    /// Human reason when `skipped` (absent otherwise).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Root of every export-all archive. [`MANIFEST_FORMAT`] / v1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveManifest {
    /// Must equal [`MANIFEST_FORMAT`].
    pub format: String,
    /// Must equal [`ARCHIVE_SCHEMA_VERSION`].
    pub version: u64,
    /// The tool that wrote the archive (and how to reproduce it).
    pub tool: ToolInfo,
    /// Wall-clock moment the archive was made (RFC 3339 UTC). File payloads
    /// stay deterministic; this stamps the event, not the bytes.
    pub created_at: String,
    /// The vault this archive came from.
    pub source_vault: VaultIdentity,
    /// Rollup counts over the whole archive.
    pub summary: ArchiveSummary,
    /// The retention policy snapshot in force at export time (defaults unless
    /// an admin override was supplied; see `docs/delivery/disaster-recovery/retention-policy.md`).
    pub retention: RetentionSnapshot,
    /// Every artifact (and every recorded skip) in the archive.
    pub artifacts: Vec<ArtifactRecord>,
}

/// The retention-policy snapshot embedded in an archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionSnapshot {
    /// Must equal [`RETENTION_FORMAT`].
    pub format: String,
    /// Must equal [`ARCHIVE_SCHEMA_VERSION`].
    pub version: u64,
    /// Absolute/relative reference to the retained policy doc.
    pub reference: String,
    /// True when the archive carries the built-in defaults (no admin override).
    pub defaults: bool,
}

/// The vault metadata dump: every item / revision / file the archive covered.
/// [`VAULT_EXPORT_FORMAT`] / v1. This is the machine-readable record that
/// re-imports into a fresh vault (see `oep-archive verify-import`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultExport {
    /// Must equal [`VAULT_EXPORT_FORMAT`].
    pub format: String,
    /// Must equal [`ARCHIVE_SCHEMA_VERSION`].
    pub version: u64,
    /// Wall-clock moment of the dump (RFC 3339 UTC).
    pub exported_at: String,
    /// The dump tool identity.
    pub tool: ToolInfo,
    /// Vault owner/instance tag.
    pub owner: String,
    /// The items, each carrying its full revision/file tree.
    pub items: Vec<VaultExportItem>,
}

/// One vault item in the export dump.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultExportItem {
    /// Opaque url-safe item id.
    pub id: String,
    /// Human item name (unique within a store).
    pub name: String,
    /// Item kind (`part`, `assembly`, `drawing`, ... per the P8 contract).
    pub kind: String,
    /// Free-text description, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Owner/principal tag, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Classification/context tag, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Latest revision number, or `None` before the first save-revision.
    pub latest_revision: Option<String>,
    /// Number of revisions in the dump.
    pub revision_count: u64,
    /// All revisions of this item (oldest first, deterministic).
    pub revisions: Vec<VaultExportRevision>,
}

/// One revision in the export dump.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultExportRevision {
    /// Revision number as a string (`"1"`, `"2"`, ...).
    pub number: String,
    /// Lifecycle state (`working` / `under_review` / `released` / `obsolete`).
    pub state: String,
    /// Maturity tag, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maturity: Option<String>,
    /// Change comment, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Acting principal that created the revision.
    pub created_by: String,
    /// Files in this revision (a revision currently carries exactly one).
    pub files: Vec<VaultExportFile>,
}

/// One file record in the export dump (metadata only; bytes come from the
/// archive's `geometry/` or `raw/` tree by `blob_sha256`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultExportFile {
    /// Opaque per-revision file id.
    pub id: String,
    /// Original file name as uploaded.
    pub filename: String,
    /// Format tag (`step`, `cgr`, ...).
    pub format: String,
    /// Stored media type.
    pub content_type: String,
    /// Decoded file size in bytes.
    pub size: u64,
    /// Content address: lowercase hex sha-256 of the decoded bytes.
    pub blob_sha256: String,
}

/// Outcome of the `verify-import` leg, proving the archive re-imports.
/// [`IMPORT_REPORT_FORMAT`] / v1. Written next to the archive as
/// `import-report.json` by `oep-archive verify-import`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    /// Must equal [`IMPORT_REPORT_FORMAT`].
    pub format: String,
    /// Must equal [`ARCHIVE_SCHEMA_VERSION`].
    pub version: u64,
    /// Wall-clock moment of the verification (RFC 3339 UTC).
    pub imported_at: String,
    /// The verifying tool identity.
    pub tool: ToolInfo,
    /// Path of the archive that was verified (as passed on the CLI).
    pub archive: String,
    /// Result of rebuilding a vault from the JSON metadata dump.
    pub vault: VaultRebuildSummary,
    /// Per-document re-import results from the neutral geometry.
    pub documents: Vec<DocumentImportRow>,
    /// Rollup over `documents`.
    pub summary: ImportSummary,
}

/// Result of rebuilding a fresh in-memory vault from `vault-metadata.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultRebuildSummary {
    /// Items rebuilt.
    pub items_rebuilt: u64,
    /// Revisions rebuilt.
    pub revisions_rebuilt: u64,
    /// File records attached.
    pub files: u64,
    /// True when the metadata parsed and every item/revision rebuilt.
    pub ok: bool,
}

/// One neutral geometry document's re-import into oep-geo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentImportRow {
    /// Archive-relative source path (e.g. `geometry/.../box-part.step`).
    pub source: String,
    /// oep-geo parts re-imported.
    pub parts: u64,
    /// oep-geo bodies re-imported.
    pub bodies: u64,
    /// oep-geo features carried.
    pub features: u64,
    /// True when `Document::validate` reported zero structural violations.
    pub valid: bool,
    /// `reimported` or `skipped`.
    pub status: String,
    /// Reason when `skipped`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Rollup over the document re-import rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportSummary {
    /// Total neutral STEP geometry files the archive carried.
    pub total_step: u64,
    /// STEP files re-imported into oep-geo documents.
    pub reimported: u64,
    /// STEP files deliberately skipped (e.g. geometry-less) -- recorded.
    pub skipped: u64,
    /// STEP files that failed to re-import.
    pub failed: u64,
    /// True when `failed == 0` and the vault metadata rebuilt cleanly.
    pub ok: bool,
}
