//! The P8 REST API v1 stub server: a real HTTP/1.1 service (std-only) that
//! speaks exactly the OpenAPI v1 contract (`P8-M6/openapi-v1.yaml`).
//!
//! This is the throwaway stand-in for the real P8 service. Its only job is to
//! be contract-faithful so the browser/modeler client can be built and verified
//! end-to-end now; when P8-M6-T01 ships the real server it is replaced by
//! pointing the client at the new base URL (`P7-M2/pdm-integration.md`).

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;

use crate::api::{CreateItemRequest, CreateRevisionRequest, ErrorEnvelope};
use crate::http::{self, RequestHead};
use crate::store::{StoreError, VaultStore};

/// A running stub server. `Drop` stops the accept loop (used by tests/demo;
/// the standalone binary runs forever).
pub struct StubServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    store: Arc<Mutex<VaultStore>>,
}

impl StubServer {
    /// Bind and start serving on `addr` (use port 0 for an ephemeral port).
    pub fn start(addr: &str) -> io::Result<StubServer> {
        Self::start_with_store(addr, VaultStore::new())
    }

    /// Start serving with an already-populated/loaded store (binary path).
    pub fn start_with_store(addr: &str, store: VaultStore) -> io::Result<StubServer> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let store = Arc::new(Mutex::new(store));
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle = {
            let store = Arc::clone(&store);
            let shutdown = Arc::clone(&shutdown);
            std::thread::spawn(move || accept_loop(listener, store, shutdown))
        };
        Ok(StubServer {
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
    pub fn store(&self) -> Arc<Mutex<VaultStore>> {
        Arc::clone(&self.store)
    }
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(listener: TcpListener, store: Arc<Mutex<VaultStore>>, shutdown: Arc<AtomicBool>) {
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if let Err(e) = handle_connection(&mut stream, &store) {
                    // Best-effort: the connection is already closed or an error
                    // response was attempted; keep serving.
                    let _ = e;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

/// A resolved response (status, headers, body).
struct Out {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    is_json: bool,
}

impl Out {
    fn json<T: Serialize>(status: u16, value: &T) -> Out {
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
        Out {
            status,
            headers: Vec::new(),
            body,
            is_json: true,
        }
    }

    fn error(status: u16, env: ErrorEnvelope) -> Out {
        Out::json(status, &env)
    }

    fn bytes(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Out {
        Out {
            status,
            headers,
            body,
            is_json: false,
        }
    }
}

fn handle_connection(stream: &mut TcpStream, store: &Arc<Mutex<VaultStore>>) -> io::Result<()> {
    let req = match http::read_request(stream) {
        Ok(r) => r,
        Err(_) => return Ok(()), // unparseable head: nothing sane to answer
    };

    let out = dispatch(&req, store);
    let mut headers = out.headers;
    if out.is_json {
        headers.insert(
            0,
            ("Content-Type".to_string(), "application/json".to_string()),
        );
    }
    http::write_response(stream, out.status, &headers, &out.body)
}

fn not_found(what: &str, id: String) -> ErrorEnvelope {
    ErrorEnvelope::new(
        crate::api::error_code::NOT_FOUND,
        format!("{what} not found: {id}"),
    )
}

fn dispatch(req: &RequestHead, store: &Arc<Mutex<VaultStore>>) -> Out {
    let rest = match req.path.trim_end_matches('/').strip_prefix("/api/v1") {
        Some(r) => r,
        None => return Out::error(404, not_found("path", req.path.clone())),
    };
    let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    let method = req.method.as_str();

    match segs.as_slice() {
        ["health"] => match method {
            "GET" => read_store(store, |g| Out::json(200, &g.health())),
            _ => allowed("GET", 405),
        },
        ["items"] => match method {
            "GET" => read_store(store, |g| {
                let q = parse_query(&req.query);
                let prefix = q.get("prefix").cloned().unwrap_or_default();
                let limit = q
                    .get("limit")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(100)
                    .min(500);
                let offset = q
                    .get("offset")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                Out::json(200, &g.list_items(&prefix, limit, offset))
            }),
            "POST" => match json_body::<CreateItemRequest>(req) {
                Ok(body) => write_store(store, |g| match g.create_item(&body) {
                    Ok(item) => Out::json(201, &item),
                    Err(e) => err_out(e),
                }),
                Err(e) => e,
            },
            _ => allowed("GET, POST", 405),
        },
        ["items", id] => match method {
            "GET" => read_store(store, |g| match g.get_item(id) {
                Ok(item) => Out::json(200, &item),
                Err(e) => err_out(e),
            }),
            _ => allowed("GET", 405),
        },
        ["items", id, "file", "latest"] => match method {
            "GET" => read_store(store, |g| match g.open_latest(id) {
                Ok((rev, file, bytes)) => file_out(id, &rev, &file, bytes),
                Err(e) => err_out(e),
            }),
            _ => allowed("GET", 405),
        },
        ["items", id, "revisions"] => match method {
            "GET" => read_store(store, |g| match g.list_revisions(id) {
                Ok(rl) => Out::json(200, &rl),
                Err(e) => err_out(e),
            }),
            "POST" => match json_body::<CreateRevisionRequest>(req) {
                Ok(body) => write_store(store, |g| match g.create_revision(id, &body) {
                    Ok(r) => Out::json(201, &r),
                    Err(e) => err_out(e),
                }),
                Err(e) => e,
            },
            _ => allowed("GET, POST", 405),
        },
        ["items", id, "revisions", rev] => match method {
            "GET" => read_store(store, |g| match g.get_revision(id, rev) {
                Ok(r) => Out::json(200, &r),
                Err(e) => err_out(e),
            }),
            _ => allowed("GET", 405),
        },
        ["items", id, "revisions", rev, "file"] => match method {
            "GET" => read_store(store, |g| match g.open_revision(id, rev) {
                Ok((rrev, file, bytes)) => file_out(id, &rrev, &file, bytes),
                Err(e) => err_out(e),
            }),
            _ => allowed("GET", 405),
        },
        _ => Out::error(404, not_found("path", req.path.clone())),
    }
}

/// Read under the lock, then map to a response.
fn read_store<F>(store: &Arc<Mutex<VaultStore>>, f: F) -> Out
where
    F: FnOnce(&VaultStore) -> Out,
{
    match store.lock() {
        Ok(g) => f(&g),
        Err(_) => internal("store lock poisoned"),
    }
}

/// Mutate under the lock, then map to a response.
fn write_store<F>(store: &Arc<Mutex<VaultStore>>, f: F) -> Out
where
    F: FnOnce(&mut VaultStore) -> Out,
{
    match store.lock() {
        Ok(mut g) => f(&mut g),
        Err(_) => internal("store lock poisoned"),
    }
}

/// Parse and validate the JSON request body per the contract.
fn json_body<T: for<'de> serde::Deserialize<'de>>(req: &RequestHead) -> Result<T, Out> {
    let ct_ok = req
        .header("content-type")
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false);
    if !ct_ok {
        return Err(Out::error(
            415,
            ErrorEnvelope::new(
                crate::api::error_code::UNSUPPORTED_MEDIA,
                "Content-Type must be application/json",
            ),
        ));
    }
    serde_json::from_slice(&req.body).map_err(|e| {
        Out::error(
            400,
            ErrorEnvelope::new(
                crate::api::error_code::BAD_REQUEST,
                format!("malformed JSON body: {e}"),
            ),
        )
    })
}

fn internal(msg: &str) -> Out {
    Out::error(
        500,
        ErrorEnvelope::new(crate::api::error_code::INTERNAL, msg.to_string()),
    )
}

fn err_out(e: StoreError) -> Out {
    let (status, env) = e.envelope();
    Out::error(status, env)
}

fn allowed(allow: &str, status: u16) -> Out {
    Out {
        status,
        headers: vec![("Allow".to_string(), allow.to_string())],
        body: serde_json::to_vec(&ErrorEnvelope::new(
            crate::api::error_code::METHOD_NOT_ALLOWED,
            "method not allowed for this resource",
        ))
        .unwrap_or_default(),
        is_json: true,
    }
}

fn file_out(item_id: &str, rev: &str, file: &crate::api::FileRef, bytes: Vec<u8>) -> Out {
    let headers = vec![
        ("Content-Type".to_string(), file.content_type.clone()),
        ("X-Oep-Item".to_string(), item_id.to_string()),
        ("X-Oep-Rev".to_string(), rev.to_string()),
        ("X-Oep-Filename".to_string(), file.filename.clone()),
        ("X-Oep-Format".to_string(), file.format.clone()),
        ("X-Blob-Sha256".to_string(), file.blob_sha256.clone()),
    ];
    Out::bytes(200, headers, bytes)
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), percent_decode(v));
        }
    }
    map
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
