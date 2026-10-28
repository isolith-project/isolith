//! Decision #20 surfacing gates: the update surfaces must NEVER add network
//! I/O when the feature is off or the source is `Off` (decision #11 stays:
//! non-phoning by default). Unlike the acceptance suite (which stays
//! UNTOUCHED in `update_checker_acceptance.rs`), this file proves the surface
//! config-resolution seam itself - the path the workbench, oep-web and the
//! `oep` CLI all take.

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use oep_update::config;
use oep_update::{check, sha256_hex, Probe, UpdateConfig, UpdateSource, UpdateStatus};

/// Scratch dir unique per process (repo convention).
fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oep-update-gating-{tag}-{}", std::process::id()))
}

/// The 32-byte dev/test signing key (same bytes as the acceptance suite).
fn signing_key() -> [u8; 32] {
    let mut b = [0u8; 32];
    b[0] = 0x5A;
    b[1] = 0x11;
    b[2] = 0xE9;
    b[3] = 0xDE;
    b[31] = 0x77;
    b
}

fn public_key() -> [u8; 32] {
    oep_update::signing::signing_public_key(&signing_key())
}

/// A dead TCP port that nothing listens on (the connect attempt, if any,
/// must fail fast - this is the I/O canary).
fn dead_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn write_signed_bundle(dir: &Path, version: &str, notes: Option<String>) -> PathBuf {
    use oep_update::signing::sign_bundle;
    use oep_update::{Manifest, PayloadMeta};
    let payload: Vec<u8> = (0..32).map(|i| (i % 251) as u8).collect();
    let manifest = Manifest {
        schema: "oep-update-manifest/1".into(),
        product: "oep".into(),
        version: version.into(),
        released: "2026-10-17".into(),
        notes,
        payloads: vec![PayloadMeta {
            name: "payload.bin".into(),
            sha256: sha256_hex(&payload),
            size: payload.len() as u64,
            url: None,
        }],
    };
    let bundle = sign_bundle(&manifest, &payload, &signing_key());
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join("update.bundle.json");
    std::fs::write(&file, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
    file
}

// ---------------------------------------------------------------------------
// 1) Feature off => Off, and the I/O canary proves nothing connected.
// ---------------------------------------------------------------------------

#[test]
fn disabled_config_with_remote_source_does_zero_io() {
    // enabled=false against a dead remote: if a single socket were opened, the
    // connect to a closed port would fail into Unreachable. It must return Off.
    let cfg = UpdateConfig {
        enabled: false,
        source: UpdateSource::RemoteUrl(format!("http://127.0.0.1:{}/m.json", dead_port())),
        ..UpdateConfig::default()
    };
    assert!(matches!(check(&cfg), UpdateStatus::Off));
    assert!(matches!(
        Probe::new(&cfg).preflight(),
        oep_update::LaunchDecision::Proceed { .. }
    ));
}

#[test]
fn disabled_config_with_bundle_source_reads_no_file() {
    // enabled=false against a bundle path that does not exist: no file read,
    // no error, plain Off.
    let missing = scratch("nofile").join("update.bundle.json");
    let cfg = UpdateConfig {
        enabled: false,
        source: UpdateSource::Bundle(missing),
        ..UpdateConfig::default()
    };
    assert!(matches!(check(&cfg), UpdateStatus::Off));
}

#[test]
fn source_off_does_zero_io_even_when_enabled_flag_set() {
    // The master switch can be on while the source is Off (env contract: on
    // with no source). Still zero I/O - the source guard fires first.
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Off,
        ..UpdateConfig::default()
    };
    assert!(matches!(check(&cfg), UpdateStatus::Off));
}

// ---------------------------------------------------------------------------
// 2) The JSON config file the surfaces share honours the same gates.
// ---------------------------------------------------------------------------

fn write_config(dir: &Path, enabled: bool, remote: bool) -> (PathBuf, u16) {
    let port = dead_port();
    let source = if remote {
        format!(r#""source": {{"url": "http://127.0.0.1:{port}/m.json"}}"#)
    } else {
        r#""source": null"#.to_string()
    };
    let text = format!(
        r#"{{"enabled": {}, {}, "public_key": null, "current_version": "0.1.0", "timeout_ms": 250}}"#,
        enabled, source
    );
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join("update-check.json");
    std::fs::write(&file, text).unwrap();
    (file, port)
}

#[test]
fn config_file_disabled_does_zero_io() {
    let dir = scratch("cfgdis");
    let (file, _port) = write_config(&dir, false, true);
    let cfg = config::from_path(&file).expect("config parses");
    assert!(!cfg.enabled);
    assert!(matches!(cfg.source, UpdateSource::RemoteUrl(_)));
    // Off status = no socket was opened against the canary port.
    assert!(matches!(check(&cfg), UpdateStatus::Off));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_file_enabled_touches_network_and_flags_unreachable() {
    // Same file, master switch on: now the I/O happens and the dead port
    // surfaces as Unreachable - proving the gate above is what stopped it.
    let dir = scratch("cfgen");
    let (file, _port) = write_config(&dir, true, true);
    let cfg = config::from_path(&file).expect("config parses");
    assert!(matches!(check(&cfg), UpdateStatus::Unreachable { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 3) Advisory text is byte-stable (proofs and scripts assert on it).
// ---------------------------------------------------------------------------

#[test]
fn advisory_lines_are_byte_stable() {
    assert_eq!(
        UpdateStatus::Off.advisory_line(),
        "update check: OFF (no source configured; non-phoning default)"
    );
    assert_eq!(
        UpdateStatus::Unreachable {
            detail: "boom".into()
        }
        .advisory_line(),
        "update check: unavailable (boom)"
    );
    assert_eq!(
        UpdateStatus::Untrusted {
            detail: "boom".into()
        }
        .advisory_line(),
        "update check: source NOT TRUSTED - no payloads consumed (boom)"
    );
    assert_eq!(
        UpdateStatus::UpToDate.advisory_line(),
        "update check: up to date"
    );
    assert_eq!(
        UpdateStatus::UpdateAvailable {
            version: "0.2.0".into(),
            released: "2026-10-17".into(),
            release_notes_url: None,
            payloads: vec![],
        }
        .advisory_line(),
        "update check: update available -> 0.2.0 (advisory; install is a separate operator action)"
    );
}

// ---------------------------------------------------------------------------
// 4) The notice carries version, release date and the release-notes link.
// ---------------------------------------------------------------------------

#[test]
fn update_available_notice_carries_display_fields() {
    let dir = scratch("notice");
    let bundle = write_signed_bundle(
        &dir,
        "0.9.0",
        Some("https://updates.example/isolith/release-notes/0.9.0".to_string()),
    );
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(public_key()),
        current_version: "0.1.0".into(),
        ..UpdateConfig::default()
    };
    let status = check(&cfg);
    let notice = match &status {
        UpdateStatus::UpdateAvailable { .. } => status.notice().expect("notice present"),
        other => panic!("expected UpdateAvailable, got {other:?}"),
    };
    assert_eq!(notice.version, "0.9.0");
    assert_eq!(notice.released, "2026-10-17");
    assert_eq!(
        notice.release_notes_url.as_deref(),
        Some("https://updates.example/isolith/release-notes/0.9.0")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_available_statuses_have_no_notice() {
    for status in [
        UpdateStatus::Off,
        UpdateStatus::Unreachable { detail: "x".into() },
        UpdateStatus::Untrusted { detail: "x".into() },
        UpdateStatus::UpToDate,
    ] {
        assert!(status.notice().is_none(), "{status:?} must have no notice");
    }
}
