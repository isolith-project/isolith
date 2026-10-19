//! P8-M4-T02 web-surface acceptance, Rust side -- the approval flow driven
//! over real HTTP sockets (the "Web UI" half of the card).
//!
//! This is part of the RED commit. It boots the std-only approval web server
//! on an ephemeral port (no in-process shortcut, no mock transport) and
//! drives the full lifecycle as a real user would through HTML forms:
//!
//!   GET  /approval                 -> list tickets
//!   POST /approval/new             -> designer creates an ECR
//!   POST /approval/{id}/submit     -> designer submits for review
//!   POST /approval/{id}/comment    -> reviewer comments
//!   POST /approval/{id}/decide     -> reviewer approves with an
//!                                      intent-affirmation statement
//!   GET  /approval/{id}            -> detail page shows the persisted
//!                                      comments AND the signing record
//!
//! The principal (by + role) is carried in the form until the P8-M3-T01
//! identity layer and P8-M6-T01 REST surface bind real sessions; that seam
//! is documented in workstreams/P8-M4/approval-flows.md.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use oep_pdm::approval::ApprovalBoard;
use oep_pdm::web::{self, ServerHandle};

fn post(addr: &str, path: &str, form: &[(&str, &str)]) -> (u16, String) {
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    http(addr, "POST", path, Some(&body))
}

fn get(addr: &str, path: &str) -> (u16, String) {
    http(addr, "GET", path, None)
}

fn http(addr: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect to server");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes()).expect("write request");
    stream.flush().expect("flush");

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read response");

    let text = String::from_utf8_lossy(&buf).into_owned();
    let head = text.split("\r\n\r\n").next().unwrap_or("");
    // status line: "HTTP/1.1 200 OK"
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let page = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, page)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn boot() -> (ServerHandle, String) {
    let board = ApprovalBoard::new();
    let handle = web::start("127.0.0.1:0", board).expect("web server starts");
    let s = handle.addr().to_string();
    (handle, s)
}

/// Pull a ticket id out of a list page: the first `/approval/ticket-...` href.
fn list_ids(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in page.split("/approval/ticket-").skip(1) {
        let id = tok.split('"').next().unwrap_or("").trim();
        if !id.is_empty() {
            out.push(format!("ticket-{id}"));
        }
    }
    out
}

fn first_ticket_id(page: &str) -> String {
    list_ids(page)
        .into_iter()
        .next()
        .expect("at least one ticket id on the page")
}

#[test]
fn full_approval_flow_over_http_end_to_end() {
    let (handle, addr) = boot();

    // 1. list starts empty
    let (st, page) = get(&addr, "/approval");
    assert_eq!(st, 200);
    assert!(page.contains("no tickets"), "empty board page: {page}");

    // 2. designer creates an ECR
    let (st, _) = post(
        &addr,
        "/approval/new",
        &[
            ("by", "alice"),
            ("role", "designer"),
            ("kind", "ecr"),
            ("title", "Web tab change"),
        ],
    );
    assert_eq!(st, 303, "create redirects to the detail page");

    // find the created ticket id on the list page
    let (st, page) = get(&addr, "/approval");
    assert_eq!(st, 200);
    assert!(page.contains("Web tab change"), "ticket listed: {page}");
    let id = first_ticket_id(&page);

    // 3. submit for review (designer)
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/submit"),
        &[("by", "alice"), ("role", "designer")],
    );
    assert_eq!(st, 303, "submit redirects");

    // 4. reviewer comments twice
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/comment"),
        &[
            ("by", "carol"),
            ("role", "reviewer"),
            ("body", "Tab width OK."),
        ],
    );
    assert_eq!(st, 303, "comment redirects");
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/comment"),
        &[
            ("by", "carol"),
            ("role", "reviewer"),
            ("body", "Second check passed."),
        ],
    );
    assert_eq!(st, 303);

    // 5. reviewer approves with an intent-affirmation statement
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/decide"),
        &[
            ("by", "carol"),
            ("role", "reviewer"),
            ("decision", "approve"),
            ("affirm", "I affirm I reviewed this ticket and approve."),
        ],
    );
    assert_eq!(st, 303, "decide redirects");

    // 6. detail page shows persisted comments AND the signing record
    let (st, page) = get(&addr, &format!("/approval/{id}"));
    assert_eq!(st, 200);
    assert!(page.contains(">approved<"), "status shown: {page}");
    assert!(
        page.contains("Tab width OK."),
        "comment 1 persisted: {page}"
    );
    assert!(
        page.contains("Second check passed."),
        "comment 2 persisted: {page}"
    );
    assert!(
        page.contains("I affirm I reviewed this ticket and approve."),
        "signing intent shown with the ticket: {page}"
    );
    assert!(
        page.contains("intent-affirmation"),
        "page documents the signing nature: {page}"
    );
    let _ = handle; // drop stops the accept loop
}

#[test]
fn reject_over_http_and_default_deny() {
    let (handle, addr) = boot();

    let (st, _) = post(
        &addr,
        "/approval/new",
        &[
            ("by", "alice"),
            ("role", "designer"),
            ("kind", "ecr"),
            ("title", "Reject me"),
        ],
    );
    assert_eq!(st, 303);
    let (_, page) = get(&addr, "/approval");
    let id = first_ticket_id(&page);

    // submit then a designer (not reviewer) tries to approve -> denied
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/submit"),
        &[("by", "alice"), ("role", "designer")],
    );
    assert_eq!(st, 303);
    let (st, page) = post(
        &addr,
        &format!("/approval/{id}/decide"),
        &[
            ("by", "mallory"),
            ("role", "designer"),
            ("decision", "approve"),
            ("affirm", "I affirm I approve."),
        ],
    );
    assert_eq!(st, 403, "default-deny: designer cannot approve: {page}");

    // reviewer rejects for real
    let (st, _) = post(
        &addr,
        &format!("/approval/{id}/decide"),
        &[
            ("by", "dave"),
            ("role", "reviewer"),
            ("decision", "reject"),
            ("affirm", "I affirm I reviewed and reject."),
        ],
    );
    assert_eq!(st, 303);
    let (_, page) = get(&addr, &format!("/approval/{id}"));
    assert!(page.contains(">rejected<"), "rejected status shown: {page}");
    let _ = handle;
}
