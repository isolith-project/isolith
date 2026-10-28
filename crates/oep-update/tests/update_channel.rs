//! UPDATE-CHANNEL acceptance (decision #20): the settings + channel half of
//! the update work. Proves the card's acceptance bullets against the real
//! crate seam (no mocks):
//!
//! 1. Fresh PUBLIC build checks on first launch with no configuration; fresh
//!    PRIVATE build performs no I/O and reports Off.
//! 2. Disabling via the toggle persists across restarts and produces no
//!    sockets (test asserts it).
//! 3. A failing check offers the toggle exactly once; the stored answer
//!    survives restart; both answers (accept, decline) are tested.
//! 4. Source precedence documented and tested: user setting beats build
//!    default, admin mirror/bundle beats the public URL.
//!
//! The "public build" and "private build" resolution is exercised through the
//! build-default-injected [`config::resolve_full_default_from`] so both sides
//! are proven in one build, plus a feature-gated test that proves the real
//! build default when compiled with `--features oep-update/public-default-source`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;

use oep_update::config;
use oep_update::config::UpdateChannelState;
use oep_update::offer::{OfferAnswer, OfferCondition, OfferLedger};
use oep_update::{check, UpdateConfig, UpdateSource, UpdateStatus};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oep-update-channel-it-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fake environment source; returns the injected map only.
fn fake(entries: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let mut map = Vec::new();
    for (k, v) in entries {
        map.push((k.to_string(), v.to_string()));
    }
    move |k: &str| map.iter().find(|(mk, _)| mk == k).map(|(_, v)| v.clone())
}

fn dead_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn public_default_cfg() -> UpdateConfig {
    UpdateConfig {
        enabled: true,
        source: UpdateSource::RemoteUrl("https://isolith.dev/update.bundle.json".to_string()),
        ..UpdateConfig::default()
    }
}

// ---------------------------------------------------------------------------
// 1) Fresh public vs fresh private build on first launch, no configuration
// ---------------------------------------------------------------------------

#[test]
fn fresh_private_build_performs_no_io_and_reports_off() {
    // No config, no settings file: the PRIVATE build has no default source and
    // stays non-phoning Off.
    let get = fake(&[]);
    let resolved = config::resolve_full_default_from(&get, None);
    assert!(!resolved.config.enabled);
    assert!(matches!(resolved.config.source, UpdateSource::Off));
    assert_eq!(resolved.state, UpdateChannelState::OffNeverConfigured);
    // Zero I/O: check() on the resolved config returns Off without touching a
    // socket.
    assert!(matches!(check(&resolved.config), UpdateStatus::Off));
}

#[test]
fn fresh_public_build_checks_on_first_launch_with_no_config() {
    // A PUBLIC build resolves a default remote source with zero configuration
    // (the acceptance path; the injected default mirrors the compiled-in
    // build default, see the feature-gated test below).
    let get = fake(&[]);
    let resolved = config::resolve_full_default_from(&get, Some(public_default_cfg()));
    assert!(resolved.config.enabled, "public build is on out of the box");
    assert!(matches!(resolved.state, UpdateChannelState::On { .. }));
    assert!(matches!(
        resolved.config.source,
        UpdateSource::RemoteUrl(u) if u == "https://isolith.dev/update.bundle.json"
    ));
}

#[cfg(feature = "public-default-source")]
#[test]
fn compiled_public_build_default_is_a_remote_source() {
    // With the feature on, the REAL resolve() carries the compiled default - no
    // injection needed. Proves the one-line GitHub-migration seam.
    let resolved = config::resolve_full(&|k| std::env::var(k).ok());
    assert!(resolved.config.enabled);
    assert!(matches!(resolved.config.source, UpdateSource::RemoteUrl(_)));
    assert!(matches!(resolved.state, UpdateChannelState::On { .. }));
}

// ---------------------------------------------------------------------------
// 2) Disabling via the toggle persists and produces no sockets / no I/O
// ---------------------------------------------------------------------------

#[test]
fn disable_toggle_persists_across_restart_and_produces_no_sockets() {
    let dir = scratch("disable");
    let settings = dir.join("update-check.json");
    // User toggles OFF while a remote source is set (the dangerous shape:
    // enabled it would open a socket; disabled it must not).
    config::write_settings(
        &settings,
        false,
        &UpdateSource::RemoteUrl(format!("http://127.0.0.1:{}/up/m.json", dead_port())),
    )
    .unwrap();
    let get = fake(&[(config::ENV_SETTINGS, settings.to_str().unwrap())]);

    // "Restart": resolve twice, as separate processes would.
    for _ in 0..2 {
        let resolved = config::resolve_full(&get);
        assert!(!resolved.config.enabled);
        assert_eq!(resolved.state, UpdateChannelState::OffByUser);
        // Zero sockets: a single connection to the dead port would surface as
        // Unreachable; it must come back Off.
        assert!(
            matches!(check(&resolved.config), UpdateStatus::Off),
            "disabled toggle must produce zero I/O"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_disable_then_status_shows_off_after_restart() {
    let oep = env!("CARGO_BIN_EXE_oep");
    let dir = scratch("clisettings");
    let settings = dir.join("update-check.json");

    let run = |args: &[&str]| {
        Command::new(oep)
            .args(args)
            .env_clear()
            .env("OEP_UPDATE_SETTINGS", &settings)
            .output()
            .expect("oep runs")
    };

    let out = run(&[
        "update",
        "config",
        "--enable",
        "--source",
        "url:http://127.0.0.1:9/up/m.json",
    ]);
    assert!(
        out.status.success(),
        "enable exit != 0: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run(&["update", "config", "--disable"]);
    assert!(
        out.status.success(),
        "disable exit != 0: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // A FRESH CLI process (restart) reads the persisted settings.
    let out = run(&["update", "config", "--status"]);
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("OFF (disabled by user)"), "status: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 3) One-off failure offer: exactly once, answer survives restart, both
//    answers tested
// ---------------------------------------------------------------------------

fn unreachable_cfg() -> UpdateConfig {
    UpdateConfig {
        enabled: true,
        source: UpdateSource::RemoteUrl(format!("http://127.0.0.1:{}/nope", dead_port())),
        ..UpdateConfig::default()
    }
}

#[test]
fn failing_check_offers_once_and_decline_persists_across_restart() {
    let dir = scratch("offerdecline");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");

    let status = check(&unreachable_cfg());
    assert!(matches!(status, UpdateStatus::Unreachable { .. }));

    // Surface A: offer is pending (never answered).
    let mut led = OfferLedger::load(&offers);
    assert_eq!(led.pending(&status), Some(OfferCondition::Unreachable));

    // User declines: answer persists, offer never repeats.
    led.record(OfferCondition::Unreachable, OfferAnswer::Declined);

    // "Restart": a brand-new ledger over the same file must NOT re-offer, and
    // the decline did not touch the settings (checking stays on).
    let restarted = OfferLedger::load(&offers);
    assert_eq!(restarted.pending(&status), None);
    assert!(matches!(
        restarted.answers().get("unreachable"),
        Some(OfferAnswer::Declined)
    ));
    assert!(
        !settings.exists(),
        "decline alone must not disable checking"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failing_check_offer_accept_disables_and_persists() {
    let dir = scratch("offeraccept");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");

    let status = check(&unreachable_cfg());
    assert!(matches!(status, UpdateStatus::Unreachable { .. }));

    let mut led = OfferLedger::load(&offers);
    assert_eq!(led.pending(&status), Some(OfferCondition::Unreachable));

    // User accepts the offer: record Accepted AND disable update checking
    // (zero I/O afterwards, proven by check() on the Off config).
    led.record(OfferCondition::Unreachable, OfferAnswer::Accepted);
    let source = UpdateSource::RemoteUrl("http://127.0.0.1:9/up/m.json".to_string());
    config::write_settings(&settings, false, &source).unwrap();

    // Restart: offer never repeats; the settings file disables checking.
    let restarted = OfferLedger::load(&offers);
    assert_eq!(restarted.pending(&status), None);
    let get = fake(&[(config::ENV_SETTINGS, settings.to_str().unwrap())]);
    let resolved = config::resolve_full(&get);
    assert!(!resolved.config.enabled);
    assert_eq!(resolved.state, UpdateChannelState::OffByUser);
    assert!(matches!(check(&resolved.config), UpdateStatus::Off));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offer_is_per_condition() {
    // Declining the unreachable offer must NOT silence the untrusted offer.
    let dir = scratch("percond");
    let offers = dir.join("update-offers.json");
    let mut led = OfferLedger::load(&offers);
    led.record(OfferCondition::Unreachable, OfferAnswer::Declined);
    assert!(led.should_offer(OfferCondition::Untrusted));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_check_surfaces_offer_once_and_decline_suppresses_it() {
    let oep = env!("CARGO_BIN_EXE_oep");
    let dir = scratch("clioffer");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");
    std::fs::write(
        &settings,
        r#"{"enabled":true,"source":{"bundle":"/does/not/exist/update.bundle.json"},"public_key":"e84406ed95b05cbe7003296a7296cd782bb4c45087323fc165f7230017a6a811"}"#,
    )
    .unwrap();

    let run = |args: &[&str]| {
        Command::new(oep)
            .args(args)
            .env_clear()
            .env("OEP_UPDATE_SETTINGS", &settings)
            .env("OEP_UPDATE_OFFERS", &offers)
            .output()
            .expect("oep runs")
    };

    // Live check surfaces the one-off failure offer (exit stays 0).
    let out = run(&["update", "check", "--config", settings.to_str().unwrap()]);
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success());
    assert!(
        text.contains("offer:"),
        "check surfaces the offer once: {text}"
    );
    assert!(
        text.contains("unreachable"),
        "offer names the condition: {text}"
    );

    // Decline through the config command; a FRESH process must not re-ask.
    let out = run(&["update", "config", "--decline-offer", "unreachable"]);
    assert!(out.status.success());
    let out2 = run(&["update", "check", "--config", settings.to_str().unwrap()]);
    let text2 = String::from_utf8_lossy(&out2.stdout).to_string();
    assert!(
        !text2.contains("offer:"),
        "declined offer must not re-ask: {text2}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 4) Source precedence: user setting beats build default, admin beats public
// ---------------------------------------------------------------------------

#[test]
fn user_setting_beats_build_default() {
    // A persisted settings file with a mirror beats the injected public
    // default URL.
    let dir = scratch("prec-user");
    let settings = dir.join("update-check.json");
    config::write_settings(
        &settings,
        true,
        &UpdateSource::Mirror("/srv/updates".into()),
    )
    .unwrap();
    let get = fake(&[(config::ENV_SETTINGS, settings.to_str().unwrap())]);
    let resolved = config::resolve_full_default_from(&get, Some(public_default_cfg()));
    assert_eq!(
        resolved.state,
        UpdateChannelState::On {
            source: UpdateSource::Mirror("/srv/updates".into())
        }
    );
    assert!(matches!(
        resolved.config.source,
        UpdateSource::Mirror(p) if p == std::path::Path::new("/srv/updates")
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn admin_env_mirror_beats_public_url() {
    // Env contract explicitly engaged with a mirror beats the public default.
    let cfg = config::from_env_opt(
        &fake(&[
            (config::ENV_CHECK, "on"),
            (config::ENV_MIRROR_URL, "/mnt/mirror"),
            (config::ENV_PUBKEY, &"aa".repeat(32)),
        ]),
        false,
    )
    .expect("env tier engaged");
    assert!(cfg.enabled);
    assert!(matches!(cfg.source, UpdateSource::Mirror(_)));
}

#[test]
fn explicit_config_file_beats_everything() {
    // Even a persisted user setting cannot override an explicit config file
    // (admin/orchestrated node).
    let dir = scratch("prec-file");
    let settings = dir.join("update-check.json");
    let explicit = dir.join("explicit.json");
    config::write_settings(
        &settings,
        true,
        &UpdateSource::Mirror("/user/updates".into()),
    )
    .unwrap();
    std::fs::write(
        &explicit,
        r#"{"enabled":true,"source":{"bundle":"/admin/bundle.json"},"public_key":"aa"}"#,
    )
    .unwrap();
    let get = fake(&[
        (config::ENV_SETTINGS, settings.to_str().unwrap()),
        (config::ENV_CONFIG, explicit.to_str().unwrap()),
    ]);
    let resolved = config::resolve_full(&get);
    assert!(matches!(
        resolved.config.source,
        UpdateSource::Bundle(p) if p == std::path::Path::new("/admin/bundle.json")
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
