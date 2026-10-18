//! `oep-docs` - the P13-T03 docs server on the P10 delivery stack.
//!
//! A zero-dependency, std-only static file server that serves the committed
//! `docs-site/` over HTTP. It is one more service in the same
//! `keelworks/oep-stack` image and compose file as the PDM, viewer and P2P
//! services -- "docs served from the sum, no separate infra" (P13-T03). It is
//! deliberately tiny and honest:
//!
//! * `GET /healthz` returns `{"status":"ok"}` so the stack's health-check
//!   contract (curl -fsS) applies to the docs service too.
//! * `GET /` serves `index.html` when present.
//! * `GET /<path>` serves a file from the root with a content type chosen from
//!   an allow-list by extension; anything else is 404 (not a fabricated page).
//! * Path traversal (`..`) is rejected outright; the request head is parsed
//!   from a bounded read so malformed input never panics the handler.
//!
//! AGPL-3.0-or-later: this service is part of the server/PDM delivery surface
//! (program decision 3: server/PDM is AGPL-3.0).

#![deny(missing_docs)]

use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Maximum request head we are willing to read (headers are ignored; this just
/// bounds the first read so a hostile connection cannot make us buffer unbounded
/// input).
const MAX_HEAD: usize = 4096;

/// The MIME allow-list. Unknown extensions are served as `text/plain` -- we
/// never guess at a type for a path we do not ship.
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "text/plain; charset=utf-8",
    }
}

/// A running docs server. Dropping it stops the accept loop and joins threads.
pub struct DocsServer {
    addr: SocketAddr,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handles: Vec<thread::JoinHandle<()>>,
}

impl DocsServer {
    /// Bind `addr` and start serving files from `root` on a background accept
    /// loop. Returns once the socket is listening.
    pub fn start(addr: &str, root: PathBuf) -> std::io::Result<DocsServer> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let bound = listener.local_addr()?;
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut server = DocsServer {
            addr: bound,
            shutdown: shutdown.clone(),
            handles: Vec::new(),
        };
        for _ in 0..4 {
            let listener = listener.try_clone()?;
            let shutdown = shutdown.clone();
            let root = root.clone();
            server.handles.push(thread::spawn(move || {
                accept_loop(listener, root, shutdown);
            }));
        }
        Ok(server)
    }

    /// The bound address (`host:port`).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop the accept loop and join worker threads.
    pub fn stop(mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for DocsServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    root: PathBuf,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let root = root.clone();
                let shutdown = shutdown.clone();
                thread::spawn(move || handle_client(stream, root, shutdown));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn handle_client(
    mut stream: TcpStream,
    root: PathBuf,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };

    let mut head = Vec::with_capacity(512);
    let mut buf = [0u8; 1024];
    loop {
        if head.len() >= MAX_HEAD {
            let _ = write_simple(&mut stream, 400, "Bad Request");
            return;
        }
        match reader.read(&mut buf) {
            Ok(0) => {
                let _ = write_simple(&mut stream, 400, "Bad Request");
                return;
            }
            Ok(n) => {
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n")
                    || head.windows(2).any(|w| w == b"\n\n")
                {
                    break;
                }
            }
            Err(_) => {
                let _ = write_simple(&mut stream, 408, "Request Timeout");
                return;
            }
        }
    }

    if !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        serve(&root, &head, &mut stream);
    }
}

/// Parse the request line and serve the file. Honest 404/403 for anything that
/// is not a well-formed GET of a path under the root.
fn serve(root: &Path, head: &[u8], stream: &mut TcpStream) {
    let (method, target) = match parse_request_line(head) {
        Some(pair) => pair,
        None => {
            let _ = write_simple(stream, 400, "Bad Request");
            return;
        }
    };
    if method != "GET" && method != "HEAD" {
        let _ = write_simple(stream, 405, "Method Not Allowed");
        return;
    }
    if target == "/healthz" || target == "/healthz/" {
        // Stack health-check contract: GET /healthz -> {"status":"ok"}.
        let body = b"{\"status\":\"ok\"}\n";
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body);
        return;
    }
    match resolve_target(root, target.as_str()) {
        Some(path) => serve_file(stream, &path, method),
        None => {
            let _ = write_simple(stream, 404, "Not Found");
        }
    }
}

/// Extract (method, decoded path) from the request head's first line.
fn parse_request_line(head: &[u8]) -> Option<(&str, String)> {
    let line_end = head
        .windows(2)
        .position(|w| w == b"\r\n")
        .or_else(|| head.iter().position(|&b| b == b'\n'))?;
    let line = std::str::from_utf8(&head[..line_end]).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if method.is_empty() || target.is_empty() {
        return None;
    }
    // Only absolute-path style targets are meaningful to a static server.
    if !target.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(target);
    Some((method, decoded))
}

/// Percent-decode a URL path (bytes), best-effort: on invalid UTF-8 keep the
/// raw bytes so the 404 path is deterministic, never a panic.
fn percent_decode(target: &str) -> String {
    let bytes = target.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            let hi = hex.and_then(|h| u8::from_str_radix(&h[..1], 16).ok());
            let lo = hex.and_then(|h| u8::from_str_radix(&h[1..], 16).ok());
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Map a request target to a file under `root`, or None (404/403).
fn resolve_target(root: &Path, target: &str) -> Option<PathBuf> {
    if target.contains("..") {
        return None; // traversal: treat as not found
    }
    let mut rel = PathBuf::new();
    for seg in target.trim_start_matches('/').split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }
        rel.push(seg);
    }
    let mut path = root.join(rel);
    if path.is_dir() {
        path = path.join("index.html");
    }
    if !path.exists() || !path.is_file() {
        return None;
    }
    // Stay under root even if a symlink escaped it.
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !canonical.starts_with(&canonical_root) {
        return None;
    }
    Some(canonical)
}

fn serve_file(stream: &mut TcpStream, path: &Path, method: &str) {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            let _ = write_simple(stream, 404, "Not Found");
            return;
        }
    };
    let kind = content_type(path);
    let _ = stream.write_all(&format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\n").into_bytes());
    let _ = stream.write_all(
        &format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            data.len()
        )
        .into_bytes(),
    );
    if method == "GET" {
        let _ = stream.write_all(&data);
    }
}

fn write_simple(stream: &mut TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let body = format!("{code} {reason}\n");
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn body(resp: &[u8]) -> String {
        let idx = resp
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|i| i + 4)
            .unwrap_or(resp.len());
        String::from_utf8_lossy(&resp[idx..]).into_owned()
    }

    fn status_line(resp: &[u8]) -> String {
        let end = resp
            .windows(2)
            .position(|w| w == b"\r\n")
            .unwrap_or(resp.len());
        String::from_utf8_lossy(&resp[..end]).into_owned()
    }

    /// Boot a server on an ephemeral port over a temp dir and run one request.
    fn roundtrip(files: &[(&str, &str)], request: &str) -> (String, String) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("oep-docs-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (name, contents) in files {
            fs::write(dir.join(name), contents).unwrap();
        }
        let server = DocsServer::start("127.0.0.1:0", dir.clone()).unwrap();
        let addr = server.addr();
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        let (status, body) = (status_line(&resp), body(&resp));
        server.stop();
        let _ = fs::remove_dir_all(&dir);
        (status, body)
    }

    #[test]
    fn serves_index_at_root() {
        let (status, body) = roundtrip(
            &[("index.html", "<h1>hi</h1>")],
            "GET / HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(status.contains("200"), "{status}");
        assert!(body.contains("hi"), "{body}");
    }

    #[test]
    fn serves_static_file_with_content_type() {
        let (status, body) = roundtrip(
            &[("format-matrix.json", "{}")],
            "GET /format-matrix.json HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(status.contains("200"), "{status}");
        assert_eq!(body.trim(), "{}");
    }

    #[test]
    fn missing_file_is_404() {
        let (status, body) = roundtrip(
            &[("index.html", "x")],
            "GET /nope.html HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(status.contains("404"), "{status}");
        assert!(body.contains("Not Found"), "{body}");
    }

    #[test]
    fn traversal_is_rejected() {
        let (status, _) = roundtrip(
            &[("index.html", "x")],
            "GET /../etc/passwd HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(status.contains("404"), "{status}");
    }

    #[test]
    fn healthz_is_ok() {
        let (status, body) = roundtrip(
            &[("index.html", "x")],
            "GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(status.contains("200"), "{status}");
        assert!(body.contains("\"status\":\"ok\""), "{body}");
    }

    #[test]
    fn garbage_request_does_not_panic() {
        // Malformed request line: parsed safely, answered with 4xx, never a panic.
        let (status, _) = roundtrip(&[], "GARBAGE\r\n\r\n");
        assert!(status.contains("400"), "{status}");
    }
}
