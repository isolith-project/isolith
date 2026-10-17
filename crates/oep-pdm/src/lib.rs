//! oep-pdm -- P8: self-hosted PDM / ENOVIA-lite.
//!
//! Serves the P8 REST API v1 contract (`P8-M6/openapi-v1.yaml`) on base path
//! `/api/v1` via the P8-M1-T01 service skeleton, whose web-stack decision is
//! recorded in `P8-M1/web-stack-adr.md` (Rust axum, AGPL-3.0):
//!
//! * [`api`]     -- the contract types (Rust projection of the OpenAPI spec).
//! * [`store`]   -- the vault object model subset (item/revision/file) with
//!   content-addressed blobs and optional snapshot persistence.
//! * [`service`] -- the real service skeleton: an axum Router over the store
//!   (binary: `oep-pdm`).
//! * [`stub`]    -- retained P7-M2-T02 identity: [`stub::StubServer`] now
//!   names the axum skeleton, so the P7 contract tests and browser demos run
//!   unchanged. The stub is the contract test, not a throwaway.
//! * [`client`]  -- [`client::PdmClient`], the browser/modeler face: open from
//!   PDM (`open_latest_file`), save revisions to PDM (`save_revision`), and
//!   the P8-M5-T02 offline-sync verbs (`sync_push`, conflict view/resolve).
//! * [`sync`]    -- P8-M5-T02 offline working set + sync-back on reconnect
//!   ([`sync::OfflineWorkingSet`], [`sync::SyncEngine`]).
//!
//! Integration points: `P7-M2/pdm-integration.md`.
#![deny(missing_docs)]

pub mod api;
pub mod client;
pub mod http;
pub mod service;
pub mod store;
pub mod stub;
pub mod sync;

pub use api::API_VERSION;
pub use sync::{EditStatus, LocalEdit, OfflineWorkingSet, SyncEngine, SyncError, SyncReport};

/// Workstream anchor kept for workspace-layout tooling.
pub const WORKSTREAM: &str = "P8-PDM";
