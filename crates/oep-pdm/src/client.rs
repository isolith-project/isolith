//! The OEP modeler/browser PDM client -- the durable half of P7-M2-T02.
//!
//! `PdmClient` talks to whatever service is serving the P8 REST API v1 contract
//! (`P8-M6/openapi-v1.yaml`). Today that is the stub server in this crate;
//! when the real P8 service (P8-M6-T01) comes online, P8 comes online WITHOUT a
//! client rewrite: point this client at the new base URL and it keeps speaking
//! the same contract. See `P7-M2/pdm-integration.md` for the swap contract.
//!
//! Transport is std-only HTTP/1.1 (see `crate::http`); JSON via serde_json.

use std::fmt;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::api::{ErrorEnvelope, Health, Item, ItemList, Revision, RevisionList, SyncPushResult};
use crate::http;

/// Errors surfaced to the workbench/browser layer.
#[derive(Debug)]
pub enum PdmError {
    /// A non-2xx response carrying the contract error envelope.
    Http {
        /// HTTP status code.
        status: u16,
        /// Contract error code (see `api::error_code`).
        code: String,
        /// Human-readable server message.
        message: String,
    },
    /// Could not reach or read the PDM service (connection/IO level).
    Transport(String),
    /// The base URL did not parse into an http://host[:port] service.
    BadUrl(String),
    /// A 2xx response did not match the contract (bad JSON / missing fields).
    Protocol(String),
}

impl fmt::Display for PdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdmError::Http {
                status,
                code,
                message,
            } => write!(f, "PDM HTTP {status} ({code}): {message}"),
            PdmError::Transport(m) => write!(f, "PDM transport error: {m}"),
            PdmError::BadUrl(m) => write!(f, "bad PDM base URL: {m}"),
            PdmError::Protocol(m) => write!(f, "PDM protocol error: {m}"),
        }
    }
}

impl std::error::Error for PdmError {}

impl PdmError {
    /// Convenience: is this a 404 `not_found`/`no_file` condition?
    pub fn is_not_found(&self) -> bool {
        matches!(self, PdmError::Http { status: 404, .. })
    }
}

/// A downloaded file plus the metadata the server attached to it.
#[derive(Debug, Clone, PartialEq)]
pub struct FilePayload {
    /// Item the file belongs to.
    pub item_id: String,
    /// Revision number the file was served from.
    pub rev: String,
    /// Original file name (from the `X-Oep-Filename` header).
    pub filename: String,
    /// Format tag (from the `X-Oep-Format` header).
    pub format: String,
    /// Media type (from `Content-Type`, falling back to octet-stream).
    pub content_type: String,
    /// Content address of the bytes (from `X-Blob-Sha256`; verify against it).
    pub blob_sha256: String,
    /// The raw file bytes.
    pub bytes: Vec<u8>,
}

/// Bytes-level input for the save-revision verb (the P7-facing seam).
pub struct SaveRevisionInput {
    /// Original file name to store.
    pub filename: String,
    /// Format tag for client-side routing (`step`, `cgr`, `3dxml`, ...).
    pub format: String,
    /// Optional media type (defaults to `application/octet-stream`).
    pub content_type: Option<String>,
    /// The raw file bytes to upload (base64-encoded by the client).
    pub bytes: Vec<u8>,
    /// Optional lifecycle state; defaults to `working`.
    pub state: Option<String>,
    /// Optional change comment.
    pub comment: Option<String>,
    /// Optional acting principal tag.
    pub created_by: Option<String>,
}

/// Contract client for the P8 REST API v1.
#[derive(Debug, Clone)]
pub struct PdmClient {
    host: String,
    port: u16,
    prefix: String,
}

impl PdmClient {
    /// Parse a base URL and build a client. Accepts either the host root
    /// (`http://host:port`) or the anchored contract root
    /// (`http://host:port/api/v1`).
    pub fn new(base_url: &str) -> Result<PdmClient, PdmError> {
        let (host, port, prefix) = parse_base(base_url)?;
        Ok(PdmClient { host, port, prefix })
    }

    fn path(&self, path: &str) -> String {
        format!("{}{}", self.prefix, path)
    }

    fn exchange(
        &self,
        method: &str,
        path: &str,
        json_body: Option<&str>,
    ) -> Result<http::Response, PdmError> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr)
            .map_err(|e| PdmError::Transport(format!("connect {addr}: {e}")))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|e| PdmError::Transport(format!("set timeouts: {e}")))?;

        let mut req = format!("{method} {} HTTP/1.1\r\n", self.path(path));
        req.push_str(&format!("Host: {addr}\r\n"));
        req.push_str("Connection: close\r\n");
        req.push_str("Accept: application/json, application/octet-stream\r\n");
        if let Some(body) = json_body {
            req.push_str("Content-Type: application/json\r\n");
            req.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        req.push_str("\r\n");

        stream
            .write_all(req.as_bytes())
            .map_err(|e| PdmError::Transport(format!("write request: {e}")))?;
        if let Some(body) = json_body {
            stream
                .write_all(body.as_bytes())
                .map_err(|e| PdmError::Transport(format!("write body: {e}")))?;
        }
        stream
            .flush()
            .map_err(|e| PdmError::Transport(format!("flush: {e}")))?;

        http::read_response(&mut stream)
            .map_err(|e| PdmError::Transport(format!("read response: {e}")))
    }

    fn request_typed<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<T, PdmError> {
        let resp = self.exchange(method, path, body)?;
        if !(200..300).contains(&resp.status) {
            return Err(err_from_response(&resp));
        }
        serde_json::from_slice(&resp.body).map_err(|e| {
            PdmError::Protocol(format!(
                "unexpected response body ({}) as {} for {method} {path}: {e}",
                resp.status,
                std::any::type_name::<T>(),
            ))
        })
    }

    /// Raw response for binary file endpoints.
    fn request_bytes(&self, method: &str, path: &str) -> Result<http::Response, PdmError> {
        let resp = self.exchange(method, path, None)?;
        if !(200..300).contains(&resp.status) {
            return Err(err_from_response(&resp));
        }
        Ok(resp)
    }

    /// The contract root this client speaks (debug aid).
    pub fn contract_root(&self) -> String {
        self.prefix.clone()
    }

    // ---- Contract verbs --------------------------------------------------

    /// Probe the service at `GET /health` (contract identity handshake).
    pub fn health(&self) -> Result<Health, PdmError> {
        self.request_typed("GET", "/health", None)
    }

    /// List the vault catalogue (`GET /items`), optionally prefix-filtered.
    pub fn list_items(&self, prefix: Option<&str>) -> Result<ItemList, PdmError> {
        let path = match prefix {
            Some(p) if !p.is_empty() => format!("/items?prefix={}", http::encode_segment(p)),
            _ => "/items".to_string(),
        };
        self.request_typed("GET", &path, None)
    }

    /// Create a new vault item (`POST /items`).
    pub fn create_item(&self, req: &crate::api::CreateItemRequest) -> Result<Item, PdmError> {
        let body = serde_json::to_string(req).map_err(PdmError::protocolify)?;
        self.request_typed("POST", "/items", Some(&body))
    }

    /// Fetch one item by id.
    pub fn get_item(&self, id: &str) -> Result<Item, PdmError> {
        let path = format!("/items/{}", http::encode_segment(id));
        self.request_typed("GET", &path, None)
    }

    /// List all revisions of an item, newest first.
    pub fn list_revisions(&self, id: &str) -> Result<RevisionList, PdmError> {
        let path = format!("/items/{}/revisions", http::encode_segment(id));
        self.request_typed("GET", &path, None)
    }

    /// Save the next revision of an item (`POST .../revisions`).
    pub fn create_revision(
        &self,
        id: &str,
        req: &crate::api::CreateRevisionRequest,
    ) -> Result<Revision, PdmError> {
        let body = serde_json::to_string(req).map_err(PdmError::protocolify)?;
        let path = format!("/items/{}/revisions", http::encode_segment(id));
        self.request_typed("POST", &path, Some(&body))
    }

    /// Fetch revision metadata only (no file bytes).
    pub fn get_revision(&self, id: &str, rev: &str) -> Result<Revision, PdmError> {
        let path = format!(
            "/items/{}/revisions/{}",
            http::encode_segment(id),
            http::encode_segment(rev)
        );
        self.request_typed("GET", &path, None)
    }

    /// Download a specific revision's file bytes.
    pub fn open_revision_file(&self, id: &str, rev: &str) -> Result<FilePayload, PdmError> {
        let path = format!(
            "/items/{}/revisions/{}/file",
            http::encode_segment(id),
            http::encode_segment(rev)
        );
        self.request_bytes("GET", &path)
            .and_then(|r| parse_file_payload(id, &r))
    }

    /// Open-from-PDM: fetch the latest file of an item in one call.
    pub fn open_latest_file(&self, id: &str) -> Result<FilePayload, PdmError> {
        let path = format!("/items/{}/file/latest", http::encode_segment(id));
        self.request_bytes("GET", &path)
            .and_then(|r| parse_file_payload(id, &r))
    }

    /// Save-revision-to-PDM: upload raw bytes as the item's next revision.
    /// The P7 workbench seam (browser panel) calls exactly this.
    pub fn save_revision(&self, id: &str, input: SaveRevisionInput) -> Result<Revision, PdmError> {
        use base64::Engine as _;
        let content = base64::engine::general_purpose::STANDARD.encode(&input.bytes);
        let req = crate::api::CreateRevisionRequest {
            state: input.state,
            comment: input.comment,
            created_by: input.created_by,
            file: crate::api::FileUpload {
                filename: input.filename,
                format: input.format,
                content_type: input.content_type,
                content,
            },
        };
        self.create_revision(id, &req)
    }

    // ---- P8-M5-T02 offline-sync verbs -----------------------------------

    /// P8-M5-T02: push one offline edit with its base identity
    /// (`POST /api/v1/sync/push`). Returns the tagged result: clean sync
    /// (`Synced`) on 201, or a recorded pending merge (`Conflict`) on 409.
    /// A conflict is an expected outcome with a payload, so this verb does NOT
    /// surface it as an error -- the caller routes it to the resolution UI.
    pub fn sync_push(&self, req: &crate::api::SyncPushRequest) -> Result<SyncPushResult, PdmError> {
        let body = serde_json::to_string(req).map_err(PdmError::protocolify)?;
        let resp = self.exchange("POST", "/sync/push", Some(&body))?;
        match resp.status {
            201 | 409 => serde_json::from_slice(&resp.body).map_err(|e| {
                PdmError::Protocol(format!(
                    "cannot parse sync_push result ({}): {e}",
                    resp.status
                ))
            }),
            _ => Err(err_from_response(&resp)),
        }
    }

    /// P8-M5-T02: the three snapshots of a pending merge
    /// (`GET /api/v1/sync/conflicts/{sync_id}`).
    pub fn get_sync_conflict(
        &self,
        sync_id: &str,
    ) -> Result<crate::api::SyncConflictView, PdmError> {
        let path = format!("/sync/conflicts/{}", http::encode_segment(sync_id));
        self.request_typed("GET", &path, None)
    }

    /// P8-M5-T02: raw bytes of one side of a pending merge
    /// (`GET /api/v1/sync/conflicts/{sync_id}/file/{side}`), hash-verifiable.
    pub fn open_conflict_side(&self, sync_id: &str, side: &str) -> Result<FilePayload, PdmError> {
        let path = format!(
            "/sync/conflicts/{}/file/{}",
            http::encode_segment(sync_id),
            http::encode_segment(side)
        );
        self.request_bytes("GET", &path)
            .and_then(|r| parse_file_payload(sync_id, &r))
    }

    /// P8-M5-T02: resolve a pending merge
    /// (`POST /api/v1/sync/conflicts/{sync_id}/resolve`). Returns the new
    /// revision; callers verify `files[0].blob_sha256` against the resolved
    /// bytes they intended (hash-after-commit acceptance).
    pub fn resolve_conflict(
        &self,
        sync_id: &str,
        req: &crate::api::SyncResolveRequest,
    ) -> Result<Revision, PdmError> {
        let body = serde_json::to_string(req).map_err(PdmError::protocolify)?;
        let path = format!("/sync/conflicts/{}/resolve", http::encode_segment(sync_id));
        self.request_typed("POST", &path, Some(&body))
    }
}

fn parse_file_payload(id: &str, resp: &http::Response) -> Result<FilePayload, PdmError> {
    macro_rules! hdr {
        ($name:literal, $fallback:expr) => {
            resp.header($name).unwrap_or($fallback).to_string()
        };
        ($name:literal) => {
            resp.header($name)
                .ok_or_else(|| {
                    PdmError::Protocol(format!("download response missing {0} header", $name))
                })?
                .to_string()
        };
    }
    let rev = hdr!("x-oep-rev");
    let filename = hdr!("x-oep-filename");
    let format = hdr!("x-oep-format");
    let content_type = hdr!("content-type", "application/octet-stream");
    let blob_sha256 = hdr!("x-blob-sha256");
    let item_id = resp.header("x-oep-item").unwrap_or(id).to_string();
    Ok(FilePayload {
        item_id,
        rev,
        filename,
        format,
        content_type,
        blob_sha256,
        bytes: resp.body.clone(),
    })
}

fn err_from_response(resp: &http::Response) -> PdmError {
    match serde_json::from_slice::<ErrorEnvelope>(&resp.body) {
        Ok(env) => PdmError::Http {
            status: resp.status,
            code: env.error.code,
            message: env.error.message,
        },
        Err(_) => PdmError::Protocol(format!(
            "{} response did not carry the contract error envelope",
            resp.status
        )),
    }
}

fn parse_base(url: &str) -> Result<(String, u16, String), PdmError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| PdmError::BadUrl(format!("only http:// is supported: {url}")))?;
    let (authority, raw_path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, String::new()),
    };
    if authority.is_empty() {
        return Err(PdmError::BadUrl(url.to_string()));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(port) => (h.to_string(), port),
            Err(_) => (authority.to_string(), 80),
        },
        None => (authority.to_string(), 80),
    };
    let prefix = match raw_path.trim_end_matches('/') {
        "" | "/" => crate::api::API_BASE.to_string(),
        p if p == crate::api::API_BASE => crate::api::API_BASE.to_string(),
        other => {
            return Err(PdmError::BadUrl(format!(
                "base path must be empty or {0}: got '{other}'",
                crate::api::API_BASE
            )))
        }
    };
    Ok((host, port, prefix))
}

impl PdmError {
    fn protocolify(e: serde_json::Error) -> PdmError {
        PdmError::Protocol(format!("could not serialize request: {e}"))
    }
}
