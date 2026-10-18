//! P8-M1-T01: the oep-pdm service skeleton, served on the ratified web stack.
//!
//! The web-stack decision lives in `P8-M1/web-stack-adr.md` (Rust axum). This
//! module is the real service skeleton: an axum [Router] over the vault store
//! ([`crate::store::VaultStore`]) speaking the P8 REST API v1 contract
//! (`P8-M6/openapi-v1.yaml`, base `/api/v1`) exactly as the P7-M2-T02 stub
//! did, so the P7 browser/modeler client keeps working unchanged.
//!
//! The P7-M2-T02 stub is not a throwaway: its stable symbols ([`StubServer`]
//! in [`crate::stub`], the `oep-pdm` and `oep-pdm-stub` binaries) are
//! retained and now back onto this axum skeleton, and the P7 contract test
//! suites drive this service over real sockets.
//!
//! AGPL-3.0-or-later (see `P8-M1/web-stack-adr.md` section 3): this is the
//! PDM server, which the program license posture (decision 3) licenses AGPL.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::header::{self, HeaderMap};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::api::{CreateItemRequest, CreateRevisionRequest, ErrorEnvelope, FileRef, ITEMS_PATH};
use crate::store::{StoreError, VaultStore};

/// Shared store handle handed to every handler via axum state.
pub type Store = Arc<Mutex<VaultStore>>;

/// A running oep-pdm service. `Drop` stops the accept loop (used by tests and
/// the demo); the standalone binaries run forever.
pub struct PdmService {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    store: Store,
}

impl PdmService {
    /// Bind and start serving on `addr` (use port 0 for an ephemeral port).
    pub fn start(addr: &str) -> io::Result<PdmService> {
        Self::start_with_store(addr, VaultStore::new())
    }

    /// Start serving with an already-populated/loaded store (binary path).
    pub fn start_with_store(addr: &str, store: VaultStore) -> io::Result<PdmService> {
        let listener = std::net::TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let store = Arc::new(Mutex::new(store));
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle = {
            let store = Arc::clone(&store);
            let shutdown = Arc::clone(&shutdown);
            std::thread::spawn(move || run_runtime(listener, store, shutdown))
        };
        Ok(PdmService {
            addr,
            shutdown,
            handle: Some(handle),
            store,
        })
    }

    /// The bound port.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Contract-anchored base URL the client should talk to (always `/api/v1`).
    pub fn base_url(&self) -> String {
        format!("http://{}/api/v1", self.addr)
    }

    /// Access to the live store (tests and the demo inspect state).
    pub fn store(&self) -> Store {
        Arc::clone(&self.store)
    }
}

impl Drop for PdmService {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Boot a tokio runtime for `listener` and serve the axum router until told
/// to stop. Runs inside the boundary thread; returning here means shutdown.
fn run_runtime(listener: std::net::TcpListener, store: Store, shutdown: Arc<AtomicBool>) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("oep-pdm: cannot build tokio runtime: {e}");
            return;
        }
    };
    rt.block_on(async move {
        let app = build_router(store);
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("oep-pdm: cannot adopt listener: {e}");
                return;
            }
        };
        let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(shutdown));
        if let Err(e) = server.await {
            eprintln!("oep-pdm: serve error: {e}");
        }
    });
}

/// Cooperative shutdown: poll the atomic flag instead of installing a signal
/// handler, so `Drop` (tests, demos, embedders) can stop the server cleanly.
async fn shutdown_signal(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Build the contract router over `store`. Exposed so tests and the binaries
/// can construct it directly; `PdmService` also uses this. The router is
/// returned ready-to-serve (`Router<()>`: the store is baked in by
/// [`Router::with_state`], which is what [`axum::serve`] accepts).
pub fn build_router(store: Store) -> Router<()> {
    Router::new()
        .route("/api/v1/health", get(health))
        .route(ITEMS_PATH, get(list_items).post(create_item))
        .route("/api/v1/items/{item_id}", get(get_item))
        .route("/api/v1/items/{item_id}/file/latest", get(open_latest_file))
        .route(
            "/api/v1/items/{item_id}/revisions",
            get(list_revisions).post(create_revision),
        )
        .route("/api/v1/items/{item_id}/revisions/{rev}", get(get_revision))
        .route(
            "/api/v1/items/{item_id}/revisions/{rev}/file",
            get(open_revision_file),
        )
        // ---- P8-M6-T03 additive gate surface (lock / transition / audit) ----
        .route(
            "/api/v1/items/{item_id}/lock",
            post(acquire_lock).delete(release_lock),
        )
        .route(
            "/api/v1/items/{item_id}/revisions/{rev}/transition",
            post(transition_revision),
        )
        .route("/api/v1/audit", get(get_audit))
        .route("/api/v1/audit/verify", get(verify_audit))
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(store)
}

/// `POST /api/v1/items/{item_id}/lock` -- acquire the working-set lock.
async fn acquire_lock(
    State(store): State<Store>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body = match json_body::<crate::api::LockAcquireRequest>(&headers, &body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    match store.lock() {
        Ok(mut g) => match g.acquire_lock(&item_id, &body.holder) {
            Ok(view) => json_response(StatusCode::OK, &view),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `DELETE /api/v1/items/{item_id}/lock` -- release a lock by its token.
async fn release_lock(
    State(store): State<Store>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body = match json_body::<crate::api::LockReleaseRequest>(&headers, &body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    match store.lock() {
        Ok(mut g) => match g.release_lock(&item_id, &body.token) {
            Ok(view) => json_response(StatusCode::OK, &view),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `POST /api/v1/items/{item_id}/revisions/{rev}/transition` -- apply a
/// lock-gated lifecycle hop (P8-M2-T01 state machine behind the wire).
async fn transition_revision(
    State(store): State<Store>,
    Path((item_id, rev)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body = match json_body::<crate::api::TransitionRequest>(&headers, &body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    match store.lock() {
        Ok(mut g) => match g.transition(&item_id, &rev, &body.to, &body.token) {
            Ok(r) => json_response(StatusCode::OK, &r),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/audit` -- all audit entries (oldest first).
async fn get_audit(State(store): State<Store>) -> Response {
    match store.lock() {
        Ok(g) => match g.audit_view() {
            Ok(view) => json_response(StatusCode::OK, &view),
            Err(e) => audit_unavailable(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/audit/verify` -- re-derive and verify the tamper-evident chain.
async fn verify_audit(State(store): State<Store>) -> Response {
    match store.lock() {
        Ok(g) => match g.audit_verify() {
            Ok(reply) => json_response(StatusCode::OK, &reply),
            Err(e) => audit_unavailable(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// Audit endpoints when the service has no audit log: honest 409
/// `audit_unavailable` (never a fabricated PASS).
fn audit_unavailable(e: &crate::store::StoreError) -> Response {
    error_json(
        StatusCode::CONFLICT,
        crate::api::ErrorEnvelope::new(crate::api::error_code::AUDIT_UNAVAILABLE, e.to_string()),
    )
}

/// `GET /api/v1/health` -- service liveness + contract identity.
async fn health(State(store): State<Store>) -> Response {
    match store.lock() {
        Ok(g) => json_response(StatusCode::OK, &g.health()),
        Err(_) => internal("store lock poisoned"),
    }
}

/// Query parameters on `GET /api/v1/items` (prefix filter + pagination).
#[derive(Debug, serde::Deserialize)]
pub struct ItemsQuery {
    /// Filter by item-name prefix.
    pub prefix: Option<String>,
    /// Page size (contract: 1..=500, default 100).
    pub limit: Option<usize>,
    /// Page offset (default 0).
    pub offset: Option<usize>,
}

/// `GET /api/v1/items` -- browser catalogue, newest first.
async fn list_items(State(store): State<Store>, Query(query): Query<ItemsQuery>) -> Response {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);
    let prefix = query.prefix.unwrap_or_default();
    match store.lock() {
        Ok(g) => json_response(StatusCode::OK, &g.list_items(&prefix, limit, offset)),
        Err(_) => internal("store lock poisoned"),
    }
}

/// `POST /api/v1/items` -- create a vault item.
async fn create_item(State(store): State<Store>, headers: HeaderMap, body: Bytes) -> Response {
    let body = match json_body::<CreateItemRequest>(&headers, &body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    match store.lock() {
        Ok(mut g) => match g.create_item(&body) {
            Ok(item) => json_response(StatusCode::CREATED, &item),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/items/{item_id}` -- one item with its latest revision number.
async fn get_item(State(store): State<Store>, Path(item_id): Path<String>) -> Response {
    match store.lock() {
        Ok(g) => match g.get_item(&item_id) {
            Ok(item) => json_response(StatusCode::OK, &item),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/items/{item_id}/file/latest` -- open-from-PDM, latest bytes.
async fn open_latest_file(State(store): State<Store>, Path(item_id): Path<String>) -> Response {
    match store.lock() {
        Ok(mut g) => match g.open_latest(&item_id) {
            Ok((ref rev, ref file, bytes)) => {
                g.record_audit(
                    "pdm",
                    crate::audit::Action::Read,
                    &format!("{item_id}/rev:{rev}"),
                );
                file_response(&item_id, rev, file, bytes)
            }
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/items/{item_id}/revisions` -- revisions of an item, newest first.
async fn list_revisions(State(store): State<Store>, Path(item_id): Path<String>) -> Response {
    match store.lock() {
        Ok(g) => match g.list_revisions(&item_id) {
            Ok(rl) => json_response(StatusCode::OK, &rl),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `POST /api/v1/items/{item_id}/revisions` -- save-revision-to-PDM.
async fn create_revision(
    State(store): State<Store>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body = match json_body::<CreateRevisionRequest>(&headers, &body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    match store.lock() {
        Ok(mut g) => match g.create_revision(&item_id, &body) {
            Ok(r) => json_response(StatusCode::CREATED, &r),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/items/{item_id}/revisions/{rev}` -- revision metadata.
async fn get_revision(
    State(store): State<Store>,
    Path((item_id, rev)): Path<(String, String)>,
) -> Response {
    match store.lock() {
        Ok(g) => match g.get_revision(&item_id, &rev) {
            Ok(r) => json_response(StatusCode::OK, &r),
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// `GET /api/v1/items/{item_id}/revisions/{rev}/file` -- specific revision bytes.
async fn open_revision_file(
    State(store): State<Store>,
    Path((item_id, rev)): Path<(String, String)>,
) -> Response {
    match store.lock() {
        Ok(mut g) => match g.open_revision(&item_id, &rev) {
            Ok((rrev, file, bytes)) => {
                g.record_audit(
                    "pdm",
                    crate::audit::Action::Read,
                    &format!("{item_id}/rev:{rrev}"),
                );
                file_response(&item_id, &rrev, &file, bytes)
            }
            Err(e) => error_response(&e),
        },
        Err(_) => internal("store lock poisoned"),
    }
}

/// Router-level 405: wrong verb on a known path, per the contract envelope.
async fn method_not_allowed() -> Response {
    error_json(
        StatusCode::METHOD_NOT_ALLOWED,
        ErrorEnvelope::new(
            crate::api::error_code::METHOD_NOT_ALLOWED,
            "method not allowed for this resource",
        ),
    )
}

/// Router-level 404 fallback for unknown paths, per the contract envelope.
async fn not_found() -> Response {
    error_json(
        StatusCode::NOT_FOUND,
        ErrorEnvelope::new(
            crate::api::error_code::NOT_FOUND,
            "path not found on this service",
        ),
    )
}

/// Parse and validate a JSON request body per the contract (415 media guard,
/// then 400 on malformed JSON), mirroring the stub's `json_body`. The error
/// side is boxed to keep the `Result` small (a full `Response` is large).
fn json_body<T: DeserializeOwned>(headers: &HeaderMap, body: &[u8]) -> Result<T, Box<Response>> {
    let ct = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !ct.starts_with("application/json") {
        return Err(Box::new(error_json(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorEnvelope::new(
                crate::api::error_code::UNSUPPORTED_MEDIA,
                "Content-Type must be application/json",
            ),
        )));
    }
    serde_json::from_slice(body).map_err(|e| {
        Box::new(error_json(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new(
                crate::api::error_code::BAD_REQUEST,
                format!("malformed JSON body: {e}"),
            ),
        ))
    })
}

/// Map a domain store error to the contract error envelope + status.
fn error_response(e: &StoreError) -> Response {
    let (status, env) = e.envelope();
    error_json(
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        env,
    )
}

/// A contract error response (`{"error":{code,message}}`).
fn error_json(status: StatusCode, env: ErrorEnvelope) -> Response {
    json_response(status, &env)
}

/// A JSON response with the given status.
fn json_response<T: Serialize>(status: StatusCode, value: &T) -> Response {
    (status, Json(value)).into_response()
}

/// The 500 internal-error response used on store-lock failure.
fn internal(msg: &str) -> Response {
    error_json(
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorEnvelope::new(crate::api::error_code::INTERNAL, msg),
    )
}

/// The file-download response: raw bytes plus the contract's `X-Oep-*` /
/// `X-Blob-Sha256` headers.
fn file_response(item_id: &str, rev: &str, file: &FileRef, bytes: Vec<u8>) -> Response {
    let rb = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, file.content_type.clone())
        .header("X-Oep-Item", item_id)
        .header("X-Oep-Rev", rev)
        .header("X-Oep-Filename", &file.filename)
        .header("X-Oep-Format", &file.format)
        .header("X-Blob-Sha256", &file.blob_sha256)
        .body(axum::body::Body::from(bytes));
    match rb {
        Ok(r) => r,
        Err(e) => internal(&format!("cannot build file response: {e}")),
    }
}

/// Shared standalone-binary entrypoint: parse the CLI args, load the store
/// (optionally from a persistence dir), and serve until killed.
///
/// Usage: `oep-pdm [--addr 127.0.0.1:8787] [--persist-dir DIR]`
///
/// `service_name` only affects the banner/help text (`oep-pdm` vs the retained
/// `oep-pdm-stub` compat binary); both serve the same contract and report the
/// same service identity in health.
pub fn serve_from_args(args: &[String], service_name: &str) -> i32 {
    let mut addr = "127.0.0.1:8787".to_string();
    let mut persist_dir: Option<std::path::PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--addr" => {
                i += 1;
                addr = args.get(i).cloned().unwrap_or_default();
            }
            "--persist-dir" => {
                i += 1;
                persist_dir = Some(std::path::PathBuf::from(
                    args.get(i).cloned().unwrap_or_default(),
                ));
            }
            "--help" | "-h" => {
                print_help(service_name);
                return 0;
            }
            other => {
                eprintln!("{service_name}: unknown argument: {other}");
                print_help(service_name);
                return 2;
            }
        }
        i += 1;
    }

    let mut store = VaultStore::new();
    if let Some(dir) = &persist_dir {
        match store.set_persist_dir(dir) {
            Ok(()) => eprintln!(
                "{service_name}: persistence dir {} (blobs/<sha256> + store.json)",
                dir.display()
            ),
            Err(e) => {
                eprintln!("{service_name}: cannot use persistence dir: {e}");
                return 1;
            }
        }
        // P8-M6-T03: a persisted vault also carries the tamper-evident audit
        // log (P8-M3-T03), so the multi-user gate's "everything verifiable via
        // the audit log" runs against the real file-backed chain. Refusing to
        // start without audit on a persisted vault is NOT done: an operator who
        // runs with --persist-dir but wants no audit gets a live but
        // un-audited store (GET /api/v1/audit then honestly reports
        // audit_unavailable). Audit is enabled here because the gate uses it.
        match store.enable_audit() {
            Ok(()) => eprintln!(
                "{service_name}: audit log enabled under {}/audit",
                dir.display()
            ),
            Err(e) => eprintln!("{service_name}: audit enable failed (continuing): {e}"),
        }
    }

    let server = match PdmService::start_with_store(&addr, store) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{service_name}: cannot bind {addr}: {e}");
            return 1;
        }
    };

    // Contract-anchored, machine-readable first line for scripts/tests.
    println!("{service_name} listening: {}", server.base_url());

    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn print_help(service_name: &str) {
    eprintln!(
        "{service_name} -- P8 REST API v1 service skeleton (axum)\n\n\
         usage: {service_name} [--addr 127.0.0.1:8787] [--persist-dir DIR]\n\n\
         --addr        listen address (default 127.0.0.1:8787)\n\
         --persist-dir keep a snapshot + content-addressed blobs in DIR\n\
         --help        this message"
    );
}
