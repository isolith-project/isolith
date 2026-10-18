//! The P9 job-facing convert worker: executes a [`oep_job::JobSpec`] against
//! the live P8 vault.
//!
//! [`run_job`] is the one entry point the scheduler (P9-M2) will call and the
//! batch driver (P9-M3-T02) calls today. It is deliberately short and
//! stateless: resolve the vault input through the P8-M6-T02
//! [`oep_pdm::adapter::PdmAdapter`] seam, run the registered convert worker,
//! publish the artifact to the declared output target, and — the GA-gate
//! contract from P9-M5-T02 — hash-verify that the vault's own content address
//! equals the artifact's sha-256.
//!
//! The worker never panics and never fabricates: every path yields a typed
//! [`WorkerError`] or a [`JobOutcome`] whose status is `ok`, `skip`, or
//! `error`. Non-convertible inputs are *classified skips* (valid container,
//! no mesh the lane maps), not failures — the batch keeps running.

use std::fmt;

use oep_job::spec::VaultObjectRef;
use oep_job::{JobSpec, OutputTarget};
use oep_pdm::adapter::PdmAdapter;

use crate::batch::BLOB_CATALOG_ITEM;
use crate::convert::{Artifact, ConvertError};
use crate::registry;

/// Outcome status of one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// The output was produced, stored, and hash-verified in the vault.
    Ok,
    /// A classified skip (valid input, no convertible content). Not a
    /// failure.
    Skipped(String),
    /// The job failed (typed message).
    Failed(String),
}

/// The typed result of [`run_job`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    /// Job label from the spec (or the executor function when absent).
    pub job_name: String,
    /// Executor `kind` from the spec (always `convert` in this lane).
    pub executor_kind: String,
    /// Executor `function` (`cgr2gltf`, `step2mesh`).
    pub executor_function: String,
    /// The input vault ref that was consumed (display form).
    pub input_ref: String,
    /// Content address of the input bytes as served by the vault.
    pub input_sha256: String,
    /// Declared output name.
    pub output_name: String,
    /// Output routing format (`gltf`, `oep-mesh`).
    pub output_format: String,
    /// Content address of the artifact bytes (client-side).
    pub artifact_sha256: String,
    /// Content address the vault recorded for the stored file
    /// (`FileRef.blob_sha256`). Must equal [`JobOutcome::artifact_sha256`]
    /// for status [`JobStatus::Ok`].
    pub vault_blob_sha256: String,
    /// Vault item the output was stored under.
    pub output_item: String,
    /// Revision number the output was stored as (server-assigned).
    pub output_revision: String,
    /// Outcome status.
    pub status: JobStatus,
    /// Human note (artifact summary for ok, skip reason for skip).
    pub note: String,
}

/// Typed, non-panicking worker failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerError {
    /// The spec failed structural validation (every violation joined).
    InvalidSpec(String),
    /// The job did not declare exactly one input (the convert lane contract).
    InputRefShape(String),
    /// The declared input ref kind cannot be resolved by this lane yet.
    UnsupportedInputRef(String),
    /// The vault could not be reached or the item/revision was not found.
    Vault(String),
    /// The executor names a kind/function this lane does not own.
    UnknownWorker(String),
    /// The underlying convert function failed (internal/invariant class, not
    /// a classified skip).
    Convert(ConvertError),
    /// The artifact sha-256 did not match the declared expected_sha256.
    OutputHashMismatch {
        /// The declared output name.
        output: String,
        /// The declared `expected_sha256`.
        expected: String,
        /// The computed artifact sha-256.
        actual: String,
    },
    /// The vault's recorded content address did not match the artifact's.
    VaultHashMismatch {
        /// The declared output name.
        output: String,
        /// The vault `FileRef.blob_sha256`.
        vault: String,
        /// The artifact sha-256.
        actual: String,
    },
    /// The worker emits exactly one artifact; the job declared a shape this
    /// lane does not honor.
    OutputShape(String),
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerError::InvalidSpec(m) => write!(f, "invalid job spec: {m}"),
            WorkerError::InputRefShape(m) => write!(f, "input ref shape: {m}"),
            WorkerError::UnsupportedInputRef(m) => write!(f, "unsupported input ref: {m}"),
            WorkerError::Vault(m) => write!(f, "vault: {m}"),
            WorkerError::UnknownWorker(m) => write!(f, "unknown worker: {m}"),
            WorkerError::Convert(e) => write!(f, "convert: {e}"),
            WorkerError::OutputHashMismatch {
                output,
                expected,
                actual,
            } => write!(
                f,
                "output '{output}' sha256 mismatch: expected {expected}, got {actual}"
            ),
            WorkerError::VaultHashMismatch {
                output,
                vault,
                actual,
            } => write!(
                f,
                "output '{output}' vault content address {vault} != artifact {actual}"
            ),
            WorkerError::OutputShape(m) => write!(f, "output shape: {m}"),
        }
    }
}

impl std::error::Error for WorkerError {}

/// Human display form of a vault input ref.
pub fn describe_input(reference: &VaultObjectRef) -> String {
    match reference {
        VaultObjectRef::Item { item_id } => format!("item={item_id}"),
        VaultObjectRef::Revision { item_id, revision } => {
            format!("revision={item_id}#{revision}")
        }
        VaultObjectRef::File { blob_sha256 } => format!("file={blob_sha256}"),
    }
}

/// Execute one convert job against the live vault.
///
/// # Contract
///
/// - The spec must be structurally valid ([`JobSpec::validate`]).
/// - Exactly one input ref and exactly one required output ref (the convert
///   lane emits one artifact per job).
/// - `item`/`revision` input refs resolve through the adapter's open verbs;
///   `file` (blob) refs are a scheduler-preflight concern until the P8 vault
///   exposes an item-less blob vertex.
/// - Outputs are stored by target: `attach_item` and `new_item` (`part`)
///   land through the adapter's save verb; `blob` outputs land under a
///   deterministic output-catalog item because the v1 REST contract has no
///   item-less blob vertex yet (documented mapping).
/// - Success requires the vault's recorded `FileRef.blob_sha256` to equal the
///   artifact sha-256, and (when declared) the artifact sha-256 to equal the
///   output's `expected_sha256`.
pub fn run_job(
    spec: &JobSpec,
    vault: &mut PdmAdapter,
    job_id: &str,
) -> Result<JobOutcome, WorkerError> {
    let violations = spec.validate();
    if !violations.is_empty() {
        let joined = violations
            .iter()
            .map(|v| format!("{}: {}", v.code, v.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(WorkerError::InvalidSpec(joined));
    }
    if spec.input.len() != 1 {
        return Err(WorkerError::InputRefShape(
            "the convert lane requires exactly one input ref per job".into(),
        ));
    }
    if spec.output.len() != 1 {
        return Err(WorkerError::OutputShape(
            "the convert lane emits exactly one artifact per job (P9-M3-T02)".into(),
        ));
    }
    let input = &spec.input[0];
    let output = &spec.output[0];
    let job_name = spec
        .name
        .clone()
        .unwrap_or_else(|| spec.executor.function.clone());

    let kind =
        registry::resolve(&spec.executor.kind, &spec.executor.function).ok_or_else(|| {
            WorkerError::UnknownWorker(format!(
                "{}:{} (this lane owns convert/cgr2gltf and convert/step2mesh)",
                spec.executor.kind, spec.executor.function
            ))
        })?;

    // Resolve the input bytes through the vault (the P8-M6-T02 adapter seam).
    // `source_name` is the canonical artifact naming basis: the vault-stored
    // file name, so the same logical file yields the same content address.
    let (input_ref, source_name, input_bytes, input_sha) = match &input.reference {
        VaultObjectRef::Item { item_id } => {
            let f = vault
                .open_from_pdm(item_id)
                .map_err(|e| WorkerError::Vault(e.to_string()))?;
            (
                describe_input(&input.reference),
                f.filename,
                f.bytes,
                f.blob_sha256,
            )
        }
        VaultObjectRef::Revision { item_id, revision } => {
            let f = vault
                .open_from_pdm_at(item_id, revision)
                .map_err(|e| WorkerError::Vault(e.to_string()))?;
            (
                describe_input(&input.reference),
                f.filename,
                f.bytes,
                f.blob_sha256,
            )
        }
        VaultObjectRef::File { blob_sha256 } => {
            return Err(WorkerError::UnsupportedInputRef(format!(
                "file/blob ref {} resolves through the scheduler preflight (P9-M2); the \
                 P8-M6 adapter has no item-less blob GET vertex yet",
                blob_sha256
            )));
        }
    };

    // Run the convert function (P2 oep-cgr or P4 oep-step, by registry).
    let artifact = match crate::convert::run(kind, &input_bytes, &source_name) {
        Ok(a) => a,
        Err(ConvertError::ClassifiedSkip(n))
        | Err(ConvertError::EmptyMesh(n))
        | Err(ConvertError::StepParse(n)) => {
            return Ok(skipped_outcome(
                spec,
                &job_name,
                &input_ref,
                &input_sha,
                output,
                JobStatus::Skipped(n.clone()),
                format!("classified skip: {n}"),
            ));
        }
        Err(ConvertError::NotCgr) => {
            return Ok(skipped_outcome(
                spec,
                &job_name,
                &input_ref,
                &input_sha,
                output,
                JobStatus::Skipped("bytes carry no CGR container this lane reads".into()),
                "not a CGR container this lane reads".into(),
            ));
        }
        Err(ConvertError::NotStep) => {
            return Ok(skipped_outcome(
                spec,
                &job_name,
                &input_ref,
                &input_sha,
                output,
                JobStatus::Skipped("bytes carry no STEP physical file".into()),
                "not a STEP physical file".into(),
            ));
        }
        Err(e) => return Err(WorkerError::Convert(e)),
    };

    // Declared expected_sha256 (P9-M5-T02 gate), when present.
    if let Some(expected) = &output.expected_sha256 {
        if *expected != artifact.sha256 {
            return Err(WorkerError::OutputHashMismatch {
                output: output.name.clone(),
                expected: expected.clone(),
                actual: artifact.sha256.clone(),
            });
        }
    }

    // Store the artifact at the declared target and read the vault's own
    // content address back.
    let (output_item, output_revision, vault_blob_sha256) =
        match output.target.clone().unwrap_or(OutputTarget::Blob {}) {
            OutputTarget::AttachItem { item_id } => store(
                vault,
                &item_id,
                &artifact,
                &format!("{job_id} output: {}", output.name),
            )
            .map_err(WorkerError::Vault)?,
            OutputTarget::NewItem {
                item_name,
                item_kind,
                ..
            } => match item_kind.as_deref() {
                None | Some("part") => store(
                    vault,
                    &item_name,
                    &artifact,
                    &format!("{job_id} output: {}", output.name),
                )
                .map_err(WorkerError::Vault)?,
                Some(kind) => {
                    return Err(WorkerError::OutputShape(format!(
                        "new_item kind '{kind}' is not auto-creatable through the P8-M6 \
                         adapter (part-only auto-create); use item_kind 'part' or attach_item"
                    )));
                }
            },
            OutputTarget::Blob {} => {
                // No item-less blob vertex in the v1 contract yet; land under
                // the deterministic output-catalog item (documented mapping).
                store(
                    vault,
                    BLOB_CATALOG_ITEM,
                    &artifact,
                    &format!("{job_id} blob output: {}", output.name),
                )
                .map_err(WorkerError::Vault)?
            }
        };

    // Hash-verify: the vault's recorded content address must equal the
    // artifact's (this is the P9-M5-T02 GA-gate assertion, live).
    if vault_blob_sha256 != artifact.sha256 {
        return Err(WorkerError::VaultHashMismatch {
            output: output.name.clone(),
            vault: vault_blob_sha256,
            actual: artifact.sha256.clone(),
        });
    }

    Ok(JobOutcome {
        job_name,
        executor_kind: spec.executor.kind.clone(),
        executor_function: spec.executor.function.clone(),
        input_ref,
        input_sha256: input_sha,
        output_name: output.name.clone(),
        output_format: output.format.clone(),
        artifact_sha256: artifact.sha256.clone(),
        vault_blob_sha256,
        output_item,
        output_revision,
        status: JobStatus::Ok,
        note: artifact.summary.clone(),
    })
}

/// A typed skip outcome (shared shape across the skip classes).
fn skipped_outcome(
    spec: &JobSpec,
    job_name: &str,
    input_ref: &str,
    input_sha: &str,
    output: &oep_job::spec::OutputRef,
    status: JobStatus,
    note: String,
) -> JobOutcome {
    JobOutcome {
        job_name: job_name.to_string(),
        executor_kind: spec.executor.kind.clone(),
        executor_function: spec.executor.function.clone(),
        input_ref: input_ref.to_string(),
        input_sha256: input_sha.to_string(),
        output_name: output.name.clone(),
        output_format: output.format.clone(),
        artifact_sha256: String::new(),
        vault_blob_sha256: String::new(),
        output_item: String::new(),
        output_revision: String::new(),
        status,
        note,
    }
}

/// Store an artifact as the next revision of `item` through the adapter.
fn store(
    vault: &mut PdmAdapter,
    item: &str,
    artifact: &Artifact,
    summary: &str,
) -> Result<(String, String, String), String> {
    let saved = vault
        .save_revisions_to_pdm(
            item,
            &artifact.filename,
            &artifact.format,
            artifact.bytes.clone(),
            summary,
        )
        .map_err(|e| e.to_string())?;
    Ok((saved.item_id, saved.revision, saved.blob_sha256))
}
