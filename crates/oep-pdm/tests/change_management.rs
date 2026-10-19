//! P8-M2-T02 acceptance, Rust side -- change management tickets (ECR/ECO)
//! linked to revision sets, with tracked statuses.
//!
//! This is the RED commit for the card, written against the planned public
//! surface of `oep_pdm::change` before the module exists. It exercises the
//! model through its public API exactly the way the PDM lane's downstream
//! cards (approval flows P8-M4-T02, REST surface P8-M6-T01) will call it:
//! create a ticket in a store, link a revision set, advance status through
//! the lite lifecycle, and query back by status and by affected item.

use oep_pdm::change::{
    ChangeError, ChangeStore, CreateTicketRequest, RevisionRef, TicketKind, TicketStatus,
};

fn request(kind: TicketKind, title: &str, by: &str) -> CreateTicketRequest {
    CreateTicketRequest {
        kind,
        title: title.to_string(),
        description: Some("acceptance change".to_string()),
        created_by: by.to_string(),
    }
}

#[test]
fn ecr_ticket_links_a_revision_set() {
    let mut store = ChangeStore::new();
    let ticket = store
        .create_ticket(&request(TicketKind::Ecr, "Widen mounting tab", "alice"))
        .expect("create ECR");
    assert_eq!(ticket.kind, TicketKind::Ecr);
    assert_eq!(ticket.status, TicketStatus::Draft);
    assert!(ticket.revision_set.is_empty());

    let t = store.ticket_mut(&ticket.id).expect("ticket exists");
    let added = t.add_revision(RevisionRef::new("item-bracket", "1").unwrap());
    assert!(added, "first link added");
    let dup = t.add_revision(RevisionRef::new("item-bracket", "1").unwrap());
    assert!(!dup, "duplicate link is a no-op");
    t.add_revision(RevisionRef::new("item-fixing", "2").unwrap());

    assert_eq!(
        t.revision_set.len(),
        2,
        "revision set holds 2 distinct refs"
    );
    assert!(t
        .revision_set
        .contains(&RevisionRef::new("item-bracket", "1").unwrap()));
}

#[test]
fn eco_status_lifecycle_is_tracked_end_to_end() {
    let mut store = ChangeStore::new();
    let ticket = store
        .create_ticket(&request(TicketKind::Eco, "Release bracket v3", "bob"))
        .expect("create ECO");

    {
        let t = store.ticket_mut(&ticket.id).unwrap();
        t.add_revision(RevisionRef::new("item-bracket", "2").unwrap());
        t.set_status(TicketStatus::Open, "bob", None).unwrap();
        t.set_status(TicketStatus::UnderReview, "bob", None)
            .unwrap();
        t.set_status(TicketStatus::Approved, "carol", Some("meets spec".into()))
            .unwrap();
        t.set_status(TicketStatus::Implemented, "bob", None)
            .unwrap();
        assert!(t.is_terminal());
        assert_eq!(t.status, TicketStatus::Implemented);
    }

    let stored = store.get_ticket(&ticket.id).unwrap();
    assert_eq!(
        stored.history.len(),
        4,
        "four real transitions, all tracked"
    );
    let approvals: Vec<_> = stored
        .history
        .iter()
        .filter(|c| c.to == TicketStatus::Approved)
        .collect();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].by, "carol");
    assert_eq!(approvals[0].note.as_deref(), Some("meets spec"));
    assert!(
        stored
            .history
            .iter()
            .all(|c| c.from != c.to && !c.at.is_empty() && !c.by.is_empty()),
        "every history entry is a real, attributed transition"
    );
}

#[test]
fn invalid_transition_is_rejected_and_history_stays_accurate() {
    let mut store = ChangeStore::new();
    let ticket = store
        .create_ticket(&request(TicketKind::Ecr, "Jump the queue", "dave"))
        .unwrap();

    {
        let t = store.ticket_mut(&ticket.id).unwrap();
        assert_eq!(
            t.set_status(TicketStatus::Approved, "dave", None),
            Err(ChangeError::InvalidTransition {
                from: TicketStatus::Draft,
                to: TicketStatus::Approved,
            })
        );
        assert_eq!(
            t.status,
            TicketStatus::Draft,
            "status unchanged on rejection"
        );
        assert!(t.history.is_empty(), "failed transition records nothing");
    }
}

#[test]
fn tickets_are_queryable_by_status_and_by_affected_item() {
    let mut store = ChangeStore::new();
    let a = store
        .create_ticket(&request(TicketKind::Eco, "Change A", "erin"))
        .unwrap();
    let b = store
        .create_ticket(&request(TicketKind::Ecr, "Change B", "erin"))
        .unwrap();

    {
        let t = store.ticket_mut(&a.id).unwrap();
        t.add_revision(RevisionRef::new("item-42", "1").unwrap());
        t.set_status(TicketStatus::Open, "erin", None).unwrap();
    }
    {
        let t = store.ticket_mut(&b.id).unwrap();
        t.add_revision(RevisionRef::new("item-42", "1").unwrap());
        t.add_revision(RevisionRef::new("item-7", "3").unwrap());
    }

    let open = store.tickets_by_status(TicketStatus::Open);
    assert_eq!(open.len(), 1, "exactly one ticket is Open");

    let mut for_42: Vec<&str> = store
        .tickets_for_item("item-42")
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    for_42.sort_unstable();
    assert_eq!(for_42, vec![a.id.as_str(), b.id.as_str()]);

    assert_eq!(store.tickets_for_item("item-7").len(), 1);
    assert!(store.tickets_for_item("item-nowhere").is_empty());
}

#[test]
fn kinds_and_statuses_parse_case_insensitively() {
    assert_eq!(TicketKind::parse("ecr").unwrap(), TicketKind::Ecr);
    assert_eq!(TicketKind::parse("ECO").unwrap(), TicketKind::Eco);
    assert_eq!(
        TicketStatus::parse("under_review").unwrap(),
        TicketStatus::UnderReview
    );
    // Parsed statuses drive the same validated transitions.
    let mut store = ChangeStore::new();
    let ticket = store
        .create_ticket(&request(TicketKind::Ecr, "Using tags", "frank"))
        .unwrap();
    {
        let t = store.ticket_mut(&ticket.id).unwrap();
        let from_open = TicketStatus::parse("open").unwrap();
        let to_review = TicketStatus::parse("under_review").unwrap();
        t.set_status(from_open, "frank", None).unwrap();
        t.set_status(to_review, "frank", None).unwrap();
        assert_eq!(t.status, TicketStatus::UnderReview);
    }
}

#[test]
fn terminal_status_blocks_all_further_transition() {
    let mut store = ChangeStore::new();
    let ticket = store
        .create_ticket(&request(
            TicketKind::Ecr,
            "Cancelled before submit",
            "grace",
        ))
        .unwrap();
    {
        let t = store.ticket_mut(&ticket.id).unwrap();
        t.set_status(TicketStatus::Cancelled, "grace", None)
            .unwrap();
        assert!(t.is_terminal());
        assert_eq!(
            t.set_status(TicketStatus::Draft, "grace", None),
            Err(ChangeError::InvalidTransition {
                from: TicketStatus::Cancelled,
                to: TicketStatus::Draft,
            })
        );
    }
}
