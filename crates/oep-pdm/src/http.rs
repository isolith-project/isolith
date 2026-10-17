//! Minimal HTTP/1.1 wire helpers (std-only), shared by the stub server and the
//! `PdmClient`.
//!
//! Deliberately tiny: the v1 contract is a fixed set of JSON + binary-file
//! endpoints, and the stub intentionally uses no web framework so that P8's
//! real service (P8-M6-T01) owns the framework decision. Requests are
//! single-shot (`Connection: close`), bodies are bounded by `Content-Length`.

use std::io::{self, Read, Write};
use std::net::TcpStream;

/// Max bytes we accept for a request head (request line + headers).
const MAX_HEAD: usize = 16 * 1024;

/// A parsed HTTP/1.1 request head + body.
#[derive(Debug)]
pub struct RequestHead {
    /// HTTP method, e.g. `GET`, `POST`.
    pub method: String,
    /// Path component of the request target (no query).
    pub path: String,
    /// Raw query string (no leading `?`), empty when absent.
    pub query: String,
    /// Header name/value pairs, names lower-cased.
    pub headers: Vec<(String, String)>,
    /// Request body bytes.
    pub body: Vec<u8>,
}

impl RequestHead {
    /// Case-insensitive header lookup by lower-cased name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A parsed HTTP/1.1 response head + body.
#[derive(Debug)]
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Reason phrase (informational only).
    pub reason: String,
    /// Header name/value pairs, names lower-cased.
    pub headers: Vec<(String, String)>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// Case-insensitive header lookup by lower-cased name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

fn reason_for(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

/// Read a full request (head + body up to Content-Length) from the stream.
pub fn read_request(stream: &mut TcpStream) -> io::Result<RequestHead> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if buf.len() > MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head exceeds 16KiB",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed before request head",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos;
        }
    };

    let head = &buf[..head_end];
    let mut lines = head.split(|&b| b == b'\n');
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty request line"))?;
    let request_line = String::from_utf8_lossy(strip_cr(request_line)).to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    let _version = parts.next().unwrap_or("");

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.clone(), String::new()),
    };

    let mut headers = Vec::new();
    for line in lines {
        let line = strip_cr(line);
        if line.is_empty() {
            continue;
        }
        if let Some(colon) = line.iter().position(|&b| b == b':') {
            let k = String::from_utf8_lossy(&line[..colon])
                .trim()
                .to_lowercase();
            let v = String::from_utf8_lossy(&line[colon + 1..])
                .trim()
                .to_string();
            headers.push((k, v));
        }
    }

    let content_length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);

    let body_start = head_end + 4;
    let mut body = buf[body_start..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed during body",
            ));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Ok(RequestHead {
        method,
        path,
        query,
        headers,
        body,
    })
}

/// Write a full HTTP/1.1 response and flush.
pub fn write_response(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> io::Result<()> {
    let reason = reason_for(status);
    let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    head.push_str("Connection: close\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// Read a full response (head + body up to Content-Length) from the stream.
pub fn read_response(stream: &mut TcpStream) -> io::Result<Response> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if buf.len() > MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response head exceeds 16KiB",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break buf.len(); // EOF-terminated (no Content-Length) -> empty body
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos;
        }
    };

    let head = &buf[..head_end];
    let mut lines = head.split(|&b| b == b'\n');
    let status_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "empty status line"))?;
    let status_line = String::from_utf8_lossy(strip_cr(status_line)).to_string();
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next().unwrap_or("");
    let status: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let reason = parts.next().unwrap_or("").to_string();

    let mut headers = Vec::new();
    for line in lines {
        let line = strip_cr(line);
        if line.is_empty() {
            continue;
        }
        if let Some(colon) = line.iter().position(|&b| b == b':') {
            let k = String::from_utf8_lossy(&line[..colon])
                .trim()
                .to_lowercase();
            let v = String::from_utf8_lossy(&line[colon + 1..])
                .trim()
                .to_string();
            headers.push((k, v));
        }
    }

    let content_length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok());

    let body_start = head_end + 4;
    let mut body = buf[body_start..].to_vec();
    match content_length {
        Some(len) => {
            while body.len() < len {
                let n = stream.read(&mut chunk)?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..n]);
            }
            body.truncate(len);
        }
        None => {
            // No Content-Length: read to EOF (we always send Connection: close).
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest)?;
            body.extend_from_slice(&rest);
        }
    }

    Ok(Response {
        status,
        reason,
        headers,
        body,
    })
}

/// Naive but correct-for-our-single-response reads: byte-subslice search.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Strip one trailing `\r` (line endings inside HTTP/1.1 heads).
fn strip_cr(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Percent-encode a path segment (keep unreserved chars as-is).
pub fn encode_segment(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
