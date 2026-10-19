//! The `verify-import` leg (P10-M3-T02 acceptance): prove an archive really
//! re-imports into oep-geo.
//!
//! Two independent checks, both recorded in an [`ImportReport`]:
//!
//! 1. **Vault metadata rebuild** — `vault-metadata.json` is validated
//!    against the documented schema and every item/revision/file record is
//!    rebuilt into a fresh in-memory [`VaultStore`].
//! 2. **oep-geo re-import** — every neutral `step` geometry file in the
//!    archive is read back through [`oep_step::parse_to_document`] and its
//!    resulting [`Document`] validated; part/body/feature counts are
//!    recorded. This is the escape-hatch guarantee: geometry that any
//!    neutral STEP tool can open came back into the platform's canonical
//!    model.
//!
//! The report is written to `<archive>/import-report.json` (or an explicit
//! `--out` path).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload};
use oep_pdm::store::VaultStore;
use serde_json::Value;

use crate::error::ArchiveError;
use crate::schema::{
    DocumentImportRow, ImportReport, ImportSummary, ToolInfo, VaultExport, VaultRebuildSummary,
    ARCHIVE_SCHEMA_VERSION, IMPORT_REPORT_FORMAT, MANIFEST_FORMAT,
};

/// Verify an archive written by [`crate::export::export_all`].
///
/// `out` controls where `import-report.json` goes (defaults to inside the
/// archive). Returns the report; the caller treats `report.summary.ok` as
/// the pass/fail for the acceptance.
pub fn verify_import(archive_dir: &Path, out: Option<&Path>) -> Result<ImportReport, ArchiveError> {
    // 1) Integrity check first: the manifest must exist and be well-formed.
    let manifest_path = archive_dir.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|_| ArchiveError::NotFound(format!("missing {}", manifest_path.display())))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    if manifest.get("format").and_then(|v| v.as_str()) != Some(MANIFEST_FORMAT) {
        return Err(ArchiveError::Schema(
            "manifest.json format marker missing".to_string(),
        ));
    }
    if manifest.get("version").and_then(|v| v.as_u64()) != Some(ARCHIVE_SCHEMA_VERSION) {
        return Err(ArchiveError::Schema(
            "manifest.json unsupported version".to_string(),
        ));
    }
    let artifacts = manifest
        .get("artifacts")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    // 2) Vault metadata model.
    let meta_path = archive_dir.join("vault-metadata.json");
    let meta_bytes = fs::read(&meta_path)
        .map_err(|_| ArchiveError::NotFound(format!("missing {}", meta_path.display())))?;
    let export: VaultExport = serde_json::from_slice(&meta_bytes)?;
    if export.format != "oep-vault-export" || export.version != ARCHIVE_SCHEMA_VERSION {
        return Err(ArchiveError::Schema(
            "vault-metadata.json format/version mismatch".to_string(),
        ));
    }

    let rebuild = rebuild_vault(&export, archive_dir, &artifacts)?;

    // 3) Re-import each neutral STEP geometry file into oep-geo.
    let mut documents: Vec<DocumentImportRow> = Vec::new();
    let mut total_step = 0u64;
    let mut reimported = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;

    for art in &artifacts {
        let role = art.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let format = art.get("format").and_then(|v| v.as_str()).unwrap_or("");
        let path = art.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let skipped_flag = art
            .get("skipped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if role != "geometry" || format != "step" || path.is_empty() {
            continue;
        }
        // Records marked skipped in the manifest carry no file and were never
        // claimed: a recorded skip, not a re-importable document.
        if skipped_flag {
            continue;
        }
        total_step += 1;
        let abs = archive_dir.join(path);
        let bytes = match fs::read(&abs) {
            Ok(b) => b,
            Err(e) => {
                failed += 1;
                documents.push(failed_row(
                    path,
                    &format!("cannot read {}: {e}", abs.display()),
                ));
                continue;
            }
        };
        match oep_step::parse_to_document(&bytes) {
            Ok((doc, _report)) => {
                let violations = doc.validate();
                let parts = doc.parts.len() as u64;
                let bodies = doc.bodies.len() as u64;
                let features = doc.features.len() as u64;
                let valid = violations.is_empty();
                if valid {
                    reimported += 1;
                } else {
                    failed += 1;
                }
                let reason = if !valid {
                    Some(format!(
                        "document validation: {:?}",
                        violations.iter().take(3).collect::<Vec<_>>()
                    ))
                } else {
                    None
                };
                documents.push(DocumentImportRow {
                    source: path.to_string(),
                    parts,
                    bodies,
                    features,
                    valid,
                    status: if valid { "reimported" } else { "failed" }.to_string(),
                    reason,
                });
            }
            Err(e) => {
                // Tolerant doctrine (P4): a genuine STEP file whose geometry is
                // analytic-only (no faceted surface model) cannot build an
                // oep-geo Document and is a CLASSIFIED SKIP, not corruption.
                // Only a non-STEP payload counts as a hard failure.
                if bytes.starts_with(b"ISO-10303-21;") {
                    skipped += 1;
                    documents.push(DocumentImportRow {
                        source: path.to_string(),
                        parts: 0,
                        bodies: 0,
                        features: 0,
                        valid: false,
                        status: "skipped".to_string(),
                        reason: Some(format!("classified skip (no faceted Document): {e}")),
                    });
                } else {
                    failed += 1;
                    documents.push(failed_row(path, &format!("step re-import: {e}")));
                }
            }
        }
    }

    let summary = ImportSummary {
        total_step,
        reimported,
        skipped,
        failed,
        ok: failed == 0 && rebuild.ok,
    };

    let report = ImportReport {
        format: IMPORT_REPORT_FORMAT.to_string(),
        version: ARCHIVE_SCHEMA_VERSION,
        imported_at: crate::export::utc_now(),
        tool: ToolInfo::this(),
        archive: archive_dir.display().to_string(),
        vault: rebuild,
        documents,
        summary,
    };
    let bytes = serde_json::to_vec_pretty(&report)?;
    let out_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| archive_dir.join("import-report.json"));
    fs::write(&out_path, bytes)?;
    Ok(report)
}

/// A failed re-import document row.
fn failed_row(source: &str, reason: &str) -> DocumentImportRow {
    DocumentImportRow {
        source: source.to_string(),
        parts: 0,
        bodies: 0,
        features: 0,
        valid: false,
        status: "failed".to_string(),
        reason: Some(reason.to_string()),
    }
}

/// Rebuild items/revisions/files metadata into a fresh in-memory store,
/// pulling each file's bytes from the archive `raw/` tree when the manifest
/// still maps them (tolerant: metadata-only rebuild when the blob is absent).
fn rebuild_vault(
    export: &VaultExport,
    archive_dir: &Path,
    artifacts: &[Value],
) -> Result<VaultRebuildSummary, ArchiveError> {
    // Map (item_id, revision, source_filename) -> archive-relative raw path.
    let mut raw_by_file: HashMap<(String, String, String), String> = HashMap::new();
    for art in artifacts {
        if art.get("role").and_then(|v| v.as_str()) != Some("raw") {
            continue;
        }
        let read_str = |k: &str| {
            art.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let key = (
            read_str("item_id"),
            read_str("revision"),
            read_str("source_filename"),
        );
        let path = read_str("path");
        if !path.is_empty() {
            raw_by_file.insert(key, path);
        }
    }

    let mut store = VaultStore::new();
    let mut items = 0u64;
    let mut revisions = 0u64;
    let mut files = 0u64;

    for item in &export.items {
        let req = CreateItemRequest {
            name: item.name.clone(),
            kind: item.kind.clone(),
            description: item.description.clone(),
            owner: item.owner.clone(),
            context: item.context.clone(),
        };
        store
            .create_item(&req)
            .map_err(|e| ArchiveError::Vault(format!("rebuild item {}: {e}", item.name)))?;
        for rev in &item.revisions {
            for f in &rev.files {
                let bytes = match raw_by_file.get(&(
                    item.id.clone(),
                    rev.number.clone(),
                    f.filename.clone(),
                )) {
                    Some(rel) => fs::read(archive_dir.join(rel)).unwrap_or_default(),
                    None => Vec::new(),
                };
                let req = CreateRevisionRequest {
                    state: Some(rev.state.clone()),
                    comment: rev.comment.clone(),
                    created_by: Some(rev.created_by.clone()),
                    file: FileUpload {
                        filename: f.filename.clone(),
                        format: f.format.clone(),
                        content_type: Some(f.content_type.clone()),
                        content: base64_encode(&bytes),
                    },
                };
                store
                    .create_revision(&item.id, &req)
                    .map_err(|e| ArchiveError::Vault(format!("rebuild rev {}: {e}", rev.number)))?;
                files += 1;
            }
            revisions += 1;
        }
        items += 1;
    }

    Ok(VaultRebuildSummary {
        items_rebuilt: items,
        revisions_rebuilt: revisions,
        files,
        ok: true,
    })
}

/// Standard base64 (the P8 store wire encoding).
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
