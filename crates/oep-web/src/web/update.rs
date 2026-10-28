//! Update-banner + settings surface for the PDM web UI (decision #20 +
//! UPDATE-CHANNEL).
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
//! ## UPDATE-CHANNEL (decision #20): settings toggle + one-off offer
//!
//! - The settings page (`GET /ui/update/settings`) shows the tri-state and an
//!   on/off control plus a source field; the toggle persists to the settings
//!   file (`<settings-path>` = `--settings PATH`, else the config-dir default
//!   that `oep_update::config` resolves) - the SAME file the `oep update
//!   config` CLI and the workbench read. Turning it off writes
//!   `enabled:false`, and the crate's gating proves that means zero sockets.
//! - When a check returns `Unreachable` or `Untrusted`, the server offers ONCE
//!   to turn update checking off. The answer is stored per condition in
//!   `<offers-file>` (`update-offers.json` next to the settings file), so it
//!   survives restarts and never repeats for that condition; declining
//!   (`/ui/update/offer/decline`) does not re-ask on the next launch, and
//!   accepting (`/ui/update/offer/accept`) also disables checking.
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

use oep_update::config::{self, UpdateChannelState};
use oep_update::offer::{OfferAnswer, OfferCondition, OfferLedger};
use oep_update::{Probe, UpdateConfig, UpdateSource, UpdateStatus};

/// The update-banner state shared by the UI handlers: the last advisory
/// status plus the set of offered versions the operator dismissed.
pub struct UpdateBanner {
    inner: Mutex<BannerInner>,
    /// Config source used at startup and by `check now` (explicit file path,
    /// else the env contract via [`oep_update::config::resolve`]).
    config_path: Option<PathBuf>,
    /// Persisted-settings path for the toggle (UPDATE-CHANNEL): the file the
    /// toggle writes and `config::resolve` reads.
    settings_path: Option<PathBuf>,
    /// Optional mirror of the dismissed set (persist-dir deployments).
    dismissals_file: Option<PathBuf>,
    /// One-off failure-offer ledger (UPDATE-CHANNEL): where per-condition
    /// answers are persisted.
    offers_file: Option<PathBuf>,
}

struct BannerInner {
    status: UpdateStatus,
    dismissed: BTreeSet<String>,
}

impl UpdateBanner {
    /// A banner with an explicit status and no persistence. Used by tests and
    /// embedders that already ran the check.
    pub fn new(status: UpdateStatus) -> Self {
        Self::with_options(status, None, None, None, None)
    }

    /// The shipped-server constructor: resolve the config (explicit path
    /// wins, else the persisted settings / env contract), run the advisory
    /// preflight exactly once, log its advisory line, and remember dismissals
    /// from the given file.
    pub fn from_settings(config_path: Option<PathBuf>, dismissals_file: Option<PathBuf>) -> Self {
        let cfg = load_config(config_path.as_deref(), None);
        let status = run_preflight(&cfg);
        Self::with_options(status, config_path, None, dismissals_file, None)
    }

    /// The shipped-server constructor with UPDATE-CHANNEL wiring: an explicit
    /// settings file (toggle path) and an offer ledger.
    pub fn with_channel_files(
        config_path: Option<PathBuf>,
        settings_path: Option<PathBuf>,
        dismissals_file: Option<PathBuf>,
        offers_file: Option<PathBuf>,
    ) -> Self {
        let cfg = load_config(config_path.as_deref(), settings_path.as_deref());
        let status = run_preflight(&cfg);
        Self::with_options(
            status,
            config_path,
            settings_path,
            dismissals_file,
            offers_file,
        )
    }

    /// A banner with an explicit status plus dismissal persistence (used when
    /// the caller ran the check itself - the test path).
    pub fn with_dismissals_file(status: UpdateStatus, dismissals_file: Option<PathBuf>) -> Self {
        Self::with_options(status, None, None, dismissals_file, None)
    }

    /// A banner with an explicit status plus UPDATE-CHANNEL paths: the
    /// persisted-settings file (accept disables here) and the one-off
    /// failure-offer ledger (used by tests and embedders that ran the check
    /// themselves and need the offer to target specific files).
    pub fn with_status_and_channel_files(
        status: UpdateStatus,
        settings_path: Option<PathBuf>,
        offers_file: Option<PathBuf>,
    ) -> Self {
        Self::with_options(status, None, settings_path, None, offers_file)
    }

    fn with_options(
        status: UpdateStatus,
        config_path: Option<PathBuf>,
        settings_path: Option<PathBuf>,
        dismissals_file: Option<PathBuf>,
        offers_file: Option<PathBuf>,
    ) -> Self {
        let dismissed = dismissals_file
            .as_deref()
            .map(load_dismissals)
            .unwrap_or_default();
        UpdateBanner {
            inner: Mutex::new(BannerInner { status, dismissed }),
            config_path,
            settings_path,
            dismissals_file,
            offers_file,
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
        let cfg = load_config(self.config_path.as_deref(), self.settings_path.as_deref());
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

    // --- UPDATE-CHANNEL: settings toggle + one-off offer -------------------

    /// The resolved settings path (for the settings page to display and the
    /// toggle to write).
    pub fn settings_path(&self) -> Option<&Path> {
        self.settings_path.as_deref()
    }

    /// Apply a toggle: persist `enabled` (+ optional `source`) to the settings
    /// file, then re-resolve and refresh the banner so the page reflects the
    /// new state immediately. Returns the new tri-state.
    pub fn apply_toggle(
        &self,
        enabled: bool,
        source: &oep_update::UpdateSource,
    ) -> UpdateChannelState {
        let path = self.settings_path.clone().or_else(default_settings_path);
        if let Some(p) = &path {
            if let Err(e) = config::write_settings(p, enabled, source) {
                eprintln!(
                    "oep-web: cannot persist update settings {}: {e}",
                    p.display()
                );
            }
        } else {
            eprintln!("oep-web: no settings path to persist the toggle (set HOME/XDG_CONFIG_HOME or OEP_UPDATE_SETTINGS)");
        }
        let cfg = load_config(self.config_path.as_deref(), self.settings_path.as_deref());
        let status = run_preflight(&cfg);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.status = status;
        }
        self.resolved_state()
    }

    /// The tri-state the SERVER actually resolves (same precedence as
    /// [`load_config`]: explicit config path, then the persisted settings
    /// file, then the env ladder), for the settings page and after a toggle.
    pub fn resolved_state(&self) -> UpdateChannelState {
        if let Some(cp) = &self.config_path {
            if let Ok(cfg) = config::from_path(cp) {
                return trip_type(&cfg);
            }
        }
        if let Some(p) = &self.settings_path {
            if let Ok(cfg) = config::from_path(p) {
                return trip_type(&cfg);
            }
        }
        config::resolve_full(&|k| std::env::var(k).ok()).state
    }

    /// The `--source` equivalent for the settings page: persist the given
    /// source (implies enabled) and refresh.
    pub fn apply_source(&self, source: &oep_update::UpdateSource) -> UpdateChannelState {
        self.apply_toggle(true, source)
    }

    /// Whether an offer is currently pending for the banner's status (an
    /// Unreachable/Untrusted condition with no stored answer).
    pub fn pending_offer(&self) -> Option<OfferCondition> {
        let status = self.status();
        self.ledger().pending(&status)
    }

    /// The stored answer for a condition, if any.
    pub fn offer_answer(&self, condition: OfferCondition) -> Option<OfferAnswer> {
        self.ledger().answers().get(condition.key()).copied()
    }

    /// The offer ledger for this banner: persisted to the offers file when one
    /// is configured, else an empty in-memory ledger (answers can still be
    /// given; they are persisted to the default offers path by [`record_offer`]).
    fn ledger(&self) -> OfferLedger {
        match &self.offers_file {
            Some(p) => OfferLedger::load(p),
            None => OfferLedger::load(Path::new("")),
        }
    }

    /// Accept an offer for `condition`: record Accepted AND disable update
    /// checking (persist `enabled:false`). Never re-asks afterwards.
    pub fn accept_offer(&self, condition: OfferCondition) {
        self.record_offer(condition, OfferAnswer::Accepted);
        let path = self.settings_path.clone().or_else(default_settings_path);
        if let Some(p) = &path {
            // Keep the current source so a later re-enable remembers it.
            let cfg = load_config(self.config_path.as_deref(), self.settings_path.as_deref());
            if let Err(e) = config::write_settings(p, false, &cfg.source) {
                eprintln!(
                    "oep-web: cannot persist update settings {}: {e}",
                    p.display()
                );
            }
        }
        self.refresh();
    }

    /// Decline an offer for `condition`: record Declined (no disable). Never
    /// re-asks about this condition on the next launch.
    pub fn decline_offer(&self, condition: OfferCondition) {
        self.record_offer(condition, OfferAnswer::Declined);
    }

    fn record_offer(&self, condition: OfferCondition, answer: OfferAnswer) {
        let path = self
            .offers_file
            .clone()
            .or_else(|| default_settings_path().map(|p| p.with_file_name("update-offers.json")));
        if let Some(p) = &path {
            let mut ledger = OfferLedger::load(p);
            ledger.record(condition, answer);
        } else {
            eprintln!("oep-web: no offers file; answer not persisted");
        }
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

fn default_settings_path() -> Option<PathBuf> {
    config::settings_path_from(&|k| std::env::var(k).ok())
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

/// `GET /ui/update/settings` -- the UPDATE-CHANNEL settings page: tri-state,
/// an on/off control, and a source field. Persists to the settings file.
pub async fn settings_page(State((_store, banner)): State<crate::web::AppState>) -> Response {
    let state = match banner.resolved_state() {
        UpdateChannelState::On { source } => match source {
            oep_update::UpdateSource::Off => "ON (no source configured)".to_string(),
            oep_update::UpdateSource::Mirror(p) => {
                format!("ON (source: mirror {})", p.display())
            }
            oep_update::UpdateSource::RemoteUrl(u) => format!("ON (source: {u})"),
            oep_update::UpdateSource::Bundle(p) => {
                format!("ON (source: bundle {})", p.display())
            }
        },
        UpdateChannelState::OffByUser => "OFF (disabled by user)".to_string(),
        UpdateChannelState::OffNeverConfigured => "OFF (never configured)".to_string(),
    };
    let settings_path = banner
        .settings_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unset".to_string());
    let offer = banner
        .pending_offer()
        .map(|c| {
            format!(
                "<p class=\"offer\">A check failed for condition <code>{}</code>: \
                 to stop update checking, \
                 <a href=\"/ui/update/offer/accept?condition={}\">accept</a> or \
                 <a href=\"/ui/update/offer/decline?condition={}\">decline</a> (the answer persists; \
                 declining will not re-ask for this condition).</p>",
                c.key(),
                c.key(),
                c.key()
            )
        })
        .unwrap_or_default();
    let body = format!(
        "<h1>Update settings</h1>\
         <p>State: <strong>{state}</strong> (settings file: <code>{settings_path}</code>)</p>\
         {offer}\
         <h2>On / off</h2>\
         <p><a class=\"update-link\" href=\"/ui/update/toggle?enabled=off\">turn update checking off</a>\
         &nbsp;|&nbsp; \
         <a class=\"update-link\" href=\"/ui/update/toggle?enabled=on\">turn update checking on</a></p>\
         <h2>Source</h2>\
         <form method=\"get\" action=\"/ui/update/source\">\
           <label>Source (url:&lt;URL&gt;, mirror:&lt;PATH&gt;, bundle:&lt;PATH&gt;, or off): \
           <input type=\"text\" name=\"source\" value=\"\" size=\"48\"></label> \
           <button type=\"submit\">set source</button>\
         </form>\
         <p><a href=\"/\">back to explorer</a></p>",
    );
    crate::web::page("Update settings", &body)
}

/// `GET /ui/update/toggle?enabled=on|off` -- the on/off control. Persists and
/// refreshes.
pub async fn toggle_update(
    State((_store, banner)): State<crate::web::AppState>,
    Query(q): Query<ToggleQuery>,
) -> Response {
    let enabled = q.enabled.trim().eq_ignore_ascii_case("on")
        || q.enabled.trim() == "1"
        || q.enabled.trim().eq_ignore_ascii_case("true");
    banner.apply_toggle(enabled, &UpdateSource::Off);
    Redirect::to("/ui/update/settings").into_response()
}

/// Query parameters for the toggle endpoint.
#[derive(serde::Deserialize)]
pub struct ToggleQuery {
    /// `on`/`off` (or `1`/`true`).
    pub enabled: String,
}

/// `GET /ui/update/source?source=SPEC` -- set the update source (implies
/// enable, matching the CLI `--source` semantics).
pub async fn set_source(
    State((_store, banner)): State<crate::web::AppState>,
    Query(q): Query<SourceQuery>,
) -> Response {
    let source = match parse_source(&q.source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("oep-web: bad source spec: {e}");
            return Redirect::to("/ui/update/settings").into_response();
        }
    };
    banner.apply_source(&source);
    Redirect::to("/ui/update/settings").into_response()
}

/// Query parameters for the source endpoint.
#[derive(serde::Deserialize)]
pub struct SourceQuery {
    /// The source spec (`off` / `url:URL` / `mirror:PATH` / `bundle:PATH`).
    pub source: String,
}

/// `GET /ui/update/offer/accept?condition=unreachable|untrusted` -- accept a
/// failure offer: disables checking, records the answer, never re-asks.
pub async fn accept_offer(
    State((_store, banner)): State<crate::web::AppState>,
    Query(q): Query<ConditionQuery>,
) -> Response {
    match parse_condition(&q.condition) {
        Some(c) => banner.accept_offer(c),
        None => eprintln!("oep-web: unknown offer condition '{}'", q.condition),
    }
    Redirect::to("/ui/update/settings").into_response()
}

/// `GET /ui/update/offer/decline?condition=unreachable|untrusted` -- decline a
/// failure offer: keeps checking, records the answer, never re-asks for this
/// condition.
pub async fn decline_offer(
    State((_store, banner)): State<crate::web::AppState>,
    Query(q): Query<ConditionQuery>,
) -> Response {
    match parse_condition(&q.condition) {
        Some(c) => banner.decline_offer(c),
        None => eprintln!("oep-web: unknown offer condition '{}'", q.condition),
    }
    Redirect::to("/ui/update/settings").into_response()
}

/// Query parameters for the offer endpoints.
#[derive(serde::Deserialize)]
pub struct ConditionQuery {
    /// `unreachable` or `untrusted`.
    pub condition: String,
}

fn parse_condition(s: &str) -> Option<OfferCondition> {
    match s.trim() {
        "unreachable" => Some(OfferCondition::Unreachable),
        "untrusted" => Some(OfferCondition::Untrusted),
        _ => None,
    }
}

fn parse_source(spec: &str) -> Result<oep_update::UpdateSource, String> {
    let s = spec.trim();
    if s == "off" || s == "Off" || s.is_empty() {
        return Ok(oep_update::UpdateSource::Off);
    }
    if let Some(url) = s.strip_prefix("url:") {
        if url.trim().is_empty() {
            return Err("url: needs a URL".into());
        }
        return Ok(oep_update::UpdateSource::RemoteUrl(url.trim().to_string()));
    }
    if let Some(p) = s.strip_prefix("mirror:") {
        if p.trim().is_empty() {
            return Err("mirror: needs a path".into());
        }
        return Ok(oep_update::UpdateSource::Mirror(PathBuf::from(p.trim())));
    }
    if let Some(p) = s.strip_prefix("bundle:") {
        if p.trim().is_empty() {
            return Err("bundle: needs a path".into());
        }
        return Ok(oep_update::UpdateSource::Bundle(PathBuf::from(p.trim())));
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Ok(oep_update::UpdateSource::RemoteUrl(s.to_string()));
    }
    Err(format!(
        "source spec '{spec}' must be off|url:URL|mirror:PATH|bundle:PATH"
    ))
}

/// Classify a loaded config into the tri-state for display (mirrors the
/// crate's `config::state_of_config` semantics for a banner-held config).
fn trip_type(cfg: &UpdateConfig) -> UpdateChannelState {
    match &cfg.source {
        UpdateSource::Off => {
            if cfg.enabled {
                UpdateChannelState::On {
                    source: UpdateSource::Off,
                }
            } else {
                UpdateChannelState::OffByUser
            }
        }
        src => UpdateChannelState::On {
            source: src.clone(),
        },
    }
}

/// Resolve the update config: an explicit file path wins; otherwise the
/// persisted settings file (when one exists at the resolved settings path),
/// then the env contract (`oep_update::config::resolve()`). A bad explicit
/// file logs and falls back to the non-phoning default - never a launch
/// failure.
fn load_config(path: Option<&Path>, settings_path: Option<&Path>) -> UpdateConfig {
    if let Some(p) = path {
        match oep_update::config::from_path(p) {
            Ok(c) => return c,
            Err(e) => {
                eprintln!(
                    "oep-web: update config {} could not be loaded ({e}); running OFF",
                    p.display()
                );
                return UpdateConfig::default();
            }
        }
    }
    // UPDATE-CHANNEL: prefer a persisted settings file at the explicit/banner
    // path, else let config::resolve walk its tiers.
    if let Some(p) = settings_path {
        if p.is_file() {
            if let Ok(c) = oep_update::config::from_path(p) {
                return c;
            }
        }
    }
    oep_update::config::resolve()
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
