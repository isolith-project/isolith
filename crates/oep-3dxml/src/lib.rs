//! P3 -- 3DXML reader/writer (workstream P3).
//!
//! Reader/writer for Dassault 3DXML: outer ZIP/BOM container parse (P3-M1-T01),
//! binary `.3DRep` header documentation (P3-M3-T01), surface decode via
//! Gregory patches to NURBS (P3-M2-T01), mesh-representation to internal mesh
//! (P3-M1-T03), tessellation with quality control (P3-M2-T02), and a
//! license-compliant internal-use ASCII writer (P3-M4-T01/T02 round-trip).
//!
//! Modules:
//! - [`container`]/[`manifest`]/[`bom`]: outer ZIP container + BOM parse (P3-M1-T01).
//! - [`v5rep`]: binary `.3DRep` V5_CFV3 header/format documentation (P3-M3-T01).
//! - [`vec3`]/[`lattice`]/[`gregory`]: own-math Gregory-patch surface decode to NURBS (P3-M2-T01).
//! - [`mesh`]: tessellated mesh rep -> internal mesh (P3-M1-T03).
//! - [`tessellate`]: surface->mesh tessellation with QC + golden (P3-M2-T02).
//! - [`ascii`]: internal mesh -> 3DXML ASCII writer (P3-M4-T01).
#![deny(missing_docs)]

/// Placeholder identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P3-3DXML";

/// Internal mesh -> 3DXML ASCII writer (P3-M4-T01).
pub mod ascii;
/// Bill-of-materials / reference-listing parse (P3-M1-T01).
pub mod bom;
/// Outer ZIP container opening (P3-M1-T01).
pub mod container;
/// Gregory-patch surface decode to NURBS (P3-M2-T01, own math).
pub mod gregory;
/// Lattice helpers for Gregory-patch decode (P3-M2-T01).
pub mod lattice;
/// 3DXML manifest parsing (P3-M1-T01).
pub mod manifest;
/// Tessellated mesh rep -> internal mesh (P3-M1-T03).
pub mod mesh;
/// `.3DRep` markup document parse (P3-M1-T02, carried by the JT-reader stack).
pub mod rep;
/// Surface->mesh tessellation with quality control (P3-M2-T02).
pub mod tessellate;
/// Binary `.3DRep` V5_CFV3 header documentation (P3-M3-T01).
pub mod v5rep;
/// Minimal 3-vector math for surface decode (P3-M2-T01).
pub mod vec3;
