//! Render-to-vault worker function: the render half of the P9-M5-T02 wave.
//!
//! Mirrors the convert lane's `oep_worker::worker::run_job` contract exactly
//! so a render job is interchangeable with a convert job in the same wave:
//! resolve the vault input through the `PdmAdapter`, run the real render
//! worker ([`oep_render_worker::worker::render_document`]), encode the image
//! as PNG, store the artifact at the declared output target in the P8 vault,
//! and hash-verify that the vault's recorded content address equals the
//! artifact sha-256 (and the declared `expected_sha256` when present).

use std::fmt;

use oep_geo::Document;
use oep_job::spec::{JobSpec, OutputTarget, VaultObjectRef};
use oep_pdm::adapter::PdmAdapter;
use oep_render_worker::RenderOptions;
use oep_viewer::render::encode_png;

/// Outcome status of one render job (mirrors the convert lane's vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderStatus {
    /// PNG produced, stored and hash-verified in the vault.
    Ok,
    /// A classified skip (e.g. input item is not a parseable oep-geo model).
    Skipped(String),
    /// The job failed (typed message).
    Failed(String),
}

/// Typed result of [`run_render_job`] -- the render peer of
/// `oep_worker::JobOutcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutcome {
    /// Job label or executor function.
    pub job_name: String,
    /// Executor kind (always `render`).
    pub executor_kind: String,
    /// Executor function (`oep-geo-render`).
    pub executor_function: String,
    /// Display form of the input vault ref.
    pub input_ref: String,
    /// Content address of the input bytes as the vault served them.
    pub input_sha256: String,
    /// Declared output name.
    pub output_name: String,
    /// Output format (`png`).
    pub output_format: String,
    /// Artifact (PNG bytes) sha-256, client-computed.
    pub artifact_sha256: String,
    /// Content address the vault recorded for the stored file.
    pub vault_blob_sha256: String,
    /// Vault item the PNG was stored under.
    pub output_item: String,
    /// Revision the PNG was stored as.
    pub output_revision: String,
    /// Status.
    pub status: RenderStatus,
    /// Human note (image summary for ok, reason for skip).
    pub note: String,
}

/// Repeated by every error path: a non-panicking typed error, mirroring
/// `oep_worker::WorkerError`'s shape (the wave treats `Err` as FAIL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The vault could not be reached or the item/revision was not found.
    Vault(String),
    /// The declared input ref kind cannot be resolved by this lane yet
    /// (`file` blob refs are a scheduler-preflight concern, as in convert).
    UnsupportedInputRef(String),
    /// Input bytes did not parse as an `oep-geo` document.
    NotGeoModel(String),
    /// Exactly one input and one required output are expected.
    Shape(String),
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
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Vault(m) => write!(f, "vault: {m}"),
            RenderError::UnsupportedInputRef(m) => write!(f, "unsupported input ref: {m}"),
            RenderError::NotGeoModel(m) => write!(f, "not an oep-geo model: {m}"),
            RenderError::Shape(m) => write!(f, "spec shape: {m}"),
            RenderError::OutputHashMismatch {
                output,
                expected,
                actual,
            } => write!(
                f,
                "output '{output}' sha256 mismatch: expected {expected}, got {actual}"
            ),
            RenderError::VaultHashMismatch {
                output,
                vault,
                actual,
            } => write!(
                f,
                "output '{output}' vault content address {vault} != artifact {actual}"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// Resolve the input ref shaped exactly like the convert lane: exactly one
/// input, `item`/`revision` refs resolve through the adapter; `file` blob
/// refs are unsupported here (same seam as convert).
fn resolve_input(
    vault: &mut PdmAdapter,
    reference: &VaultObjectRef,
) -> Result<(String, String, Vec<u8>, String), RenderError> {
    match reference {
        VaultObjectRef::Item { item_id } => {
            let f = vault
                .open_from_pdm(item_id)
                .map_err(|e| RenderError::Vault(e.to_string()))?;
            Ok((
                describe_input(reference),
                f.filename,
                f.bytes,
                f.blob_sha256,
            ))
        }
        VaultObjectRef::Revision { item_id, revision } => {
            let f = vault
                .open_from_pdm_at(item_id, revision)
                .map_err(|e| RenderError::Vault(e.to_string()))?;
            Ok((
                describe_input(reference),
                f.filename,
                f.bytes,
                f.blob_sha256,
            ))
        }
        VaultObjectRef::File { blob_sha256 } => Err(RenderError::UnsupportedInputRef(format!(
            "file/blob ref {} resolves through the scheduler preflight; the P8-M6 adapter \
             has no item-less blob GET vertex yet",
            blob_sha256
        ))),
    }
}

/// Display form of a vault input ref (matches convert's `describe_input`).
pub fn describe_input(reference: &VaultObjectRef) -> String {
    match reference {
        VaultObjectRef::Item { item_id } => format!("item={item_id}"),
        VaultObjectRef::Revision { item_id, revision } => {
            format!("revision={item_id}#{revision}")
        }
        VaultObjectRef::File { blob_sha256 } => format!("file={blob_sha256}"),
    }
}

/// Parse render parameterization from the job's `executor.args` in the P9-M1
/// shape (`--resolution WxH` / `--fidelity low|medium|high`), with the render
/// worker's documented defaults when absent.
fn render_options(args: &[String]) -> Result<RenderOptions, RenderError> {
    use oep_render_worker::options::Resolution;
    use oep_render_worker::Fidelity;
    let mut resolution = Resolution::default();
    let mut fidelity = Fidelity::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--resolution" => {
                let val = args
                    .get(i + 1)
                    .ok_or_else(|| RenderError::Shape("--resolution needs WxH".into()))?;
                resolution = val
                    .parse::<Resolution>()
                    .map_err(|e| RenderError::Shape(format!("bad --resolution {val}: {e}")))?;
                i += 2;
            }
            "--fidelity" => {
                let val = args
                    .get(i + 1)
                    .ok_or_else(|| RenderError::Shape("--fidelity needs low|medium|high".into()))?;
                fidelity = val
                    .parse::<Fidelity>()
                    .map_err(|e| RenderError::Shape(format!("bad --fidelity {val}: {e}")))?;
                i += 2;
            }
            _ => {
                return Err(RenderError::Shape(format!(
                    "unknown render executor arg: {flag}"
                )));
            }
        }
    }
    Ok(RenderOptions::new(resolution, fidelity))
}

/// Store PNG bytes as the next revision of `item` through the adapter.
fn store_png(
    vault: &mut PdmAdapter,
    item: &str,
    filename: &str,
    png: &[u8],
    summary: &str,
) -> Result<(String, String, String), RenderError> {
    let saved = vault
        .save_revisions_to_pdm(item, filename, "png", png.to_vec(), summary)
        .map_err(|e| RenderError::Vault(e.to_string()))?;
    Ok((saved.item_id, saved.revision, saved.blob_sha256))
}

/// Execute one render job against the live vault. The render peer of
/// `oep_worker::worker::run_job`; returns the typed outcome, never panics.
pub fn run_render_job(
    spec: &JobSpec,
    vault: &mut PdmAdapter,
    job_id: &str,
) -> Result<RenderOutcome, RenderError> {
    let violations = spec.validate();
    if !violations.is_empty() {
        let joined = violations
            .iter()
            .map(|v| format!("{}: {}", v.code, v.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(RenderError::Shape(joined));
    }
    if spec.input.len() != 1 {
        return Err(RenderError::Shape(
            "the render lane requires exactly one input ref per job".into(),
        ));
    }
    if spec.output.len() != 1 {
        return Err(RenderError::Shape(
            "the render lane emits exactly one artifact per job".into(),
        ));
    }
    let input = &spec.input[0];
    let output = &spec.output[0];
    let job_name = spec
        .name
        .clone()
        .unwrap_or_else(|| spec.executor.function.clone());

    let (input_ref, source_name, input_bytes, input_sha) = resolve_input(vault, &input.reference)?;

    // Parse the model; a non-model is a classified skip (valid input, nothing
    // to render), not a failure.
    let text = String::from_utf8(input_bytes)
        .map_err(|e| RenderError::NotGeoModel(format!("input is not UTF-8: {e}")))?;
    let doc = Document::from_json_pretty(&text).map_err(|e| {
        RenderError::NotGeoModel(format!("{} could not parse as oep-geo: {e}", source_name))
    })?;

    let opts = render_options(&spec.executor.args)?;
    let result = oep_render_worker::worker::render_document(&doc, opts);
    let png = encode_png(&result.image);
    let artifact_sha256 = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&png);
        format!("{:x}", h.finalize())
    };
    let prims = oep_viewer::scene::build_scene(&doc).primitives.len();

    if let Some(expected) = &output.expected_sha256 {
        if *expected != artifact_sha256 {
            return Err(RenderError::OutputHashMismatch {
                output: output.name.clone(),
                expected: expected.clone(),
                actual: artifact_sha256.clone(),
            });
        }
    }

    let (output_item, output_revision, vault_blob_sha256) =
        match output.target.clone().unwrap_or(OutputTarget::Blob {}) {
            OutputTarget::AttachItem { item_id } => store_png(
                vault,
                &item_id,
                &format!("{}.png", output.name),
                &png,
                &format!("{job_id} render output: {}", output.name),
            )?,
            OutputTarget::NewItem {
                item_name,
                item_kind,
                ..
            } => match item_kind.as_deref() {
                None | Some("part") | Some("document") => store_png(
                    vault,
                    &item_name,
                    &format!("{}.png", output.name),
                    &png,
                    &format!("{job_id} render output: {}", output.name),
                )?,
                Some(kind) => {
                    return Err(RenderError::Shape(format!(
                        "new_item kind '{kind}' is not auto-creatable through the P8-M6 \
                         adapter; use item_kind part/document or attach_item"
                    )));
                }
            },
            OutputTarget::Blob {} => {
                store_png(
                    vault,
                    // Same deterministic blob-output catalog as the convert lane.
                    oep_worker::batch::BLOB_CATALOG_ITEM,
                    &format!("{}.png", output.name),
                    &png,
                    &format!("{job_id} render blob output: {}", output.name),
                )?
            }
        };

    // The P9-M5-T02 gate contract: vault-recorded address == artifact sha-256.
    if vault_blob_sha256 != artifact_sha256 {
        return Err(RenderError::VaultHashMismatch {
            output: output.name.clone(),
            vault: vault_blob_sha256,
            actual: artifact_sha256,
        });
    }

    let note = format!(
        "rendered {}x{} fidelity={} -> {} ({} bytes, {} primitives)",
        result.image.width,
        result.image.height,
        match result.fidelity {
            oep_render_worker::Fidelity::Low => "low",
            oep_render_worker::Fidelity::Medium => "medium",
            oep_render_worker::Fidelity::High => "high",
        },
        output_item,
        png.len(),
        prims
    );

    Ok(RenderOutcome {
        job_name,
        executor_kind: spec.executor.kind.clone(),
        executor_function: spec.executor.function.clone(),
        input_ref,
        input_sha256: input_sha,
        output_name: output.name.clone(),
        output_format: output.format.clone(),
        artifact_sha256,
        vault_blob_sha256,
        output_item,
        output_revision,
        status: RenderStatus::Ok,
        note,
    })
}
