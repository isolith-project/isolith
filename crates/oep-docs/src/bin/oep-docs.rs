//! `oep-docs` - the P13-T03 docs server on the P10 delivery stack.
//!
//! Usage:
//!
//!     oep-docs                          # serve ./docs-site at 127.0.0.1:8792
//!     oep-docs --addr 0.0.0.0:8792 --root /srv/docs
//!
//! Health surface (the stack's curl -fsS contract):
//!
//!     curl -fsS http://127.0.0.1:8792/healthz   # -> {"status":"ok"}

use std::path::PathBuf;

fn main() {
    let mut addr = "127.0.0.1:8792".to_string();
    let mut root = PathBuf::from("docs-site");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--addr" => {
                i += 1;
                addr = args.get(i).cloned().unwrap_or_default();
            }
            "--root" => {
                i += 1;
                root = PathBuf::from(args.get(i).cloned().unwrap_or_default());
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("oep-docs: unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if addr.is_empty() || !root.exists() || !root.is_dir() {
        eprintln!(
            "oep-docs: bad --root {:?} (must be an existing directory) or --addr {:?}",
            root.display(),
            addr
        );
        std::process::exit(2);
    }
    match oep_docs::DocsServer::start(&addr, root) {
        Ok(server) => {
            println!(
                "oep-docs: serving at http://{}/ (healthz at /healthz)",
                server.addr()
            );
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            eprintln!("oep-docs: failed to start on {addr}: {e}");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "oep-docs - OEP docs site static server (P13-T03)\n\
         \n\
         usage: oep-docs [--addr HOST:PORT] [--root DIR]\n\
         \n\
         options:\n\
         \x20 --addr HOST:PORT   listen address (default 127.0.0.1:8792)\n\
         \x20 --root DIR         directory to serve (default ./docs-site)\n\
         \x20 --help, -h         show this help\n"
    );
}
