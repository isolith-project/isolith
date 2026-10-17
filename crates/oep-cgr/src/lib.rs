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
