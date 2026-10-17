//! The P9-M3-T02 batch convert driver: run a corpus of convert jobs across
//! real material and emit a hash-verified [`BatchReport`].
//!
//! Formats:
//!
//! - [`BatchManifest`] (`format = "oep-batch"`, version 1): the description +
//!   the list of [`oep_job::JobSpec`]s to run. Input refs name the P8 vault
//!   items/revisions that were seeded with the corpus files.
//! - [`SeedManifest`] (`format = "oep-seed"`, version 1): local real-material
//!   paths -> vault items (dev-only reference material, never committed —
//!   clean-room invariant, license posture of the P2/P4 corpus).
//! - [`BatchReport`] (`format = "oep-batch-report"`, version 1): the durable
//!   proof — per-job outcome plus `Summary.hash_verified` = the count of jobs
//!   whose vault content address equals the artifact sha-256.
//!
//! The driver runs each job through [`crate::worker::run_job`], so it eats
//! the same dogfood the future scheduler will: spec validated, input resolved
//! through the P8-M6-T02 adapter, output stored, hash-verified.

use std::fmt;

use oep_job::JobSpec;
use oep_pdm::adapter::PdmAdapter;
use serde::{Deserialize, Serialize};

/// Batch manifest wire format.
pub const FORMAT: &str = "oep-batch";
/// Batch report wire format.
pub const REPORT_FORMAT: &str = "oep-batch-report";
/// Seed manifest wire format.
pub const SEED_FORMAT: &str = "oep-seed";
/// The P9 blob-output catalog item name (target `blob` mapping, see
/// [`crate::worker`]).
pub const BLOB_CATALOG_ITEM: &str = "p9-blob-outputs";

/// A local file to seed into the vault as a corpus item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeedFile {
    /// Vault item (name) the file becomes revision 1 of.
    pub item_name: String,
    /// Local filesystem path to the real material (dev-only).
    pub path: String,
    /// Format tag the vault records (`cgr`, `step`, ...).
    pub format: String,
}

/// Seed manifest: which local real files become which vault items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeedManifest {
    /// Must equal [`SEED_FORMAT`].
    pub format: String,
    /// Must equal 1.
    pub version: u64,
    /// Human description of the seed batch.
    pub description: Option<String>,
    /// The files to seed.
    pub files: Vec<SeedFile>,
}

impl SeedManifest {
    /// Load a seed manifest from disk.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read seed manifest {path}: {e}"))?;
        let m: SeedManifest =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse seed manifest: {e}"))?;
        if m.format != SEED_FORMAT {
            return Err(format!("bad seed manifest format: {}", m.format));
        }
        Ok(m)
    }
}

/// Batch manifest: the jobs to run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchManifest {
    /// Must equal [`FORMAT`].
    pub format: String,
    /// Must equal 1.
    pub version: u64,
    /// Human description (e.g. "corpus batch 2026-10-17").
    pub description: Option<String>,
    /// The convert jobs, each a full [`JobSpec`].
    pub jobs: Vec<JobSpec>,
}

impl BatchManifest {
    /// Load a batch manifest from disk.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read batch manifest {path}: {e}"))?;
        let m: BatchManifest =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse batch manifest: {e}"))?;
        if m.format != FORMAT {
            return Err(format!("bad batch manifest format: {}", m.format));
        }
        Ok(m)
    }
}

/// Per-job row of a [`BatchReport`] — the durable proof line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchJobRecord {
    /// Job name from the spec (or executor function).
    pub job_name: String,
    /// Executor kind (`convert`).
    pub executor_kind: String,
    /// Executor function (`cgr2gltf`, `step2mesh`).
    pub executor_function: String,
    /// Input ref display (`item=<name>` / `revision=<name>#<n>`).
    pub input_ref: String,
    /// Content address of the input bytes as the vault served them.
    pub input_sha256: String,
    /// Output name (declared).
    pub output_name: String,
    /// Output format (`gltf`, `oep-mesh`).
    pub output_format: String,
    /// Artifact content address (client-computed).
    pub artifact_sha256: String,
    /// Vault-recorded content address of the stored file.
    pub vault_blob_sha256: String,
    /// Vault item the output landed under.
    pub output_item: String,
    /// Revision the output was stored as.
    pub output_revision: String,
    /// `ok` | `skip` | `error`.
    pub status: String,
    /// Human note (summary for ok, the classified reason for skip, the error
    /// message for error).
    pub note: String,
}

/// Summary counters of a batch run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSummary {
    /// Total jobs submitted.
    pub total: u64,
    /// Jobs that produced + stored + hash-verified an output.
    pub ok: u64,
    /// Jobs that classified a skip (valid input, no convertible content).
    pub skipped: u64,
    /// Jobs that failed.
    pub failed: u64,
    /// Jobs whose vault `FileRef.blob_sha256` equals the artifact sha-256.
    /// For an honest run this equals `ok`.
    pub hash_verified: u64,
}

/// The durable, hash-verified proof of a batch convert run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchReport {
    /// Must equal [`REPORT_FORMAT`].
    pub format: String,
    /// Must equal 1.
    pub version: u64,
    /// Description echoed from the manifest.
    pub description: Option<String>,
    /// The vault URL the run targeted.
    pub vault_url: String,
    /// RFC 3339 UTC run timestamp (when the report was produced).
    pub run_at: String,
    /// One record per submitted job, in manifest order.
    pub jobs: Vec<BatchJobRecord>,
    /// Roll-up counters.
    pub summary: BatchSummary,
}

impl BatchReport {
    /// Write the report as deterministic JSON to `path`.
    pub fn save(&self, path: &str) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(self).map_err(|e| format!("encode report: {e}"))?;
        bytes.push(b'\n');
        std::fs::write(path, &bytes).map_err(|e| format!("write report {path}: {e}"))
    }

    /// Load a report back from disk (replays / re-verifies).
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read report {path}: {e}"))?;
        let r: BatchReport =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse report: {e}"))?;
        if r.format != REPORT_FORMAT {
            return Err(format!("bad report format: {}", r.format));
        }
        Ok(r)
    }
}

/// Seed local corpus files into the vault as revision 1 items.
///
/// Returns the `(item, revision)` pairs in manifest order. Local paths are
/// dev-only reference material (never committed — clean-room invariant); this
/// is the CLI `oep-batch-convert seed` verb.
pub fn seed_from_manifest(
    seed: &SeedManifest,
    vault: &mut PdmAdapter,
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for f in &seed.files {
        let bytes = std::fs::read(&f.path).map_err(|e| format!("read {}: {e}", f.path))?;
        let filename = path_file_name(&f.path).unwrap_or_else(|| f.item_name.clone());
        let saved = vault
            .save_revisions_to_pdm(
                &f.item_name,
                &filename,
                &f.format,
                bytes,
                "oep-batch-convert seed (P9-M3-T02)",
            )
            .map_err(|e| format!("seed {} from {}: {e}", f.item_name, f.path))?;
        out.push((saved.item_id, saved.revision));
    }
    Ok(out)
}

/// Run every job in the manifest through the convert worker and produce the
/// durable report.
///
/// A job whose worker returns a typed [`crate::worker::WorkerError`] becomes
/// an `error` record (the batch keeps going, like the scheduler will);
/// classified skips become `skip` records. The report's
/// [`BatchSummary::hash_verified`] counts the jobs whose vault content
/// address equals the artifact sha-256.
pub fn run_batch(
    manifest: &BatchManifest,
    vault: &mut PdmAdapter,
    vault_url: &str,
) -> Result<BatchReport, String> {
    let mut records = Vec::new();
    for spec in &manifest.jobs {
        let job_name = spec
            .name
            .clone()
            .unwrap_or_else(|| spec.executor.function.clone());
        let record = match crate::worker::run_job(spec, vault, &job_name) {
            Ok(outcome) => BatchJobRecord {
                job_name,
                executor_kind: outcome.executor_kind,
                executor_function: outcome.executor_function,
                input_ref: outcome.input_ref,
                input_sha256: outcome.input_sha256,
                output_name: outcome.output_name,
                output_format: outcome.output_format,
                artifact_sha256: outcome.artifact_sha256,
                vault_blob_sha256: outcome.vault_blob_sha256,
                output_item: outcome.output_item,
                output_revision: outcome.output_revision,
                status: match outcome.status {
                    crate::worker::JobStatus::Ok => "ok".to_string(),
                    crate::worker::JobStatus::Skipped(_) => "skip".to_string(),
                    crate::worker::JobStatus::Failed(_) => "error".to_string(),
                },
                note: outcome.note,
            },
            Err(e) => BatchJobRecord {
                job_name,
                executor_kind: spec.executor.kind.clone(),
                executor_function: spec.executor.function.clone(),
                input_ref: spec
                    .input
                    .first()
                    .map(|i| crate::worker::describe_input(&i.reference))
                    .unwrap_or_default(),
                input_sha256: String::new(),
                output_name: spec
                    .output
                    .first()
                    .map(|o| o.name.clone())
                    .unwrap_or_default(),
                output_format: spec
                    .output
                    .first()
                    .map(|o| o.format.clone())
                    .unwrap_or_default(),
                artifact_sha256: String::new(),
                vault_blob_sha256: String::new(),
                output_item: String::new(),
                output_revision: String::new(),
                status: "error".to_string(),
                note: e.to_string(),
            },
        };
        records.push(record);
    }

    let total = records.len() as u64;
    let ok = records.iter().filter(|r| r.status == "ok").count() as u64;
    let skipped = records.iter().filter(|r| r.status == "skip").count() as u64;
    let failed = records.iter().filter(|r| r.status == "error").count() as u64;
    let hash_verified = records
        .iter()
        .filter(|r| r.status == "ok" && r.vault_blob_sha256 == r.artifact_sha256)
        .count() as u64;

    Ok(BatchReport {
        format: REPORT_FORMAT.to_string(),
        version: 1,
        description: manifest.description.clone(),
        vault_url: vault_url.to_string(),
        run_at: utc_now_rfc3339(),
        jobs: records,
        summary: BatchSummary {
            total,
            ok,
            skipped,
            failed,
            hash_verified,
        },
    })
}

/// RFC 3339 UTC timestamp (no external deps).
fn utc_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;

    // Civil from days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let mth = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if mth <= 2 { y + 1 } else { y } as u64;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, mth, d, h, m, s
    )
}

/// The file name (last path component) of a local path.
fn path_file_name(path: &str) -> Option<String> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

impl fmt::Display for BatchReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "batch: {}", self.description.clone().unwrap_or_default())?;
        writeln!(f, "  vault: {}", self.vault_url)?;
        writeln!(
            f,
            "  summary: {} ok / {} skip / {} failed (hash-verified {})",
            self.summary.ok, self.summary.skipped, self.summary.failed, self.summary.hash_verified
        )?;
        for j in &self.jobs {
            writeln!(
                f,
                "  [{:>5}] {:<24} {} -> {} ({}:{})",
                j.status,
                j.job_name,
                j.executor_function,
                j.output_format,
                j.output_item,
                j.output_revision
            )?;
        }
        Ok(())
    }
}
