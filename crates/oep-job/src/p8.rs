//! The P8-M6-T01 REST v1 vertex contract: every job ref maps onto a resource.
//!
//! P9-M1-T01 is designed so **P8 object refs are the input currency** (the
//! card's contract note points at P8-M6-T01, the REST API v1 vertex). This
//! module nails that seam: each [`VaultObjectRef`] a job declares corresponds
//! to exactly one resource of the P8-M6-T01 OpenAPI v1 contract (`workstreams/P8-M6/
//! openapi-v1.yaml`, base `/api/v1`), and each output [`OutputTarget`]
//! corresponds to the vault write the GA gate verifies.
//!
//! The scheduler (P9-M2-T01) uses [`JobSpec::p8_preflight`] to enumerate the
//! resources a job needs before enqueue; the GA gate (P9-M5-T02) uses it to
//! know which vault objects to verify afterwards. Neither re-implements the
//! mapping — this module is the single source, and its tests pin each ref
//! kind to its documented REST resource.

use crate::spec::{JobSpec, OutputTarget, VaultObjectRef};

/// A P8-M6-T01 REST v1 resource a job needs, addressed relative to `/api/v1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum P8RestResource {
    /// `GET /api/v1/health`.
    Health,
    /// `GET /api/v1/items/{item_id}`.
    Item(String),
    /// `GET /api/v1/items/{item_id}/revisions/{rev}`.
    Revision(String, String),
    /// `GET /api/v1/items/{item_id}/revisions/{rev}/file`.
    RevisionFile(String, String),
    /// `GET /api/v1/items/{item_id}/file/latest`.
    ItemLatestFile(String),
    /// A content-addressed blob, verified by re-hash (the `X-Blob-Sha256`
    /// header on file responses / the blob store `GET <root>/blobs/<sha256>`).
    Blob(String),
}

impl P8RestResource {
    /// The path this resource is addressed at, relative to the `/api/v1`
    /// base path of the P8-M6-T01 contract.
    pub fn path(&self) -> String {
        match self {
            P8RestResource::Health => "/health".into(),
            P8RestResource::Item(id) => format!("/items/{}", id),
            P8RestResource::Revision(id, rev) => format!("/items/{}/revisions/{}", id, rev),
            P8RestResource::RevisionFile(id, rev) => {
                format!("/items/{}/revisions/{}/file", id, rev)
            }
            P8RestResource::ItemLatestFile(id) => format!("/items/{}/file/latest", id),
            P8RestResource::Blob(_) => "<blob-store>".into(),
        }
    }
}

/// The exact REST v1 resource that serves the bytes/state of a vault ref.
pub fn ref_resource(reference: &VaultObjectRef) -> P8RestResource {
    match reference {
        VaultObjectRef::Item { item_id } => P8RestResource::Item(item_id.clone()),
        VaultObjectRef::Revision { item_id, revision } => {
            P8RestResource::Revision(item_id.clone(), revision.clone())
        }
        VaultObjectRef::File { blob_sha256 } => P8RestResource::Blob(blob_sha256.clone()),
    }
}

/// The vault resource the GA gate verifies an output against: a new item to be
/// created, an item to be attached to, or `None` for a bare blob (verified by
/// address only).
pub fn output_target_resource(target: &OutputTarget) -> Option<P8RestResource> {
    match target {
        OutputTarget::Blob {} => None,
        OutputTarget::NewItem { .. } => None, // no id yet; created by the run
        OutputTarget::AttachItem { item_id } => Some(P8RestResource::Item(item_id.clone())),
    }
}

impl JobSpec {
    /// Enumerate the P8-M6-T01 REST v1 resources this job requires as inputs
    /// (preflighted by the scheduler before enqueue).
    pub fn p8_input_resources(&self) -> Vec<P8RestResource> {
        self.input
            .iter()
            .map(|i| ref_resource(&i.reference))
            .collect()
    }

    /// Enumerate all P8 REST resources the job touches, inputs first, then the
    /// resources the GA gate must verify for declared outputs (deduplicated,
    /// order-preserving).
    pub fn p8_preflight(&self) -> Vec<P8RestResource> {
        let mut out: Vec<P8RestResource> = Vec::new();
        let mut seen: std::collections::HashSet<P8RestResource> = std::collections::HashSet::new();
        for r in self.p8_input_resources() {
            if seen.insert(r.clone()) {
                out.push(r);
            }
        }
        for o in &self.output {
            if let Some(r) = o.target.as_ref().and_then(output_target_resource) {
                if seen.insert(r.clone()) {
                    out.push(r);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ExecutorRef, GpuRequest, InputRef, ResourceCaps, VaultObjectRef};
    use crate::spec::{OutputRef, OutputTarget};

    const BLOB: &str = "b12d95603073a6e0035c853be4661cee615dad4465d0b5f8e0e69fd03183ea31";

    fn job() -> JobSpec {
        let mut spec = JobSpec::new(
            ExecutorRef::new("convert", "cgr2gltf"),
            ResourceCaps::conservative_default(),
        );
        spec.input.push(InputRef::new(
            "a",
            VaultObjectRef::Item {
                item_id: "part-1".into(),
            },
        ));
        spec.input.push(InputRef::new(
            "b",
            VaultObjectRef::Revision {
                item_id: "part-1".into(),
                revision: "2".into(),
            },
        ));
        spec.input.push(InputRef::new(
            "c",
            VaultObjectRef::File {
                blob_sha256: BLOB.into(),
            },
        ));
        spec.output.push(OutputRef::blob("out", "gltf"));
        spec
    }

    #[test]
    fn ref_resource_maps_to_rest_surface() {
        assert_eq!(
            ref_resource(&VaultObjectRef::Item {
                item_id: "p".into()
            })
            .path(),
            "/items/p"
        );
        assert_eq!(
            ref_resource(&VaultObjectRef::Revision {
                item_id: "p".into(),
                revision: "2".into()
            })
            .path(),
            "/items/p/revisions/2"
        );
        assert_eq!(
            ref_resource(&VaultObjectRef::File {
                blob_sha256: BLOB.into()
            })
            .path(),
            "<blob-store>"
        );
    }

    #[test]
    fn output_target_maps_to_verify_resource() {
        assert_eq!(
            output_target_resource(&OutputTarget::AttachItem {
                item_id: "part-1".into()
            }),
            Some(P8RestResource::Item("part-1".into()))
        );
        assert_eq!(output_target_resource(&OutputTarget::Blob {}), None);
        assert_eq!(
            output_target_resource(&OutputTarget::NewItem {
                item_name: "x".into(),
                item_kind: None,
                context_id: None
            }),
            None
        );
    }

    #[test]
    fn preflight_enumerates_and_dedups() {
        let spec = job();
        let res = spec.p8_preflight();
        let paths: Vec<String> = res.iter().map(|r| r.path()).collect();
        assert_eq!(
            paths,
            vec!["/items/part-1", "/items/part-1/revisions/2", "<blob-store>"]
        );
        // input item + output attach to the same item dedupe:
        let mut spec2 = job();
        spec2.output[0].target = Some(OutputTarget::AttachItem {
            item_id: "part-1".into(),
        });
        let paths2: Vec<String> = spec2.p8_preflight().iter().map(|r| r.path()).collect();
        assert_eq!(
            paths2,
            vec!["/items/part-1", "/items/part-1/revisions/2", "<blob-store>"]
        );
    }

    #[test]
    fn gpu_caps_are_cpu_fallback_ready() {
        // P9-M4-T01: a non-required GPU request must never break preflight.
        let mut spec = job();
        spec.resources.gpu = Some(GpuRequest {
            count: 1,
            required: false,
            model: None,
        });
        assert!(spec.is_valid(), "{:?}", spec.validate());
        assert_eq!(spec.p8_input_resources().len(), 3);
    }

    #[test]
    fn health_is_always_implicit() {
        // health is a service-level resource, not per-job; preflight excludes it.
        assert_eq!(P8RestResource::Health.path(), "/health");
        assert!(!job()
            .p8_preflight()
            .iter()
            .any(|r| matches!(r, P8RestResource::Health)));
    }
}
