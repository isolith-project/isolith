//! P7-M2-T01 acceptance for the P8 PDM API contract + local stub (written
//! first): the workbench's PDM save/open surface is stubbed against this
//! contract, so the contract itself must be measured, not guessed.
//!
//! Contract locked here:
//!
//! 1. Every `PdmClient` op is typed and non-panicking; missing item/revision
//!    are typed `PdmError`s.
//! 2. `save_revision` returns a record whose `blob_digest` equals
//!    `digest::content_digest` of the saved bytes (content-addressed).
//! 3. Equal content under different revisions shares one digest (honest
//!    dedup at the content-addressing layer).
//! 4. `open_revision` returns the exact bytes saved; `list_revisions` returns
//!    newest-first; `head_revision` is the latest by revision string.
//! 5. Empty item/revision ids are rejected as inconsistent (never panic).

use oep_pdm::{LocalPdmVault, PdmClient, PdmError, PdmWriteRequest, RevisionState};

#[test]
fn save_and_open_round_trip_are_byte_exact() {
    let mut vault = LocalPdmVault::new();
    let content = br#"{"format":"oep-geo","units":"mm"}"#.to_vec();
    let req = PdmWriteRequest::new("PART-001", "A", content.clone()).with_summary("first");
    let rec = vault.save_revision(&req).expect("save ok");
    assert_eq!(rec.item_id, "PART-001");
    assert_eq!(rec.revision, "A");
    assert_eq!(rec.state, RevisionState::Working);
    assert_eq!(
        rec.blob_digest.as_deref(),
        Some(oep_pdm::digest::content_digest(&content).as_str())
    );
    assert_eq!(rec.summary.as_deref(), Some("first"));
    assert!(rec.created_at > 0, "timestamp recorded");

    let bytes = vault.open_revision("PART-001", "A").expect("open ok");
    assert_eq!(bytes, content, "open returns the exact saved bytes");

    let same = vault.digest_of("PART-001", "A");
    assert_eq!(same.as_deref(), Some(rec.blob_digest.as_deref().unwrap()));
}

#[test]
fn equal_content_addresses_equal_digest() {
    let mut vault = LocalPdmVault::new();
    let content = b"same-content".to_vec();
    let _ = vault
        .save_revision(&PdmWriteRequest::new("ITEM", "A", content.clone()))
        .expect("save A");
    let _ = vault
        .save_revision(&PdmWriteRequest::new("ITEM", "B", content.clone()))
        .expect("save B");
    let a = vault.digest_of("ITEM", "A");
    let b = vault.digest_of("ITEM", "B");
    assert_eq!(a, b, "identical bytes, one content address");
}

#[test]
fn list_is_newest_first_and_head_is_latest() {
    let mut vault = LocalPdmVault::new();
    for rev in ["A", "B", "C"] {
        let req = PdmWriteRequest::new(
            "PART-001",
            rev.to_string(),
            format!("rev-{rev}").into_bytes(),
        );
        vault.save_revision(&req).expect("save ok");
    }
    let list = vault.list_revisions("PART-001").expect("list ok");
    let revs: Vec<&str> = list.iter().map(|r| r.revision.as_str()).collect();
    assert_eq!(revs, vec!["C", "B", "A"], "newest first");
    assert_eq!(
        vault.head_revision("PART-001").expect("head ok").revision,
        "C"
    );
}

#[test]
fn missing_item_and_revision_are_typed_errors() {
    let mut vault = LocalPdmVault::new();
    assert!(matches!(
        vault.open_revision("nope", "A"),
        Err(PdmError::ItemNotFound { .. })
    ));
    let _ = vault
        .save_revision(&PdmWriteRequest::new("ITEM", "A", b"x".to_vec()))
        .expect("save ok");
    assert!(matches!(
        vault.open_revision("ITEM", "Z"),
        Err(PdmError::RevisionNotFound { .. })
    ));
    assert!(matches!(
        vault.list_revisions("nope"),
        Err(PdmError::ItemNotFound { .. })
    ));
    assert!(matches!(
        vault.head_revision("nope"),
        Err(PdmError::ItemNotFound { .. })
    ));
}

#[test]
fn empty_ids_are_rejected_not_panics() {
    let mut vault = LocalPdmVault::new();
    let req = PdmWriteRequest::new("", "A", b"x".to_vec());
    assert!(vault.save_revision(&req).is_err(), "empty item rejected");
    let req = PdmWriteRequest::new("ITEM", "", b"x".to_vec());
    assert!(
        vault.save_revision(&req).is_err(),
        "empty revision rejected"
    );
    assert!(vault.is_empty(), "nothing was stored");
}
