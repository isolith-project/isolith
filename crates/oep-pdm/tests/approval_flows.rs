//! P8-M4-T02 acceptance, Rust side -- approval flows with reviewer comments
//! and our own e-signature-equivalent (documented not-parity).
//!
//! This is the RED commit for the card, written against the planned public
//! surface of `oep_pdm::approval` before the module exists. It exercises the
//! full submit / review / approve / reject lifecycle on change tickets
//! (P8-M2-T02) with RBAC gating (P8-M3-T02 roles), and asserts the
//! acceptance criteria:
//!
//!   * submit/review/approve/reject works end to end through the board;
//!   * reviewer comments PERSIST with the ticket (retrievable alongside it,
//!     in order, after the decision is recorded);
//!   * the signing concept is recorded as intent-affirmation, explicitly NOT
//!     a legal seal: every decision carries an affirmation statement the
//!     signer (reviewer) asserts, and every recorded signature carries the
//!     `SigningNature::IntentAffirmation` nature (never `LegalSeal`).
//!
//! Roles come from the P8-M3-T02 matrix: Reviewer may `Review`, Designer may
//! `SubmitForReview`; everything is default-deny, so a principal without the
//! right role is denied (that denial is part of the acceptance).

use oep_pdm::approval::{ApprovalBoard, ApprovalDecision, SigningNature, SigningRequest};
use oep_pdm::change::{ChangeError, CreateTicketRequest, RevisionRef, TicketKind, TicketStatus};
use oep_pdm::rbac::{Role, RoleSubject};

fn subject(name: &str, roles: &[Role]) -> RoleSubject {
    RoleSubject::new(name, roles.to_vec())
}

fn designer(name: &str) -> RoleSubject {
    subject(name, &[Role::Designer])
}

fn reviewer(name: &str) -> RoleSubject {
    subject(name, &[Role::Reviewer])
}

fn request(kind: TicketKind, title: &str, by: &str) -> CreateTicketRequest {
    CreateTicketRequest {
        kind,
        title: title.to_string(),
        description: Some("approval acceptance change".to_string()),
        created_by: by.to_string(),
    }
}

#[test]
fn submit_review_approve_works_end_to_end_with_persisted_comments() {
    let mut board = ApprovalBoard::new();

    // A designer drafts an ECR and submits it for review.
    let designer = designer("alice");
    let reviewer = reviewer("carol");

    let ticket = board
        .create_ticket(&request(TicketKind::Ecr, "Widen bracket tab", "alice"))
        .expect("create ECR");
    board
        .ticket_mut(&ticket.id)
        .expect("ticket exists")
        .add_revision(RevisionRef::new("item-bracket", "1").expect("ref"));

    board
        .submit_for_review(&ticket.id, &designer, Some("ready for review".to_string()))
        .expect("designer submits");

    let under_review = board.get_ticket(&ticket.id).expect("still there");
    assert_eq!(under_review.status, TicketStatus::UnderReview);
    assert_eq!(
        under_review.history.len(),
        2,
        "draft->open then open->under_review, both tracked"
    );

    // A reviewer reads the ticket, comments, then approves with an
    // intent-affirmation statement (our e-signature-equivalent).
    board
        .add_reviewer_comment(&ticket.id, &reviewer, "Tab width matches mating part.")
        .expect("reviewer comments");
    board
        .add_reviewer_comment(
            &ticket.id,
            &reviewer,
            "Verified against revision 1 geometry.",
        )
        .expect("second comment persists too");

    let intent = board
        .decide(
            &ticket.id,
            &reviewer,
            ApprovalDecision::Approve,
            SigningRequest {
                affirm: "I affirm I reviewed this ticket and approve the change.".to_string(),
            },
        )
        .expect("reviewer approves with intent");

    // Comments persist WITH the ticket after the decision.
    let comments = board.comments_for(&ticket.id).expect("comments found");
    assert_eq!(comments.len(), 2, "both reviewer comments persisted");
    assert_eq!(comments[0].by, "carol");
    assert!(comments[1].body.contains("revision 1"));
    assert!(comments[0].at.starts_with("20"), "timestamp is RFC3339");

    // The signature is an intent-affirmation record, not a legal seal.
    assert_eq!(intent.decision, ApprovalDecision::Approve);
    assert!(intent.affirm.contains("I affirm"));
    assert_eq!(intent.by, "carol");
    assert_eq!(
        intent.nature,
        SigningNature::IntentAffirmation,
        "recorded signature is intent-affirmation"
    );
    assert_ne!(
        intent.nature,
        SigningNature::LegalSeal,
        "this concept expressly does NOT produce a legal seal"
    );

    let final_ticket = board.get_ticket(&ticket.id).expect("ticket exists");
    assert_eq!(final_ticket.status, TicketStatus::Approved);
    let approvals: Vec<_> = final_ticket
        .history
        .iter()
        .filter(|c| c.to == TicketStatus::Approved)
        .collect();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].by, "carol");
}

#[test]
fn reject_flow_persists_comment_and_denial_intent() {
    let mut board = ApprovalBoard::new();
    let designer = designer("alice");
    let reviewer = reviewer("dave");

    let ticket = board
        .create_ticket(&request(TicketKind::Ecr, "Thinner fastener", "alice"))
        .expect("create ECR");
    board
        .submit_for_review(&ticket.id, &designer, None)
        .expect("submit");
    board
        .add_reviewer_comment(
            &ticket.id,
            &reviewer,
            "Fastener below min wall after derating.",
        )
        .expect("comment");

    let intent = board
        .decide(
            &ticket.id,
            &reviewer,
            ApprovalDecision::Reject,
            SigningRequest {
                affirm: "I affirm I reviewed this ticket and reject the change.".to_string(),
            },
        )
        .expect("reviewer rejects");

    assert_eq!(intent.decision, ApprovalDecision::Reject);
    assert_eq!(intent.nature, SigningNature::IntentAffirmation);
    assert_eq!(
        board.get_ticket(&ticket.id).unwrap().status,
        TicketStatus::Rejected,
        "rejection is a terminal tracked state"
    );
    assert_eq!(board.comments_for(&ticket.id).unwrap().len(), 1);
}

#[test]
fn reviewer_without_review_role_is_denied_default_deny() {
    let mut board = ApprovalBoard::new();
    let alice = designer("alice");
    // A second designer is NOT a reviewer: the matrix has no Review grant.
    let mallory = designer("mallory");

    let ticket = board
        .create_ticket(&request(TicketKind::Ecr, "Another tab", "alice"))
        .expect("create ECR");
    board
        .submit_for_review(&ticket.id, &alice, None)
        .expect("submit");

    match board.decide(
        &ticket.id,
        &mallory,
        ApprovalDecision::Approve,
        SigningRequest {
            affirm: "I affirm I approve.".to_string(),
        },
    ) {
        Err(ChangeError::BadRequest(msg)) if msg.contains("denied") => {
            // default-deny held: a designer cannot approve a review
        }
        other => panic!("expected denial, got {other:?}"),
    }

    // The ticket is unchanged and no signature was recorded.
    assert_eq!(
        board.get_ticket(&ticket.id).unwrap().status,
        TicketStatus::UnderReview
    );
    assert!(board.signatures_for(&ticket.id).unwrap().is_empty());
}

#[test]
fn cannot_approve_a_ticket_that_is_not_under_review() {
    let mut board = ApprovalBoard::new();
    let carol = reviewer("carol");
    let ticket = board
        .create_ticket(&request(TicketKind::Ecr, "Not submitted", "alice"))
        .expect("create ECR");

    // A ticket still in Draft cannot be approved, even by a reviewer.
    match board.decide(
        &ticket.id,
        &carol,
        ApprovalDecision::Approve,
        SigningRequest {
            affirm: "I affirm I approve.".to_string(),
        },
    ) {
        Err(ChangeError::InvalidTransition { from, to }) => {
            assert_eq!(from, TicketStatus::Draft);
            assert_eq!(to, TicketStatus::Approved);
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}
