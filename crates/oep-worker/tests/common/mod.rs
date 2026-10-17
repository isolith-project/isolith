//! Shared helpers for oep-worker tests.
#![allow(dead_code)] // each test binary uses a different subset of these

use std::path::{Path, PathBuf};

/// Fixtures directory (committed, redistributable).
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Read a committed fixture by name.
pub fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixtures_dir().join(name))
        .unwrap_or_else(|e| panic!("fixture {name} should be present: {e}"))
}

/// Path to a real CGR (dev-only, license-clean) if one is available on this
/// host. Resolution: `OEP_CGR_SAMPLES` env var, then a conventional corpus
/// path. The P2/P4 corpus posture keeps real CGR binaries OUT of the shared
/// repo (clean-room invariant), so the CGR end-to-end tests skip with a clear
/// message when no local copy is present; the STEP lane uses committed
/// fixtures and always runs.
pub fn real_cgr_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OEP_CGR_SAMPLES") {
        if !p.is_empty() && Path::new(&p).is_file() {
            return Some(PathBuf::from(p));
        }
    }
    // Conventional dev location used during the P9-M3-T02 proof run.
    let conventional =
        Path::new("/home/r1a/keelworks-main/corpus/cgr").join("nist_stc_06_asme1_V56R2023.CATPart");
    if conventional.is_file() {
        return Some(conventional);
    }
    None
}

/// sha-256 hex of bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}
