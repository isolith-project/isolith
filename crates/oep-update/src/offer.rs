//! One-off failure-offer ledger (UPDATE-CHANNEL, decision #20).
//!
//! When an update check returns `Unreachable` or `Untrusted`, the program
//! offers ONCE to turn update checking off. The answer persists, the offer
//! never repeats for the same condition, and declining it does not re-ask on
//! the next launch.
//!
//! The ledger stores, per condition (`unreachable` / `untrusted`), the user's
//! answer: `Accepted` (turn update checking off) or `Declined` (keep checking,
//! but stop asking about THIS condition). Persisted to
//! `<config-dir>/update-offers.json` (or `OEP_UPDATE_OFFERS`), so the answer
//! survives restarts across every surface that shares the config dir.
//!
//! Accepting an offer does the same thing as the settings toggle `--disable`:
//! it writes `enabled: false` into the persisted settings file. Declining
//! records only the ledger entry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::UpdateStatus;

/// The two conditions that trigger the one-off offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferCondition {
    /// The update source could not be reached.
    Unreachable,
    /// The update source could not be trusted.
    Untrusted,
}

impl OfferCondition {
    /// Stable key used in the ledger file and on CLI/web surfaces.
    pub fn key(self) -> &'static str {
        match self {
            OfferCondition::Unreachable => "unreachable",
            OfferCondition::Untrusted => "untrusted",
        }
    }

    /// The condition behind an advisory status, if it should ever be offered.
    pub fn from_status(status: &UpdateStatus) -> Option<OfferCondition> {
        match status {
            UpdateStatus::Unreachable { .. } => Some(OfferCondition::Unreachable),
            UpdateStatus::Untrusted { .. } => Some(OfferCondition::Untrusted),
            _ => None,
        }
    }
}

/// The stored answer for a condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferAnswer {
    /// Turn update checking off (persist `enabled: false`).
    Accepted,
    /// Keep update checking on, but never ask about this condition again.
    Declined,
}

impl OfferAnswer {
    pub fn is_accepted(self) -> bool {
        matches!(self, OfferAnswer::Accepted)
    }
}

/// Persisted file shape.
#[derive(Debug, Default, Serialize, Deserialize)]
struct OffersFile {
    schema: String,
    answers: BTreeMap<String, OfferAnswer>,
}

/// The persisted one-off offer ledger. Cheap clone: the caller holds it once
/// and calls [`OfferLedger::load`] / [`OfferLedger::record`] as needed.
#[derive(Debug, Clone)]
pub struct OfferLedger {
    path: PathBuf,
    answers: BTreeMap<String, OfferAnswer>,
}

impl OfferLedger {
    /// Load the ledger from `path` (missing/corrupt file => empty ledger, and
    /// never fail: an offer banner must not break launch).
    pub fn load(path: &Path) -> OfferLedger {
        let answers = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<OffersFile>(&text).ok())
            .map(|f| f.answers)
            .unwrap_or_default();
        OfferLedger {
            path: path.to_path_buf(),
            answers,
        }
    }

    /// Whether an offer should be shown for `condition`: the condition is one
    /// that can trigger an offer AND the user has not answered it yet.
    pub fn should_offer(&self, condition: OfferCondition) -> bool {
        !self.answers.contains_key(condition.key())
    }

    /// The condition, if any, that should be offered right now given the
    /// advisory status of the latest check.
    pub fn pending(&self, status: &UpdateStatus) -> Option<OfferCondition> {
        let cond = OfferCondition::from_status(status)?;
        if self.should_offer(cond) {
            Some(cond)
        } else {
            None
        }
    }

    /// Record an answer for `condition` and persist. Does NOT write the
    /// settings file: the caller performs the actual disable when
    /// `OfferAnswer::Accepted` (via `config::write_settings`).
    pub fn record(&mut self, condition: OfferCondition, answer: OfferAnswer) {
        self.answers.insert(condition.key().to_string(), answer);
        let _ = self.save();
    }

    /// Persist the current ledger (best effort: a read-only config dir must
    /// not break the surface).
    pub fn save(&self) -> std::io::Result<()> {
        let file = OffersFile {
            schema: "isolith.update-offers.v1".to_string(),
            answers: self.answers.clone(),
        };
        let text = serde_json::to_string_pretty(&file).expect("offers serialize");
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&self.path, text)
    }

    /// The ledger file path (for tests and status surfacing).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The current answers (tests).
    pub fn answers(&self) -> &BTreeMap<String, OfferAnswer> {
        &self.answers
    }
}

/// Resolve the offer-ledger path from the environment contract: `OEP_UPDATE_OFFERS`
/// wins; else `<settings-config-dir>/update-offers.json`; else `None` (no
/// persistence location -> in-memory only, offer never re-asked this process).
pub fn offer_path_from(get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(p) = get("OEP_UPDATE_OFFERS") {
        let p = p.trim();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    crate::config::settings_path_from(get).map(|p| p.with_file_name("update-offers.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("oep-update-offers-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("update-offers.json")
    }

    #[test]
    fn unreachable_and_untrusted_trigger_an_offer() {
        assert_eq!(
            OfferCondition::from_status(&UpdateStatus::Unreachable { detail: "x".into() }),
            Some(OfferCondition::Unreachable)
        );
        assert_eq!(
            OfferCondition::from_status(&UpdateStatus::Untrusted { detail: "x".into() }),
            Some(OfferCondition::Untrusted)
        );
        for s in [
            UpdateStatus::Off,
            UpdateStatus::UpToDate,
            UpdateStatus::UpdateAvailable {
                version: "1".into(),
                released: "2026-10-17".into(),
                release_notes_url: None,
                payloads: vec![],
            },
        ] {
            assert_eq!(OfferCondition::from_status(&s), None);
        }
    }

    #[test]
    fn offer_appears_then_never_repeats_after_decline() {
        let path = scratch("decline");
        let status = UpdateStatus::Unreachable {
            detail: "boom".into(),
        };

        let mut led = OfferLedger::load(&path);
        assert!(led.should_offer(OfferCondition::Unreachable));
        assert_eq!(led.pending(&status), Some(OfferCondition::Unreachable));

        // Decline: record the answer; the offer must not appear again, even in
        // a brand-new ledger over the same file ("declining does not re-ask").
        led.record(OfferCondition::Unreachable, OfferAnswer::Declined);
        assert!(!led.should_offer(OfferCondition::Unreachable));

        let reloaded = OfferLedger::load(&path);
        assert_eq!(reloaded.pending(&status), None);
        assert!(path.is_file(), "answer persisted");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn accept_persists_and_marks_accepted() {
        let path = scratch("accept");
        let status = UpdateStatus::Untrusted {
            detail: "no key".into(),
        };
        let mut led = OfferLedger::load(&path);
        assert_eq!(led.pending(&status), Some(OfferCondition::Untrusted));

        led.record(OfferCondition::Untrusted, OfferAnswer::Accepted);
        // Accepting means the program turns checking off - settings write is
        // the caller's job; here we prove the ledger remembers it.
        let reloaded = OfferLedger::load(&path);
        assert_eq!(
            reloaded.answers().get("untrusted"),
            Some(&OfferAnswer::Accepted)
        );
        assert_eq!(reloaded.pending(&status), None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn conditions_are_independent() {
        let path = scratch("independent");
        let mut led = OfferLedger::load(&path);
        led.record(OfferCondition::Unreachable, OfferAnswer::Declined);
        // Declining unreachable must not silence the untrusted offer.
        assert!(led.should_offer(OfferCondition::Untrusted));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn corrupt_ledger_falls_back_empty_never_fails() {
        let path = scratch("corrupt");
        std::fs::write(&path, "{not json").unwrap();
        let led = OfferLedger::load(&path);
        assert!(led.should_offer(OfferCondition::Unreachable));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
