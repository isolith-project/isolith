//! P10-M5-T01 acceptance: the update checker (decision #11).
//!
//! Advisory, NON-BLOCKING update check against a CONFIGURABLE source
//! (remote URL / on-site mirror / offline signed bundle placed by an admin).
//! The card's hard rules, each mapped to a real test below:
//!
//! 1. **Non-bloking / never fails closed.** `check` NEVER returns an error
//!    that a launcher could treat as a gate: it returns an advisory
//!    `UpdateStatus`, and `Probe::preflight` maps EVERY status to
//!    `LaunchDecision::Proceed`. Launch-without-source and
//!    launch-with-outdated-bundle both succeed and only flag status.
//! 2. **Program runs identically whether the source is found, empty, or
//!    unreachable.** A missing mirror dir, a refused remote, a corrupt or
//!    unsigned manifest all collapse to advisory statuses (never to a
//!    blocking error), and launch still proceeds.
//! 3. **Signed manifest verified before any download.** `check` only exposes
//!    payloads from a signature-verified manifest; a `verify_payload`/apply
//!    path refuses to touch bytes whose sha256 does not match or whose
//!    manifest signature is invalid.
//! 4. **Disable-able and non-phoning by default.** `UpdateConfig::default()`
//!    and the shipped sample config have an EMPTY source (off); with no
//!    source the checker performs zero I/O (no DNS, no sockets, no files).
//!    No telemetry/analytics surface exists on the config or the status.
//!
//! RED at this commit: the `oep-update` crate does not exist yet, so this
//! file does not compile. GREEN lands the crate.

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use oep_update::{
    check, verify_payload, LaunchDecision, PayloadMeta, Probe, UpdateConfig, UpdateSource,
    UpdateStatus,
};

/// A scratch directory unique to this process+tag (repo convention).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oep-update-accept-{tag}-{}-{}",
        std::process::id(),
        tag.len().wrapping_mul(0x5DEECE66D)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Fixed 32-byte test signing key (never used anywhere else; dev/test only).
fn test_signing_key_bytes() -> [u8; 32] {
    let mut b = [0u8; 32];
    b[0] = 0x5A;
    b[1] = 0x11;
    b[2] = 0xE9;
    b[3] = 0xDE;
    b[31] = 0x77;
    b
}

fn test_public_key_bytes() -> [u8; 32] {
    use oep_update::signing::signing_public_key;
    signing_public_key(&test_signing_key_bytes())
}

/// Write a signed offline bundle (manifest + signature + inline payload bytes).
/// `version` is the version the SOURCE offers; `tamper` optionally flips a byte
/// in the serialized file after signing to corrupt the signature or body.
fn write_signed_bundle(
    dir: &Path,
    offered_version: &str,
    payload_bytes: &[u8],
    tamper_byte: Option<usize>,
) -> PathBuf {
    use oep_update::signing::{sign_bundle, BundleFile};
    let manifest = oep_update::Manifest {
        schema: "oep-update-manifest/1".into(),
        product: "oep".into(),
        version: offered_version.into(),
        released: "2026-10-17".into(),
        notes: Some("acceptance fixture".into()),
        payloads: vec![PayloadMeta {
            name: "oep-pdm.bin".into(),
            sha256: oep_update::sha256_hex(payload_bytes),
            size: payload_bytes.len() as u64,
            url: None,
        }],
    };
    let bundle = sign_bundle(&manifest, payload_bytes, &test_signing_key_bytes());
    let file = dir.join("update.bundle.json");
    std::fs::write(&file, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
    if let Some(off) = tamper_byte {
        let mut data = std::fs::read(&file).unwrap();
        let idx = off % data.len();
        data[idx] ^= 0xFF;
        std::fs::write(&file, data).unwrap();
    }
    file
}

// ---------------------------------------------------------------------------
// 1) Launch-without-source: default config is off, check is advisory, launch
//    proceeds and only flags "update unavailable".
// ---------------------------------------------------------------------------

#[test]
fn default_config_is_off_and_launch_proceeds() {
    let cfg = UpdateConfig::default();
    // Non-phoning by default: with an empty source nothing can happen.
    assert!(matches!(cfg.source, UpdateSource::Off));

    let status = check(&cfg);
    assert!(matches!(status, UpdateStatus::Off));

    let decision = Probe::new(&cfg).preflight();
    assert!(matches!(decision, LaunchDecision::Proceed { .. }));
}

#[test]
fn launch_without_source_missing_mirror_flags_unavailable_and_proceeds() {
    let dir = scratch("nosource");
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Mirror(dir.join("does-not-exist")),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    match &status {
        UpdateStatus::Unreachable { detail } => {
            assert!(!detail.is_empty(), "detail must explain the failure");
        }
        other => panic!("expected Unreachable, got {other:?}"),
    }
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}

// An enabled source with a missing signature key must never be trusted: the
// source is untrusted (advisory), launch still proceeds.
#[test]
fn source_without_public_key_is_untrusted_not_consumed() {
    let dir = scratch("nokey");
    let payload: Vec<u8> = vec![0x42; 64];
    let bundle = write_signed_bundle(&dir, "0.2.0", &payload, None);
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: None,
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    assert!(matches!(status, UpdateStatus::Untrusted { .. }));
    assert!(!matches!(status, UpdateStatus::UpdateAvailable { .. }));
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}

// ---------------------------------------------------------------------------
// 2) Launch-with-old-bundle: a signed offline bundle offers a newer version,
//    check flags "update available", launch proceeds (no gate).
// ---------------------------------------------------------------------------

#[test]
fn launch_with_old_bundle_flags_update_available_and_proceeds() {
    let dir = scratch("oldbundle");
    let payload: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
    let bundle = write_signed_bundle(&dir, "0.2.0", &payload, None);

    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(test_public_key_bytes()),
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    match &status {
        UpdateStatus::UpdateAvailable { version, payloads, .. } => {
            assert_eq!(version, "0.2.0");
            assert_eq!(payloads.len(), 1);
            assert_eq!(payloads[0].name, "oep-pdm.bin");
            assert_eq!(payloads[0].size, payload.len() as u64);
        }
        other => panic!("expected UpdateAvailable, got {other:?}"),
    }
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}

#[test]
fn offline_bundle_payload_verifies_against_manifest_sha256() {
    let dir = scratch("payload");
    let payload: Vec<u8> = (0..2048).map(|i| (i * 3 % 257) as u8).collect();
    let bundle = write_signed_bundle(&dir, "0.2.0", &payload, None);

    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(test_public_key_bytes()),
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    let UpdateStatus::UpdateAvailable { payloads, .. } = &status else {
        panic!("expected UpdateAvailable, got {status:?}");
    };

    // The payload referenced by the verified manifest must hash-match.
    assert!(verify_payload(&payloads[0], &payload).is_ok());
}

// ---------------------------------------------------------------------------
// 3) Signed manifest verified BEFORE any download.
// ---------------------------------------------------------------------------

#[test]
fn tampered_bundle_signature_never_exposes_payloads() {
    let dir = scratch("tamper");
    let payload: Vec<u8> = vec![0xAB; 256];
    // Flip a byte somewhere in the serialized bundle (bleeds into the body /
    // signature), so the recomputed signature no longer matches.
    let bundle = write_signed_bundle(&dir, "0.2.0", &payload, Some(140));

    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(test_public_key_bytes()),
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    match &status {
        UpdateStatus::Untrusted { detail } => {
            assert!(!detail.is_empty(), "must explain why the source is untrusted");
        }
        UpdateStatus::Unreachable { .. } => {
            // A tamper can also surface as a parse failure; still never a
            // blocking error and still never an UpdateAvailable.
        }
        other => panic!("expected Untrusted/Unreachable, got {other:?}"),
    }
    // No UpdateAvailable: the tampered bundle must not be consumed.
    assert!(!matches!(status, UpdateStatus::UpdateAvailable { .. }));
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}

#[test]
fn payload_hash_mismatch_is_refused() {
    // A valid manifest but wrong payload bytes must be refused by the
    // verify-payload gate (content not trusted, nothing "downloaded").
    let dir = scratch("hashtamper");
    let payload: Vec<u8> = vec![0x11; 128];
    let bundle = write_signed_bundle(&dir, "0.2.0", &payload, None);
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(test_public_key_bytes()),
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let UpdateStatus::UpdateAvailable { payloads, .. } = check(&cfg) else {
        panic!("bundle is valid: expected UpdateAvailable");
    };
    let wrong: Vec<u8> = vec![0x22; 128];
    assert!(verify_payload(&payloads[0], &wrong).is_err());
}

// ---------------------------------------------------------------------------
// 4) Unreachable remote source: launch proceeds, no analytics reach-back.
// ---------------------------------------------------------------------------

#[test]
fn refused_remote_source_flags_unavailable_and_proceeds() {
    // Bind, grab the port, then drop so nothing listens there.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::RemoteUrl(format!("http://127.0.0.1:{port}/manifest.json")),
        timeout_ms: 250,
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    assert!(matches!(status, UpdateStatus::Unreachable { .. }));
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}

// A local mirror dir whose manifest is missing must also be advisory.
#[test]
fn empty_local_mirror_is_advisory_not_blocking() {
    let dir = scratch("emptymirror");
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Mirror(dir.clone()),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    assert!(matches!(status, UpdateStatus::Unreachable { .. }));
    assert!(matches!(Probe::new(&cfg).preflight(), LaunchDecision::Proceed { .. }));
}
