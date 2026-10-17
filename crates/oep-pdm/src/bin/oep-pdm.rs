//! `oep-pdm` -- the P8 PDM service skeleton (Rust axum), stand-alone.
//!
//! Speaks the OpenAPI v1 contract at `P8-M6/openapi-v1.yaml` on base path
//! `/api/v1`. This is the real P8 service skeleton decided by
//! `P8-M1/web-stack-adr.md` (Rust axum, AGPL-3.0). The P7 workbench browser
//! points at the P7-M2-T02 stub today and can point here without any client
//! change (same contract, same base URL).
//!
//! Usage:
//!   oep-pdm [--addr 127.0.0.1:8787] [--persist-dir DIR]

use oep_pdm::service;

const SERVICE_NAME: &str = "oep-pdm";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(service::serve_from_args(&args, SERVICE_NAME));
}
