//! The `export-all` driver: walk a P8 vault and write the documented neutral
//! archive (P10-M3-T02 interop escape hatch).
//!
//! ## What the archive guarantees
//!
//! For every revision file, the archive emits:
//!
//! - `raw/<slug>/<rev>/<file>` — the original vault blob, byte-for-byte
//!   (preservation; may be a proprietary format, kept apart from `geometry`).
//! - `geometry/<slug>/<rev>/<stem>.{ext}` — neutral interchange geometry
//!   derived through the platform's own exporters:
//!   - `step`   — AP242 passthrough (the original STEP file is already the
//!     neutral B-Rep interchange format), via P4.
//!   - `glb`    — glTF 2.0 binary from the step/cgr mesh, via P2
//!     (`oep_cgr::gltf::write_glb`).
//!   - `stl`    — binary STL from the step/cgr mesh (this crate's writer).
//!   - `3dxml`  — 3DXML ASCII package from the mesh, via P3
//!     (`oep_3dxml::ascii::write_package`).
//! - `model/<slug>/<rev>/<stem>.oep-geo.json` — the oep-geo own-format
//!   document (open + documented JSON), proving the re-import into oep-geo.
//!
//! Root-level machine dumps: `manifest.json`, `vault-metadata.json`,
//! `retention-policy.json`, and `checksums.sha256` over every file.
//!
//! ## Tolerant per-file failure
//!
//! A file that cannot be derived (no faceted geometry, unknown format, a
//! writer rejection) is recorded in the manifest as a **skip with a reason**,
//! never as a hard failure. The archive still contains the raw blob and the
//! metadata; only I/O, schema, and vault-shape problems abort the run.

use std::fs;
use std::path::Path;

use oep_pdm::api::{FileRef, Item, Revision};
use oep_pdm::store::VaultStore;
use sha2::{Digest, Sha256};

use crate::error::ArchiveError;
use crate::retention::RetentionPolicy;
use crate::schema::{
    ArchiveManifest, ArchiveSummary, ArtifactRecord, ArtifactRole, RetentionSnapshot, ToolInfo,
    VaultExport, VaultExportFile, VaultExportItem, VaultExportRevision, VaultIdentity,
    ARCHIVE_SCHEMA_VERSION, MANIFEST_FORMAT, RETENTION_FORMAT, VAULT_EXPORT_FORMAT,
};
use crate::stl::write_stl_binary;

/// A derived neutral mesh carrying triangle soup as plain f32 slices, so the
/// glTF / STL / 3DXML writers stay independent of any concrete mesh type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlatMesh {
    /// Vertex positions (x,y,z triples).
    pub positions: Vec<[f32; 3]>,
    /// 0-based index triples into `positions`.
    pub triangles: Vec<[u32; 3]>,
}

impl FlatMesh {
    /// True when there is no geometry to write.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty() || self.triangles.is_empty()
    }

    /// Drop zero-area (degenerate) triangles. A triangle whose cross-product
    /// area is below `1e-12` carries no face, so removing it loses nothing
    /// and lets the strict writers (glTF, 3DXML) accept the mesh. Indices
    /// stay valid (they reference `positions`, which is unchanged).
    pub fn cleaned(&self) -> FlatMesh {
        let mut out = FlatMesh {
            positions: self.positions.clone(),
            triangles: Vec::with_capacity(self.triangles.len()),
        };
        for tri in &self.triangles {
            let outside = tri.iter().any(|&i| i as usize >= self.positions.len());
            if outside {
                continue;
            }
            let a = self.positions[tri[0] as usize].map(|v| v as f64);
            let b = self.positions[tri[1] as usize].map(|v| v as f64);
            let c = self.positions[tri[2] as usize].map(|v| v as f64);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cx = u[1] * v[2] - u[2] * v[1];
            let cy = u[2] * v[0] - u[0] * v[2];
            let cz = u[0] * v[1] - u[1] * v[0];
            let area2 = 0.5 * (cx * cx + cy * cy + cz * cz).sqrt();
            if area2 >= 1e-12 {
                out.triangles.push(*tri);
            }
        }
        out
    }

    /// From a STEP facet model (f64 vertices are truncated to f32 for the
    /// mesh interchange formats; recorded honesty, not exactness).
    pub fn from_facets(facets: &oep_step::facets::FacetedModel) -> Self {
        let positions = facets
            .vertices
            .iter()
            .map(|v| [v[0] as f32, v[1] as f32, v[2] as f32])
            .collect();
        Self {
            positions,
            triangles: facets.triangles.clone(),
        }
    }

    /// From a CGR `oep_cgr::mesh::Mesh` (already f32).
    pub fn from_cgr_mesh(m: &oep_cgr::mesh::Mesh) -> Self {
        Self {
            positions: m.vertices().to_vec(),
            triangles: m.triangles().to_vec(),
        }
    }
}

/// The kind of a vault file, decided by its format tag then its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// ISO 10303-21 STEP file.
    Step,
    /// CATIA Graphics Representation file.
    Cgr,
    /// oep-geo own-format document JSON.
    OepGeo,
    /// Anything else: preserved raw, no derived geometry.
    Passthrough,
}

impl FileKind {
    /// Classify vault bytes + format tag into a [`FileKind`].
    pub fn classify(bytes: &[u8], format_tag: &str) -> Self {
        if format_tag.eq_ignore_ascii_case("step") || bytes.starts_with(b"ISO-10303-21;") {
            FileKind::Step
        } else if format_tag.eq_ignore_ascii_case("cgr") {
            FileKind::Cgr
        } else if format_tag.eq_ignore_ascii_case("oep-geo")
            || format_tag.eq_ignore_ascii_case("json")
        {
            FileKind::OepGeo
        } else {
            FileKind::Passthrough
        }
    }
}

/// One on-disk artifact together with its manifest record.
pub struct Emitted {
    /// The manifest record for this artifact (`sha256` filled after write).
    pub record: ArtifactRecord,
    /// The bytes written (`None` for a recorded skip).
    pub bytes: Option<Vec<u8>>,
}

impl Emitted {
    fn written(record: ArtifactRecord, bytes: Vec<u8>) -> Self {
        Self {
            record,
            bytes: Some(bytes),
        }
    }

    fn skipped(record: ArtifactRecord, reason: &str) -> Self {
        let mut r = record;
        r.skipped = true;
        r.reason = Some(reason.to_string());
        Self {
            record: r,
            bytes: None,
        }
    }
}

/// Export a loaded vault store into a documented neutral archive at `out`.
///
/// Returns the archive manifest (already written to disk). Deterministic for
/// a given store + `owner_label` except the wall-clock `created_at` stamp.
pub fn export_all(
    store: &VaultStore,
    out: &Path,
    owner_label: Option<&str>,
) -> Result<ArchiveManifest, ArchiveError> {
    if out.exists() && out.read_dir()?.next().is_some() {
        return Err(ArchiveError::Inconsistent(
            "archive output directory is not empty; refusing to overwrite".to_string(),
        ));
    }
    let owner = owner_label.unwrap_or("keelworks").to_string();

    fs::create_dir_all(out.join("geometry"))?;
    fs::create_dir_all(out.join("model"))?;
    fs::create_dir_all(out.join("raw"))?;

    let mut artifacts: Vec<ArtifactRecord> = Vec::new();
    let mut written: Vec<(String, Vec<u8>)> = Vec::new(); // (rel_path, bytes)
    let mut vault_items: Vec<VaultExportItem> = Vec::new();
    let mut summary = ArchiveSummary {
        items: 0,
        revisions: 0,
        files: 0,
        geometry_artifacts: 0,
        model_artifacts: 0,
        raw_artifacts: 0,
        skipped: 0,
    };

    // Items, deterministic order by name.
    let mut item_ids: Vec<String> = store
        .list_items("", usize::MAX, 0)
        .items
        .iter()
        .map(|s| s.id.clone())
        .collect();
    item_ids.sort();

    for item_id in item_ids {
        let item = store.get_item(&item_id)?;
        let mut revisions = store.list_revisions(&item_id)?.revisions;
        revisions.sort_by_key(|a| natural_num(&a.number));

        let mut export_revs: Vec<VaultExportRevision> = Vec::new();
        for rev in &revisions {
            let (_number, file, bytes) = store.open_revision(&item_id, &rev.number)?;
            let emitted = derive_file(&item, rev, &file, &bytes)?;
            for mut e in emitted {
                if let Some(b) = e.bytes {
                    let dest = out.join(&e.record.path);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&dest, &b)?;
                    e.record.sha256 = hash_hex(&b);
                    summary_add(&mut summary, &e.record);
                    written.push((e.record.path.clone(), b));
                }
                artifacts.push(e.record);
            }
            export_revs.push(export_revision(rev));
            summary.revisions += 1;
            summary.files += 1;
        }
        summary.items += 1;
        vault_items.push(VaultExportItem {
            id: item.id.clone(),
            name: item.name.clone(),
            kind: item.kind.clone(),
            description: item.description.clone(),
            owner: item.owner.clone(),
            context: item.context.clone(),
            created_at: item.created_at.clone(),
            latest_revision: item.latest_revision.clone(),
            revision_count: item.revision_count,
            revisions: export_revs,
        });
    }

    // Root machine dumps. First, the skipped counter: it is computed from the
    // recorded skip rows (geometry/model derivations deliberately not written),
    // which never pass through the write loop.
    summary.skipped = artifacts
        .iter()
        .filter(|r| r.skipped && (r.role == "geometry" || r.role == "model"))
        .count() as u64;

    let export = VaultExport {
        format: VAULT_EXPORT_FORMAT.to_string(),
        version: ARCHIVE_SCHEMA_VERSION,
        exported_at: utc_now(),
        tool: ToolInfo::this(),
        owner: owner.clone(),
        items: vault_items,
    };
    let export_bytes = serde_json::to_vec_pretty(&export)?;
    fs::write(out.join("vault-metadata.json"), &export_bytes)?;
    written.push(("vault-metadata.json".to_string(), export_bytes));

    let policy = RetentionPolicy::defaults();
    let policy_bytes = policy.to_json_pretty()?.into_bytes();
    fs::write(out.join("retention-policy.json"), &policy_bytes)?;
    written.push(("retention-policy.json".to_string(), policy_bytes));

    let retention = RetentionSnapshot {
        format: RETENTION_FORMAT.to_string(),
        version: ARCHIVE_SCHEMA_VERSION,
        reference: "docs/P10-M3/retention-policy.md".to_string(),
        defaults: true,
    };

    let manifest = ArchiveManifest {
        format: MANIFEST_FORMAT.to_string(),
        version: ARCHIVE_SCHEMA_VERSION,
        tool: ToolInfo::this(),
        created_at: utc_now(),
        source_vault: VaultIdentity {
            owner: owner.clone(),
            name: owner,
        },
        summary,
        retention,
        artifacts: artifacts.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    fs::write(out.join("manifest.json"), &manifest_bytes)?;
    written.push(("manifest.json".to_string(), manifest_bytes));

    // Integrity manifest over every archive member (not itself).
    let mut sums: Vec<String> = Vec::new();
    for (path, bytes) in &written {
        sums.push(format!("{}  {}", hash_hex(bytes), path));
    }
    sums.sort();
    let checksum_bytes = format!("{}\n", sums.join("\n")).into_bytes();
    fs::write(out.join("checksums.sha256"), checksum_bytes)?;

    Ok(manifest)
}

/// Emitted records for a single revision file (nothing is written yet; the
/// caller writes `bytes` to `record.path` and fills `record.sha256`).
fn derive_file(
    item: &Item,
    rev: &Revision,
    file: &FileRef,
    bytes: &[u8],
) -> Result<Vec<Emitted>, ArchiveError> {
    let slug = slugify(&item.name);
    let stem = stem_of(&file.filename);
    let revnum = &rev.number;
    let base = ArtifactRecord {
        path: String::new(),
        sha256: String::new(),
        role: String::new(),
        format: String::new(),
        item_id: item.id.clone(),
        item_name: item.name.clone(),
        revision: revnum.clone(),
        source_filename: file.filename.clone(),
        skipped: false,
        reason: None,
    };
    let mut out_list = Vec::new();

    // 1) raw passthrough (byte-for-byte original vault blob).
    let raw_name = sanitize_filename(&file.filename);
    let raw_rel = format!("raw/{slug}/{revnum}/{raw_name}");
    out_list.push(Emitted::written(
        rec(&base, &raw_rel, ArtifactRole::Raw, &file.format),
        bytes.to_vec(),
    ));

    match FileKind::classify(bytes, &file.format) {
        FileKind::Step => {
            // The STEP file is already the neutral B-Rep interchange format:
            // pass it through into geometry/ verbatim too.
            let step_rel = format!("geometry/{slug}/{revnum}/{stem}.step");
            out_list.push(Emitted::written(
                rec(&base, &step_rel, ArtifactRole::Geometry, "step"),
                bytes.to_vec(),
            ));

            let facets = match oep_step::parse(bytes) {
                Ok(o) => Some(o.facets),
                Err(e) => {
                    push_mesh_skips(
                        &base,
                        &slug,
                        revnum,
                        &stem,
                        &mut out_list,
                        &format!("STEP read: {e}"),
                        false,
                    );
                    None
                }
            };
            if let Some(facets) = facets {
                let mesh = FlatMesh::from_facets(&facets);
                push_mesh_artifacts(&base, &slug, revnum, &stem, &mesh, &mut out_list, true)?;

                // The oep-geo own-format model (open + documented JSON).
                let rel = format!("model/{slug}/{revnum}/{stem}.oep-geo.json");
                match oep_step::parse_to_document(bytes) {
                    Ok((doc, _report)) => match serde_json::to_vec_pretty(&doc) {
                        Ok(json) => out_list.push(Emitted::written(
                            rec(&base, &rel, ArtifactRole::Model, "json"),
                            json,
                        )),
                        Err(e) => out_list.push(Emitted::skipped(
                            rec(&base, &rel, ArtifactRole::Model, "json"),
                            &format!("oep-geo serialize: {e}"),
                        )),
                    },
                    Err(e) => out_list.push(Emitted::skipped(
                        rec(&base, &rel, ArtifactRole::Model, "json"),
                        &format!("oep-geo re-import: {e}"),
                    )),
                }
            }
        }
        FileKind::Cgr => match extract_cgr_mesh(bytes) {
            Ok(Some(mesh)) => {
                let flat = FlatMesh::from_cgr_mesh(&mesh);
                push_mesh_artifacts(&base, &slug, revnum, &stem, &flat, &mut out_list, false)?;
            }
            Ok(None) => {
                push_mesh_skips(
                    &base,
                    &slug,
                    revnum,
                    &stem,
                    &mut out_list,
                    "CGR decode produced no triangles",
                    true,
                );
            }
            Err(e) => {
                push_mesh_skips(
                    &base,
                    &slug,
                    revnum,
                    &stem,
                    &mut out_list,
                    &format!("CGR decode: {e}"),
                    true,
                );
            }
        },
        FileKind::OepGeo => {
            // The native document: keep it as a model artifact (it is the doc).
            let rel = format!("model/{slug}/{revnum}/{stem}.oep-geo.json");
            out_list.push(Emitted::written(
                rec(&base, &rel, ArtifactRole::Model, "json"),
                bytes.to_vec(),
            ));
            push_mesh_skips(
                &base,
                &slug,
                revnum,
                &stem,
                &mut out_list,
                "oep-geo own-format carries no tessellation in the current schema (P6-M2); derived geometry deferred",
                true,
            );
        }
        FileKind::Passthrough => {
            push_mesh_skips(
                &base,
                &slug,
                revnum,
                &stem,
                &mut out_list,
                &format!("no neutral exporter for format {}", file.format),
                true,
            );
        }
    }

    Ok(out_list)
}

/// Try structural (old-era) then modern (V5_CFV2) CGR extraction. `Ok(None)`
/// means the file decoded but carried no triangles (a classified skip).
fn extract_cgr_mesh(bytes: &[u8]) -> Result<Option<oep_cgr::mesh::Mesh>, String> {
    let extracted = match oep_cgr::mesh::extract(bytes) {
        Ok(m) => m,
        Err(oep_cgr::mesh::MeshError::NotCgrPreamble) => {
            match oep_cgr::modern::extract_modern(bytes) {
                Ok((_counts, m)) => m,
                Err(outcome) => return Err(format!("modern CGR decode: {outcome:?}")),
            }
        }
        Err(e) => return Err(format!("structural CGR decode: {e}")),
    };
    if extracted.triangles().is_empty() {
        return Ok(None);
    }
    extracted
        .validate()
        .map_err(|e| format!("mesh invariants: {e}"))?;
    Ok(Some(extracted))
}

/// GLB + STL + 3DXML from a mesh; a single writer failure is recorded as a
/// skip so the rest of the file still ships. `step_written` tells whether the
/// archive already carries a real STEP passthrough (Step kind), so the empty-
/// mesh skip row does not duplicate it.
fn push_mesh_artifacts(
    base: &ArtifactRecord,
    slug: &str,
    revnum: &str,
    stem: &str,
    mesh: &FlatMesh,
    out_list: &mut Vec<Emitted>,
    step_written: bool,
) -> Result<(), ArchiveError> {
    if mesh.is_empty() {
        push_mesh_skips(
            base,
            slug,
            revnum,
            stem,
            out_list,
            "empty mesh",
            !step_written,
        );
        return Ok(());
    }
    // Drop zero-area triangles so the strict glTF/3DXML writers accept the
    // mesh (a degenerate triangle carries no face). Honest: nothing is lost.
    let mesh = mesh.cleaned();
    if mesh.is_empty() {
        push_mesh_skips(
            base,
            slug,
            revnum,
            stem,
            out_list,
            "mesh empty after degenerate-triangle filter",
            !step_written,
        );
        return Ok(());
    }

    // glTF / GLB (P2 exporter).
    let glb_rel = format!("geometry/{slug}/{revnum}/{stem}.glb");
    let mut glb = Vec::new();
    match oep_cgr::gltf::write_glb(&mesh.positions, &mesh.triangles, &mut glb) {
        Ok(()) => {
            out_list.push(Emitted::written(
                rec(base, &glb_rel, ArtifactRole::Geometry, "glb"),
                glb,
            ));
        }
        Err(e) => out_list.push(Emitted::skipped(
            rec(base, &glb_rel, ArtifactRole::Geometry, "glb"),
            &format!("glb write: {e}"),
        )),
    }

    // Binary STL (this crate's writer).
    let stl_rel = format!("geometry/{slug}/{revnum}/{stem}.stl");
    let mut stl = Vec::new();
    match write_stl_binary(stem, &mesh.positions, &mesh.triangles, &mut stl) {
        Ok(()) => {
            out_list.push(Emitted::written(
                rec(base, &stl_rel, ArtifactRole::Geometry, "stl"),
                stl,
            ));
        }
        Err(e) => out_list.push(Emitted::skipped(
            rec(base, &stl_rel, ArtifactRole::Geometry, "stl"),
            &format!("stl write: {e}"),
        )),
    }

    // 3DXML ASCII package (P3 writer).
    let arc_rel = format!("geometry/{slug}/{revnum}/{stem}.3dxml");
    let opts = oep_3dxml::ascii::WriterOpts {
        product_name: stem,
        author: "Keelworks OEP",
        // Empty timestamp keeps geometry bytes deterministic; the wall clock
        // lives only in manifest.json.
        timestamp: "",
    };
    let mut arc = Vec::new();
    match oep_3dxml::ascii::write_package(&mesh.positions, &mesh.triangles, &opts, &mut arc) {
        Ok(()) => {
            out_list.push(Emitted::written(
                rec(base, &arc_rel, ArtifactRole::Geometry, "3dxml"),
                arc,
            ));
        }
        Err(e) => out_list.push(Emitted::skipped(
            rec(base, &arc_rel, ArtifactRole::Geometry, "3dxml"),
            &format!("3dxml write: {e}"),
        )),
    }

    Ok(())
}

/// Record skip rows for the derived-geometry formats. `include_step` is true
/// when no real STEP file was written for this item (CGR / oep-geo /
/// passthrough), so a `step` skip row is part of the honest record; it is
/// false after a successful STEP passthrough (no duplicate row).
fn push_mesh_skips(
    base: &ArtifactRecord,
    slug: &str,
    revnum: &str,
    stem: &str,
    out_list: &mut Vec<Emitted>,
    reason: &str,
    include_step: bool,
) {
    let fmts: &[&str] = if include_step {
        &["step", "glb", "stl", "3dxml"]
    } else {
        &["glb", "stl", "3dxml"]
    };
    for fmt in fmts {
        let rel = format!("geometry/{slug}/{revnum}/{stem}.{fmt}");
        out_list.push(Emitted::skipped(
            rec(base, &rel, ArtifactRole::Geometry, fmt),
            reason,
        ));
    }
}

/// Build a concrete `ArtifactRecord` from the skeleton (`sha256` filled after write).
fn rec(base: &ArtifactRecord, path: &str, role: ArtifactRole, format: &str) -> ArtifactRecord {
    ArtifactRecord {
        path: path.to_string(),
        sha256: String::new(),
        role: role.as_str().to_string(),
        format: format.to_string(),
        item_id: base.item_id.clone(),
        item_name: base.item_name.clone(),
        revision: base.revision.clone(),
        source_filename: base.source_filename.clone(),
        skipped: false,
        reason: None,
    }
}

/// Maintain the summary counters for a written artifact.
fn summary_add(summary: &mut ArchiveSummary, record: &ArtifactRecord) {
    match record.role.as_str() {
        "geometry" => summary.geometry_artifacts += 1,
        "model" => summary.model_artifacts += 1,
        "raw" => summary.raw_artifacts += 1,
        _ => {}
    }
}

fn export_revision(rev: &Revision) -> VaultExportRevision {
    VaultExportRevision {
        number: rev.number.clone(),
        state: rev.state.clone(),
        maturity: rev.maturity.clone(),
        comment: rev.comment.clone(),
        created_at: rev.created_at.clone(),
        created_by: rev.created_by.clone(),
        files: rev
            .files
            .iter()
            .map(|f| VaultExportFile {
                id: f.id.clone(),
                filename: f.filename.clone(),
                format: f.format.clone(),
                content_type: f.content_type.clone(),
                size: f.size,
                blob_sha256: f.blob_sha256.clone(),
            })
            .collect(),
    }
}

/// Stable numeric ordering for `"1" <= "2" <= "10"` style revision numbers.
fn natural_num(s: &str) -> u64 {
    s.parse::<u64>().unwrap_or(u64::MAX)
}

/// File stem (path + extension stripped).
fn stem_of(source_name: &str) -> String {
    let base = source_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_name);
    match base.split_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ if base.is_empty() => "untitled".to_string(),
        _ => base.to_string(),
    }
}

/// Filesystem-safe slug from an item name (lowercase alnum + dashes).
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("item");
    }
    if out.len() > 80 {
        out.truncate(80);
    }
    out
}

/// Filesystem-safe original filename (keeps the extension for the raw tree).
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut out = String::with_capacity(base.len());
    for c in base.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with('.') {
        out.insert_str(0, "file-");
    }
    if out.len() > 160 {
        out.truncate(160);
    }
    out
}

/// RFC 3339 UTC now (stamped in the manifest/export, never the bytes).
pub(crate) fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Civil-from-days (Howard Hinnant algorithm), used for the UTC stamp.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let adj = z + 719_468;
    let era = if adj >= 0 { adj } else { adj - 146_096 } / 146_097;
    let doe = (adj - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    ((if m <= 2 { y + 1 } else { y }), m, d)
}

/// Lowercase hex sha-256 of bytes.
fn hash_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}
