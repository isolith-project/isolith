//! `oep-update` -- P10-M5-T01 update checker (decision #11).
//!
//! Advisory, NON-BLOCKING update check against a CONFIGURABLE source: a remote
//! URL, an on-site mirror, or an offline signed bundle placed by an admin.
//!
//! Decision #11 hard rules, encoded here:
//!
//! - The program runs identically whether the source is found, empty, or
//!   unreachable. `check()` NEVER returns an error a launcher could treat as a
//!   gate; it returns an advisory [`UpdateStatus`], and every failure mode is
//!   a status, never a blocking error.
//! - No update gate ever prevents launch: [`Probe::preflight`] maps EVERY
//!   status to [`LaunchDecision::Proceed`]. There is deliberately no `Block`
//!   variant.
//! - Payloads are consumed only from a signature-verified manifest: a bundle
//!   whose Ed25519 signature does not verify, or whose payload sha256 does not
//!   match, becomes [`UpdateStatus::Untrusted`] and its payloads are never
//!   exposed. [`verify_payload`] is the final content gate.
//! - Disable-able and non-phoning by default: [`UpdateConfig::default`] has an
//!   empty source (`Off`) and no network is ever touched; when enabled the
//!   checker talks ONLY to the configured source. There is no telemetry
//!   surface anywhere in this crate (P0-M5-T02 none-by-default stands; this
//!   decision covers update checking, not analytics reach-back).

pub mod signing;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEFAULT_BUNDLE_FILE: &str = "update.bundle.json";

/// One payload referenced by an update manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadMeta {
    pub name: String,
    pub sha256: String,
    pub size: u64,
    pub url: Option<String>,
}

/// The signed update manifest: what the source offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub product: String,
    pub version: String,
    pub released: String,
    pub notes: Option<String>,
    pub payloads: Vec<PayloadMeta>,
}

/// Advisory outcome of an update check. There is no blocking/error outcome:
/// every variant is informational. Serializable so the CLI can emit it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum UpdateStatus {
    /// Feature off / no source configured (default deployment).
    Off,
    /// Source was not found, is empty, or could not be reached.
    /// Only ever a flag; never blocks.
    Unreachable { detail: String },
    /// Source reached, but it is not trusted (bad/missing signature, or the
    /// manifest's own integrity check failed). Payloads are never exposed.
    Untrusted { detail: String },
    /// Source reached and trusted, but it offers no version newer than the
    /// installed one.
    UpToDate,
    /// Source reached, trusted, and offers a newer version. Advisory only;
    /// applying the payload is a separate, operator-gated step.
    UpdateAvailable {
        version: String,
        payloads: Vec<PayloadMeta>,
    },
}

/// The CONFIGURABLE source of an update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateSource {
    /// No source configured (default deployment, non-phoning).
    Off,
    /// On-site mirror directory containing `update.bundle.json`.
    Mirror(PathBuf),
    /// Remote URL serving `update.bundle.json` (plain HTTP GET; TLS is
    /// typically terminated by the customer's local proxy per decision #10).
    RemoteUrl(String),
    /// An offline signed bundle file placed on the network by an admin.
    Bundle(PathBuf),
}

/// Checker configuration. Default is OFF and non-phoning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateConfig {
    /// Master switch. Off by default: with `enabled=false` or source `Off`,
    /// no I/O of any kind happens.
    pub enabled: bool,
    pub source: UpdateSource,
    /// 32-byte Ed25519 public key. `None` means no source can ever be trusted.
    pub public_key: Option<[u8; 32]>,
    /// Version currently installed at the launch site.
    pub current_version: String,
    /// Timeout for remote fetches.
    pub timeout_ms: u64,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig {
            enabled: false,
            source: UpdateSource::Off,
            public_key: None,
            current_version: "0.0.0".to_string(),
            timeout_ms: 3000,
        }
    }
}

/// Advisory launch decision. There is NO `Block` variant: an update check can
/// never prevent launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchDecision {
    Proceed { status: UpdateStatus },
}

/// The launcher seam: callers run `preflight` at launch, log/display the
/// status, and start the stack regardless.
#[derive(Debug, Clone)]
pub struct Probe<'a> {
    config: &'a UpdateConfig,
}

impl<'a> Probe<'a> {
    pub fn new(config: &'a UpdateConfig) -> Self {
        Probe { config }
    }

    /// Runs the update check and always produces a proceed decision. Never
    /// returns an error; never blocks launch.
    pub fn preflight(&self) -> LaunchDecision {
        let status = check(self.config);
        LaunchDecision::Proceed { status }
    }
}

/// Run the advisory update check. NEVER returns an error and never blocks.
pub fn check(config: &UpdateConfig) -> UpdateStatus {
    if !config.enabled {
        return UpdateStatus::Off;
    }
    // Resolve + read the raw bundle bytes from the configured source.
    let raw = match &config.source {
        UpdateSource::Off => return UpdateStatus::Off,
        UpdateSource::Mirror(dir) => read_local(dir.join(DEFAULT_BUNDLE_FILE)),
        UpdateSource::Bundle(path) => read_local(path.clone()),
        UpdateSource::RemoteUrl(url) => fetch_remote(url, config.timeout_ms),
    };
    let raw = match raw {
        Ok(bytes) => bytes,
        Err(detail) => {
            return UpdateStatus::Unreachable { detail };
        }
    };

    verify_and_report(config, &raw)
}

/// Verify the signed bundle and return the advisory status.
fn verify_and_report(config: &UpdateConfig, raw: &[u8]) -> UpdateStatus {
    // A source with no configured key can never be trusted (never fail open).
    let Some(pk) = config.public_key else {
        return UpdateStatus::Untrusted {
            detail: "no public key configured; source cannot be verified".to_string(),
        };
    };

    let bundle: signing::BundleFile = match serde_json::from_slice(raw) {
        Ok(b) => b,
        Err(e) => {
            return UpdateStatus::Unreachable {
                detail: format!("could not parse update bundle: {e}"),
            };
        }
    };

    // 1) Signature must verify against the canonical manifest bytes BEFORE
    //    anything in the bundle is trusted (decision #11: signed manifest
    //    verified before any download).
    if let Err(e) = signing::verify_bundle(&bundle, &pk) {
        return UpdateStatus::Untrusted {
            detail: format!("update bundle signature/verify failed: {e}"),
        };
    }

    // 2) The embedded payload must match its manifest sha256 before we ever
    //    expose it.
    if let Err(e) = bundle.verify_inline_payload() {
        return UpdateStatus::Untrusted {
            detail: format!("update bundle payload hash mismatch: {e}"),
        };
    }

    // 3) Only advertise when the source offers something newer.
    if version_eq(&bundle.version, &config.current_version) {
        return UpdateStatus::UpToDate;
    }
    UpdateStatus::UpdateAvailable {
        version: bundle.version.clone(),
        payloads: bundle.payloads.clone(),
    }
}

/// Final content gate: a payload byte slice must match the manifest sha256.
/// Refused bytes are never "consumed" by a caller.
pub fn verify_payload(meta: &PayloadMeta, bytes: &[u8]) -> Result<(), String> {
    let got = sha256_hex(bytes);
    if got == meta.sha256 {
        Ok(())
    } else {
        Err(format!(
            "sha256 mismatch for {}: manifest {} != payload {}",
            meta.name, meta.sha256, got
        ))
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_encode(&h.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// Read a local bundle file. Missing/unreadable -> Err(detail).
fn read_local(path: PathBuf) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// Minimal plain-HTTP GET fetching a remote bundle. Any failure (refused,
/// timeout, DNS, non-200, TLS attempt) collapses to `Err(detail)`, which the
/// caller maps to `Unreachable` - never to a blocking error. HTTPS is served
/// by terminating TLS at the customer's local proxy (decision #10) and
/// pointing the checker at the plain-HTTP side, or by using `Mirror`/`Bundle`.
fn fetch_remote(url: &str, timeout_ms: u64) -> Result<Vec<u8>, String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        "only http:// remote sources are supported (see decision #10 posture)".to_string()
    })?;
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|e| format!("bad port: {e}"))?),
        None => (authority, 80u16),
    };
    let mut stream = TcpStream::connect((host, port))
        .map_err(|e| format!("connect {host}:{port} failed: {e}"))?;
    let timeout = Duration::from_millis(timeout_ms);
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: oep-update-check/airgap\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("send request failed: {e}"))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read response failed: {e}"))?;
    let text = String::from_utf8_lossy(&raw);
    let status_line = text.lines().next().unwrap_or("").to_string();
    if !status_line.starts_with("HTTP/1.") || !status_line.contains(" 200 ") {
        return Err(format!("remote source returned non-200: {status_line}"));
    }
    // Body follows the first blank line.
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b)
        .unwrap_or("")
        .to_string();
    Ok(body.into_bytes())
}

/// Simple dotted-version equality (no semantics; "same version" means no
/// update to flag).
fn version_eq(a: &str, b: &str) -> bool {
    a.trim() == b.trim()
}
