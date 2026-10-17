//! oep-pdm -- P8: self-hosted PDM / ENOVIA-lite.
//!
//! Delivers the P7-M2-T02 stub-first integration surface:
//!
//! * [`api`]   -- the P8 REST API v1 contract types (Rust projection of the
//!   OpenAPI spec at `P8-M6/openapi-v1.yaml`).
//! * [`store`] -- the vault object model subset (item/revision/file) with
//!   content-addressed blobs and optional snapshot persistence.
//! * [`stub`]  -- a real HTTP/1.1 stub server speaking that contract
//!   (binary: `oep-pdm-stub`).
//! * [`client`] -- [`client::PdmClient`], the browser/modeler face: open from
//!   PDM (`open_latest_file`), save revisions to PDM (`save_revision`). This
//!   is the part the real P8 service (P8-M6-T01) must keep serving unchanged.
//!
//! Integration points: `P7-M2/pdm-integration.md`.
#![deny(missing_docs)]

pub mod api;
pub mod client;
pub mod http;
pub mod store;
pub mod stub;

pub use api::API_VERSION;

/// Workstream anchor kept for workspace-layout tooling.
pub const WORKSTREAM: &str = "P8-PDM";
