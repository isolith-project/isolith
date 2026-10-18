//! Std-only HTTP/1.1 server exposing the dashboard. Keeps the crate std-only
//! (matching the lane doctrine) while still serving live state on the NAS.
//!
//! Endpoints:
//!
//! - `GET /`                        rendered HTML dashboard
//! - `GET /api/snapshot`            full JSON snapshot
//! - `GET /api/queue`               queue-depth counters (+ dead-letter log)
//! - `GET /api/nodes`               node health rows
//! - `GET /api/jobs`                job statuses + log lines
//! - `GET /api/cost`                cost/energy ledger + totals
//! - `GET /healthz`                 liveness (200 ok)
//!
//! The server holds one shared `Dashboard` behind a `Mutex`; each request
//! takes a fresh [`snapshot`](crate::dashboard::Dashboard::snapshot) so the
//! response reflects live state at request time.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::dashboard::Dashboard;
use crate::model::DashboardSnapshot;

/// Shared dashboard handle the server queries on every request.
pub type SharedDashboard = Arc<Mutex<Dashboard>>;

/// Response envelope for one request.
struct Response {
    status: &'static str,
    content_type: &'static str,
    body: String,
}

impl Response {
    fn ok_json(body: String) -> Self {
        Response {
            status: "200 OK",
            content_type: "application/json",
            body,
        }
    }
    fn ok_html(body: String) -> Self {
        Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body,
        }
    }
    fn not_found() -> Self {
        Response {
            status: "404 Not Found",
            content_type: "text/plain",
            body: "not found\n".to_string(),
        }
    }
}

/// Serialize a snapshot to the minimal JSON sub-views the API exposes.
fn json_of(snap: &DashboardSnapshot) -> String {
    serde_json::to_string(snap).unwrap_or_else(|_| "{}".to_string())
}

/// Route a request and build the response from LIVE dashboard state.
fn route(method: &str, path: &str, dash: &SharedDashboard) -> Response {
    if !(method == "GET" || method == "HEAD") {
        return Response {
            status: "405 Method Not Allowed",
            content_type: "text/plain",
            body: "method not allowed\n".to_string(),
        };
    }
    let d = dash.lock().unwrap();
    match path {
        "/" | "/index.html" => Response::ok_html(crate::html::render_html(&d.snapshot())),
        "/api/snapshot" => Response::ok_json(json_of(&d.snapshot())),
        "/api/queue" => {
            let s = d.snapshot();
            let mut obj = serde_json::Map::new();
            obj.insert("queue".into(), serde_json::to_value(s.queue).unwrap());
            obj.insert(
                "dead_letters".into(),
                serde_json::to_value(s.dead_letters).unwrap(),
            );
            Response::ok_json(serde_json::to_string(&obj).unwrap())
        }
        "/api/nodes" => Response::ok_json(serde_json::to_string(&d.snapshot().nodes).unwrap()),
        "/api/jobs" => {
            let s = d.snapshot();
            let mut obj = serde_json::Map::new();
            obj.insert("jobs".into(), serde_json::to_value(s.jobs).unwrap());
            obj.insert("log".into(), serde_json::to_value(s.log).unwrap());
            Response::ok_json(serde_json::to_string(&obj).unwrap())
        }
        "/api/cost" => {
            let s = d.snapshot();
            let mut obj = serde_json::Map::new();
            obj.insert("cost".into(), serde_json::to_value(s.cost).unwrap());
            obj.insert(
                "totals".into(),
                serde_json::to_value(s.cost_totals).unwrap(),
            );
            Response::ok_json(serde_json::to_string(&obj).unwrap())
        }
        "/healthz" => Response::ok_json("{\"status\":\"ok\"}".to_string()),
        _ => Response::not_found(),
    }
}

/// Serve `dash` on `addr`, blocking forever (returns on fatal bind error).
pub fn serve(dash: SharedDashboard, addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dash = Arc::clone(&dash);
                std::thread::spawn(move || {
                    let _ = handle(stream, &dash);
                });
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

/// Handle one connection: read the request line, route, write the response.
fn handle(mut stream: TcpStream, dash: &SharedDashboard) -> std::io::Result<()> {
    let mut buffer = [0u8; 8192];
    let n = stream.read(&mut buffer)?;
    let head = String::from_utf8_lossy(&buffer[..n]);
    // We only need the request line.
    let request_line = head.lines().next().unwrap_or("GET / ").to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let path = path.split('?').next().unwrap_or("/");
    let resp = route(&method, path, dash);
    let body = resp.body.as_bytes();
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
        status = resp.status,
        ct = resp.content_type,
        len = body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
