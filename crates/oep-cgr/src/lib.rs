//! P2 — CGR reader (workstream P2, geometry/tessellation).
//!
//! Clean-room reader for CATIA Graphics Representation (`.cgr`) files:
//! an indexed-triangle internal mesh model ([`mesh`]) extracted from the
//! real bytes by structural validation (P2-M1-T04 tolerant-failure
//! doctrine — never fabricates geometry, always a typed outcome), plus
//! exporters on that mesh: glTF 2.0 / GLB ([`gltf`]).
//!
//! P9-M3-T02 (compute lane) dogfoods this crate directly: the `cgr2gltf`
//! convert worker feeds `oep_cgr::mesh::extract` + `oep_cgr::gltf::write_glb`
//! so P2 output becomes P9 input without a second, bespoke reader.
#![deny(missing_docs)]

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P2-CGR";

/// Internal indexed-triangle mesh model and the clean-room CGR extractor
/// (P2-M2-T01).
pub mod mesh;

/// glTF 2.0 / GLB mesh export (P2-M2-T03).
pub mod gltf;

/// Modern-era (V5-6R2015+ / 3DEXPERIENCE) embedded CGR visualization decode
/// (P2-M7-T01) — real spatial mesh extraction from the documented `V5_CFV2`
/// grammar. Needed because the P2-M2 structural extractor only latches the
/// old-era (2007-era) sample spine; modern material (e.g. NIST V5-6R2023
/// STC-06) decodes through [`modern::extract_modern`].
pub mod modern;

/// Clean-room byte-level CGR header spine parser (P2-M1-T01).
pub mod header;

/// Real-material modern-era CGR locate latch (P2-M7-T01, P7-M2-T01 importer).
pub mod locate;

/// Central table-of-blocks walker with bounds checks (P2-M1-T03).
pub mod blocks;

/// Color/material table parse + glTF materials export (P2-M4-T01).
pub mod material;

/// Normal/vertex-attribute recovery where present (P2-M3-T01).
pub mod attributes;

/// Face/edge topology reconstruction from the extracted mesh (P2-M3-T02).
pub mod topology;

/// Section/view extraction via planar cuts over the internal mesh (P2-M3-T03).
pub mod section;

/// Transform/instance hierarchy (assembly CGR inside part) with matrix stack (P2-M4-T02).
pub mod instance;
pub mod assembly;

/// Binary STL mesh export (P2-M2-T02).
pub mod stl;

/// STEP B-Rep passthrough for a tessellated mesh, honestly marked tessellated (P2-M5-T02).
pub mod step;

/// True streaming parse for large assemblies (100k+ parts) (P2-M6-T02).
pub mod stream;

/// Tolerant, bounds-checked, depth-limited CGR record walker (P2-M6-T01 fuzz round 2).
pub mod structure;

/// Deterministic round-2 fuzz campaign over the record walker (P2-M6-T01).
pub mod fuzz;

/// Deterministic round-1 fuzz campaign over the header parser (P2-M1-T05).
pub mod fuzz_round1;
