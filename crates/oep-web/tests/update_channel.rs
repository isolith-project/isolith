//! Decision #20 UPDATE-CHANNEL acceptance over real HTTP: the oep-web settings
//! page exposes the on/off toggle and the one-off failure offer, both persisted
//! to files that survive a server restart.
//!
//! Proves:
//! 1. The settings page renders the tri-state (OFF disabled by user vs ON with
//!    a source) and the toggle writes the persisted settings file.
//! 2. Disabling via the toggle persists across a server RESTART and the banner
//!    re-resolves to Off (the crate gating tests prove zero sockets).
//! 3. A failing check (Unreachable) offers the toggle once; answering (decline
//!    or accept) persists in the offers file and never re-asks - verified by
//!    starting a brand-new banner over the same files.

use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use oep_pdm::http;
use oep_pdm::store::VaultStore;
use oep_update::config;
use oep_update::offer::{OfferAnswer, OfferCondition, OfferLedger};
use oep_update::{UpdateSource, UpdateStatus};
use oep_web::{PdmWeb, UpdateBanner};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oep-web-channel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn raw_get(addr: &str, path: &str) -> http::Response {
    let mut stream = TcpStream::connect(addr).expect("connect to web server");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set timeout");
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).expect("write request");
    stream.flush().ok();
    http::read_response(&mut stream).expect("parse HTTP response")
}

fn dead_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

/// A banner whose cached status is `Unreachable` (the failing-check offer
/// path) with explicit UPDATE-CHANNEL files.
fn unreachable_banner(settings: Option<PathBuf>, offers: Option<PathBuf>) -> Arc<UpdateBanner> {
    let status = UpdateStatus::Unreachable {
        detail: "dead port".into(),
    };
    Arc::new(UpdateBanner::with_status_and_channel_files(
        status, settings, offers,
    ))
}

#[test]
fn settings_page_renders_tri_state_and_toggle_persists() {
    let dir = scratch("settingspage");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");
    let banner = unreachable_banner(Some(settings.clone()), Some(offers));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("web starts");
    let addr = web.addr();

    // Fresh: OFF (never configured) on the settings page (no settings file
    // exists and the banner carries an Unreachable status).
    let page = String::from_utf8_lossy(&raw_get(&addr, "/ui/update/settings").body).into_owned();
    assert!(page.contains("Update settings"), "settings page renders");
    assert!(
        page.contains("OFF (never configured)"),
        "tri-state shown: {page}"
    );

    // Toggle off -> persisted settings file written.
    let resp = raw_get(&addr, "/ui/update/toggle?enabled=off");
    assert!(resp.status == 302 || resp.status == 303, "toggle redirects");
    assert!(settings.is_file(), "toggle persists the settings file");
    let parsed = config::from_path(&settings).expect("written file parses");
    assert!(!parsed.enabled);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failing_check_offer_declined_persists_and_never_reasks() {
    let dir = scratch("offerdecl");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");
    let banner = unreachable_banner(Some(settings.clone()), Some(offers.clone()));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("web starts");
    let addr = web.addr();

    // Pre-seed an enabled remote source so the toggle is "on" and the offer is
    // meaningful (declining must NOT flip it off).
    config::write_settings(
        &settings,
        true,
        &UpdateSource::RemoteUrl(format!("http://127.0.0.1:{}/m.json", dead_port())),
    )
    .unwrap();

    // The settings page carries the pending offer for the unreachable condition.
    let page = String::from_utf8_lossy(&raw_get(&addr, "/ui/update/settings").body).into_owned();
    assert!(
        page.contains("unreachable"),
        "offer names the condition: {page}"
    );

    // Decline via the offer route.
    let resp = raw_get(&addr, "/ui/update/offer/decline?condition=unreachable");
    assert!(
        resp.status == 302 || resp.status == 303,
        "decline redirects"
    );
    assert!(offers.is_file(), "answer persisted");

    // A BRAND-NEW banner/server over the same files must NOT re-ask.
    let banner2 = unreachable_banner(Some(settings.clone()), Some(offers.clone()));
    let web2 = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner2)
        .expect("web2 starts");
    let page2 =
        String::from_utf8_lossy(&raw_get(&web2.addr(), "/ui/update/settings").body).into_owned();
    assert!(
        !page2.contains("unreachable"),
        "declined offer must not re-ask: {page2}"
    );
    // And the settings file was NOT disabled by a decline.
    assert!(config::from_path(&settings).unwrap().enabled);
    let ledger = OfferLedger::load(&offers);
    assert_eq!(
        ledger.answers().get(OfferCondition::Unreachable.key()),
        Some(&OfferAnswer::Declined)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn accept_offer_disables_and_persists_across_restart() {
    let dir = scratch("offeracc");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");
    let banner = unreachable_banner(Some(settings.clone()), Some(offers.clone()));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("web starts");
    let addr = web.addr();

    // Pre-seed an enabled remote source so accepting disables a real check.
    config::write_settings(
        &settings,
        true,
        &UpdateSource::RemoteUrl(format!("http://127.0.0.1:{}/m.json", dead_port())),
    )
    .unwrap();

    let resp = raw_get(&addr, "/ui/update/offer/accept?condition=unreachable");
    assert!(resp.status == 302 || resp.status == 303, "accept redirects");

    // Accepted: settings file now disabled, answer recorded.
    let parsed = config::from_path(&settings).expect("written file parses");
    assert!(!parsed.enabled, "accept disables the toggle");
    let ledger = OfferLedger::load(&offers);
    assert_eq!(
        ledger.answers().get(OfferCondition::Unreachable.key()),
        Some(&OfferAnswer::Accepted)
    );

    // Brand-new banner over the same files: no offer, settings still off.
    let banner2 = unreachable_banner(Some(settings.clone()), Some(offers.clone()));
    let web2 = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner2)
        .expect("web2 starts");
    let page2 =
        String::from_utf8_lossy(&raw_get(&web2.addr(), "/ui/update/settings").body).into_owned();
    assert!(
        !page2.contains("unreachable"),
        "accepted offer must not re-ask"
    );
    assert!(!config::from_path(&settings).unwrap().enabled);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn source_route_applies_and_enables() {
    let dir = scratch("source");
    let settings = dir.join("update-check.json");
    let offers = dir.join("update-offers.json");
    let banner = unreachable_banner(Some(settings.clone()), Some(offers));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("web starts");
    let addr = web.addr();

    let spec = "url:https://up.example/b.json";
    let resp = raw_get(
        &addr,
        &format!("/ui/update/source?source={}", urlencoding_simple(spec)),
    );
    assert!(resp.status == 302 || resp.status == 303);
    let parsed = config::from_path(&settings).expect("written file parses");
    assert!(parsed.enabled, "setting a source enables");
    assert!(matches!(
        parsed.source,
        UpdateSource::RemoteUrl(u) if u == "https://up.example/b.json"
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
