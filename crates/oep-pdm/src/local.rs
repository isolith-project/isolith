//! Local in-memory PDM stub (`LocalPdmVault`) - P7-M2-T01.
//!
//! A dependency-free, non-panicking implementation of the [`PdmClient`]
//! contract that stores blobs in process memory, content-addressed by
//! [`crate::digest::content_digest`]. It makes the OEP workbench's PDM
//! load/save surface real behavior against a locked contract with zero
//! external services; the real self-hosted PDM server (P8) and the
//! P7-M2-T02 browser integration replace this stub without touching the
//! workbench's call sites.
//!
//! This stub is deliberately in-memory (one vault per process). On-disk
//! persistence, revision immutability under release state, RBAC, and the audit
//! log are P8 deliverables (P8-M1-T02/T03, P8-M3); they do not change the
//! contract this module implements.

use crate::api::{PdmClient, PdmError, PdmWriteRequest, RevisionSummary};
use crate::digest::content_digest;
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

/// One stored revision in the vault: its metadata plus the raw blob.
struct Entry {
    /// The metadata record returned to callers.
    summary: RevisionSummary,
    /// The stored blob bytes (the revision content).
    content: Vec<u8>,
}

/// In-memory PDM vault. `item -> {revision -> entry}`.
///
/// The store is a `BTreeMap` per item so revision listing and head selection
/// are deterministic (sorted by revision string) and reproducible across
/// calls with identical saves - the deterministic-by-default bar the program
/// applies to its canonical output.
pub struct LocalPdmVault {
    /// `item_id -> revision -> entry`.
    items: HashMap<String, BTreeMap<String, Entry>>,
}

impl LocalPdmVault {
    /// A new, empty in-memory vault.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// True when no item has ever been saved to the vault.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of distinct items stored.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

impl Default for LocalPdmVault {
    fn default() -> Self {
        Self::new()
    }
}

impl PdmClient for LocalPdmVault {
    fn save_revision(&mut self, request: &PdmWriteRequest) -> Result<RevisionSummary, PdmError> {
        if request.item_id.trim().is_empty() || request.revision.trim().is_empty() {
            return Err(PdmError::InconsistentState {
                message: "item_id and revision must be non-empty".to_string(),
            });
        }
        let blob_digest = content_digest(&request.content);
        let summary = RevisionSummary {
            item_id: request.item_id.clone(),
            revision: request.revision.clone(),
            state: request.state,
            blob_digest: Some(blob_digest),
            summary: request.summary.clone(),
            created_at: Self::now_unix(),
        };
        let entry = Entry {
            summary: summary.clone(),
            content: request.content.clone(),
        };
        self.items
            .entry(request.item_id.clone())
            .or_default()
            .insert(request.revision.clone(), entry);
        Ok(summary)
    }

    fn open_revision(&mut self, item_id: &str, revision: &str) -> Result<Vec<u8>, PdmError> {
        let revs = self
            .items
            .get(item_id)
            .ok_or_else(|| PdmError::ItemNotFound {
                item_id: item_id.to_string(),
            })?;
        let entry = revs
            .get(revision)
            .ok_or_else(|| PdmError::RevisionNotFound {
                item_id: item_id.to_string(),
                revision: revision.to_string(),
            })?;
        Ok(entry.content.clone())
    }

    fn list_revisions(&mut self, item_id: &str) -> Result<Vec<RevisionSummary>, PdmError> {
        let revs = self
            .items
            .get(item_id)
            .ok_or_else(|| PdmError::ItemNotFound {
                item_id: item_id.to_string(),
            })?;
        let mut out: Vec<RevisionSummary> = revs.values().map(|e| e.summary.clone()).collect();
        // Newest first: deterministic reverse of the sorted revision order.
        out.sort_by(|a, b| b.revision.cmp(&a.revision));
        Ok(out)
    }

    fn head_revision(&mut self, item_id: &str) -> Result<RevisionSummary, PdmError> {
        let revs = self
            .items
            .get(item_id)
            .ok_or_else(|| PdmError::ItemNotFound {
                item_id: item_id.to_string(),
            })?;
        let last = revs
            .values()
            .max_by(|a, b| a.summary.revision.cmp(&b.summary.revision))
            .ok_or_else(|| PdmError::InconsistentState {
                message: format!("item {item_id} exists but has no revisions"),
            })?;
        Ok(last.summary.clone())
    }

    fn digest_of(&self, item_id: &str, revision: &str) -> Option<String> {
        self.items
            .get(item_id)
            .and_then(|revs| revs.get(revision))
            .and_then(|e| e.summary.blob_digest.clone())
    }
}
