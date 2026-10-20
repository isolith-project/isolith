//! P8-M4-T02: approval flows, reviewer comments and our own
//! e-signature-equivalent for the P8 PDM lane.
//!
//! This module is the approval layer over the lite change-management ticket
//! model (`crate::change`, P8-M2-T02): a ticket moves **Draft -> Open ->
//! UnderReview -> Approved|Rejected** through explicit operations that are
//! checked against the RBAC role matrix (`crate::rbac`, P8-M3-T02). It adds
//! the three things the change model deliberately left out (see its module
//! doc):
//!
//! * an **[`ApprovalBoard`]** that owns the lifecycle operations
//!   (submit / review / approve / reject) and gates them by role;
//! * **reviewer comments** that persist *with* the ticket
//!   ([`ApprovalBoard::comments_for`] retrieves them alongside the ticket,
//!   in insertion order, after decisions are recorded);
//! * our own **e-signature-equivalent**: not a legal seal, but an
//!   *intent-affirmation* record ([`SigningIntent`], `nature ==
//!   [`SigningNature::IntentAffirmation`]) that captures who asserted what,
//!   when, with a free-text affirmation statement. This is documented
//!   not-parity (see `workstreams/pdm/web/approval-flows.md`): a genuine legal signature
//!   (X.509/PAdES, identity proof, legal effect) is explicitly out of scope.
//!
//! Everything is default-deny: the board uses
//! [`RoleMatrix::standard`](crate::rbac::RoleMatrix::standard) plus an empty
//! rule policy, so a principal whose role does not grant the action is denied,
//! and no signature is ever recorded on a denied decision.
//!
//! The module is std-only (no serde, no tokio) and in-memory, mirroring the
//! lite posture of its dependencies; persistence and the REST/identity wiring
//! are P8-M1-T02 / P8-M6-T01 / P8-M3-T01 cards. The web surface in
//! [`crate::web`] drives this board over real HTTP, and
//! [`ApprovalBoard`] itself is transport-agnostic.

use std::collections::BTreeMap;

use crate::change::{ChangeError, ChangeStore, ChangeTicket, CreateTicketRequest, TicketStatus};
use crate::rbac::{Action, Authorizer, Decision, RoleMatrix, RoleSubject};

/// The e-signature-equivalent nature of a recorded decision.
///
/// Only [`SigningNature::IntentAffirmation`] is ever produced by this
/// concept: a decision is recorded as an affirmation of intent by the acting
/// principal. [`SigningNature::LegalSeal`] exists to make the non-parity
/// explicit and testable -- this card deliberately produces *no* legal seal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningNature {
    /// Intent-affirmation: the principal asserts they reviewed and decided.
    IntentAffirmation,
    /// Legal seal. Never produced by this concept (documented not-parity).
    LegalSeal,
}

/// The decision a reviewer records on a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Approve the change (UnderReview -> Approved).
    Approve,
    /// Reject the change (UnderReview -> Rejected).
    Reject,
}

/// Input for [`ApprovalBoard::decide`]: the signer's free-text
/// intent-affirmation statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningRequest {
    /// The affirmation statement the reviewer asserts, e.g. "I affirm I
    /// reviewed this change and approve it." Must be non-empty.
    pub affirm: String,
}

/// A reviewer comment persisted with a ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewComment {
    /// Opaque comment id.
    pub id: String,
    /// Ticket id the comment belongs to.
    pub ticket_id: String,
    /// Principal that wrote the comment.
    pub by: String,
    /// Comment body (non-empty).
    pub body: String,
    /// RFC 3339 UTC timestamp.
    pub at: String,
}

/// The e-signature-equivalent record: an intent-affirmation on a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningIntent {
    /// Opaque record id.
    pub id: String,
    /// Ticket id the signature is attached to.
    pub ticket_id: String,
    /// The decision that was signed (approve or reject).
    pub decision: ApprovalDecision,
    /// Principal that signed.
    pub by: String,
    /// The free-text affirmation statement.
    pub affirm: String,
    /// RFC 3339 UTC timestamp.
    pub at: String,
    /// Always [`SigningNature::IntentAffirmation`]; documented not-parity.
    pub nature: SigningNature,
}

/// The approval board: ticket lifecycle + comments + signing intents, gated
/// by the RBAC matrix.
///
/// Owns a [`ChangeStore`] (tickets/history), a comment thread and a signing
/// ledger per ticket. All lifecycle methods take a [`RoleSubject`] (whose
/// identity/roles will come from the P8-M3-T01 identity layer once wired)
/// and authorize with default-deny.
#[derive(Debug, Clone)]
pub struct ApprovalBoard {
    change: ChangeStore,
    authorizer: Authorizer,
    comments: BTreeMap<String, Vec<ReviewComment>>,
    signatures: BTreeMap<String, Vec<SigningIntent>>,
    next_seq: u64,
}

impl ApprovalBoard {
    /// A fresh, empty board over the standard role matrix (no ABAC rules).
    pub fn new() -> Self {
        ApprovalBoard {
            change: ChangeStore::new(),
            authorizer: Authorizer::new(RoleMatrix::standard(), Vec::new()),
            comments: BTreeMap::new(),
            signatures: BTreeMap::new(),
            next_seq: 0,
        }
    }

    /// Create a ticket in `Draft` status (delegates to the change store).
    pub fn create_ticket(
        &mut self,
        req: &CreateTicketRequest,
    ) -> Result<ChangeTicket, ChangeError> {
        self.change.create_ticket(req)
    }

    /// Mutable handle to a ticket (link revisions / inspect history).
    pub fn ticket_mut(&mut self, id: &str) -> Result<&mut ChangeTicket, ChangeError> {
        self.change.ticket_mut(id)
    }

    /// Fetch one ticket.
    pub fn get_ticket(&self, id: &str) -> Result<ChangeTicket, ChangeError> {
        self.change.get_ticket(id)
    }

    /// All tickets held by the board's store.
    pub fn all_tickets(&self) -> Vec<&ChangeTicket> {
        self.change.all_tickets()
    }

    /// Submit a ticket for review.
    ///
    /// Gates on [`Action::SubmitForReview`] (the Designer grant), then walks
    /// the ticket Draft -> Open -> UnderReview. Returns the usual
    /// `ChangeError` for unknown tickets or illegal transitions.
    pub fn submit_for_review(
        &mut self,
        ticket_id: &str,
        subject: &RoleSubject,
        note: Option<String>,
    ) -> Result<(), ChangeError> {
        self.authorize(subject, Action::SubmitForReview)?;
        {
            let t = self.change.ticket_mut(ticket_id)?;
            match t.status {
                TicketStatus::Draft => {
                    t.set_status(TicketStatus::Open, subject.name(), None)?;
                    t.set_status(TicketStatus::UnderReview, subject.name(), note)?;
                }
                TicketStatus::Open => {
                    t.set_status(TicketStatus::UnderReview, subject.name(), note)?;
                }
                TicketStatus::UnderReview => { /* already in review: no-op */ }
                other => {
                    return Err(ChangeError::InvalidTransition {
                        from: other,
                        to: TicketStatus::UnderReview,
                    });
                }
            }
        }
        Ok(())
    }

    /// Add a reviewer comment that persists with the ticket.
    ///
    /// Gated on [`Action::Read`] (any authenticated principal may comment;
    /// decisions are what require the Reviewer grant). The comment is
    /// stored under the ticket id and retrieved by
    /// [`ApprovalBoard::comments_for`].
    pub fn add_reviewer_comment(
        &mut self,
        ticket_id: &str,
        subject: &RoleSubject,
        body: &str,
    ) -> Result<ReviewComment, ChangeError> {
        self.authorize(subject, Action::Read)?;
        let body = body.trim();
        if body.is_empty() {
            return Err(ChangeError::BadRequest(
                "reviewer comment body must not be empty".into(),
            ));
        }
        self.change.get_ticket(ticket_id)?; // 404 if unknown
        let seq = self.next_seq;
        self.next_seq += 1;
        let comment = ReviewComment {
            id: format!("comment-{seq:x}"),
            ticket_id: ticket_id.to_string(),
            by: subject.name().to_string(),
            body: body.to_string(),
            at: now_rfc3339(),
        };
        self.comments
            .entry(ticket_id.to_string())
            .or_default()
            .push(comment.clone());
        Ok(comment)
    }

    /// Approve or reject a ticket that is currently UnderReview.
    ///
    /// Gates on [`Action::Review`] (the Reviewer grant). On success the
    /// ticket is moved to `Approved`/`Rejected` (via the change model's
    /// validated transition, which also appends the tracked history entry)
    /// and a [`SigningIntent`] with `nature == IntentAffirmation` is
    /// appended to the ticket's signing ledger.
    ///
    /// `affirm` must be non-empty: a decision without an affirmation
    /// statement is refused, so every recorded signature is a real
    /// intent-affirmation.
    pub fn decide(
        &mut self,
        ticket_id: &str,
        subject: &RoleSubject,
        decision: ApprovalDecision,
        signing: SigningRequest,
    ) -> Result<SigningIntent, ChangeError> {
        self.authorize(subject, Action::Review)?;
        let affirm = signing.affirm.trim();
        if affirm.is_empty() {
            return Err(ChangeError::BadRequest(
                "intent-affirmation statement must not be empty".into(),
            ));
        }
        let target = match decision {
            ApprovalDecision::Approve => TicketStatus::Approved,
            ApprovalDecision::Reject => TicketStatus::Rejected,
        };
        // Validates the UnderReview -> target transition and tracks it.
        let t = self.change.ticket_mut(ticket_id)?;
        t.set_status(target, subject.name(), Some(affirm.to_string()))?;

        let seq = self.next_seq;
        self.next_seq += 1;
        let intent = SigningIntent {
            id: format!("sign-{seq:x}"),
            ticket_id: ticket_id.to_string(),
            decision,
            by: subject.name().to_string(),
            affirm: affirm.to_string(),
            at: now_rfc3339(),
            nature: SigningNature::IntentAffirmation,
        };
        self.signatures
            .entry(ticket_id.to_string())
            .or_default()
            .push(intent.clone());
        Ok(intent)
    }

    /// All reviewer comments persisted with a ticket, in insertion order.
    pub fn comments_for(&self, ticket_id: &str) -> Result<Vec<ReviewComment>, ChangeError> {
        self.change.get_ticket(ticket_id)?; // 404 if unknown
        Ok(self.comments.get(ticket_id).cloned().unwrap_or_default())
    }

    /// All signing intents recorded on a ticket, in insertion order.
    pub fn signatures_for(&self, ticket_id: &str) -> Result<Vec<SigningIntent>, ChangeError> {
        self.change.get_ticket(ticket_id)?;
        Ok(self.signatures.get(ticket_id).cloned().unwrap_or_default())
    }

    fn authorize(&self, subject: &RoleSubject, action: Action) -> Result<(), ChangeError> {
        match self.authorizer.decide(subject, &[], &action, &[]) {
            Decision::Allow(_) => Ok(()),
            Decision::Deny(reason) => Err(ChangeError::BadRequest(format!(
                "action {action} denied for principal '{}': {reason:?}",
                subject.name()
            ))),
        }
    }
}

impl Default for ApprovalBoard {
    fn default() -> Self {
        Self::new()
    }
}

/// RFC 3339 UTC timestamp from unix epoch seconds (stdlib-only, deterministic).
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
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
