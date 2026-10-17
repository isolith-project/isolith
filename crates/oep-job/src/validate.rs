//! Structural validation of a [`JobSpec`].
//!
//! [`JobSpec::validate`] mirrors the P8-M1-T02 doctrine: it returns **every**
//! violation as a `{code, message}` pair, never panics, and rejects nothing at
//! load time (tolerant-failure; a partially-specified job can still be
//! inspected). Resolving refs and caps against the *live* P8 vault / node
//! inventory is NOT this function's job: that is the scheduler's preflight
//! (P9-M2), which uses [`crate::p8`] to enumerate the P8-M6-T01 REST
//! resources each job requires. Here we validate that refs **have a legal
//! shape** (`item_id`, decimal `revision`, 64-hex `blob_sha256`, ...) and
//! caps **are within bounds**, so a malformed spec never reaches a node.

use crate::spec::{CHECKPOINT_MAX_SECONDS, CHECKPOINT_MIN_SECONDS};
use crate::spec::{
    EXECUTOR_CUSTOM, EXECUTOR_KINDS, FORMAT, ITEM_KINDS, MAX_CPU_MILLICORES, MAX_DISK_MIB,
    MAX_GPU_COUNT, MAX_MEMORY_MIB, MAX_TIMEOUT_SECONDS, VERSION,
};

/// One documented structural violation with a stable kebab-case code and a
/// human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Stable machine code, e.g. `invalid-blob-address`.
    pub code: &'static str,
    /// Human-readable description of the violation.
    pub message: String,
}

impl ValidationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        ValidationError {
            code,
            message: message.into(),
        }
    }
}

/// A blob content address is exactly [`crate::spec::BLOB_HEX_LEN`] lowercase
/// hex chars (identical rule to oep-pdm's `invalid-blob-address`).
pub fn is_blob_address(s: &str) -> bool {
    s.len() == crate::spec::BLOB_HEX_LEN
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// A revision number is a positive decimal string (`"1"`, `"2"`, ...), matching
/// the P8-M6-T01 `Rev` path parameter contract.
pub fn is_revision_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && s != "0" && !s.starts_with('0')
}

/// An opaque url-safe vault id shape (non-empty, no whitespace or path
/// metacharacters).
pub fn is_item_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
}

/// A container image tag shape: non-empty, no whitespace, only typically
/// allowed tag characters.
pub fn is_image_tag(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b':' | b'@' | b'-')
        })
}

/// A structural RFC 3339 UTC timestamp shape (YYYY-MM-DDTHH:MM:SS with a Z or
/// ±HH:MM offset). Structural only — ordering/semantics are the store's job.
pub fn is_rfc3339(s: &str) -> bool {
    let mut b = s.bytes();
    fn digits(b: &mut dyn Iterator<Item = u8>, n: usize) -> bool {
        for _ in 0..n {
            let Some(c) = b.next() else { return false };
            if !c.is_ascii_digit() {
                return false;
            }
        }
        true
    }
    digits(&mut b, 4)
        && b.next() == Some(b'-')
        && digits(&mut b, 2)
        && b.next() == Some(b'-')
        && digits(&mut b, 2)
        && b.next() == Some(b'T')
        && digits(&mut b, 2)
        && b.next() == Some(b':')
        && digits(&mut b, 2)
        && b.next() == Some(b':')
        && digits(&mut b, 2)
        && match b.next() {
            Some(b'Z') => true,
            Some(b'+') | Some(b'-') => {
                digits(&mut b, 2)
                    && b.next() == Some(b':')
                    && digits(&mut b, 2)
                    && b.next().is_none()
            }
            _ => false,
        }
}

impl crate::spec::JobSpec {
    /// Validate the whole spec; returns every violation (empty = valid).
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errs = Vec::new();
        validate_header(self, &mut errs);
        validate_inputs(self, &mut errs);
        validate_executor(self, &mut errs);
        validate_resources(self, &mut errs);
        validate_outputs(self, &mut errs);
        errs
    }

    /// `true` when [`validate`][Self::validate] reports no violations.
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

fn validate_header(spec: &crate::spec::JobSpec, errs: &mut Vec<ValidationError>) {
    if spec.format != FORMAT {
        errs.push(ValidationError::new(
            "bad-format",
            format!("expected format {:?}, got {:?}", FORMAT, spec.format),
        ));
    }
    if spec.version != VERSION {
        errs.push(ValidationError::new(
            "unsupported-version",
            format!("expected version {:?}, got {:?}", VERSION, spec.version),
        ));
    }
    if let Some(id) = &spec.job_id {
        if id.trim().is_empty() {
            errs.push(ValidationError::new(
                "invalid-job-id",
                "job_id must be a non-empty string when present",
            ));
        }
    }
    if let Some(name) = &spec.name {
        if name.trim().is_empty() {
            errs.push(ValidationError::new(
                "invalid-job-name",
                "name must be a non-empty string when present",
            ));
        }
    }
    if let Some(ts) = &spec.created_at {
        if !is_rfc3339(ts) {
            errs.push(ValidationError::new(
                "invalid-timestamp",
                format!("created_at {:?} is not RFC 3339 (structural) UTC", ts),
            ));
        }
    }
}

fn validate_inputs(spec: &crate::spec::JobSpec, errs: &mut Vec<ValidationError>) {
    let mut seen = std::collections::HashSet::new();
    for input in &spec.input {
        if input.name.trim().is_empty() {
            errs.push(ValidationError::new(
                "invalid-input-name",
                "an input named empty is not addressable by a worker",
            ));
        } else if !seen.insert(input.name.clone()) {
            errs.push(ValidationError::new(
                "duplicate-input-name",
                format!("input name {:?} is declared more than once", input.name),
            ));
        }
        match &input.reference {
            crate::spec::VaultObjectRef::Item { item_id } => {
                if !is_item_id(item_id) {
                    errs.push(ValidationError::new(
                        "invalid-item-id",
                        format!(
                            "input {:?}: item_id {:?} is not an opaque url-safe id",
                            input.name, item_id
                        ),
                    ));
                }
            }
            crate::spec::VaultObjectRef::Revision { item_id, revision } => {
                if !is_item_id(item_id) {
                    errs.push(ValidationError::new(
                        "invalid-item-id",
                        format!(
                            "input {:?}: item_id {:?} is not an opaque url-safe id",
                            input.name, item_id
                        ),
                    ));
                }
                if !is_revision_number(revision) {
                    errs.push(ValidationError::new(
                        "invalid-revision",
                        format!(
                            "input {:?}: revision {:?} is not a positive decimal number",
                            input.name, revision
                        ),
                    ));
                }
            }
            crate::spec::VaultObjectRef::File { blob_sha256 } => {
                if !is_blob_address(blob_sha256) {
                    errs.push(ValidationError::new(
                        "invalid-blob-address",
                        format!(
                            "input {:?}: blob_sha256 is not 64 lowercase hex",
                            input.name
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_executor(spec: &crate::spec::JobSpec, errs: &mut Vec<ValidationError>) {
    let ex = &spec.executor;
    if !EXECUTOR_KINDS.contains(&ex.kind.as_str()) {
        errs.push(ValidationError::new(
            "unknown-executor-kind",
            format!("executor kind {:?} not in {:?}", ex.kind, EXECUTOR_KINDS),
        ));
    }
    if ex.function.trim().is_empty() {
        errs.push(ValidationError::new(
            "empty-executor-function",
            "executor.function must name a worker function (e.g. oep-geo-render)",
        ));
    }
    if ex.kind == EXECUTOR_CUSTOM && ex.image.is_none() {
        errs.push(ValidationError::new(
            "custom-executor-needs-image",
            "custom executors must carry an image (no built-in worker function)",
        ));
    }
    if let Some(image) = &ex.image {
        if !is_image_tag(image) {
            errs.push(ValidationError::new(
                "invalid-image-tag",
                format!("executor image {:?} has an illegal tag shape", image),
            ));
        }
    }
}

fn validate_resources(spec: &crate::spec::JobSpec, errs: &mut Vec<ValidationError>) {
    let r = &spec.resources;
    if !(1..=MAX_CPU_MILLICORES).contains(&r.cpu_millicores) {
        errs.push(ValidationError::new(
            "invalid-cpu-caps",
            format!(
                "cpu_millicores {} outside 1..={}",
                r.cpu_millicores, MAX_CPU_MILLICORES
            ),
        ));
    }
    if !(1..=MAX_MEMORY_MIB).contains(&r.memory_mib) {
        errs.push(ValidationError::new(
            "invalid-memory-caps",
            format!("memory_mib {} outside 1..={}", r.memory_mib, MAX_MEMORY_MIB),
        ));
    }
    if let Some(disk) = r.disk_mib {
        if !(1..=MAX_DISK_MIB).contains(&disk) {
            errs.push(ValidationError::new(
                "invalid-disk-caps",
                format!("disk_mib {disk} outside 1..={MAX_DISK_MIB}"),
            ));
        }
    }
    if let Some(gpu) = &r.gpu {
        if !(1..=MAX_GPU_COUNT).contains(&gpu.count) {
            errs.push(ValidationError::new(
                "invalid-gpu-request",
                format!("gpu.count {} outside 1..={}", gpu.count, MAX_GPU_COUNT),
            ));
        }
        if let Some(model) = &gpu.model {
            if model.trim().is_empty() {
                errs.push(ValidationError::new(
                    "invalid-gpu-request",
                    "gpu.model must be a non-empty string when present",
                ));
            }
        }
    }
    if let Some(t) = r.timeout_seconds {
        if !(1..=MAX_TIMEOUT_SECONDS).contains(&t) {
            errs.push(ValidationError::new(
                "invalid-timeout",
                format!("timeout_seconds {t} outside 1..={MAX_TIMEOUT_SECONDS}"),
            ));
        }
    }
    if let Some(c) = r.checkpoint_every_seconds {
        if !(CHECKPOINT_MIN_SECONDS..=CHECKPOINT_MAX_SECONDS).contains(&c) {
            errs.push(ValidationError::new(
                "invalid-checkpoint-interval",
                format!(
                    "checkpoint_every_seconds {c} outside {}..={}",
                    CHECKPOINT_MIN_SECONDS, CHECKPOINT_MAX_SECONDS
                ),
            ));
        }
    }
}

fn validate_outputs(spec: &crate::spec::JobSpec, errs: &mut Vec<ValidationError>) {
    let mut seen = std::collections::HashSet::new();
    for out in &spec.output {
        if out.name.trim().is_empty() {
            errs.push(ValidationError::new(
                "invalid-output-name",
                "an output named empty is not addressable by the GA gate",
            ));
        } else if !seen.insert(out.name.clone()) {
            errs.push(ValidationError::new(
                "duplicate-output-name",
                format!("output name {:?} is declared more than once", out.name),
            ));
        }
        if out.format.trim().is_empty() {
            errs.push(ValidationError::new(
                "empty-output-format",
                format!("output {:?} must declare a format tag", out.name),
            ));
        }
        if let Some(sha) = &out.expected_sha256 {
            if !is_blob_address(sha) {
                errs.push(ValidationError::new(
                    "invalid-blob-address",
                    format!(
                        "output {:?}: expected_sha256 is not 64 lowercase hex",
                        out.name
                    ),
                ));
            }
        }
        match &out.target {
            Some(crate::spec::OutputTarget::Blob {}) => {}
            Some(crate::spec::OutputTarget::NewItem {
                item_name,
                item_kind,
                context_id,
            }) => {
                if item_name.trim().is_empty() {
                    errs.push(ValidationError::new(
                        "new-item-needs-name",
                        format!("output {:?}: new_item target needs item_name", out.name),
                    ));
                }
                if let Some(kind) = item_kind {
                    if !ITEM_KINDS.contains(&kind.as_str()) {
                        errs.push(ValidationError::new(
                            "invalid-item-kind",
                            format!(
                                "output {:?}: item_kind {:?} not in {:?}",
                                out.name, kind, ITEM_KINDS
                            ),
                        ));
                    }
                }
                if let Some(ctx) = context_id {
                    if !is_item_id(ctx) {
                        errs.push(ValidationError::new(
                            "invalid-context-id",
                            format!(
                                "output {:?}: context_id {:?} is not a url-safe id",
                                out.name, ctx
                            ),
                        ));
                    }
                }
            }
            Some(crate::spec::OutputTarget::AttachItem { item_id }) if !is_item_id(item_id) => {
                errs.push(ValidationError::new(
                    "attach-item-needs-item-id",
                    format!(
                        "output {:?}: attach_item target needs a valid item_id",
                        out.name
                    ),
                ));
            }
            Some(crate::spec::OutputTarget::AttachItem { .. }) => {}
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{
        ExecutorRef, GpuRequest, InputRef, JobSpec, OutputRef, OutputTarget, ResourceCaps,
        VaultObjectRef, EXECUTOR_CONVERT, EXECUTOR_CUSTOM,
    };
    use crate::spec::{TARGET_ATTACH_ITEM, TARGET_BLOB, TARGET_NEW_ITEM};

    const BLOB: &str = "b12d95603073a6e0035c853be4661cee615dad4465d0b5f8e0e69fd03183ea31";
    const GOOD: &str = "ebb33faa9835e5daf63c81ee9eb146f9619329d62b42b96a8b7b98fd5ed21883";
    const ITEM: &str = "cowl-panel";

    fn base() -> ResourceCaps {
        ResourceCaps::conservative_default()
    }

    fn render_executor() -> ExecutorRef {
        ExecutorRef::new("render", "oep-geo-render")
    }

    fn ok_spec() -> JobSpec {
        let mut spec = JobSpec::new(render_executor(), base());
        spec.input.push(InputRef::new(
            "mesh",
            VaultObjectRef::Revision {
                item_id: ITEM.into(),
                revision: "3".into(),
            },
        ));
        spec.output.push(OutputRef {
            name: "thumbnail".into(),
            format: "png".into(),
            content_type: Some("image/png".into()),
            target: Some(OutputTarget::Blob {}),
            expected_sha256: None,
            required: true,
        });
        spec
    }

    fn codes(spec: &JobSpec) -> Vec<&'static str> {
        let mut v: Vec<_> = spec.validate().iter().map(|e| e.code).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn header_ok() {
        let spec = ok_spec();
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }

    #[test]
    fn rejects_bad_format() {
        let mut spec = ok_spec();
        spec.format = "oep-vault".into();
        assert_eq!(codes(&spec), vec!["bad-format"]);
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut spec = ok_spec();
        spec.version = 2;
        assert_eq!(codes(&spec), vec!["unsupported-version"]);
    }

    #[test]
    fn rejects_empty_job_id_and_bad_timestamp() {
        let mut spec = ok_spec();
        spec.job_id = Some("  ".into());
        spec.created_at = Some("not-a-date".into());
        let c = codes(&spec);
        assert!(c.contains(&"invalid-job-id"));
        assert!(c.contains(&"invalid-timestamp"));
    }

    #[test]
    fn rejects_bad_item_id() {
        // item ref with a slash is not url-safe
        let mut spec = ok_spec();
        spec.input[0].reference = VaultObjectRef::Item {
            item_id: "../etc/passwd".into(),
        };
        assert_eq!(codes(&spec), vec!["invalid-item-id"]);
    }

    #[test]
    fn rejects_bad_revision() {
        let mut spec = ok_spec();
        spec.input[0].reference = VaultObjectRef::Revision {
            item_id: ITEM.into(),
            revision: "0".into(),
        };
        assert_eq!(codes(&spec), vec!["invalid-revision"]);
        let mut spec = ok_spec();
        spec.input[0].reference = VaultObjectRef::Revision {
            item_id: ITEM.into(),
            revision: "v3".into(),
        };
        assert_eq!(codes(&spec), vec!["invalid-revision"]);
    }

    #[test]
    fn rejects_bad_blob_address() {
        let mut spec = ok_spec();
        spec.input[0].reference = VaultObjectRef::File {
            blob_sha256: "nothex".into(),
        };
        assert_eq!(codes(&spec), vec!["invalid-blob-address"]);
        // uppercase hex is rejected
        let mut spec = ok_spec();
        spec.input[0].reference = VaultObjectRef::File {
            blob_sha256: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
        };
        assert_eq!(codes(&spec), vec!["invalid-blob-address"]);
    }

    #[test]
    fn rejects_duplicate_input_names() {
        let mut spec = ok_spec();
        spec.input.push(InputRef::new(
            "mesh",
            VaultObjectRef::Item {
                item_id: "other".into(),
            },
        ));
        assert_eq!(codes(&spec), vec!["duplicate-input-name"]);
    }

    #[test]
    fn rejects_unknown_executor_kind() {
        let mut spec = ok_spec();
        spec.executor.kind = "quantum".into();
        assert_eq!(codes(&spec), vec!["unknown-executor-kind"]);
    }

    #[test]
    fn rejects_empty_function() {
        let mut spec = ok_spec();
        spec.executor.function = "  ".into();
        assert_eq!(codes(&spec), vec!["empty-executor-function"]);
    }

    #[test]
    fn custom_executor_requires_image() {
        let mut spec = ok_spec();
        spec.executor.kind = EXECUTOR_CUSTOM.into();
        spec.executor.image = None;
        assert_eq!(codes(&spec), vec!["custom-executor-needs-image"]);
    }

    #[test]
    fn custom_executor_with_image_is_fine() {
        let mut spec = ok_spec();
        spec.executor.kind = EXECUTOR_CUSTOM.into();
        spec.executor.image = Some("oep-workers/custom:v1".into());
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }

    #[test]
    fn rejects_bad_image_tag() {
        let mut spec = ok_spec();
        spec.executor.image = Some("has space".into());
        assert_eq!(codes(&spec), vec!["invalid-image-tag"]);
    }

    #[test]
    fn rejects_zero_cpu() {
        let mut spec = ok_spec();
        spec.resources.cpu_millicores = 0;
        assert_eq!(codes(&spec), vec!["invalid-cpu-caps"]);
    }

    #[test]
    fn rejects_over_capped_memory() {
        let mut spec = ok_spec();
        spec.resources.memory_mib = MAX_MEMORY_MIB + 1;
        assert_eq!(codes(&spec), vec!["invalid-memory-caps"]);
    }

    #[test]
    fn rejects_zero_disk() {
        let mut spec = ok_spec();
        spec.resources.disk_mib = Some(0);
        assert_eq!(codes(&spec), vec!["invalid-disk-caps"]);
    }

    #[test]
    fn gpu_request_bounds() {
        let mut spec = ok_spec();
        spec.resources.gpu = Some(GpuRequest {
            count: 0,
            required: true,
            model: None,
        });
        assert_eq!(codes(&spec), vec!["invalid-gpu-request"]);
        let mut spec = ok_spec();
        spec.resources.gpu = Some(GpuRequest {
            count: 1,
            required: false,
            model: Some("  ".into()),
        });
        assert_eq!(codes(&spec), vec!["invalid-gpu-request"]);
    }

    #[test]
    fn gpu_required_false_falls_back_to_cpu() {
        let mut spec = ok_spec();
        spec.resources.gpu = Some(GpuRequest {
            count: 2,
            required: false,
            model: Some("dgx".into()),
        });
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }

    #[test]
    fn timeout_bounds() {
        let mut spec = ok_spec();
        spec.resources.timeout_seconds = Some(0);
        assert_eq!(codes(&spec), vec!["invalid-timeout"]);
        let mut spec = ok_spec();
        spec.resources.timeout_seconds = Some(MAX_TIMEOUT_SECONDS + 1);
        assert_eq!(codes(&spec), vec!["invalid-timeout"]);
    }

    #[test]
    fn checkpoint_bounds() {
        let mut spec = ok_spec();
        spec.resources.checkpoint_every_seconds = Some(1);
        assert_eq!(codes(&spec), vec!["invalid-checkpoint-interval"]);
        let mut spec = ok_spec();
        spec.resources.checkpoint_every_seconds = Some(300);
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }

    #[test]
    fn rejects_duplicate_output_names() {
        let mut spec = ok_spec();
        spec.output.push(OutputRef::blob("thumbnail", "png"));
        assert_eq!(codes(&spec), vec!["duplicate-output-name"]);
    }

    #[test]
    fn rejects_empty_output_format() {
        let mut spec = ok_spec();
        spec.output[0].format = "".into();
        assert_eq!(codes(&spec), vec!["empty-output-format"]);
    }

    #[test]
    fn rejects_bad_expected_sha256() {
        let mut spec = ok_spec();
        spec.output[0].expected_sha256 = Some("zz".into());
        assert_eq!(codes(&spec), vec!["invalid-blob-address"]);
    }

    #[test]
    fn rejects_new_item_without_name() {
        let mut spec = ok_spec();
        spec.output[0].target = Some(OutputTarget::NewItem {
            item_name: " ".into(),
            item_kind: None,
            context_id: None,
        });
        assert_eq!(codes(&spec), vec!["new-item-needs-name"]);
    }

    #[test]
    fn rejects_bad_item_kind_on_new_item() {
        let mut spec = ok_spec();
        spec.output[0].target = Some(OutputTarget::NewItem {
            item_name: "cowl-render".into(),
            item_kind: Some("gadget".into()),
            context_id: None,
        });
        assert_eq!(codes(&spec), vec!["invalid-item-kind"]);
    }

    #[test]
    fn rejects_bad_attach_item_id() {
        let mut spec = ok_spec();
        spec.output[0].target = Some(OutputTarget::AttachItem {
            item_id: "no spaces here".into(),
        });
        assert_eq!(codes(&spec), vec!["attach-item-needs-item-id"]);
    }

    #[test]
    fn all_target_kinds_accepted_when_valid() {
        for (target, kind) in [
            (OutputTarget::Blob {}, TARGET_BLOB),
            (
                OutputTarget::NewItem {
                    item_name: "rendered".into(),
                    item_kind: Some("document".into()),
                    context_id: Some("ctx-1".into()),
                },
                TARGET_NEW_ITEM,
            ),
            (
                OutputTarget::AttachItem {
                    item_id: ITEM.into(),
                },
                TARGET_ATTACH_ITEM,
            ),
        ] {
            let mut spec = ok_spec();
            spec.output[0].target = Some(target);
            assert!(spec.is_valid(), "kind={kind}: {:?}", spec.validate());
        }
    }

    #[test]
    fn empty_inputs_is_valid_for_generate_only_jobs() {
        let spec = JobSpec::new(render_executor(), base());
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }

    #[test]
    fn helper_predicates() {
        assert!(is_blob_address(BLOB));
        assert!(!is_blob_address(""));
        assert!(!is_blob_address(&BLOB[..BLOB.len() - 1]));
        assert!(is_revision_number("1"));
        assert!(is_revision_number("42"));
        assert!(!is_revision_number("0"));
        assert!(!is_revision_number("01"));
        assert!(!is_revision_number("v2"));
        assert!(is_item_id("cowl-panel_1.2~3-4"));
        assert!(!is_item_id(""));
        assert!(!is_item_id("a/b"));
        assert!(is_image_tag("oep-workers/render:v1.2"));
        assert!(is_image_tag("deb@sha256:abc"));
        assert!(!is_image_tag("has space"));
        assert!(is_rfc3339("2026-10-17T00:00:00Z"));
        assert!(is_rfc3339("2026-10-17T00:00:00+02:00"));
        assert!(!is_rfc3339("2026/10/17"));
        // shape-checked only: separators and digit counts, not calendar range
        assert!(is_rfc3339("2026-13-40T99:99:99Z"));
        assert!(!is_rfc3339("2026-10-17 00:00:00Z")); // space, not T
        assert!(!is_rfc3339("26-1-7T0:0:0Z")); // short digits
    }

    #[test]
    fn convert_executor_with_file_input_is_valid() {
        let mut spec = JobSpec::new(ExecutorRef::new(EXECUTOR_CONVERT, "cgr2gltf"), base());
        spec.input.push(InputRef::new(
            "source",
            VaultObjectRef::File {
                blob_sha256: GOOD.into(),
            },
        ));
        spec.output.push(OutputRef::blob("out", "gltf"));
        assert!(spec.is_valid(), "{:?}", spec.validate());
    }
}
