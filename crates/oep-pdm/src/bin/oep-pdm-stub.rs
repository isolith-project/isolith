//! `oep-pdm-stub` -- the P8 REST API v1 stub server, stand-alone.
//!
//! Speaks the OpenAPI v1 contract at `P8-M6/openapi-v1.yaml`. This is the
//! throwaway stand-in for the real P8 service (P8-M6-T01); the P7 workbench
//! browser points at it today and at the real service later (same base URL,
//! same contract, no client change).
//!
//! Usage:
//!   oep-pdm-stub [--addr 127.0.0.1:8787] [--persist-dir DIR]

use std::path::PathBuf;
use std::process::exit;

use oep_pdm::store::VaultStore;
use oep_pdm::stub::StubServer;

const DEFAULT_ADDR: &str = "127.0.0.1:8787";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut addr = DEFAULT_ADDR.to_string();
    let mut persist_dir: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--addr" => {
                i += 1;
                addr = args.get(i).cloned().unwrap_or_default();
            }
            "--persist-dir" => {
                i += 1;
                persist_dir = Some(PathBuf::from(args.get(i).cloned().unwrap_or_default()));
            }
            "--help" | "-h" => {
                print_help();
                exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                exit(2);
            }
        }
        i += 1;
    }

    let mut store = VaultStore::new();
    if let Some(dir) = &persist_dir {
        match store.set_persist_dir(dir) {
            Ok(()) => eprintln!(
                "oep-pdm-stub: persistence dir {} (blobs/sha256 + store.json)",
                dir.display()
            ),
            Err(e) => {
                eprintln!("oep-pdm-stub: cannot use persistence dir: {e}");
                exit(1);
            }
        }
    }

    let server = match StubServer::start_with_store(&addr, store) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("oep-pdm-stub: cannot bind {addr}: {e}");
            exit(1);
        }
    };

    // Contract-anchored, machine-readable first line for scripts/tests.
    println!("oep-pdm-stub listening: {}", server.base_url());

    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn print_help() {
    eprintln!(
        "oep-pdm-stub -- P8 REST API v1 stub server\n\n\
         usage: oep-pdm-stub [--addr 127.0.0.1:8787] [--persist-dir DIR]\n\n\
         --addr        listen address (default {DEFAULT_ADDR})\n\
         --persist-dir keep a snapshot + content-addressed blobs in DIR\n\
         --help        this message"
    );
}
