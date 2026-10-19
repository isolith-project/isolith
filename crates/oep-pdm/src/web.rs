//! P8-M4-T02: std-only web surface for the approval flows.
//!
//! Serves the approval UI (list, create, submit, comment, decide) over a
//! tiny HTTP/1.1 server on real sockets -- the same std-only transport
//! posture the P7 stub used before axum was ratified (P8-M1/web-stack-adr.md
//! keeps the std-only seam for tooling and contract-test clients). Because
//! this card lands in parallel with the axum service skeleton (P8-M1-T01),
//! it deliberately carries no web framework dependency: the [`ApprovalBoard`]
//! is transport-agnostic and [`start`] is a thin, disposable HTTP shim that
//! proves the flow end to end over real sockets. The REST wiring
//! (P8-M6-T01) and the axum-mounted web app (P8-M4-T01) can reuse this
//! surface or mount the same board with their own handlers.
//!
//! Forms carry `by` (principal) + `role` (one of the P8-M3-T02 role names)
//! until the P8-M3-T01 identity layer binds real sessions; the board denies
//! anything the role matrix does not grant (default-deny). See
//! `P8-M4/approval-flows.md`.
//!
//! Routes (all under `/approval`):
//!
//! * `GET  /approval`                 -- list tickets
//! * `GET  /approval/{id}`            -- ticket detail + comments + decide
//! * `POST /approval/new`             -- create a ticket (form)
//! * `POST /approval/{id}/submit`     -- submit for review
//! * `POST /approval/{id}/comment`    -- add a reviewer comment
//! * `POST /approval/{id}/decide`     -- approve/reject with affirmation
//!
//! Decides redirect (303) to the detail page, so the browser reflects the
//! new state; denial returns 403 with a message.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::approval::{ApprovalBoard, ApprovalDecision, SigningRequest};
use crate::change::{CreateTicketRequest, TicketKind, TicketStatus};
use crate::rbac::{Role, RoleSubject};

/// A running approval web server. `Drop` stops the accept loop.
pub struct ServerHandle {
    addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// The bound address (use `.to_string()` for a client).
    pub fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Bind and start serving `board` on `addr` (port 0 = ephemeral).
pub fn start(addr: &str, board: ApprovalBoard) -> std::io::Result<ServerHandle> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let local = listener.local_addr()?;
    let board = Arc::new(Mutex::new(board));
    let shutdown = Arc::new(AtomicBool::new(false));
    let handle = {
        let board = Arc::clone(&board);
        let shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || accept_loop(listener, board, shutdown))
    };
    Ok(ServerHandle {
        addr: local,
        shutdown,
        handle: Some(handle),
    })
}

fn accept_loop(listener: TcpListener, board: Arc<Mutex<ApprovalBoard>>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let board = Arc::clone(&board);
                let _ = std::thread::spawn(move || {
                    let _ = handle_connection(stream, board);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return,
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    board: Arc<Mutex<ApprovalBoard>>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let req = match read_request(&mut stream) {
        Ok(r) => r,
        Err(_) => return write_response(&mut stream, &raw(400, "Bad Request".into())),
    };

    let resp = route(&req, &board);
    write_response(&mut stream, &resp)
}

// ---- request / response (std-only HTTP/1.1) ----

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    // Read until the end of headers (\r\n\r\n) or 16k.
    let mut header_end = None;
    while buf.len() < 16 * 1024 {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            header_end = Some(pos + 4);
            break;
        }
    }
    let header_end = header_end.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "no header terminator")
    })?;
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let start_line = lines.next().unwrap_or("");
    let mut parts = start_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string().to_string();

    let mut content_length = 0usize;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    Ok(Request { method, path, body })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A prepared HTTP response.
struct Resp {
    status: u16,
    body: String,
    location: Option<String>,
}

fn raw(status: u16, body: String) -> Resp {
    Resp {
        status,
        body,
        location: None,
    }
}

fn redirect(status: u16, location: &str) -> Resp {
    Resp {
        status,
        body: String::new(),
        location: Some(location.to_string()),
    }
}

fn html(status: u16, title: &str, body: &str) -> Resp {
    Resp {
        status,
        body: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
             <body><main>{}</main></body></html>",
            esc(title),
            body,
        ),
        location: None,
    }
}

fn write_response(stream: &mut TcpStream, resp: &Resp) -> std::io::Result<()> {
    let reason = match resp.status {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let mut out = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
        resp.status,
        reason,
        resp.body.len()
    );
    if let Some(loc) = &resp.location {
        out.push_str(&format!("Location: {}\r\n", loc));
    }
    out.push_str("\r\n");
    out.push_str(&resp.body);
    stream.write_all(out.as_bytes())?;
    stream.flush()
}

// ---- routing ----

fn route(req: &Request, board: &Mutex<ApprovalBoard>) -> Resp {
    let path = req.path.split('?').next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if (req.method == "GET" || req.method == "POST")
        && segments.len() == 1
        && segments[0] == "approval"
    {
        return handle_list(req, board);
    }
    if req.method == "POST"
        && segments.len() == 2
        && segments[0] == "approval"
        && segments[1] == "new"
    {
        return handle_create(&req.body, board);
    }
    if req.method == "GET" && segments.len() == 2 && segments[0] == "approval" {
        let id = segments[1];
        let b = board.lock().unwrap();
        return match b.get_ticket(id) {
            Ok(t) => html(200, &format!("Ticket {}", t.title), &detail_page(&b, &t)),
            Err(_) => html(404, "Not found", "<p>Ticket not found.</p>"),
        };
    }
    if req.method == "POST" && segments.len() == 3 && segments[0] == "approval" {
        let id = segments[1];
        return match segments[2] {
            "submit" => {
                let form = parse_form(&req.body);
                let subj = subject_from_form(&form);
                let mut b = board.lock().unwrap();
                match b.submit_for_review(id, &subj, None) {
                    Ok(()) => redirect(303, &format!("/approval/{id}")),
                    Err(e) => deny_or_error(403, &e.to_string()),
                }
            }
            "comment" => {
                let form = parse_form(&req.body);
                let subj = subject_from_form(&form);
                let body = form.get("body").cloned().unwrap_or_default();
                let mut b = board.lock().unwrap();
                match b.add_reviewer_comment(id, &subj, &body) {
                    Ok(_) => redirect(303, &format!("/approval/{id}")),
                    Err(e) => deny_or_error(403, &e.to_string()),
                }
            }
            "decide" => {
                let form = parse_form(&req.body);
                let subj = subject_from_form(&form);
                let decision = match form.get("decision").map(|s| s.as_str()) {
                    Some("approve") => ApprovalDecision::Approve,
                    Some("reject") => ApprovalDecision::Reject,
                    _ => {
                        return html(
                            400,
                            "Bad request",
                            "<p>decision must be approve or reject.</p>",
                        )
                    }
                };
                let affirm = form.get("affirm").cloned().unwrap_or_default();
                let mut b = board.lock().unwrap();
                match b.decide(id, &subj, decision, SigningRequest { affirm }) {
                    Ok(_) => redirect(303, &format!("/approval/{id}")),
                    Err(e) => deny_or_error(403, &e.to_string()),
                }
            }
            _ => html(404, "Not found", "<p>Unknown action.</p>"),
        };
    }
    html(404, "Not found", "<p>No such route.</p>")
}

fn deny_or_error(status: u16, msg: &str) -> Resp {
    html(
        status,
        "Decision not recorded",
        &format!(
            "<p><strong>Action refused.</strong></p><p>{}</p>\
             <p><a href=\"/approval\">Back to list</a></p>",
            esc(msg)
        ),
    )
}

fn handle_list(_req: &Request, board: &Mutex<ApprovalBoard>) -> Resp {
    let b = board.lock().unwrap();
    let tickets = b.all_tickets();
    if tickets.is_empty() {
        return html(
            200,
            "Approvals",
            &format!("<h1>Approval board</h1><p>no tickets</p>{}", new_form()),
        );
    }
    let rows: Vec<String> = tickets
        .iter()
        .map(|t| {
            format!(
                "<li><a href=\"/approval/{id}\">{title}</a> \
                 <span class=\"status\">[{status}]</span></li>",
                id = t.id,
                title = esc(&t.title),
                status = t.status.as_str(),
            )
        })
        .collect();
    html(
        200,
        "Approvals",
        &format!(
            "<h1>Approval board</h1><ul>{}</ul>{}",
            rows.join("\n"),
            new_form()
        ),
    )
}

fn new_form() -> String {
    "<h2>New change request</h2><form method=\"post\" action=\"/approval/new\">\
     <input type=\"hidden\" name=\"by\" value=\"alice\">\
     <input type=\"hidden\" name=\"role\" value=\"designer\">\
     <label>Kind <select name=\"kind\"><option value=\"ecr\">ECR</option>\
     <option value=\"eco\">ECO</option></select></label>\
     <label>Title <input name=\"title\" required></label>\
     <button type=\"submit\">Create</button></form>"
        .to_string()
}

fn detail_page(b: &ApprovalBoard, t: &crate::change::ChangeTicket) -> String {
    let comments = b.comments_for(&t.id).unwrap_or_default();
    let signatures = b.signatures_for(&t.id).unwrap_or_default();

    let comment_items: Vec<String> = comments
        .iter()
        .map(|c| {
            format!(
                "<li><strong>{}</strong> <time>{}</time><p>{}</p></li>",
                esc(&c.by),
                esc(&c.at),
                esc(&c.body),
            )
        })
        .collect();

    let decision_block = if t.status == TicketStatus::UnderReview {
        format!(
            "<h2>Review decision</h2>\
             <form method=\"post\" action=\"/approval/{id}/decide\">\
             <input type=\"hidden\" name=\"by\" value=\"carol\">\
             <input type=\"hidden\" name=\"role\" value=\"reviewer\">\
             <label>Decision <select name=\"decision\">\
             <option value=\"approve\">Approve</option>\
             <option value=\"reject\">Reject</option></select></label>\
             <label>Intent-affirmation statement\
             <textarea name=\"affirm\" required>I affirm I reviewed this ticket and \
             </textarea></label>\
             <p class=\"note\">This records intent-affirmation only, not a legal seal.</p>\
             <button type=\"submit\">Record decision</button></form>",
            id = t.id,
        )
    } else {
        String::new()
    };

    let submit_block = if t.status == TicketStatus::Draft || t.status == TicketStatus::Open {
        format!(
            "<form method=\"post\" action=\"/approval/{id}/submit\">\
             <input type=\"hidden\" name=\"by\" value=\"alice\">\
             <input type=\"hidden\" name=\"role\" value=\"designer\">\
             <button type=\"submit\">Submit for review</button></form>",
            id = t.id
        )
    } else {
        String::new()
    };

    let sign_lines: Vec<String> = signatures
        .iter()
        .map(|s| {
            format!(
                "<li><strong>{}</strong> ({}) <time>{}</time> \
                 <blockquote>{}</blockquote>\
                 <span class=\"nature\">intent-affirmation</span></li>",
                esc(&s.by),
                match s.decision {
                    ApprovalDecision::Approve => "approve",
                    ApprovalDecision::Reject => "reject",
                },
                esc(&s.at),
                esc(&s.affirm),
            )
        })
        .collect();

    format!(
        "<p><a href=\"/approval\">Back to list</a></p>\
         <h1>{title}</h1>\
         <p>Status: <strong>{status}</strong> &middot; kind {kind}</p>\
         <p class=\"description\">{description}</p>\
         {submit_block}\
         <h2>Reviewer comments</h2><ul>{comments}</ul>\
         <h3>Add comment</h3>\
         <form method=\"post\" action=\"/approval/{id}/comment\">\
         <input type=\"hidden\" name=\"by\" value=\"carol\">\
         <input type=\"hidden\" name=\"role\" value=\"reviewer\">\
         <textarea name=\"body\" required></textarea>\
         <button type=\"submit\">Comment</button></form>\
         {decision_block}\
         <h2>Signing records (intent-affirmation)</h2><ul>{signatures}</ul>",
        title = esc(&t.title),
        status = t.status.as_str(),
        kind = t.kind.as_str(),
        description = t.description.as_deref().unwrap_or(""),
        id = t.id,
        comments = if comment_items.is_empty() {
            "<li><em>No comments yet.</em></li>".to_string()
        } else {
            comment_items.join("\n")
        },
        signatures = if sign_lines.is_empty() {
            "<li><em>No decisions recorded yet.</em></li>".to_string()
        } else {
            sign_lines.join("\n")
        },
    )
}

fn handle_create(body: &[u8], board: &Mutex<ApprovalBoard>) -> Resp {
    let form = parse_form(body);
    let by = form.get("by").cloned().unwrap_or_default();
    let kind = match form.get("kind").map(|s| s.as_str()) {
        Some("eco") => TicketKind::Eco,
        _ => TicketKind::Ecr,
    };
    let title = form.get("title").cloned().unwrap_or_default();
    let mut b = board.lock().unwrap();
    match b.create_ticket(&CreateTicketRequest {
        kind,
        title: title.clone(),
        description: Some("created from the approval web UI".to_string()),
        created_by: by,
    }) {
        Ok(t) => redirect(303, &format!("/approval/{}", t.id)),
        Err(e) => html(
            400,
            "Bad request",
            &format!("<p>{}</p>", esc(&e.to_string())),
        ),
    }
}

fn subject_from_form(form: &std::collections::HashMap<String, String>) -> RoleSubject {
    let by = form
        .get("by")
        .cloned()
        .unwrap_or_else(|| "anonymous".into());
    let role = form
        .get("role")
        .and_then(|r| Role::from_name(r))
        .unwrap_or(Role::Designer);
    RoleSubject::new(&by, vec![role])
}

fn parse_form(body: &[u8]) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let text = String::from_utf8_lossy(body);
    let mut map = HashMap::new();
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = url_decode(it.next().unwrap_or(""));
        let v = url_decode(it.next().unwrap_or(""));
        map.insert(k, v);
    }
    map
}

fn url_decode(s: &str) -> String {
    let bytes = s.replace('+', " ");
    let mut out = Vec::with_capacity(bytes.len());
    let b = bytes.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = hex(b[i + 1]);
            let lo = hex(b[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
