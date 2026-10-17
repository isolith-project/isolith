//! Pure byte->artifact conversion functions for the P9 convert lane.
//!
//! Two worker functions are registered (see [`crate::registry`]):
//!
//! - `cgr2gltf` (`ConvertKind::CgrToGltf`): real CGR bytes -> binary glTF
//!   2.0 (GLB). Dogfoods P2: it runs the clean-room CGR extractors in
//!   [`oep_cgr`] (structural for old-era material, modern `V5_CFV2` decode
//!   for current material) and emits GLB through [`oep_cgr::gltf`].
//! - `step2mesh` (`ConvertKind::StepToMesh`): real STEP bytes -> a
//!   deterministic `oep-mesh` v1 JSON document (vertices / triangles /
//!   bounding box + honest source report). Dogfoods P4: it runs the
//!   vendor-neutral facet reader in [`oep_step`].
//!
//! Both functions are total and deterministic: any byte input yields a typed
//! [`Artifact`] or a classified [`ConvertError`], never a panic and never
//! fabricated geometry (P2-M1-T04 tolerant-failure doctrine carried into the
//! compute lane). Output bytes are content-addressed at construction time, so
//! the artifact's sha-256 is stable and verifiable.

use std::fmt;

/// The two built-in convert workers of the P9-M3-T02 lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertKind {
    /// `cgr2gltf`: CGR bytes -> GLB.
    CgrToGltf,
    /// `step2mesh`: STEP bytes -> `oep-mesh` v1 JSON.
    StepToMesh,
}

impl ConvertKind {
    /// Short stable key used in reports and vault file names.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConvertKind::CgrToGltf => "cgr2gltf",
            ConvertKind::StepToMesh => "step2mesh",
        }
    }
}

/// A produced artifact: bytes, identity, and the computed content address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// The produced output bytes.
    pub bytes: Vec<u8>,
    /// Vault file name (stem from the source + the worker's extension).
    pub filename: String,
    /// Routing tag the vault stores (`gltf`, `oep-mesh`).
    pub format: String,
    /// Media type for the vault file record.
    pub content_type: String,
    /// `sha256(artifact.bytes)` hex content address, computed at build time.
    pub sha256: String,
    /// Distinct vertex count of the underlying mesh (honest measure).
    pub vertex_count: usize,
    /// Triangle count of the underlying mesh (honest measure).
    pub triangle_count: usize,
    /// One-line summary of what was produced.
    pub summary: String,
}

impl Artifact {
    /// Build an artifact, computing its sha-256 content address.
    pub fn build(
        bytes: Vec<u8>,
        filename: String,
        format: &str,
        content_type: &str,
        vertex_count: usize,
        triangle_count: usize,
        summary: String,
    ) -> Self {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&bytes);
        let sha256 = format!("{:x}", h.finalize());
        Artifact {
            bytes,
            filename,
            format: format.to_string(),
            content_type: content_type.to_string(),
            sha256,
            vertex_count,
            triangle_count,
            summary,
        }
    }

    /// Re-verify the content address against the stored bytes.
    pub fn verify_sha256(&self) -> bool {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&self.bytes);
        format!("{:x}", h.finalize()) == self.sha256
    }
}

/// Typed, non-panicking convert failures. Every input maps to an outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// The bytes do not carry the observed CGR container (either era).
    NotCgr,
    /// The bytes are not an ISO 10303-21 STEP physical file.
    NotStep,
    /// A valid container carried no mesh the reader maps (classified skip).
    ClassifiedSkip(String),
    /// The STEP parse itself failed (typed message from the read lane).
    StepParse(String),
    /// The extractor produced no triangles (classified skip).
    EmptyMesh(String),
    /// The extractor produced a geometry invariant violation (internal).
    GeometryInvariant(String),
    /// Output serialization / IO failure.
    Serialize(String),
    /// Not yet implemented (RED state).
    Unimplemented(String),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::NotCgr => write!(f, "not a CGR container"),
            ConvertError::NotStep => write!(f, "not an ISO 10303-21 STEP file"),
            ConvertError::ClassifiedSkip(d) => write!(f, "classified skip: {d}"),
            ConvertError::StepParse(m) => write!(f, "STEP parse failure: {m}"),
            ConvertError::EmptyMesh(m) => write!(f, "empty mesh: {m}"),
            ConvertError::GeometryInvariant(m) => write!(f, "geometry invariant: {m}"),
            ConvertError::Serialize(m) => write!(f, "serialize failure: {m}"),
            ConvertError::Unimplemented(m) => write!(f, "not implemented: {m}"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// CGR -> binary glTF 2.0 (GLB). Real bytes in, verified artifact out.
pub fn cgr_to_gltf(input: &[u8], source_name: &str) -> Result<Artifact, ConvertError> {
    crate::registry::cgr_to_gltf_impl(input, source_name)
}

/// STEP -> deterministic `oep-mesh` v1 JSON document.
pub fn step_to_mesh(input: &[u8], source_name: &str) -> Result<Artifact, ConvertError> {
    crate::registry::step_to_mesh_impl(input, source_name)
}

/// Dispatch by [`ConvertKind`] -- the seam the registry and the job worker use.
pub fn run(kind: ConvertKind, input: &[u8], source_name: &str) -> Result<Artifact, ConvertError> {
    match kind {
        ConvertKind::CgrToGltf => cgr_to_gltf(input, source_name),
        ConvertKind::StepToMesh => step_to_mesh(input, source_name),
    }
}
