//! P8-M4-T01: the PDM web UI server (item explorer, BOM tree, viewer embed,
//! compare view) plus the oep-pdm REST API v1 on one axum service.
//!
//! Usage:
//!
//!     oep-web            # serve UI at http://127.0.0.1:8787/
//!     oep-web --addr 127.0.0.1:8080
//!     oep-web --addr 127.0.0.1:8787 --persist-dir /var/lib/oep-pdm
//!     oep-web --update-config /etc/isolith/update-check.json
//!
//! SEC-AUTHZ-RESIDUAL: SECURE BY DEFAULT. The merged REST API v1 surface is
//! served behind the P11-T03 RBAC boundary (the same gate as the shipped
//! `oep-pdm` / `oep-pdm-stub` binaries) on every start. The open contract is
//! reachable ONLY by explicitly passing `--no-auth`, which prints a loud
//! startup warning that authorization is disabled. The browser UI routes
//! stay reachable as the read-only workbench surface (their loopback bind
//! default keeps them LAN-off until an operator publishes).
//!
//! The persistence dir is optional; a fresh in-memory vault is used when it
//! is omitted. Seed the vault via the REST API (`POST /api/v1/items`, then
//! `POST /api/v1/items/{id}/revisions`) or via the `oep-pdm` client.
//!
//! Update banner (decision #20): at startup the server resolves the update
//! config (`--update-config` path, else the `OEP_UPDATE_*` env contract),
//! runs the advisory `Probe::preflight` ONCE, logs its status line, and holds
//! it in the banner every UI page renders. Page loads never re-probe; only
//! the explicit "check now" action (`GET /ui/update/check`) touches the
//! source. With the default (off) config the server does zero update I/O.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use oep_pdm::authz::policy::strict_boundary_authorizer;

fn main() {
    let mut addr = "127.0.0.1:8787".to_string();
    let mut persist_dir: Option<PathBuf> = None;
    let mut update_config: Option<PathBuf> = None;
    // SEC-AUTHZ-RESIDUAL: authorization is ON unless explicitly disabled.
    let mut no_auth = false;
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
                persist_dir = args.get(i).map(PathBuf::from);
            }
            "--update-config" => {
                i += 1;
                update_config = args.get(i).map(PathBuf::from);
            }
            "--no-auth" => {
                no_auth = true;
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

    // Decision #20: compute the advisory update status ONCE at startup from
    // the resolved config, log its single advisory line, and bind the banner
    // to the UI pages. Page loads never re-probe; only the explicit 'check
    // now' route touches the source again.
    let dismissals_file = persist_dir.map(|d| d.join("update-dismissals.json"));
    let update_banner = Arc::new(oep_web::UpdateBanner::from_settings(
        update_config,
        dismissals_file,
    ));

    // SEC-AUTHZ-RESIDUAL: the default start path enforces the P11-T03 RBAC
    // boundary on the merged REST surface. The open contract is reachable
    // ONLY through the explicit `--no-auth` opt-out, and opting out prints a
    // loud, unmistakable startup warning that authorization is disabled.
    let web = if no_auth {
        eprintln!("oep-web: **************** WARNING ****************");
        eprintln!("oep-web: * AUTHORIZATION IS DISABLED (--no-auth). *");
        eprintln!("oep-web: * The REST API is OPEN: any client on the   *");
        eprintln!("oep-web: * network can read and write vault data.   *");
        eprintln!("oep-web: * This is for CONTROLLED TEST / AIR-GAP use *");
        eprintln!("oep-web: * only. Remove --no-auth to restore the     *");
        eprintln!("oep-web: * default RBAC boundary.                    *");
        eprintln!("oep-web: ********************************************");
        match oep_web::PdmWeb::start_with_store_and_update(&addr, store, update_banner) {
            Ok(web) => web,
            Err(e) => {
                eprintln!("oep-web: cannot bind {addr}: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let authorizer = strict_boundary_authorizer();
        match oep_web::PdmWeb::start_with_authorizer_and_update(
            &addr,
            store,
            authorizer,
            update_banner,
        ) {
            Ok(web) => web,
            Err(e) => {
                eprintln!("oep-web: cannot bind {addr}: {e}");
                std::process::exit(1);
            }
        }
    };

    // Contract-anchored, machine-readable first line for scripts/tests. The
    // authorization state is spelled out so an operator can see at a glance
    // whether this instance is enforcing the boundary.
    let auth_status = if no_auth {
        "authorization: DISABLED *** open contract)"
    } else {
        "authorization: ENFORCED *** RBAC boundary; disable ONLY with --no-auth)"
    };
    println!(
        "oep-web: serving UI + REST API v1 at {} ({auth_status})",
        web.base_url()
    );

    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn print_help() {
    println!(
        "oep-web - OEP PDM web UI (P8-M4-T01)\n\
         \n\
         usage: oep-web [--addr HOST:PORT] [--persist-dir DIR] [--update-config PATH]\n\
         \n\
         options:\n\
         \x20 --addr HOST:PORT   listen address (default 127.0.0.1:8787)\n\
         \x20 --persist-dir DIR  optional vault persistence directory (also\n\
         \x20                     persists per-version update-banner dismissals)\n\
         \x20 --update-config PATH  update-check config (JSON, update-check.sample.json\n\
         \x20                     shape); default resolves the OEP_UPDATE_* env contract\n\
         \x20                     and runs OFF with zero I/O. Advisory only (decision #20)\n\
         \x20 --no-auth          DISABLE authorization (SECURITY WARNING): serve\n\
         \x20                     the open contract for controlled test / air-gap\n\
         \x20                     use only. The default serves the P11-T03 RBAC\n\
         \x20                     boundary on the merged REST surface.\n\
         \x20 --help, -h         show this help\n"
    );
}
