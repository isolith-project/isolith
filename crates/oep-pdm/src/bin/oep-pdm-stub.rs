//! `oep-pdm-stub` -- retained P7-M2-T02 compat binary for the PDM service.
//!
//! P8-M1-T01 resolved the P7-M2-T02 "no web framework" deferral by ratifying
//! Rust axum (`workstreams/pdm/service/web-stack-adr.md`) and shipping the real service
//! skeleton (binary `oep-pdm`). This binary name is kept so the P7-M2
//! integration surface (`workstreams/modeler/interchange/pdm-integration.md`) and the retained contract
//! test (`tests/test_p8_pdm_stub_contract.py`) keep working unchanged; it runs
//! the exact same axum service and reports the same `oep-pdm` health identity.
//!
//! Usage:
//!   oep-pdm-stub [--addr 127.0.0.1:8787] [--persist-dir DIR]

use oep_pdm::service;

const SERVICE_NAME: &str = "oep-pdm-stub";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(service::serve_from_args(&args, SERVICE_NAME));
}
