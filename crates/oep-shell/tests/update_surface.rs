//! Decision #20 workbench surfacing: the shell runs `Probe::preflight` exactly
//! once at startup, logs the advisory status, and shows a non-modal notice
//! when an update is available. Nothing in this surface can block launch or
//! change an exit code - the functions return data, they cannot exit.
//!
//! The one-advisory-line and unchanged-exit-code behaviours are exercised
//! end-to-end against the real `oep-workbench` binary in the proof record
//! (workstreams/delivery/update-checker/proof/update-surface-2027.1.md).

use std::path::PathBuf;

use oep_shell::update::{render_notice, run_startup_check};
use oep_update::{UpdateConfig, UpdateSource};

fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oep-shell-update-{tag}-{}", std::process::id()))
}

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

fn write_signed_bundle(dir: &PathBuf, version: &str, notes: Option<String>) -> PathBuf {
    use oep_update::signing::sign_bundle;
    use oep_update::{sha256_hex, Manifest, PayloadMeta};
    let payload: Vec<u8> = (0..16).map(|i| i * 3).collect();
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
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join("update.bundle.json");
    std::fs::write(
        &file,
        serde_json::to_vec_pretty(&sign_bundle(&manifest, &payload, &signing_key())).unwrap(),
    )
    .unwrap();
    file
}

#[test]
fn off_config_surfaces_off_with_no_notice() {
    // Default deployment: advisory line says OFF, nothing to show, no I/O.
    let status = run_startup_check(&UpdateConfig::default());
    assert!(matches!(status, oep_update::UpdateStatus::Off));
    assert!(render_notice(&status).is_none());
}

#[test]
fn unreachable_source_surfaces_unreachable_with_no_notice() {
    // A mirror dir that does not exist: advisory Unreachable, still no notice
    // and no blocking.
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Mirror(scratch("missing").join("nope")),
        ..UpdateConfig::default()
    };
    let status = run_startup_check(&cfg);
    assert!(matches!(
        status,
        oep_update::UpdateStatus::Unreachable { .. }
    ));
    assert!(render_notice(&status).is_none());
}

#[test]
fn update_available_surfaces_non_modal_notice_with_display_fields() {
    let dir = scratch("notice");
    let bundle = write_signed_bundle(
        &dir,
        "2027.1.0",
        Some("https://updates.example/isolith/release-notes/2027.1.0".to_string()),
    );
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: Some(public_key()),
        current_version: "2027.0.0".into(),
        ..UpdateConfig::default()
    };
    let status = run_startup_check(&cfg);
    assert!(matches!(
        status,
        oep_update::UpdateStatus::UpdateAvailable { .. }
    ));
    // Non-modal notice: version + release date + release-notes link.
    let notice = render_notice(&status).expect("UpdateAvailable must render a notice");
    assert!(notice.contains("2027.1.0"), "notice names the version");
    assert!(
        notice.contains("2026-10-17"),
        "notice shows the release date"
    );
    assert!(
        notice.contains("https://updates.example/isolith/release-notes/2027.1.0"),
        "notice carries the release-notes link"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untrusted_source_never_renders_a_notice() {
    let dir = scratch("untrusted");
    let bundle = write_signed_bundle(&dir, "2027.1.0", None);
    // No public key: decision #11 - a keyless source can never be trusted.
    let cfg = UpdateConfig {
        enabled: true,
        source: UpdateSource::Bundle(bundle),
        public_key: None,
        current_version: "2027.0.0".into(),
        ..UpdateConfig::default()
    };
    let status = run_startup_check(&cfg);
    assert!(matches!(status, oep_update::UpdateStatus::Untrusted { .. }));
    assert!(render_notice(&status).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
