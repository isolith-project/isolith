//! P11-T03 auth-boundary security suite.
//!
//! Attempts cross-role and cross-tenant access on the PDM REST API v1
//! endpoints (items, revisions, files, audit log) over real sockets, against
//! the real contract server served behind the P8-M3 RBAC authorizer, and
//! asserts denial (403/404).
//!
//! Two modes prove the suite actually asserts (not a rubber stamp):
//!
//! * `strict_*`: served with `authz::policy::strict_boundary_authorizer`
//!   (the real P8-M3 implementation). Every cross-role / cross-tenant /
//!   unauthenticated probe MUST be denied (403 or 404), and every
//!   same-tenant legitimate probe MUST be allowed.
//! * `permissive_*`: served with `authz::policy::permissive_authorizer`
//!   (the default-permissive stub). The exact same attack probes must NOT be
//!   denied. `strict_mode_denies_what_permissive_allows` is the mutation
//!   proof: if the strict suite ever stopped asserting, that test goes red.
//!
//! Every probe goes over a real TCP socket (no in-process shortcuts), with a
//! real tenant-bearing vault object seeded beforehand, so the 404s tested are
//! genuinely cross-tenant (the object exists; an attacker must not learn it).
//!
//! Clean-room: the enforcement under test is our own P8-M3 RBAC engine wired
//! to our own REST contract; no vendor PDM authorization model is copied.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use oep_pdm::authz::policy::{permissive_authorizer, strict_boundary_authorizer};
use oep_pdm::service::PdmService;
use oep_pdm::store::VaultStore;

/// One security probe: a request against the live service.
struct Probe<'a> {
    /// Human name for the failing-assertion report.
    name: &'static str,
    /// Executes the request (method, path, identity headers, optional JSON
    /// body) and returns the HTTP status.
    status: ProbeFn<'a>,
}

/// The status-producing closure type used across the probe lists.
type ProbeFn<'a> = Box<dyn Fn() -> u16 + 'a>;

impl std::fmt::Debug for Probe<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "probe {}", self.name)
    }
}

/// Boot the real contract server behind the given authorizer.
fn boot(authorizer: oep_pdm::rbac::Authorizer) -> PdmService {
    PdmService::start_with_authorizer("127.0.0.1:0", VaultStore::new(), authorizer)
        .expect("authorized service starts")
}

/// One raw HTTP/1.1 request against the base URL; returns the status code.
fn req(
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
    let target = format!(
        "/{}/{}",
        prefix.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

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
    let resp = oep_pdm::http::read_response(&mut stream).expect("read response");
    resp.status
}

/// Seed: an item owned by `owner_tenant` with one revision, holding `id`.
fn seed_item(base: &str) -> String {
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
    // Fetch the created id (list and read it back).
    let list_resp = list_items(base, "seed-admin", "admin", "Tenant-A");
    debug_assert!(list_resp.is_some());
    let id = list_resp.expect("seed item listed");

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

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn base64_of_seed_file() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b"seed step bytes")
}

/// A create-revision body carrying one small file (content is a 1-byte blob).
fn edit_body() -> String {
    format!(
        r#"{{"state":"working","file":{{"filename":"x.step","format":"step","content":"{}"}}}}"#,
        base64_of_seed_file()
    )
}

/// A lifecycle-transition body to `to`.
fn rev_body(to: &str) -> String {
    format!(r#"{{"to":"{to}"}}"#)
}

/// Parse the item id out of `GET /items` for the first item.
fn list_items(base: &str, user: &str, roles: &str, tenant: &str) -> Option<String> {
    let rest = base.trim_start_matches("http://");
    let (host_port, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = host_port.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    let target = format!("/{}/items", prefix.trim_end_matches('/'));
    let mut head = format!("GET {target} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n");
    head.push_str(&format!("X-Oep-User: {user}\r\n"));
    head.push_str(&format!("X-Oep-Roles: {roles}\r\n"));
    if !tenant.is_empty() {
        head.push_str(&format!("X-Oep-Tenant: {tenant}\r\n"));
    }
    head.push_str("\r\n");
    let mut stream = TcpStream::connect((host, port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .ok()?;
    stream.write_all(head.as_bytes()).ok()?;
    stream.flush().ok()?;
    let resp = oep_pdm::http::read_response(&mut stream).ok()?;
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

// ---------------------------------------------------------------------------
// The attack probe lists
// ---------------------------------------------------------------------------

/// Probes that MUST be denied (403 or 404) under the strict boundary: every
/// cross-role, cross-tenant and role-escalation attempt. The mutation proof
/// re-runs the exact same list against the permissive stub and requires it
/// to NOT be denied there.
fn attack_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
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
fn allow_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
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

/// Unauthenticated probes: denied in BOTH modes (no roles ever means deny).
fn unauth_probes<'a>(base: &'a str, item: &'a str) -> Vec<Probe<'a>> {
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

fn is_denied(status: u16) -> bool {
    status == 403 || status == 404
}

// ---------------------------------------------------------------------------
// Strict boundary: every attack is denied; every same-tenant call is allowed
// ---------------------------------------------------------------------------

#[test]
fn strict_boundary_denies_cross_role_and_cross_tenant_attempts() {
    let srv = boot(strict_boundary_authorizer());
    let base = srv.base_url();
    let item = seed_item(&base);

    let denied = attack_probes(&base, &item);
    assert!(!denied.is_empty(), "suite must have attack probes");
    for p in &denied {
        let status = (p.status)();
        assert!(
            is_denied(status),
            "{} granted under strict boundary (status {status})",
            p.name
        );
    }

    let unauth = unauth_probes(&base, &item);
    for p in &unauth {
        let status = (p.status)();
        assert!(
            is_denied(status),
            "{} granted under strict boundary (status {status})",
            p.name
        );
    }

    // Authorizer-independent denials: the gate itself refuses an unrecognized
    // role and a role-less caller before the authorizer runs, so these are
    // asserted only against the strict service (never counted as flippable).
    let gate_denials: Vec<(&'static str, ProbeFn<'_>)> = vec![
        (
            "unknown role name denied",
            Box::new(|| {
                req(
                    &base,
                    "GET",
                    &format!("/items/{item}"),
                    "eve",
                    "designer,superuser",
                    "Tenant-A",
                    None,
                )
            }),
        ),
        (
            "no roles denied",
            Box::new(|| {
                req(
                    &base,
                    "GET",
                    &format!("/items/{item}"),
                    "bob",
                    "",
                    "Tenant-A",
                    None,
                )
            }),
        ),
    ];
    for (name, f) in gate_denials {
        let status = f();
        assert!(is_denied(status), "{name} granted (status {status})");
    }
}

#[test]
fn strict_boundary_allows_legitimate_same_tenant_operations() {
    let srv = boot(strict_boundary_authorizer());
    let base = srv.base_url();
    let item = seed_item(&base);

    for p in allow_probes(&base, &item) {
        let status = (p.status)();
        assert!(
            (200..300).contains(&status),
            "{} denied under strict boundary (status {status})",
            p.name
        );
    }
}

// ---------------------------------------------------------------------------
// The acceptance's "prove it asserts": fail on a default-permissive RBAC stub
// ---------------------------------------------------------------------------

/// The mutation proof. The SAME attack probes the strict boundary denies are
/// re-run against the default-permissive authorizer. Under permissive they
/// must NOT be denied (status outside 403/404), and the strict/permissive
/// denial counts must differ materially.
///
/// * If this test ever passes while the strict suite also passes, the suite
///   is asserting nothing (both green, no evidence) -- so the permissive-side
///   assertions are the "would fail on a default-permissive RBAC stub" check
///   the card's acceptance demands.
/// * If someone regresses the authorizer into permissiveness, the strict test
///   above trips first; if someone regresses the TEST to be vacuous, this
///   test trips (permissive no longer differs from strict). Either way one of
///   the two red legs catches it.
#[test]
fn strict_mode_denies_what_permissive_allows() {
    // Strict leg.
    let strict_srv = boot(strict_boundary_authorizer());
    let strict_base = strict_srv.base_url();
    let strict_item = seed_item(&strict_base);
    let strict_denied: Vec<u16> = attack_probes(&strict_base, &strict_item)
        .iter()
        .map(|p| (p.status)())
        .collect();
    let strict_denial_count = strict_denied.iter().filter(|s| is_denied(**s)).count();
    assert!(
        strict_denial_count >= 6,
        "strict boundary must deny at least 6 of the {} attack probes (got {strict_denial_count})",
        strict_denied.len()
    );

    // Permissive leg: the very same attack list must NOT be denied.
    let perm_srv = boot(permissive_authorizer());
    let perm_base = perm_srv.base_url();
    let perm_item = seed_item(&perm_base);
    let perm_denied: Vec<u16> = attack_probes(&perm_base, &perm_item)
        .iter()
        .map(|p| (p.status)())
        .collect();
    let perm_denial_count = perm_denied.iter().filter(|s| is_denied(**s)).count();
    assert!(
        perm_denial_count == 0,
        "permissive stub denied {} probes it was seeded to allow; the boundary or the stub is wrong",
        perm_denial_count
    );

    // The permissive side already failed above if the stub is permissive-by-
    // accident; a final guard ties the two legs together.
    assert_ne!(
        strict_denial_count, perm_denial_count,
        "strict denied {strict_denial_count}, permissive denied {perm_denial_count}: \
         the suite is not distinguishing the two authorizers"
    );
}

// ---------------------------------------------------------------------------
// SEC-AUTHZ-DEFAULT: the attack matrix against the SHIPPED DEFAULT BINARY
// (not the opt-in path). The product as it ships must deny the suite, and the
// explicit --no-auth opt-out must open it back up with a loud startup warning.
// ---------------------------------------------------------------------------

static NOAUTH_LOG_SEQ: AtomicU64 = AtomicU64::new(0);

/// A spawned `oep-pdm` binary under test. Kills it on drop so a panicking
/// test never leaks a bound port or a live process.
struct PdmBin {
    child: Child,
    /// Base URL including the `/api/v1` prefix.
    base: String,
    /// The binary's first stdout line (advertises the auth state).
    listen: String,
    /// Temp file where the child's stderr is teed (for the no-auth warning).
    stderr_log: PathBuf,
}

impl Drop for PdmBin {
    fn drop(&mut self) {
        self.child.kill().ok();
        let _ = self.child.wait();
    }
}

/// Spawn the real shipped `oep-pdm` binary (its default start path) on an
/// ephemeral port, wait for its listening line, and return the handle.
fn spawn_default_binary(extra_args: &[&str]) -> PdmBin {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oep-pdm"));
    cmd.arg("--addr").arg("127.0.0.1:0");
    for a in extra_args {
        cmd.arg(a);
    }
    let seq = NOAUTH_LOG_SEQ.fetch_add(1, Ordering::Relaxed);
    let stderr_log = std::env::temp_dir().join(format!(
        "oep-pdm-authbinary-{}-{}.log",
        std::process::id(),
        seq
    ));
    let logh = std::fs::File::create(&stderr_log).expect("create stderr log");
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::from(logh))
        .spawn()
        .expect("spawn the shipped oep-pdm binary");

    let mut stdout = child.stdout.take().expect("binary stdout");
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = String::new();
        let ok = BufReader::new(&mut stdout).read_line(&mut buf).is_ok();
        let _ = tx.send((ok, buf));
    });

    let (ok, line) = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("binary must print its listening line within 30s");
    let _ = reader.join();
    if !ok {
        child.kill().ok();
        let _ = child.wait();
        panic!(
            "oep-pdm binary produced no listening line; stderr: {}",
            std::fs::read_to_string(&stderr_log).unwrap_or_default()
        );
    }

    let host_port = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or_else(|| panic!("cannot parse listening line: {line:?}"));
    let port: u16 = host_port
        .rsplit(':')
        .next()
        .expect("port")
        .parse()
        .expect("numeric port");
    PdmBin {
        child,
        base: format!("http://127.0.0.1:{port}/api/v1"),
        listen: line.trim().to_string(),
        stderr_log,
    }
}

/// Status of every attack probe (cross-role and cross-tenant attempts).
fn attack_statuses(base: &str, item: &str) -> Vec<u16> {
    attack_probes(base, item)
        .iter()
        .map(|p| (p.status)())
        .collect()
}

/// Status of every unauthenticated probe.
fn unauth_statuses(base: &str, item: &str) -> Vec<u16> {
    unauth_probes(base, item)
        .iter()
        .map(|p| (p.status)())
        .collect()
}

/// The shipped DEFAULT binary (no flags) must deny every cross-role /
/// cross-tenant / unauthenticated probe (403 or 404), and must still allow
/// legitimate same-tenant use. This is the card's measured acceptance against
/// the product as it ships, not against the opt-in path.
#[test]
fn default_binary_denies_the_attack_matrix() {
    let bin = spawn_default_binary(&[]);
    // Deliverable 3 (inverse): the default must advertise enforcement, not a
    // warning, on startup.
    assert!(
        bin.listen.contains("authorization: ENFORCED"),
        "default binary must announce enforcement on startup; got: {}",
        bin.listen
    );

    let base = bin.base.clone();
    let item = seed_item(&base);

    let probes = attack_probes(&base, &item);
    let statuses: Vec<u16> = probes.iter().map(|p| (p.status)()).collect();
    assert!(!statuses.is_empty(), "suite must have attack probes");
    for (probe, status) in probes.iter().zip(statuses.iter()) {
        assert!(
            is_denied(*status),
            "{} granted by the DEFAULT binary (status {})",
            probe.name,
            status
        );
    }

    for p in unauth_probes(&base, &item) {
        let status = (p.status)();
        assert!(
            is_denied(status),
            "{} granted by the DEFAULT binary (status {status})",
            p.name
        );
    }

    // Legitimate same-tenant use must still work (health included: it is
    // contract-open in every mode), or the boundary is broken open/closed.
    for p in allow_probes(&base, &item) {
        let status = (p.status)();
        assert!(
            (200..300).contains(&status),
            "{} denied by the DEFAULT binary (status {status})",
            p.name
        );
    }
}

/// The explicit `--no-auth` opt-out must (a) print a loud startup warning
/// that authorization is disabled and (b) actually serve the open contract,
/// so the default is measurable as strictly more protective than the opt-out.
#[test]
fn no_auth_optout_warns_and_opens_the_contract() {
    let bin = spawn_default_binary(&["--no-auth"]);
    let base = bin.base.clone();

    // Deliverable 3: the warning is mandatory on the explicit opt-out.
    let stderr_text = std::fs::read_to_string(&bin.stderr_log).unwrap_or_default();
    let upper = stderr_text.to_ascii_uppercase();
    assert!(
        upper.contains("AUTHORIZATION IS DISABLED")
            && upper.contains("--NO-AUTH")
            && upper.contains("WARNING"),
        "--no-auth must emit a loud authorization-disabled warning; got stderr: {stderr_text:?}"
    );

    let item = seed_item(&base);
    let denied: Vec<u16> = attack_statuses(&base, &item)
        .into_iter()
        .chain(unauth_statuses(&base, &item))
        .filter(|s| is_denied(*s))
        .collect();
    assert!(
        denied.is_empty(),
        "the explicit --no-auth open contract must serve the open cave; {} probes were denied: {denied:?}",
        denied.len()
    );
}
