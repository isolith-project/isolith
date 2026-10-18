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
//!   PDM (`open_latest_file`), save revisions to PDM (`save_revision`).
//! * [`adapter`] -- P8-M6-T02 integration adapters: the thin seam the OEP
//!   shell (P7 workbench) and any adopted tools use to talk to this API --
//!   a [`adapter::PdmAdapter`] over the live service with unit-level
//!   instrumentation ([`adapter::AdapterInstrument`]).
//!
//! P8-M6-T03 (the P8 GA gate) brings the real multi-user lifecycle layers
//! online, wiring the already-tested capability modules from their milestone
//! PRs into the served contract as additive REST surface:
//!
//! * [`working_set`] -- P8-M1-T03 object locking (lock / edit / publish
//!   protocol). Its [`working_set::LockManager`] is the server-side
//!   single-writer serialization point: a publish (release) requires a live
//!   lock token, so concurrent editors never clobber each other.
//! * [`state`] -- P8-M2-T01 revision lifecycle state machine
//!   (working / under_review / released / obsolete) with a maturity axis;
//!   released revisions are immutable (only the terminal obsolete hop is
//!   legal), which is what "immutable after release" means in this gate.
//! * [`audit`] -- P8-M3-T03 append-only, tamper-evident audit log of
//!   who read / changed / released what (reads are access events under the
//!   sovereignty principle). `GET /api/v1/audit/verify` re-derives the hash
//!   chain so every claimed operation is verifiable.
//!
//! The gate's additive REST surface is documented in the OpenAPI spec as new
//! paths (lock / transition / audit) — additive, never a change to the
//! existing v1 wire. The full acceptance harness is `python/oep_tooling/
//! ga_gate_p8.py` (see `P8-M6/ga-gate.md`, card P8-M6-T03).
//!
//! Integration points: `P7-M2/pdm-integration.md` and
//! `P8-M6/integration-adapters.md`.
#![deny(missing_docs)]

pub mod adapter;
pub mod api;
pub mod audit;
pub mod client;
pub mod http;
pub mod service;
pub mod state;
pub mod store;
pub mod stub;
pub mod working_set;

pub use api::API_VERSION;
pub use state::{Maturity, RevisionState};

/// Workstream anchor kept for workspace-layout tooling.
pub const WORKSTREAM: &str = "P8-PDM";
