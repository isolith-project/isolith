//! Shared P11-T03 probe machinery (SEC-AUTHZ-RESIDUAL).
//!
//! The full attack matrix from `crates/oep-pdm/tests/auth_boundary.rs` is
//! kept here as a PUBLIC, doc-hidden module so the oep-web default-binary
//! leg (`crates/oep-web/tests/web_auth_boundary.rs`) runs the EXACT same
//! cross-role / cross-tenant / unauthenticated probes against the merged
//! REST surface of the shipped `oep-web` binary. Keeping one copy of the
//! matrix prevents the two security suites from drifting apart.
//!
//! Everything talks to a real TCP socket (no in-process shortcuts): 404s
//! tested here are genuinely cross-tenant because the object exists and an
//! attacker must not learn it.

use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

use crate::http;

/// One security probe: a request against the live service.
pub struct Probe<'a> {
    /// Human name for the failing-assertion report.
    pub name: &'static str,
    /// Executes the request and returns the HTTP status.
    pub status: ProbeFn<'a>,
}

/// The status-producing closure type used across the probe lists.
pub type ProbeFn<'a> = Box<dyn Fn() -> u16 + 'a>;

impl std::fmt::Debug for Probe<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "probe {}", self.name)
    }
}

/// One raw HTTP/1.1 request against the base URL; returns the status code.
/// `base` is the contract-anchored URL (e.g. `http://host:port/api/v1`).
pub fn req(
    base: &str,
    method: &str,
    path: &str,
    user: &str,
    roles: &str,
    tenant: &str,
    body: Option<&str>,
) -> u16 {
    let rest = base.trim_start_matches("http://");
    let (host_port, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = host_port.rsplit_once(':').expect("host:port");
    let port: u16 = port.parse().expect("numeric port");
    let path = path.trim_start_matches('/');
    let target = if prefix.is_empty() {
        format!("/{path}")
    } else {
        format!("/{}/{}", prefix.trim_end_matches('/'), path)
    };

    let mut head =
        format!("{method} {target} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n");
    if !user.is_empty() {
        head.push_str(&format!("X-Oep-User: {user}\r\n"));
    }
    if !roles.is_empty() {
        head.push_str(&format!("X-Oep-Roles: {roles}\r\n"));
    }
    if !tenant.is_empty() {
        head.push_str(&format!("X-Oep-Tenant: {tenant}\r\n"));
    }
    if let Some(b) = body {
        head.push_str("Content-Type: application/json\r\n");
        head.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    head.push_str("\r\n");

    let mut stream = TcpStream::connect((host, port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("timeout");
    stream.write_all(head.as_bytes()).expect("write head");
    if let Some(b) = body {
        stream.write_all(b.as_bytes()).expect("write body");
    }
    stream.flush().expect("flush");
    let resp = http::read_response(&mut stream).expect("read response");
    resp.status
}

/// Whether a status is one of the denial codes the matrix accepts (403/404).
pub fn is_denied(status: u16) -> bool {
    status == 403 || status == 404
}

fn base64_of_seed_file() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b"seed step bytes")
}

/// A create-revision body carrying one small file (content is a 1-byte blob).
pub fn edit_body() -> String {
    format!(
        r#"{{"state":"working","file":{{"filename":"x.step","format":"step","content":"{}"}}}}"#,
        base64_of_seed_file()
    )
}

/// A lifecycle-transition body to `to`.
pub fn rev_body(to: &str) -> String {
    format!(r#"{{"to":"{to}"}}"#)
}

/// Parse the item id out of `GET /items` for the first item (admin caller).
pub fn list_items(base: &str, user: &str, roles: &str, tenant: &str) -> Option<String> {
    let rest = base.trim_start_matches("http://");
    let (host_port, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = host_port.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    let target = format!("/{}/items", prefix.trim_end_matches('/'));
    let mut head = format!("GET {target} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n");
    head.push_str(&format!("X-Oep-User: {user}\r\n"));
    head.push_str(&format!("X-Oep-Roles: {roles}\r\n"));
    head.push_str(&format!("X-Oep-Tenant: {tenant}\r\n"));
    head.push_str("\r\n");
    let mut stream = TcpStream::connect((host, port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .ok()?;
    stream.write_all(head.as_bytes()).ok()?;
    stream.flush().ok()?;
    let resp = http::read_response(&mut stream).ok()?;
    if !(200..300).contains(&resp.status) {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&resp.body).ok()?;
    json.get("items")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()
        .map(str::to_string)
}

/// Seed: an item owned by `Tenant-A` with one revision, returning its id.
///
/// Caller must be a same-tenant admin so seeding succeeds on a gated server.
pub fn seed_item(base: &str) -> String {
    let body = r#"{"name":"BOX-A","kind":"part","owner":"Tenant-A"}"#;
    let status = req(
        base,
        "POST",
        "/items",
        "seed-admin",
        "admin",
        "Tenant-A",
        Some(body),
    );
    assert_eq!(status, 201, "admin seeds the vault item");
    let id = list_items(base, "seed-admin", "admin", "Tenant-A")
        .expect("seed item listed after creation");
    let rev_body = format!(
        r#"{{"state":"working","comment":"seed","file":{{"filename":"a.step","format":"step","content":"{}"}}}}"#,
        base64_of_seed_file()
    );
    let status = req(
        base,
        "POST",
        &format!("/items/{id}/revisions"),
        "seed-admin",
        "admin",
        "Tenant-A",
        Some(&rev_body),
    );
    assert_eq!(status, 201, "admin seeds revision 1");
    id
}

/// Probes that MUST be denied (403 or 404) under the strict boundary: every
/// cross-role, cross-tenant and role-escalation attempt. The mutation proof
/// re-runs the exact same list against the permissive stub and requires it
/// to NOT be denied there.
pub fn attack_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
    const CREATE_BODY: &str = r#"{"name":"MALLORY-ITEM","kind":"part"}"#;

    let mut v: Vec<Probe<'a>> = Vec::new();

    // -- Cross-role (same tenant, object in Tenant-A; role lacks the action) --
    let cross_role: Vec<(&'static str, ProbeFn<'a>)> = vec![
        (
            "reviewer creates item",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    "/items",
                    "rev",
                    "reviewer",
                    "Tenant-A",
                    Some(CREATE_BODY),
                )
            }),
        ),
        (
            "release-authority creates item",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    "/items",
                    "ra",
                    "release_authority",
                    "Tenant-A",
                    Some(CREATE_BODY),
                )
            }),
        ),
        (
            "reviewer edits a revision",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions"),
                    "rev",
                    "reviewer",
                    "Tenant-A",
                    Some(&edit_body()),
                )
            }),
        ),
        (
            "release-authority edits a revision",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions"),
                    "ra",
                    "release_authority",
                    "Tenant-A",
                    Some(&edit_body()),
                )
            }),
        ),
        (
            "designer releases (transition to released)",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions/1/transition"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    Some(&rev_body("released")),
                )
            }),
        ),
        (
            "reviewer releases",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions/1/transition"),
                    "rev",
                    "reviewer",
                    "Tenant-A",
                    Some(&rev_body("released")),
                )
            }),
        ),
        (
            "designer reads audit log",
            Box::new(move || req(base, "GET", "/audit", "d1", "designer", "Tenant-A", None)),
        ),
        (
            "reviewer verifies audit",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    "/audit/verify",
                    "rev",
                    "reviewer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "release-authority reads audit log",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    "/audit",
                    "ra",
                    "release_authority",
                    "Tenant-A",
                    None,
                )
            }),
        ),
    ];
    for (name, f) in cross_role {
        v.push(Probe { name, status: f });
    }

    // -- Cross-tenant (object owned by Tenant-A; caller belongs to Tenant-B) --
    let cross_tenant: Vec<(&'static str, ProbeFn<'a>)> = vec![
        (
            "cross-tenant GET item",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    None,
                )
            }),
        ),
        (
            "cross-tenant GET revisions",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/revisions"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    None,
                )
            }),
        ),
        (
            "cross-tenant GET revision",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/revisions/1"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    None,
                )
            }),
        ),
        (
            "cross-tenant GET latest file",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/file/latest"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    None,
                )
            }),
        ),
        (
            "cross-tenant GET revision file",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/revisions/1/file"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    None,
                )
            }),
        ),
        (
            "cross-tenant POST revision (write)",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    Some(&edit_body()),
                )
            }),
        ),
        (
            "cross-tenant POST lock",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/lock"),
                    "dan",
                    "designer",
                    "Tenant-B",
                    Some(r#"{"holder":"dan"}"#),
                )
            }),
        ),
        (
            "admin is tenant-scoped too (D1)",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "root",
                    "admin",
                    "Tenant-B",
                    None,
                )
            }),
        ),
    ];
    for (name, f) in cross_tenant {
        v.push(Probe { name, status: f });
    }

    v
}

/// Probes that MUST succeed (2xx) under the strict boundary for a legitimate
/// same-tenant caller, proving the boundary is not broken outright.
pub fn allow_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
    let v: Vec<(&'static str, ProbeFn<'a>)> = vec![
        (
            "designer reads own item",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "reviewer reads own item",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "rev",
                    "reviewer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "release-authority reads own item",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "ra",
                    "release_authority",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "designer lists own revisions",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/revisions"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "designer opens latest file",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/file/latest"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "designer creates an item",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    "/items",
                    "d1",
                    "designer",
                    "Tenant-A",
                    Some(r#"{"name":"MY-ITEM","kind":"part","owner":"Tenant-A"}"#),
                )
            }),
        ),
        (
            "designer saves a revision",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    &format!("/items/{item}/revisions"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    Some(&edit_body()),
                )
            }),
        ),
        (
            "designer lists catalogue",
            Box::new(move || req(base, "GET", "/items", "d1", "designer", "Tenant-A", None)),
        ),
        (
            "designer opens revision 1 file",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}/revisions/1/file"),
                    "d1",
                    "designer",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "admin reads tenant-A item",
            Box::new(move || {
                req(
                    base,
                    "GET",
                    &format!("/items/{item}"),
                    "root",
                    "admin",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "health is open",
            Box::new(move || req(base, "GET", "/health", "", "", "", None)),
        ),
    ];
    v.into_iter()
        .map(|(name, f)| Probe { name, status: f })
        .collect()
}

/// Unauthenticated probes: denied in the strict mode (no roles ever means
/// deny); opened back up by the explicit `--no-auth` opt-out.
pub fn unauth_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
    let p: Vec<(&'static str, ProbeFn<'a>)> = vec![
        (
            "unauthenticated GET item",
            Box::new(move || req(base, "GET", &format!("/items/{item}"), "", "", "", None)),
        ),
        (
            "unauthenticated POST item",
            Box::new(move || {
                req(
                    base,
                    "POST",
                    "/items",
                    "",
                    "",
                    "",
                    Some(r#"{"name":"X","kind":"part"}"#),
                )
            }),
        ),
        (
            "unauthenticated GET audit",
            Box::new(move || req(base, "GET", "/audit", "", "", "", None)),
        ),
    ];
    p.into_iter()
        .map(|(name, f)| Probe { name, status: f })
        .collect()
}
