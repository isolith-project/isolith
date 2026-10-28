//! The workbench startup update surface (decision #20).
//!
//! The workbench calls [`run_startup_check`] exactly once at launch: it runs
//! the advisory `Probe::preflight`, logs the status as ONE advisory line on
//! stderr, and lets the caller render a non-modal notice when an update is
//! available. Everything here is advisory (decision #11): no status can block
//! launch, change an exit code, or trigger any download or install.

use oep_update::{UpdateConfig, UpdateStatus};

/// Run the startup advisory update check and log it.
///
/// Returns the status so the caller can also render the non-modal notice.
/// The exit code of the workbench is NEVER changed by the result: this
/// function returns a status, it cannot exit the process. With `Off`
/// configuration (the default) it performs zero I/O.
pub fn run_startup_check(config: &UpdateConfig) -> UpdateStatus {
    let status = oep_update::Probe::new(config).preflight().status().clone();
    eprintln!("oep-workbench: {}", status.advisory_line());
    status
}

/// Render the non-modal notice text for a status, when an update is
/// available. `None` for every other advisory status (show nothing blocking).
pub fn render_notice(status: &UpdateStatus) -> Option<String> {
    let notice = status.notice()?;
    let mut text = format!(
        "oep-workbench: update available: {} (released {})",
        notice.version, notice.released
    );
    if let Some(url) = notice.release_notes_url.as_deref() {
        text.push_str(&format!("\noep-workbench: release notes: {url}"));
    }
    Some(text)
}
