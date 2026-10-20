//! Retention/archive policy (P10-M3-T02): documented defaults, machine
//! readable, admin-overridable.
//!
//! Doctrine (mirrors the program's sovereignty principle): **the platform
//! never deletes data automatically**. Retention values are *review
//! horizons* — after the horizon an admin is prompted to review a revision
//! for archival (export-all) — not auto-delete timers. Deletion of vault
//! content is always a manual, audit-logged action that additionally
//! requires the content to already exist in a verified archive
//! (`purge_requires_archive_export`).
//!
//! The defaults here are the single source of truth for the policy:
//! `oep-archive retention-policy --write <dir>` emits them as
//! `retention-policy.json`, and `oep-archive export-all` embeds a snapshot
//! into every archive so the retention context travels with the data.

use serde::{Deserialize, Serialize};

use crate::schema::{ARCHIVE_SCHEMA_VERSION, RETENTION_FORMAT};

/// Lifcycle states retention rules talk about (the P8 revision state machine
/// vocabulary, extended with a catch-all).
pub mod states {
    /// `working` — in-progress revisions.
    pub const WORKING: &str = "working";
    /// `under_review` — awaiting release approval.
    pub const UNDER_REVIEW: &str = "under_review";
    /// `released` — released (immutable in the P8 state machine).
    pub const RELEASED: &str = "released";
    /// `obsolete` — superseded/retired but retained.
    pub const OBSOLETE: &str = "obsolete";
}

/// One retention rule for one lifecycle state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionRule {
    /// Review horizon in days: after this age an admin should review the
    /// revision for archival. `None` = no horizon (keep in scope forever).
    pub review_days: Option<u64>,
    /// True when this state is retained indefinitely (released gold).
    pub keep_forever: bool,
    /// One-line rationale / behavior note (part of the documented policy).
    pub note: String,
}

/// The deletion guard rail shared by every state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeletionPolicy {
    /// The platform never deletes automatically. Always `false` today by
    /// design; flipping it intentionally is a site-admin decision, never a
    /// default.
    pub automatic_deletion_enabled: bool,
    /// A revision may only be purged when a verified export-all archive that
    /// contains it already exists.
    pub purge_requires_archive_export: bool,
    /// Days a purged revision lingers in a quarantine bucket after archiving
    /// before the deletion is finalized (a second chance, then gone).
    pub quarantine_after_archive_days: u64,
    /// Every purge and every quarantine entry is audit-logged.
    pub audit_logged_only: bool,
}

/// A single `state -> rule` entry, kept as an ordered list so the emitted
/// JSON is deterministic (a map would not be).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionRuleEntry {
    /// The lifecycle state this rule governs.
    pub state: String,
    /// The rule itself.
    pub rule: RetentionRule,
}

/// The retention policy payload. [`RETENTION_FORMAT`] / v1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Must equal [`RETENTION_FORMAT`].
    pub format: String,
    /// Must equal [`ARCHIVE_SCHEMA_VERSION`].
    pub version: u64,
    /// What the policy governs.
    pub scope: String,
    /// True when this is the built-in default policy (no admin override).
    pub defaults: bool,
    /// Per-state rules, ordered (deterministic JSON).
    pub rules: Vec<RetentionRuleEntry>,
    /// The deletion guard rails.
    pub deletion: DeletionPolicy,
    /// Pointers to retained policy documentation.
    pub references: Vec<String>,
}

impl RetentionPolicy {
    /// The built-in defaults (documented in
    /// `docs/delivery/disaster-recovery/retention-policy.md`, which this struct mirrors).
    pub fn defaults() -> Self {
        Self {
            format: RETENTION_FORMAT.to_string(),
            version: ARCHIVE_SCHEMA_VERSION,
            scope: "oep-pdm vault revision retention + archive interop".to_string(),
            defaults: true,
            rules: vec![
                RetentionRuleEntry {
                    state: states::WORKING.to_string(),
                    rule: RetentionRule {
                        review_days: Some(180),
                        keep_forever: false,
                        note: "stale in-progress work: archive-review after 180 days".to_string(),
                    },
                },
                RetentionRuleEntry {
                    state: states::UNDER_REVIEW.to_string(),
                    rule: RetentionRule {
                        review_days: Some(365),
                        keep_forever: false,
                        note: "review collateral is retained a year past its last activity"
                            .to_string(),
                    },
                },
                RetentionRuleEntry {
                    state: states::RELEASED.to_string(),
                    rule: RetentionRule {
                        review_days: None,
                        keep_forever: true,
                        note: "released revisions are immutable gold: retained indefinitely"
                            .to_string(),
                    },
                },
                RetentionRuleEntry {
                    state: states::OBSOLETE.to_string(),
                    rule: RetentionRule {
                        review_days: Some(365),
                        keep_forever: false,
                        note: "obsolete: review for archival after 1 year; purge only after a "
                            .to_string()
                            .to_owned(),
                    },
                },
            ],
            deletion: DeletionPolicy {
                automatic_deletion_enabled: false,
                purge_requires_archive_export: true,
                quarantine_after_archive_days: 90,
                audit_logged_only: true,
            },
            references: vec![
                "docs/delivery/disaster-recovery/retention-policy.md".to_string(),
                "docs/delivery/disaster-recovery/export-all.md".to_string(),
            ],
        }
    }

    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON bytes, validating the format marker + version.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let p: RetentionPolicy =
            serde_json::from_slice(bytes).map_err(|e| format!("parse retention policy: {e}"))?;
        if p.format != RETENTION_FORMAT {
            return Err(format!("bad retention policy format: {}", p.format));
        }
        if p.version != ARCHIVE_SCHEMA_VERSION {
            return Err(format!("bad retention policy version: {}", p.version));
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let p = RetentionPolicy::defaults();
        assert_eq!(p.format, RETENTION_FORMAT);
        assert!(p.defaults);
        assert!(!p.deletion.automatic_deletion_enabled);
        assert!(p.deletion.purge_requires_archive_export);
        // Released gold must exist and be keep_forever.
        let released = p
            .rules
            .iter()
            .find(|r| r.state == states::RELEASED)
            .expect("released rule present");
        assert!(released.rule.keep_forever);
        assert!(released.rule.review_days.is_none());
        // Round trip keeps identity.
        let json = p.to_json_pretty().unwrap();
        let back = RetentionPolicy::from_json(json.as_bytes()).unwrap();
        assert_eq!(back, p);
        // Deterministic: serializing twice gives the same bytes.
        assert_eq!(json, p.to_json_pretty().unwrap());
    }

    #[test]
    fn rejects_wrong_format_marker() {
        let mut p = RetentionPolicy::defaults();
        p.format = "not-a-policy".to_string();
        let json = p.to_json_pretty().unwrap();
        assert!(RetentionPolicy::from_json(json.as_bytes()).is_err());
    }
}
