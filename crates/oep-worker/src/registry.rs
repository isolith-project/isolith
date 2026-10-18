//! P9 worker registry: resolves `executor.kind` + `executor.function` from a
//! [`oep_job::JobSpec`] to a concrete convert worker, and hosts the worker
//! implementations that dogfood the P2/P4 readers.
//!
//! The P9-M1 job spec defines a closed executor vocabulary (`render`,
//! `convert`, `import`, `export`, `validate`, `custom`) and names a worker
//! *function* within the kind. This lane owns the `convert` family:
//!
//! | kind      | function      | worker                              |
//! |-----------|---------------|-------------------------------------|
//! | `convert` | `cgr2gltf`    | [`cgr_to_gltf_impl`] (P2 oep-cgr)   |
//! | `convert` | `step2mesh`   | [`step_to_mesh_impl`] (P4 oep-step) |
//!
//! The scheduler (P9-M2) and the batch driver (P9-M3-T02) resolve through
//! [`resolve`]; both workers are deterministic byte-in/byte-out functions.

use crate::convert::{Artifact, ConvertError, ConvertKind};
use crate::mesh_doc::MeshDoc;

/// Worker function name for CGR -> glTF (the P9-M1 example vocabulary).
pub const FN_CGR2GLTF: &str = "cgr2gltf";
/// Worker function name for STEP -> mesh (the P9-M1 example vocabulary).
pub const FN_STEP2MESH: &str = "step2mesh";

/// The convert workers this lane registers, as `(kind, function, kind)`.
pub const REGISTERED: [(&str, &str, ConvertKind); 2] = [
    ("convert", FN_CGR2GLTF, ConvertKind::CgrToGltf),
    ("convert", FN_STEP2MESH, ConvertKind::StepToMesh),
];

/// Resolve a job's `executor` (`kind` + `function`) to a convert worker.
///
/// Returns `None` when the executor names a kind this lane does not own
/// (render/import/export/validate/custom live on other worker lanes) or an
/// unknown function within the `convert` family.
pub fn resolve(kind: &str, function: &str) -> Option<ConvertKind> {
    REGISTERED
        .iter()
        .find(|(k, f, _)| *k == kind && *f == function)
        .map(|(_, _, c)| *c)
}

/// The `cgr2gltf` convert worker (dogfoods `oep-cgr`, P2).
///
/// Cleans two real-material paths, in order:
/// 1. old-era structural extractor [`oep_cgr::mesh::extract`] (the 2007-era
///    sample spine the P2-M2 card latched);
/// 2. modern `V5_CFV2` spatial-mesh decode [`oep_cgr::modern::extract_modern`]
///    (NIST V5-6R2023-era material the P2-M7 latch approved).
///
/// The resulting indexed triangle mesh is emitted as binary glTF 2.0 (GLB)
/// through [`oep_cgr::gltf`]. Any byte input yields a typed outcome: a GLB
/// artifact, a classified skip (valid CGR, no tessellation the reader maps),
/// or `NotCgr` (bytes carry no observed CGR container).
pub fn cgr_to_gltf_impl(input: &[u8], source_name: &str) -> Result<Artifact, ConvertError> {
    use oep_cgr::mesh::{extract, MeshError};
    use oep_cgr::modern::{extract_modern, ModernOutcome};

    let mesh = match extract(input) {
        Ok(m) => m,
        Err(MeshError::NotCgrPreamble) => match extract_modern(input) {
            Ok((_counts, m)) => m,
            Err(ModernOutcome::NotCgrContainer) => return Err(ConvertError::NotCgr),
            Err(other) => return Err(ConvertError::ClassifiedSkip(format!("{other:?}"))),
        },
        Err(e) => return Err(ConvertError::ClassifiedSkip(e.to_string())),
    };

    if mesh.triangles().is_empty() {
        return Err(ConvertError::EmptyMesh(
            "CGR extractor returned no triangles".into(),
        ));
    }
    mesh.validate()
        .map_err(|e| ConvertError::GeometryInvariant(format!("{e}")))?;

    let mut glb = Vec::new();
    oep_cgr::gltf::write_glb(mesh.vertices(), mesh.triangles(), &mut glb)
        .map_err(|e| ConvertError::Serialize(format!("glb write: {e}")))?;

    let stem = stem_of(source_name);
    Ok(Artifact::build(
        glb,
        format!("{stem}.glb"),
        "gltf",
        "model/gltf-binary",
        mesh.vertices().len(),
        mesh.triangles().len(),
        format!(
            "CGR -> GLB: {} vertices, {} triangles",
            mesh.vertices().len(),
            mesh.triangles().len()
        ),
    ))
}

/// The `step2mesh` convert worker (dogfoods `oep-step`, P4).
///
/// Runs the vendor-neutral facet reader [`oep_step::parse`] and writes the
/// parsed faceted model (vertices / triangles / bounding box + honest source
/// report) as a deterministic `oep-mesh` v1 JSON document (see
/// [`crate::mesh_doc`]). STEP files whose geometry is analytic (no faceted
/// surface model) classify as a skip, never a partial or fabricated mesh.
pub fn step_to_mesh_impl(input: &[u8], source_name: &str) -> Result<Artifact, ConvertError> {
    use oep_step::Error as StepError;

    let out = match oep_step::parse(input) {
        Ok(out) => out,
        Err(StepError::NotStep) => return Err(ConvertError::NotStep),
        Err(StepError::NoFacetedGeometry(msg)) => {
            return Err(ConvertError::ClassifiedSkip(msg));
        }
        Err(e) => return Err(ConvertError::StepParse(e.to_string())),
    };
    let facets = &out.facets;

    if facets.triangles.is_empty() {
        return Err(ConvertError::EmptyMesh(
            "STEP read lane mapped no faceted triangles (analytic-only geometry?)".into(),
        ));
    }

    let doc = MeshDoc::from_facets(facets, source_name);
    let bytes = MeshDoc::encode(&doc)?;
    let stem = stem_of(source_name);
    Ok(Artifact::build(
        bytes,
        format!("{stem}.oepmesh.json"),
        "oep-mesh",
        "application/json",
        facets.vertices.len(),
        facets.triangles.len(),
        format!(
            "STEP -> oep-mesh: {} vertices, {} triangles, {} shells",
            facets.vertices.len(),
            facets.triangles.len(),
            facets.shells.len()
        ),
    ))
}

/// File stem of a source name (path + extension stripped). Used to derive
/// deterministic vault output file names from arbitrary input refs.
pub fn stem_of(source_name: &str) -> String {
    let base = source_name.rsplit('/').next().unwrap_or(source_name);
    match base.split_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => {
            if base.is_empty() {
                "untitled".to_string()
            } else {
                base.to_string()
            }
        }
    }
}
