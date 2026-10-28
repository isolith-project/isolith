//! Decision #20 banner acceptance: the PDM web UI shows a dismissible,
//! advisory update banner driven by the SAME `oep-update` status the
//! workbench and CLI expose.
//!
//! Hard behaviours proven here:
//! 1. `UpdateAvailable` renders the banner on the explorer page (version,
//!    release date, release-notes link, check-now + dismiss actions).
//! 2. Dismissal is remembered PER VERSION: dismissing X hides X but a
//!    different offered version still shows.
//! 3. With an `Off`/UpToDate/untrusted status (or a dismissed version) the
//!    banner is absent - and page loads never re-probe the source.
//! 4. Dismissal persists to the persist dir (`update-dismissals.json`) and
//!    survives a server restart (new banner, same file).
//! 5. The explicit `check now` route re-runs the advisory preflight and
//!    redirects; with an off config it performs zero I/O and shows nothing.

use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use oep_pdm::http;
use oep_pdm::store::VaultStore;
use oep_update::{PayloadMeta, UpdateStatus};
use oep_web::{PdmWeb, UpdateBanner};

fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oep-web-banner-{tag}-{}", std::process::id()))
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

fn update_available(version: &str) -> UpdateStatus {
    UpdateStatus::UpdateAvailable {
        version: version.to_string(),
        released: "2026-10-17".to_string(),
        release_notes_url: Some(format!(
            "https://updates.example/isolith/release-notes/{version}"
        )),
        payloads: vec![PayloadMeta {
            name: "oep-pdm.bin".to_string(),
            sha256: "ab".repeat(32),
            size: 4096,
            url: None,
        }],
    }
}

// ---------------------------------------------------------------------------
// 1) UpdateAvailable renders the banner (version / date / notes / actions)
// ---------------------------------------------------------------------------

#[test]
fn update_available_renders_the_banner_on_the_page() {
    let banner = Arc::new(UpdateBanner::new(update_available("9.9.9")));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("server starts");
    let resp = raw_get(&web.addr(), "/");
    assert_eq!(resp.status, 200);
    let page = String::from_utf8_lossy(&resp.body).into_owned();
    assert!(
        page.contains("Isolith update available"),
        "banner text present"
    );
    assert!(page.contains("9.9.9"), "banner names the offered version");
    assert!(page.contains("2026-10-17"), "banner shows the release date");
    assert!(
        page.contains("release-notes/9.9.9"),
        "banner links to the release notes"
    );
    assert!(
        page.contains("check now"),
        "banner has the check-now action"
    );
    assert!(page.contains("dismiss"), "banner has the dismiss action");
    assert!(
        page.contains("/ui/update/dismiss?version=9.9.9"),
        "dismiss carries the offered version"
    );
}

// ---------------------------------------------------------------------------
// 2) Per-version dismissal: dismissing X hides X only
// ---------------------------------------------------------------------------

#[test]
fn dismiss_stops_the_banner_for_that_version_only() {
    // Library-level: the per-version rule without the HTTP dance.
    let b = UpdateBanner::new(update_available("9.9.9"));
    assert!(b.render_html().contains("9.9.9"));
    b.dismiss("9.9.9");
    assert!(
        b.render_html().is_empty(),
        "dismissed version renders no banner"
    );

    // Dismissing a DIFFERENT version must NOT hide 10.0.0.
    let b2 = UpdateBanner::new(update_available("10.0.0"));
    b2.dismiss("9.9.9");
    assert!(
        b2.render_html().contains("10.0.0"),
        "a newer offered version still shows after dismissing another"
    );
}

#[test]
fn dismiss_route_hides_the_banner_over_http() {
    let banner = Arc::new(UpdateBanner::new(update_available("9.9.9")));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("server starts");
    let addr = web.addr();

    let before = String::from_utf8_lossy(&raw_get(&addr, "/").body).into_owned();
    assert!(
        before.contains("Isolith update available"),
        "banner visible before dismiss"
    );

    let resp = raw_get(&addr, "/ui/update/dismiss?version=9.9.9");
    // Redirect (303) back to the explorer.
    assert!(
        resp.status == 303 || resp.status == 302,
        "dismiss redirects"
    );

    let after = String::from_utf8_lossy(&raw_get(&addr, "/").body).into_owned();
    assert!(
        !after.contains("Isolith update available"),
        "dismissed version no longer banners on the next load"
    );
    assert!(!after.contains("9.9.9"), "no leftover offered version");
}

// ---------------------------------------------------------------------------
// 3) Non-update statuses render nothing; page loads never re-probe
// ---------------------------------------------------------------------------

#[test]
fn off_and_uptodate_render_no_banner() {
    for status in [
        UpdateStatus::Off,
        UpdateStatus::UpToDate,
        UpdateStatus::Unreachable {
            detail: "nobody home".into(),
        },
        UpdateStatus::Untrusted {
            detail: "no key".into(),
        },
    ] {
        let banner = Arc::new(UpdateBanner::new(status));
        let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
            .expect("server starts");
        let resp = raw_get(&web.addr(), "/");
        assert_eq!(resp.status, 200);
        let page = String::from_utf8_lossy(&resp.body).into_owned();
        assert!(
            !page.contains("Isolith update available"),
            "no banner for a non-update status"
        );
    }
}

// ---------------------------------------------------------------------------
// 4) Dismissal persists across restarts when a persist dir is set
// ---------------------------------------------------------------------------

#[test]
fn dismissals_persist_to_the_persist_dir_and_survive_restart() {
    let dir = scratch("persist");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("update-dismissals.json");

    let b = UpdateBanner::with_dismissals_file(update_available("9.9.9"), Some(file.clone()));
    b.dismiss("9.9.9");
    assert!(file.exists(), "dismissal written to the persist dir");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("9.9.9"),
        "version recorded in the dismissal file"
    );
    assert!(b.render_html().is_empty());

    // A brand-new banner (fresh server) over the SAME file remembers it.
    let b2 = UpdateBanner::with_dismissals_file(update_available("9.9.9"), Some(file.clone()));
    assert!(
        b2.render_html().is_empty(),
        "dismissal remembered across server restarts"
    );
    // But a DIFFERENT offered version still shows.
    let b3 = UpdateBanner::with_dismissals_file(update_available("10.0.0"), Some(file.clone()));
    assert!(b3.render_html().contains("10.0.0"));

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// 5) check now redirects; with an off config it is clean and does zero I/O
// ---------------------------------------------------------------------------

#[test]
fn check_now_redirects_and_stays_clean_when_off() {
    // from_settings(None, None) = resolve from the (empty) test env -> Off.
    let banner = Arc::new(UpdateBanner::from_settings(None, None));
    let web = PdmWeb::start_with_store_and_update("127.0.0.1:0", VaultStore::new(), banner)
        .expect("server starts");
    let addr = web.addr();

    // Over an Off config the banner never shows on page loads.
    let page = String::from_utf8_lossy(&raw_get(&addr, "/").body).into_owned();
    assert!(!page.contains("Isolith update available"));

    // The explicit check-now action redirects and stays invisible (the Off
    // gate performs zero I/O - proven at the crate level in
    // crates/oep-update/tests/update_surface_gating.rs).
    let resp = raw_get(&addr, "/ui/update/check");
    assert!(
        resp.status == 303 || resp.status == 302,
        "check-now redirects"
    );
    let page = String::from_utf8_lossy(&raw_get(&addr, "/").body).into_owned();
    assert!(!page.contains("Isolith update available"));
}
