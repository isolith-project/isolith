//! P8-M4-T01: self-hosted PDM web UI on the ratified axum web stack.
//!
//! This crate is the browser face of P8-M4: an axum [Router] that merges the
//! oep-pdm REST API v1 router (so the P7/P8 contract clients keep working
//! unchanged) with UI routes over the same vault store:
//!
//! * [`web::explorer`]      -- `GET /` item explorer (vault catalogue).
//! * [`web::item_page`]     -- `GET /ui/items/{id}` item detail (metadata,
//!   revisions, BOM tree, embedded viewer).
//! * [`web::bom_page`]      -- `GET /ui/items/{id}/bom` BOM tree walked from
//!   the PDM relationship graph ([`oep_pdm::graph::RelationshipGraph`]).
//! * [`web::preview_png`]   -- `GET /ui/items/{id}/preview/{rev}.png` renders
//!   the real oep-geo model at that revision through the P6 viewer embed
//!   ([`oep_viewer`]) and serves a PNG (with an `X-Oep-Drawn-Pixels` header).
//! * [`web::compare_page`]  -- `GET /ui/compare?item=&a=&b=` compare view for
//!   two revisions of an item.
//!
//! The viewer embed satisfies P8-M4-T01 acceptance: the UI renders real
//! oep-geo models via the oep-viewer embed, the BOM tree walks the
//! relationship graph, and the compare view works on two revisions. See
//! `docs/P8-M4/p8m4t01-web-ui.md`.
//!
//! AGPL-3.0-or-later: this is part of the PDM server surface (program decision
//! 3: server/PDM is AGPL-3.0; see `workstreams/P8-M1/web-stack-adr.md` section 3).
#![deny(missing_docs)]

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub use oep_pdm::authz::AuthState;
pub use oep_pdm::rbac::Authorizer;
pub use oep_pdm::service::Store;
pub use web::{build_authorized_router, build_router, drawn_pixel_count};

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P8-WEB";

/// The web UI routes and their handlers.
pub mod web;

/// A running PDM web server (REST API v1 + web UI on one port).
///
/// `Drop` stops the accept loop and joins the serving thread; tests and the
/// standalone binary both use this type.
pub struct PdmWeb {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    store: Store,
}

impl PdmWeb {
    /// Bind and start serving on `addr` (use port 0 for an ephemeral port).
    pub fn start(addr: &str) -> io::Result<PdmWeb> {
        Self::start_with_store(addr, oep_pdm::store::VaultStore::new())
    }

    /// Start serving with an already-populated store, OPEN (no auth gate).
    ///
    /// This is the library/embed path (tests, in-process demos); the shipped
    /// binary boots via [`Self::start_with_authorizer`] by default so the
    /// merged REST surface is secure-by-default.
    pub fn start_with_store(addr: &str, store: oep_pdm::store::VaultStore) -> io::Result<PdmWeb> {
        Self::start_inner(addr, store, None)
    }

    /// Start serving behind the P11-T03 auth boundary (SEC-AUTHZ-RESIDUAL).
    ///
    /// The merged REST API v1 surface is gated by `build_authorized_router`
    /// with the given authorizer before any contract handler runs, exactly as
    /// the shipped `oep-pdm` binary does; the browser UI routes stay reachable
    /// as the read-only workbench surface. This is the DEFAULT boot path of
    /// the shipped `oep-web` binary (unless `--no-auth` is explicitly passed).
    pub fn start_with_authorizer(
        addr: &str,
        store: oep_pdm::store::VaultStore,
        authorizer: Authorizer,
    ) -> io::Result<PdmWeb> {
        Self::start_inner(addr, store, Some(authorizer))
    }

    /// Shared start: bind, wrap the store, and (when `authorizer` is given)
    /// serve the merged REST surface behind the P11-T03 auth gate.
    fn start_inner(
        addr: &str,
        store: oep_pdm::store::VaultStore,
        authorizer: Option<Authorizer>,
    ) -> io::Result<PdmWeb> {
        let listener = std::net::TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let store = Arc::new(Mutex::new(store));
        let auth_state = authorizer.map(|a| {
            Arc::new(AuthState {
                authorizer: Arc::new(a),
                store: Arc::clone(&store),
            })
        });
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle = {
            let store = Arc::clone(&store);
            let shutdown = Arc::clone(&shutdown);
            let auth_state = auth_state.clone();
            std::thread::spawn(move || run_runtime(listener, store, shutdown, auth_state))
        };
        Ok(PdmWeb {
            addr,
            shutdown,
            handle: Some(handle),
            store,
        })
    }

    /// The bound address (`host:port`).
    pub fn addr(&self) -> String {
        self.addr.to_string()
    }

    /// The web base URL (UI + REST both live here; REST is under `/api/v1`).
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Access to the live store (tests inspect or seed state).
    pub fn store(&self) -> Store {
        Arc::clone(&self.store)
    }
}

impl Drop for PdmWeb {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Boot a tokio runtime for `listener` and serve the merged router until told
/// to stop. Runs inside the boundary thread; returning here means shutdown.
/// When `auth_state` is given, the merged REST surface is served behind the
/// P11-T03 auth gate instead of open.
fn run_runtime(
    listener: std::net::TcpListener,
    store: Store,
    shutdown: Arc<AtomicBool>,
    auth_state: Option<Arc<AuthState>>,
) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("oep-web: cannot build tokio runtime: {e}");
            return;
        }
    };
    rt.block_on(async move {
        let app = match &auth_state {
            Some(state) => build_authorized_router(store, Arc::clone(state)),
            None => build_router(store),
        };
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("oep-web: cannot adopt listener: {e}");
                return;
            }
        };
        let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(shutdown));
        if let Err(e) = server.await {
            eprintln!("oep-web: serve error: {e}");
        }
    });
}

/// Cooperative shutdown: poll the atomic flag so `Drop` can stop cleanly.
async fn shutdown_signal(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
