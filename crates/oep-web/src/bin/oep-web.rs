//! P8-M4-T01: the PDM web UI server (item explorer, BOM tree, viewer embed,
//! compare view) plus the oep-pdm REST API v1 on one axum service.
//!
//! Usage:
//!
//!     oep-web            # serve UI at http://127.0.0.1:8787/
//!     oep-web --addr 127.0.0.1:8080
//!     oep-web --addr 127.0.0.1:8787 --persist-dir /var/lib/oep-pdm
//!
//! The persistence dir is optional; a fresh in-memory vault is used when it
//! is omitted. Seed the vault via the REST API (`POST /api/v1/items`, then
//! `POST /api/v1/items/{id}/revisions`) or via the `oep-pdm` client.

fn main() {
    let mut addr = "127.0.0.1:8787".to_string();
    let mut persist_dir: Option<std::path::PathBuf> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--addr" => {
                i += 1;
                addr = args.get(i).cloned().unwrap_or_default();
            }
            "--persist-dir" => {
                i += 1;
                persist_dir = args.get(i).map(std::path::PathBuf::from);
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let mut store = oep_pdm::store::VaultStore::new();
    if let Some(dir) = &persist_dir {
        if let Err(e) = store.set_persist_dir(dir) {
            eprintln!("cannot open persist dir {}: {e}", dir.display());
            std::process::exit(1);
        }
    }

    match oep_web::PdmWeb::start_with_store(&addr, store) {
        Ok(web) => {
            println!("oep-web: serving UI + REST API v1 at {}", web.base_url());
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            eprintln!("oep-web: failed to start on {addr}: {e}");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "oep-web - OEP PDM web UI (P8-M4-T01)\n\
         \n\
         usage: oep-web [--addr HOST:PORT] [--persist-dir DIR]\n\
         \n\
         options:\n\
         \x20 --addr HOST:PORT   listen address (default 127.0.0.1:8787)\n\
         \x20 --persist-dir DIR  optional vault persistence directory\n\
         \x20 --help, -h         show this help\n"
    );
}
