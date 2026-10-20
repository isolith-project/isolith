//! P8-M2-T02: change management -- ECR/ECO-style tickets linked to revisions
//! (lite scope).
//!
//! This module is the change-management model for the P8 PDM lane: an
//! engineering-change *ticket* (ECR = request / ECO = order) that captures
//! *what* changes, *who* owns it, *what status* it is in, and *which
//! revisions* the change touches (its **revision set**).
//!
//! Scope is deliberately lite, as the acceptance says: there is no workflow
//! engine. Statuses are a fixed enumerated lifecycle, transitions are
//! validated by the model itself ([`TicketStatus::can_transition_to`]), and
//! every real transition is appended to [`ChangeTicket::history`] so status
//! changes are *tracked*. There is no async queue, no approval object, no
//! e-signature, no RBAC, and no REST surface -- those are later P8 cards
//! (P8-M4-T02 approval flows, P8-M3 RBAC/audit, P8-M6-T01 REST API v1).
//!
//! Revision references are plain identities `(item_id, revision_number)`,
//! compatible with the P7-M2 save-revisions contract (revision numbers are
//! server-assigned strings; item ids are opaque url-safe strings). Existence
//! validation against the real vault lives in the P8-M1-T02 vault model;
//! this change model treats a reference as a claim about a revision and
//! enforces only that the reference itself is well-formed. See
//! `workstreams/pdm/revisions/change-management.md` for the design note.

use std::fmt;

/// Kind of engineering-change document a ticket represents.
///
/// Lite scope recognises the two classic PDM documents:
/// * an **ECR** (engineering change request) proposes a change;
/// * an **ECO** (engineering change order) authorizes and drives one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TicketKind {
    /// Engineering change request: a proposed change, not yet authorized.
    Ecr,
    /// Engineering change order: an authorized change to be implemented.
    Eco,
}

impl TicketKind {
    /// Canonical lowercase tag used for display and parsing (`"ecr"` / `"eco"`).
    pub fn as_str(self) -> &'static str {
        match self {
            TicketKind::Ecr => "ecr",
            TicketKind::Eco => "eco",
        }
    }

    /// Parse a kind tag case-insensitively (`"ecr"`, `"ECO"`, `"Ecr"`, ...).
    ///
    /// Returns [`ChangeError::BadRequest`] for anything that is not an ECR or
    /// an ECO, so callers get the contract's error shape instead of a raw
    /// enum panic.
    pub fn parse(tag: &str) -> Result<TicketKind, ChangeError> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "ecr" => Ok(TicketKind::Ecr),
            "eco" => Ok(TicketKind::Eco),
            other => Err(ChangeError::BadRequest(format!(
                "ticket kind must be 'ecr' or 'eco', got {other:?}"
            ))),
        }
    }
}

impl fmt::Display for TicketKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status of a change ticket.
///
/// Lite, closed set. Terminal states ([`TicketStatus`]`::Rejected`,
/// `::Implemented`, `::Cancelled`) never leave; every other transition is
/// validated by [`TicketStatus::can_transition_to`], not by a workflow engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TicketStatus {
    /// Being authored, not yet formally submitted.
    Draft,
    /// Submitted and actionable.
    Open,
    /// Under review by a change board / reviewer.
    UnderReview,
    /// The change has been authorized.
    Approved,
    /// The change was denied during review. Terminal.
    Rejected,
    /// The change has been implemented (released to the vault). Terminal.
    Implemented,
    /// The ticket was withdrawn before completion. Terminal.
    Cancelled,
}

impl TicketStatus {
    /// Canonical lowercase tag for display/parsing
    /// (`"draft"`, `"under_review"`, `"implemented"`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            TicketStatus::Draft => "draft",
            TicketStatus::Open => "open",
            TicketStatus::UnderReview => "under_review",
            TicketStatus::Approved => "approved",
            TicketStatus::Rejected => "rejected",
            TicketStatus::Implemented => "implemented",
            TicketStatus::Cancelled => "cancelled",
        }
    }

    /// Parse a status tag case-insensitively.
    pub fn parse(tag: &str) -> Result<TicketStatus, ChangeError> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(TicketStatus::Draft),
            "open" => Ok(TicketStatus::Open),
            "under_review" => Ok(TicketStatus::UnderReview),
            "approved" => Ok(TicketStatus::Approved),
            "rejected" => Ok(TicketStatus::Rejected),
            "implemented" => Ok(TicketStatus::Implemented),
            "cancelled" => Ok(TicketStatus::Cancelled),
            other => Err(ChangeError::BadRequest(format!(
                "unknown ticket status: {other:?}"
            ))),
        }
    }

    /// Whether a transition from `self` to `to` is allowed.
    ///
    /// Same-status transitions are allowed as idempotent no-ops (see
    /// [`ChangeTicket::set_status`]). Otherwise the allowed edges are:
    /// draft -> open|cancelled; open -> under_review|cancelled; under_review
    /// -> approved|rejected|cancelled; approved -> implemented|cancelled;
    /// terminal states are absorbing.
    pub fn can_transition_to(self, to: TicketStatus) -> bool {
        use TicketStatus::{Approved, Cancelled, Draft, Implemented, Open, Rejected, UnderReview};
        if self == to {
            return true;
        }
        match self {
            Draft => matches!(to, Open | Cancelled),
            Open => matches!(to, UnderReview | Cancelled),
            UnderReview => matches!(to, Approved | Rejected | Cancelled),
            Approved => matches!(to, Implemented | Cancelled),
            Rejected | Implemented | Cancelled => false,
        }
    }

    /// Whether this status is terminal (no further transitions allowed).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TicketStatus::Rejected | TicketStatus::Implemented | TicketStatus::Cancelled
        )
    }
}

impl fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One recorded transition in a ticket's status history (append-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusChange {
    /// Status before the transition.
    pub from: TicketStatus,
    /// Status after the transition.
    pub to: TicketStatus,
    /// RFC 3339 UTC timestamp of the transition.
    pub at: String,
    /// Acting principal that performed the transition.
    pub by: String,
    /// Optional free-text note (reason / reviewer comment).
    pub note: Option<String>,
}

/// A single vault revision referenced by a change ticket, identified the way
/// the P8 contract identifies revisions: `item_id` is the opaque url-safe
/// vault item id (P8-M1-T02) and `revision_number` is the server-assigned
/// string revision number (P7-M2 save-revisions contract).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RevisionRef {
    /// Opaque url-safe vault item id the revision belongs to.
    pub item_id: String,
    /// Server-assigned revision number as a string (`"1"`, `"2"`, ...).
    pub revision_number: String,
}

impl RevisionRef {
    /// Build a well-formed revision reference.
    ///
    /// Rejects empty `item_id` / `revision_number` with
    /// [`ChangeError::BadRequest`]; the reference does not claim the revision
    /// exists in the vault (that is P8-M1-T02's job).
    pub fn new(
        item_id: impl Into<String>,
        revision_number: impl Into<String>,
    ) -> Result<RevisionRef, ChangeError> {
        let item_id = item_id.into();
        let revision_number = revision_number.into();
        if item_id.trim().is_empty() {
            return Err(ChangeError::BadRequest(
                "revision link item_id must not be empty".into(),
            ));
        }
        if revision_number.trim().is_empty() {
            return Err(ChangeError::BadRequest(
                "revision link revision_number must not be empty".into(),
            ));
        }
        Ok(RevisionRef {
            item_id,
            revision_number,
        })
    }
}

impl fmt::Display for RevisionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.item_id, self.revision_number)
    }
}

/// The set of vault revisions a change ticket affects -- its **revision
/// set**.
///
/// Set semantics: inserting a revision already present is a no-op
/// ([`RevisionSet::insert`] returns `false`). A ticket links one revision set
/// as a unit, which is how a multi-item change is reasoned about as a whole
/// (impact, review, release).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RevisionSet {
    revisions: Vec<RevisionRef>,
}

impl RevisionSet {
    /// An empty revision set.
    pub fn new() -> Self {
        RevisionSet {
            revisions: Vec::new(),
        }
    }

    /// Insert a revision into the set.
    ///
    /// Returns `true` if the revision was newly added, `false` if it was
    /// already present (duplicate inserts are idempotent no-ops).
    pub fn insert(&mut self, revision: RevisionRef) -> bool {
        if self.revisions.contains(&revision) {
            return false;
        }
        self.revisions.push(revision);
        true
    }

    /// Remove a revision from the set. Returns `true` if it was present.
    pub fn remove(&mut self, revision: &RevisionRef) -> bool {
        let before = self.revisions.len();
        self.revisions.retain(|r| r != revision);
        self.revisions.len() != before
    }

    /// Whether the set contains `revision`.
    pub fn contains(&self, revision: &RevisionRef) -> bool {
        self.revisions.contains(revision)
    }

    /// Number of revisions in the set.
    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.revisions.is_empty()
    }

    /// Iterate the revisions in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &RevisionRef> {
        self.revisions.iter()
    }

    /// Slice view of the revisions.
    pub fn as_slice(&self) -> &[RevisionRef] {
        &self.revisions
    }
}

impl IntoIterator for RevisionSet {
    type Item = RevisionRef;
    type IntoIter = std::vec::IntoIter<RevisionRef>;

    fn into_iter(self) -> Self::IntoIter {
        self.revisions.into_iter()
    }
}

/// An engineering-change ticket (ECR or ECO) with its revision-set link and
/// tracked status history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeTicket {
    /// Opaque url-safe ticket id assigned by the store.
    pub id: String,
    /// Whether this is a change request (ECR) or change order (ECO).
    pub kind: TicketKind,
    /// Required, non-empty human title.
    pub title: String,
    /// Optional free-text description of the change.
    pub description: Option<String>,
    /// Current lifecycle status.
    pub status: TicketStatus,
    /// Append-only status history. Every transition validated and applied by
    /// [`ChangeTicket::set_status`] is recorded here, which is how status
    /// changes are *tracked* in lite scope.
    pub history: Vec<StatusChange>,
    /// The set of vault revisions this ticket affects.
    pub revision_set: RevisionSet,
    /// Acting principal that created the ticket.
    pub created_by: String,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
}

impl ChangeTicket {
    /// True when the ticket is in a terminal status.
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Link a revision to this ticket's revision set.
    ///
    /// Returns `true` if the revision was newly linked, `false` if it was
    /// already in the set.
    pub fn add_revision(&mut self, revision: RevisionRef) -> bool {
        self.revision_set.insert(revision)
    }

    /// Unlink a revision from this ticket's revision set.
    ///
    /// Returns `true` if the revision was present and removed.
    pub fn remove_revision(&mut self, revision: &RevisionRef) -> bool {
        self.revision_set.remove(revision)
    }

    /// Advance the ticket's status.
    ///
    /// Validates the transition against [`TicketStatus::can_transition_to`]
    /// and appends a [`StatusChange`] to `history` on a real change. A
    /// same-status call is an idempotent no-op (no history entry appended).
    /// Terminal statuses reject all movement. `by` (the acting principal)
    /// must be non-empty, mirroring the audit posture of the P8 lane.
    pub fn set_status(
        &mut self,
        to: TicketStatus,
        by: &str,
        note: Option<String>,
    ) -> Result<(), ChangeError> {
        if by.trim().is_empty() {
            return Err(ChangeError::BadRequest(
                "status actor (by) must not be empty".into(),
            ));
        }
        if !self.status.can_transition_to(to) {
            return Err(ChangeError::InvalidTransition {
                from: self.status,
                to,
            });
        }
        if self.status != to {
            let change = StatusChange {
                from: self.status,
                to,
                at: now_rfc3339(),
                by: by.to_string(),
                note,
            };
            self.status = to;
            self.history.push(change);
        }
        Ok(())
    }
}

/// Input for [`ChangeStore::create_ticket`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTicketRequest {
    /// Required ticket kind (ECR or ECO).
    pub kind: TicketKind,
    /// Required, non-empty title.
    pub title: String,
    /// Optional free-text description.
    pub description: Option<String>,
    /// Required acting principal that submits the ticket.
    pub created_by: String,
}

impl CreateTicketRequest {
    /// Convenience constructor (test ergonomics; all fields empty, kind ECR).
    /// Mirrors `CreateItemRequest::default_min` from the vault API module.
    pub fn default_min() -> Self {
        CreateTicketRequest {
            kind: TicketKind::Ecr,
            title: String::new(),
            description: None,
            created_by: String::new(),
        }
    }
}

/// Errors surfaced by the change-management model. Mirrors the shape of the
/// vault [`StoreError`](crate::store) so downstream REST mapping (P8-M6-T01)
/// has a consistent vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    /// A ticket with the given id does not exist (404-equivalent).
    NotFound(String),
    /// A validation rule was violated (400-equivalent).
    BadRequest(String),
    /// The requested status transition is not allowed by the lite lifecycle.
    InvalidTransition {
        /// Status the ticket is in.
        from: TicketStatus,
        /// Status the caller requested.
        to: TicketStatus,
    },
}

impl fmt::Display for ChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeError::NotFound(id) => write!(f, "change ticket not found: {id}"),
            ChangeError::BadRequest(msg) => write!(f, "bad ticket request: {msg}"),
            ChangeError::InvalidTransition { from, to } => {
                write!(f, "invalid status transition: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for ChangeError {}

/// In-memory change-management store: tickets with their revision-set links
/// and tracked status histories.
///
/// Pure in-memory by design (lite scope, mirroring the vault stub's posture
/// before persistence). No workflow engine: the store only creates tickets,
/// retrieves them, and answers the two queries a change board needs -- by
/// status and by affected item.
#[derive(Debug, Clone, Default)]
pub struct ChangeStore {
    tickets: Vec<ChangeTicket>,
    next_seq: u64,
}

impl ChangeStore {
    /// A fresh, empty change store.
    pub fn new() -> Self {
        ChangeStore {
            tickets: Vec::new(),
            next_seq: 0,
        }
    }

    /// Create a ticket in `Draft` status with a unique id.
    ///
    /// Validates that `title` and `created_by` are non-empty; every other
    /// field has a documented default. The revision set starts empty and is
    /// populated with [`ChangeTicket::add_revision`].
    pub fn create_ticket(
        &mut self,
        req: &CreateTicketRequest,
    ) -> Result<ChangeTicket, ChangeError> {
        if req.title.trim().is_empty() {
            return Err(ChangeError::BadRequest(
                "ticket title must not be empty".into(),
            ));
        }
        if req.created_by.trim().is_empty() {
            return Err(ChangeError::BadRequest(
                "ticket created_by must not be empty".into(),
            ));
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let tag = format!("{:012x}", fnv1a(format!("{}-{seq}", req.title).as_bytes()));
        let id = format!("ticket-{seq:x}-{}", &tag[..12]);
        let ticket = ChangeTicket {
            id: id.clone(),
            kind: req.kind,
            title: req.title.clone(),
            description: req.description.clone(),
            status: TicketStatus::Draft,
            history: Vec::new(),
            revision_set: RevisionSet::new(),
            created_by: req.created_by.clone(),
            created_at: now_rfc3339(),
        };
        self.tickets.push(ticket.clone());
        Ok(ticket)
    }

    /// Fetch one ticket by id (404-equivalent on a miss).
    pub fn get_ticket(&self, id: &str) -> Result<ChangeTicket, ChangeError> {
        self.tickets
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| ChangeError::NotFound(id.to_string()))
    }

    /// Borrow one ticket mutably (to link revisions / advance status).
    ///
    /// The store owns the ticket; callers mutate through this handle rather
    /// than round-tripping clones. 404-equivalent on a miss.
    pub fn ticket_mut(&mut self, id: &str) -> Result<&mut ChangeTicket, ChangeError> {
        self.tickets
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| ChangeError::NotFound(id.to_string()))
    }

    /// All tickets currently held by the store.
    pub fn all_tickets(&self) -> Vec<&ChangeTicket> {
        self.tickets.iter().collect()
    }

    /// Tickets currently in the given status.
    pub fn tickets_by_status(&self, status: TicketStatus) -> Vec<&ChangeTicket> {
        self.tickets.iter().filter(|t| t.status == status).collect()
    }

    /// Every ticket whose revision set references the given item (reverse
    /// look-up: "what open changes touch this part?").
    pub fn tickets_for_item(&self, item_id: &str) -> Vec<&ChangeTicket> {
        self.tickets
            .iter()
            .filter(|t| t.revision_set.iter().any(|r| r.item_id == item_id))
            .collect()
    }
}

/// RFC 3339 UTC timestamp from unix epoch seconds (stdlib-only, deterministic).
fn now_rfc3339() -> String {
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (epoch_secs / 86_400) as i64;
    let day_secs = epoch_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        day_secs / 3600,
        (day_secs % 3600) / 60,
        day_secs % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm (public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// FNV-1a 64-bit (stable, dependency-free; used for the ticket id tag).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(kind: TicketKind, title: &str, by: &str) -> CreateTicketRequest {
        CreateTicketRequest {
            kind,
            title: title.to_string(),
            description: Some("test change".to_string()),
            created_by: by.to_string(),
        }
    }

    #[test]
    fn create_ticket_defaults_to_draft_with_unique_ids() {
        let mut store = ChangeStore::new();
        let t1 = store
            .create_ticket(&req(TicketKind::Ecr, "Revise bracket", "alice"))
            .unwrap();
        let t2 = store
            .create_ticket(&req(TicketKind::Eco, "Release new fastener", "alice"))
            .unwrap();
        assert_ne!(t1.id, t2.id);
        assert!(t1.id.starts_with("ticket-"), "id shape: {}", t1.id);
        assert!(t1.history.is_empty(), "no history on creation");
        assert_eq!(t1.status, TicketStatus::Draft);
        assert_eq!(t1.kind, TicketKind::Ecr);
        assert_eq!(t2.kind, TicketKind::Eco);
        assert!(store.get_ticket(&t1.id).is_ok());
        assert!(store.get_ticket(&t2.id).is_ok());
    }

    #[test]
    fn create_ticket_rejects_empty_title_and_empty_actor() {
        let mut store = ChangeStore::new();
        assert_eq!(
            store.create_ticket(&req(TicketKind::Ecr, "   ", "alice")),
            Err(ChangeError::BadRequest(
                "ticket title must not be empty".into()
            ))
        );
        assert_eq!(
            store.create_ticket(&req(TicketKind::Eco, "ok title", "  ")),
            Err(ChangeError::BadRequest(
                "ticket created_by must not be empty".into()
            ))
        );
        assert_eq!(store.all_tickets().len(), 0);
    }

    #[test]
    fn kind_and_status_parse_are_case_insensitive_and_reject_garbage() {
        assert_eq!(TicketKind::parse("ecr").unwrap(), TicketKind::Ecr);
        assert_eq!(TicketKind::parse("ECO").unwrap(), TicketKind::Eco);
        assert_eq!(TicketKind::parse("Ecr").unwrap(), TicketKind::Ecr);
        assert!(matches!(
            TicketKind::parse("bom"),
            Err(ChangeError::BadRequest(_))
        ));

        assert_eq!(
            TicketStatus::parse("under_review").unwrap(),
            TicketStatus::UnderReview
        );
        assert_eq!(
            TicketStatus::parse("IMPLEMENTED").unwrap(),
            TicketStatus::Implemented
        );
        assert!(matches!(
            TicketStatus::parse("married"),
            Err(ChangeError::BadRequest(_))
        ));
    }

    #[test]
    fn revision_set_deduplicates_and_handles_remove() {
        let r1 = RevisionRef::new("item-1", "3").unwrap();
        let r2 = RevisionRef::new("item-2", "1").unwrap();
        let mut set = RevisionSet::new();
        assert!(set.insert(r1.clone()));
        assert!(!set.insert(r1.clone()), "duplicate insert is a no-op");
        assert!(set.insert(r2.clone()));
        assert_eq!(set.len(), 2);
        assert!(set.contains(&r1));
        assert!(!set.remove(&RevisionRef::new("item-9", "9").unwrap()));
        assert!(set.remove(&r1));
        assert_eq!(set.len(), 1);
        assert_eq!(set.as_slice(), &[r2]);
    }

    #[test]
    fn revision_ref_rejects_empty_fields() {
        assert!(RevisionRef::new("", "1").is_err());
        assert!(RevisionRef::new("item-1", "").is_err());
        assert!(RevisionRef::new("  ", "1").is_err());
    }

    #[test]
    fn happy_path_lifecycle_records_history() {
        let mut store = ChangeStore::new();
        let mut t = store
            .create_ticket(&req(TicketKind::Eco, "Bracket rework", "bob"))
            .unwrap();
        assert!(t.add_revision(RevisionRef::new("item-a", "1").unwrap()));
        assert!(t.add_revision(RevisionRef::new("item-b", "4").unwrap()));
        assert_eq!(t.revision_set.len(), 2);

        for (st, by) in [
            (TicketStatus::Open, "bob"),
            (TicketStatus::UnderReview, "bob"),
            (TicketStatus::Approved, "carol"),
            (TicketStatus::Implemented, "bob"),
        ] {
            t.set_status(st, by, None).unwrap();
        }
        assert!(t.is_terminal());
        assert_eq!(t.status, TicketStatus::Implemented);
        assert_eq!(t.history.len(), 4);
        let last = t.history.last().unwrap();
        assert_eq!(last.from, TicketStatus::Approved);
        assert_eq!(last.to, TicketStatus::Implemented);
        assert_eq!(last.by, "bob");
        assert!(!last.at.is_empty());
        // every entry records from != to
        for c in &t.history {
            assert_ne!(c.from, c.to);
        }
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let mut store = ChangeStore::new();
        let mut t = store
            .create_ticket(&req(TicketKind::Ecr, "Sneaky jump", "dave"))
            .unwrap();
        // Draft cannot jump straight to Approved.
        assert_eq!(
            t.set_status(TicketStatus::Approved, "dave", None),
            Err(ChangeError::InvalidTransition {
                from: TicketStatus::Draft,
                to: TicketStatus::Approved
            })
        );
        // Open cannot go back to Draft.
        t.set_status(TicketStatus::Open, "dave", None).unwrap();
        assert!(t.set_status(TicketStatus::Draft, "dave", None).is_err());
        // UnderReview cannot go straight to Implemented.
        t.set_status(TicketStatus::UnderReview, "dave", None)
            .unwrap();
        assert!(t
            .set_status(TicketStatus::Implemented, "dave", None)
            .is_err());
        // Nothing was recorded for the failed attempts.
        assert_eq!(t.history.len(), 2);
        assert_eq!(t.status, TicketStatus::UnderReview);
    }

    #[test]
    fn terminal_statuses_are_absorbing() {
        let mut store = ChangeStore::new();
        let mut t = store
            .create_ticket(&req(TicketKind::Ecr, "Doomed change", "erin"))
            .unwrap();
        t.set_status(TicketStatus::Open, "erin", None).unwrap();
        t.set_status(TicketStatus::UnderReview, "erin", None)
            .unwrap();
        t.set_status(
            TicketStatus::Rejected,
            "erin",
            Some("wrong hole spec".into()),
        )
        .unwrap();
        assert!(t.is_terminal());
        assert!(t.set_status(TicketStatus::Open, "erin", None).is_err());
        assert!(t
            .set_status(TicketStatus::Implemented, "erin", None)
            .is_err());
        assert_eq!(t.status, TicketStatus::Rejected);

        // Cancelled is also terminal.
        let mut c = store
            .create_ticket(&req(TicketKind::Ecr, "Withdrawn", "erin"))
            .unwrap();
        c.set_status(TicketStatus::Cancelled, "erin", None).unwrap();
        assert!(c.is_terminal());
        assert!(c.set_status(TicketStatus::Open, "erin", None).is_err());
    }

    #[test]
    fn same_status_is_an_idempotent_noop() {
        let mut store = ChangeStore::new();
        let mut t = store
            .create_ticket(&req(TicketKind::Ecr, "Idempotent", "frank"))
            .unwrap();
        t.set_status(TicketStatus::Draft, "frank", None).unwrap();
        assert_eq!(t.history.len(), 0, "no history for a no-op");
        t.set_status(TicketStatus::Open, "frank", None).unwrap();
        t.set_status(TicketStatus::Open, "frank", None).unwrap();
        assert_eq!(t.history.len(), 1);
    }

    #[test]
    fn set_status_requires_an_actor() {
        let mut store = ChangeStore::new();
        let mut t = store
            .create_ticket(&req(TicketKind::Ecr, "No actor", "grace"))
            .unwrap();
        assert!(t.set_status(TicketStatus::Open, "  ", None).is_err());
        assert_eq!(t.status, TicketStatus::Draft);
    }

    #[test]
    fn queries_by_status_and_by_item() {
        let mut store = ChangeStore::new();
        let a = store
            .create_ticket(&req(TicketKind::Eco, "Change A", "hal"))
            .unwrap();
        let b = store
            .create_ticket(&req(TicketKind::Eco, "Change B", "hal"))
            .unwrap();
        let c = store
            .create_ticket(&req(TicketKind::Ecr, "Change C", "hal"))
            .unwrap();
        // a and c affect item-42; b affects item-7.
        {
            let t = store.ticket_mut(&a.id).unwrap();
            t.add_revision(RevisionRef::new("item-42", "2").unwrap());
            t.set_status(TicketStatus::Open, "hal", None).unwrap();
        }
        {
            let t = store.ticket_mut(&b.id).unwrap();
            t.add_revision(RevisionRef::new("item-7", "1").unwrap());
        }
        {
            let t = store.ticket_mut(&c.id).unwrap();
            t.add_revision(RevisionRef::new("item-42", "1").unwrap());
            t.set_status(TicketStatus::Open, "hal", None).unwrap();
        }

        let open = store.tickets_by_status(TicketStatus::Open);
        assert_eq!(open.len(), 2);
        let drafts = store.tickets_by_status(TicketStatus::Draft);
        assert_eq!(drafts.len(), 1);

        let for_42 = store.tickets_for_item("item-42");
        let mut ids: Vec<&str> = for_42.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![a.id.as_str(), c.id.as_str()]);
        assert!(store.tickets_for_item("item-not-there").is_empty());
    }
}
