//! SEC-AUTHZ-RESIDUAL: the P11-T03 attack matrix against the SHIPPED oep-web
//! default binary.
//!
//! Mirrors the oep-pdm default-binary leg (`crates/oep-pdm/tests/
//! auth_boundary.rs`):
//!
//! * The ship boot of `oep-web` (no flags) must enforce the P11-T03 RBAC
//!   boundary on the merged REST v1 surface: every cross-role, cross-tenant
//!   and unauthenticated probe must be denied 403/404, and legitimate
//!   same-tenant use must stay green.
//! * The explicit `--no-auth` opt-out must both print a loud startup warning
//!   that authorization is disabled and serve the open contract, so the
//!   default is measurable as strictly more protective than the opt-out.
//!
//! The probe list is NOT re-implemented here: it is imported from
//! `oep_pdm::authz::probes`, the single shared copy of the P11-T03 attack
//! matrix (the same one `crates/oep-pdm/tests/auth_boundary.rs` uses), so the
//! two security suites cannot drift apart.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use oep_pdm::authz::probes::{
    allow_probes, attack_probes, is_denied, req, seed_item, unauth_probes,
};

static NOAUTH_LOG_SEQ: AtomicU64 = AtomicU64::new(0);

/// A spawned `oep-web` binary under test. Kills it on drop so a panicking
/// test never leaks a bound port or a live process.
struct WebBin {
    child: Child,
    /// Base URL including the `/api/v1` prefix (the merged REST surface).
    base: String,
    /// The bare server root (`http://host:port`) for the UI surface.
    ui_base: String,
    /// The binary's first stdout line (advertises the auth state).
    listen: String,
    /// Temp file where the child's stderr is teed (for the no-auth warning).
    stderr_log: PathBuf,
}

impl Drop for WebBin {
    fn drop(&mut self) {
        self.child.kill().ok();
        let _ = self.child.wait();
    }
}

/// Spawn the real shipped `oep-web` binary (its default start path) on an
/// ephemeral port, wait for its listening line, and return the handle.
fn spawn_default_binary(extra_args: &[&str]) -> WebBin {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oep-web"));
    cmd.arg("--addr").arg("127.0.0.1:0");
    for a in extra_args {
        cmd.arg(a);
    }
    let seq = NOAUTH_LOG_SEQ.fetch_add(1, Ordering::Relaxed);
    let stderr_log = std::env::temp_dir().join(format!(
        "oep-web-authbinary-{}-{}.log",
        std::process::id(),
        seq
    ));
    let logh = std::fs::File::create(&stderr_log).expect("create stderr log");
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::from(logh))
        .spawn()
        .expect("spawn the shipped oep-web binary");

    let mut stdout = child.stdout.take().expect("binary stdout");
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = String::new();
        let ok = BufReader::new(&mut stdout).read_line(&mut buf).is_ok();
        let _ = tx.send((ok, buf));
    });

    let (ok, line) = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("oep-web must print its listening line within 30s");
    let _ = reader.join();
    if !ok {
        child.kill().ok();
        let _ = child.wait();
        panic!(
            "oep-web binary produced no listening line; stderr: {}",
            std::fs::read_to_string(&stderr_log).unwrap_or_default()
        );
    }

    // Parse `http://127.0.0.1:PORT (...)` out of the line.
    let host_port = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next().or_else(|| s.split('(').next()))
        .map(|s| s.trim_end_matches(['(', ')', ' ']))
        .unwrap_or_else(|| panic!("cannot parse listening line: {line:?}"));
    let port: u16 = host_port
        .rsplit(':')
        .next()
        .expect("port")
        .parse()
        .expect("numeric port");
    WebBin {
        child,
        base: format!("http://127.0.0.1:{port}/api/v1"),
        ui_base: format!("http://127.0.0.1:{port}"),
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

/// The shipped DEFAULT oep-web binary (no flags) must deny every cross-role /
/// cross-tenant / unauthenticated probe (403 or 404) on its merged REST v1
/// surface, and must still allow legitimate same-tenant use and the health
/// endpoint. This is the card's measured acceptance against the product as it
/// ships, not against an opt-in path.
#[test]
fn default_binary_denies_the_attack_matrix() {
    let bin = spawn_default_binary(&[]);
    // The default must advertise enforcement, not a warning, on startup.
    assert!(
        bin.listen.contains("authorization: ENFORCED"),
        "default oep-web must announce enforcement on startup; got: {}",
        bin.listen
    );

    let base = bin.base.clone();
    let item = seed_item(&base);

    // The browser workbench surface stays reachable on the authorized boot
    // (it is a read-only HTML surface; the gate covers the REST v1 surface).
    // Loopback bind is the compensating control until the identity layer
    // lands and can gate the UI itself (accepted risk, decision #10).
    let ui_root_status = req(&bin.ui_base, "GET", "/", "", "", "", None);
    assert_eq!(
        ui_root_status, 200,
        "the UI explorer must stay browsable on the default authorized boot"
    );

    let probes = attack_probes(&base, &item);
    let statuses: Vec<u16> = probes.iter().map(|p| (p.status)()).collect();
    assert!(!statuses.is_empty(), "suite must have attack probes");
    for (probe, status) in probes.iter().zip(statuses.iter()) {
        assert!(
            is_denied(*status),
            "{} granted by the DEFAULT oep-web binary (status {})",
            probe.name,
            status
        );
    }

    for p in unauth_probes(&base, &item) {
        let status = (p.status)();
        assert!(
            is_denied(status),
            "{} granted by the DEFAULT oep-web binary (status {status})",
            p.name
        );
    }

    // Legitimate same-tenant use must still work (health included: it is
    // contract-open in every mode), or the boundary is broken open/closed.
    for p in allow_probes(&base, &item) {
        let status = (p.status)();
        assert!(
            (200..300).contains(&status),
            "{} denied by the DEFAULT oep-web binary (status {status})",
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

    // The warning is mandatory on the explicit opt-out.
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
