//! Update-banner surface for the PDM web UI (decision #20).
//!
//! The shipped `oep-web` run resolves the update config once at startup, runs
//! the advisory `Probe::preflight` (logging one advisory line), and holds the
//! status in an [`UpdateBanner`]. Every UI page renders a dismissible banner
//! when the status is `UpdateAvailable` and that offered version has not been
//! dismissed. Page loads NEVER re-probe: the only actions that touch the
//! source are the explicit `check now` endpoint (`GET /ui/update/check`) and
//! the startup check. With an `Off` config (the default) the banner does zero
//! I/O - gated by the crate's `update_surface_gating` tests and proven here
//! structurally (page loads only read the cached status).
//!
//! Dismissal is remembered per offered version, so dismissing `X` hides the
//! banner for `X` but a different offered version still shows. When a persist
//! dir is configured the dismissal set is mirrored to
//! `<persist-dir>/update-dismissals.json` and survives restarts; otherwise it
//! lives in memory for the process.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};

use oep_update::{Probe, UpdateConfig, UpdateStatus};

/// The update-banner state shared by the UI handlers: the last advisory
/// status plus the set of offered versions the operator dismissed.
pub struct UpdateBanner {
    inner: Mutex<BannerInner>,
    /// Config source used at startup and by `check now` (explicit file path,
    /// else the env contract via [`oep_update::config::resolve`]).
    config_path: Option<PathBuf>,
    /// Optional mirror of the dismissed set (persist-dir deployments).
    dismissals_file: Option<PathBuf>,
}

struct BannerInner {
    status: UpdateStatus,
    dismissed: BTreeSet<String>,
}

impl UpdateBanner {
    /// A banner with an explicit status and no persistence. Used by tests and
    /// embedders that already ran the check.
    pub fn new(status: UpdateStatus) -> Self {
        Self::with_options(status, None, None)
    }

    /// The shipped-server constructor: resolve the config (explicit path
    /// wins, else the env contract), run the advisory preflight exactly once,
    /// log its advisory line, and remember dismissals from the given file.
    pub fn from_settings(config_path: Option<PathBuf>, dismissals_file: Option<PathBuf>) -> Self {
        let cfg = load_config(config_path.as_deref());
        let status = run_preflight(&cfg);
        Self::with_options(status, config_path, dismissals_file)
    }

    /// A banner with an explicit status plus dismissal persistence (used when
    /// the caller ran the check itself - the test path).
    pub fn with_dismissals_file(status: UpdateStatus, dismissals_file: Option<PathBuf>) -> Self {
        Self::with_options(status, None, dismissals_file)
    }

    fn with_options(
        status: UpdateStatus,
        config_path: Option<PathBuf>,
        dismissals_file: Option<PathBuf>,
    ) -> Self {
        let dismissed = dismissals_file
            .as_deref()
            .map(load_dismissals)
            .unwrap_or_default();
        UpdateBanner {
            inner: Mutex::new(BannerInner { status, dismissed }),
            config_path,
            dismissals_file,
        }
    }

    /// The current advisory status (a clone; the banner is shared across
    /// handler threads).
    pub fn status(&self) -> UpdateStatus {
        self.inner.lock().unwrap().status.clone()
    }

    /// Whether the given offered version has been dismissed by the operator.
    pub fn is_dismissed(&self, version: &str) -> bool {
        self.inner.lock().unwrap().dismissed.contains(version)
    }

    /// The `check now` action: re-run the advisory preflight against the same
    /// config source and update the banner. Never fails; never blocks launch.
    pub fn refresh(&self) {
        let cfg = load_config(self.config_path.as_deref());
        let status = run_preflight(&cfg);
        self.inner.lock().unwrap().status = status;
    }

    /// Record a dismissal for `version` (per-version, so a newer offer still
    /// shows) and persist when a dismissals file is configured.
    pub fn dismiss(&self, version: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.dismissed.insert(version.to_string());
        }
        if let Some(file) = &self.dismissals_file {
            save_dismissals(file, &self.dismissed_set());
        }
    }

    /// The currently dismissed versions (a clone; a snapshot for persistence).
    pub fn dismissed_set(&self) -> BTreeSet<String> {
        self.inner.lock().unwrap().dismissed.clone()
    }

    /// The banner HTML for a UI page, or the empty string when nothing should
    /// show (status is not an update, or that version was dismissed). Reads
    /// ONLY the cached status: page loads never probe the update source.
    pub fn render_html(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let UpdateStatus::UpdateAvailable {
            version,
            released,
            release_notes_url,
            ..
        } = &inner.status
        else {
            return String::new();
        };
        if inner.dismissed.contains(version) {
            return String::new();
        }
        let mut notes = String::new();
        if let Some(url) = release_notes_url.as_deref() {
            notes.push_str(&format!(
                " <a class=\"update-link\" href=\"{url}\">release notes</a>",
                url = crate::web::esc(url)
            ));
        }
        format!(
            "<div class=\"update-banner\" role=\"status\">\
               <strong>Isolith update available: {version}</strong>\
               <span>released {released}</span>{notes}\
               <a class=\"update-link\" href=\"/ui/update/check\">check now</a>\
               <a class=\"update-link\" href=\"/ui/update/dismiss?version={version}\">dismiss</a>\
             </div>",
            version = crate::web::esc(version),
            released = crate::web::esc(released),
            notes = notes,
        )
    }
}

/// `GET /ui/update/check` -- the explicit "check now" action. Re-runs the
/// advisory preflight (this is the ONLY per-request update I/O) and returns
/// to the explorer.
pub async fn check_now(State((_store, banner)): State<crate::web::AppState>) -> Response {
    banner.refresh();
    Redirect::to("/").into_response()
}

/// `GET /ui/update/dismiss?version=X` -- dismiss the banner for that offered
/// version only. The dismissal is remembered per version (persisted when a
/// persist dir is configured) and returns to the explorer.
pub async fn dismiss_banner(
    State((_store, banner)): State<crate::web::AppState>,
    Query(q): Query<DismissQuery>,
) -> Response {
    banner.dismiss(q.version.trim());
    Redirect::to("/").into_response()
}

/// Query parameters for the dismiss endpoint.
#[derive(serde::Deserialize)]
pub struct DismissQuery {
    /// The offered version whose banner should stop showing.
    pub version: String,
}

/// Resolve the update config: an explicit file path wins; otherwise the env
/// contract (`oep_update::config::resolve()`). A bad explicit file logs and
/// falls back to the non-phoning default - never a launch failure.
fn load_config(path: Option<&Path>) -> UpdateConfig {
    match path {
        Some(p) => match oep_update::config::from_path(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "oep-web: update config {} could not be loaded ({e}); running OFF",
                    p.display()
                );
                UpdateConfig::default()
            }
        },
        None => oep_update::config::resolve(),
    }
}

/// Run the advisory preflight and log its single advisory line.
fn run_preflight(cfg: &UpdateConfig) -> UpdateStatus {
    let status = Probe::new(cfg).preflight().status().clone();
    eprintln!("oep-web: {}", status.advisory_line());
    status
}

/// Dismissal persistence file shape (`<persist-dir>/update-dismissals.json`).
#[derive(serde::Serialize, serde::Deserialize)]
struct DismissalsFile {
    dismissed: Vec<String>,
}

fn load_dismissals(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<DismissalsFile>(&text).ok())
        .map(|d| d.dismissed.into_iter().collect())
        .unwrap_or_default()
}

fn save_dismissals(path: &Path, dismissed: &BTreeSet<String>) {
    let text = serde_json::to_string_pretty(&DismissalsFile {
        dismissed: dismissed.iter().cloned().collect(),
    })
    .expect("dismissals serialize");
    if let Err(e) = std::fs::write(path, text) {
        eprintln!(
            "oep-web: cannot persist update dismissals to {}: {e}",
            path.display()
        );
    }
}
