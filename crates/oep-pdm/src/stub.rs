//! Retained P7-M2-T02 symbol: the stub's stable identity, now backed by the
//! real axum service skeleton.
//!
//! P7-M2-T02 shipped the stub (std-only HTTP/1.1, deliberately framework-free)
//! as the throwaway stand-in for the P8 service. P8-M1-T01 resolves that
//! deferral: the web-stack decision (`workstreams/pdm/service/web-stack-adr.md`) ratifies Rust
//! axum, and the real service skeleton lives in [`crate::service`]. The stub
//! is therefore NOT a throwaway: its contract tests and its public symbols
//! are retained, and [`StubServer`] now names the same axum skeleton so the P7
//! contract suites and the OEP browser demos compile and run unchanged.
//!
//! This is the "same contract, same base URL, no client rewrite" swap that
//! `workstreams/modeler/interchange/pdm-integration.md` promises for when P8 comes online.
//!
//! AGPL-3.0-or-later (see `workstreams/pdm/service/web-stack-adr.md` section 3).

pub use crate::service::PdmService as StubServer;
