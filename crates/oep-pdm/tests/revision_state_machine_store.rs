//! P8-M2-T01 integration: persisted revision state machine over the public
//! `oep_pdm` API. Covers the acceptance criteria that span state + store:
//! persistence round trips, queryability, and released-state immutability
//! enforced across a reopen (i.e. enforced by the store, not just in memory).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use oep_pdm::state::{Maturity, RevisionState};
use oep_pdm::revstore::{RevisionStore, StoreError};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique, throwaway temp directory that cleans itself up on drop.
struct TempRoot {
    dir: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("oep-pdm-revsm-it-{}-{tag}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp root");
        Self { dir }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Drive a revision to Released via the public API.
fn make_released(store: &mut RevisionStore, item: &str, by: &str) -> u32 {
    let rev = store.create_revision(item, by).unwrap();
    let n = rev.revision;
    for _ in 0..3 {
        store.advance_maturity(item, n).unwrap();
    }
    store
        .transition(item, n, RevisionState::UnderReview)
        .unwrap();
    store.transition(item, n, RevisionState::Released).unwrap();
    n
}

#[test]
fn full_lifecycle_through_public_api() {
    let t = TempRoot::new("lifecycle");
    let mut store = RevisionStore::open(&t.dir).unwrap();
    let rev = store.create_revision("bracket", "hal").unwrap();
    let n = rev.revision;
    assert_eq!(rev.state, RevisionState::Working);

    store
        .transition("bracket", n, RevisionState::UnderReview)
        .unwrap();
    // returned for rework
    store
        .transition("bracket", n, RevisionState::Working)
        .unwrap();
    store
        .set_maturity("bracket", n, Maturity::Detailed)
        .unwrap();
    store.advance_maturity("bracket", n).unwrap();
    assert_eq!(
        store.get_revision("bracket", n).unwrap().maturity,
        Some(Maturity::Final)
    );

    store
        .transition("bracket", n, RevisionState::UnderReview)
        .unwrap();
    store
        .transition("bracket", n, RevisionState::Released)
        .unwrap();
    let released = store.get_revision("bracket", n).unwrap();
    assert_eq!(released.state, RevisionState::Released);
    assert_eq!(released.maturity, Some(Maturity::Final));

    store
        .transition("bracket", n, RevisionState::Obsolete)
        .unwrap();
    assert_eq!(
        store.get_revision("bracket", n).unwrap().state,
        RevisionState::Obsolete
    );
}

#[test]
fn persistence_survives_reopen_and_stays_queryable() {
    let t = TempRoot::new("persist");
    {
        let mut store = RevisionStore::open(&t.dir).unwrap();
        for i in 1..=3 {
            let rev = store.create_revision("widget", "ida").unwrap();
            let n = rev.revision;
            if i == 3 {
                for _ in 0..3 {
                    store.advance_maturity("widget", n).unwrap();
                }
                store
                    .transition("widget", n, RevisionState::Released)
                    .unwrap();
            }
        }
    }
    let store = RevisionStore::open(&t.dir).unwrap();
    assert_eq!(store.all_items().unwrap().len(), 1);
    let list = store.list_revisions("widget").unwrap();
    assert_eq!(list.len(), 3);
    // Newest first; the released revision is revision 3.
    assert_eq!(list[0].revision, 3);
    assert_eq!(list[0].state, RevisionState::Released);
    assert_eq!(list[0].maturity, Some(Maturity::Final));
    assert_eq!(list[1].revision, 2);
    assert_eq!(list[2].revision, 1);
    let latest = store.latest_revision("widget").unwrap().unwrap();
    assert_eq!(latest.revision, 3);
    // The stored JSON files exist on disk (persisted, not just in memory).
    let rev_file = store.root().join("revisions").join("widget").join("3.json");
    assert!(rev_file.exists(), "revision snapshot must be durable");
}

#[test]
fn released_state_is_immutable_across_reopen() {
    let t = TempRoot::new("immutable-reopen");
    let root = t.dir.clone();
    let n;
    {
        let mut store = RevisionStore::open(&root).unwrap();
        n = make_released(&mut store, "fixture-9", "jan");
    }
    // Reopen a fresh handle: immutability must still be enforced.
    let mut store = RevisionStore::open(&root).unwrap();
    // Changing state back is impossible.
    assert!(matches!(
        store.transition("fixture-9", n, RevisionState::Working),
        Err(StoreError::State(_))
    ));
    assert!(matches!(
        store.transition("fixture-9", n, RevisionState::UnderReview),
        Err(StoreError::State(_))
    ));
    // Changing maturity is impossible.
    assert!(matches!(
        store.set_maturity("fixture-9", n, Maturity::Preliminary),
        Err(StoreError::State(_))
    ));
    // Attaching content is impossible.
    assert!(matches!(
        store.attach_content("fixture-9", n, "f", b"data", None),
        Err(StoreError::State(_))
    ));
    // The only remaining hop is the terminal one.
    store
        .transition("fixture-9", n, RevisionState::Obsolete)
        .unwrap();
    assert!(store
        .transition("fixture-9", n, RevisionState::Obsolete)
        .is_err());
}

#[test]
fn maturity_gate_blocks_release_and_persists_unchanged() {
    let t = TempRoot::new("gate");
    let mut store = RevisionStore::open(&t.dir).unwrap();
    let rev = store.create_revision("cap", "kim").unwrap();
    let n = rev.revision;

    // Under-review to released without final maturity: refused.
    store
        .transition("cap", n, RevisionState::UnderReview)
        .unwrap();
    assert!(matches!(
        store.transition("cap", n, RevisionState::Released),
        Err(StoreError::State(_))
    ));
    // State unchanged on disk.
    assert_eq!(
        store.get_revision("cap", n).unwrap().state,
        RevisionState::UnderReview
    );

    // Reach final maturity, then release succeeds.
    for _ in 0..3 {
        store.advance_maturity("cap", n).unwrap();
    }
    store.transition("cap", n, RevisionState::Released).unwrap();
    assert_eq!(
        store.get_revision("cap", n).unwrap().state,
        RevisionState::Released
    );
}

#[test]
fn content_attach_maturity_set_and_release_query_agree() {
    let t = TempRoot::new("agree");
    let mut store = RevisionStore::open(&t.dir).unwrap();
    let rev = store.create_revision("doc", "leo").unwrap();
    let n = rev.revision;
    store
        .attach_content("doc", n, "spec.md", b"# spec v1\n", Some("initial"))
        .unwrap();
    let rev = store.get_revision("doc", n).unwrap();
    assert!(rev.blob_sha256.is_some());
    assert_eq!(rev.comment.as_deref(), Some("initial"));
    assert!(store.verify_content("doc", n).unwrap());

    // A released revision keeps its content address and it stays verifiable.
    for _ in 0..3 {
        store.advance_maturity("doc", n).unwrap();
    }
    store.transition("doc", n, RevisionState::Released).unwrap();
    let rev = store.get_revision("doc", n).unwrap();
    assert!(
        rev.blob_sha256.is_some(),
        "content survives the release freeze"
    );
    assert!(store.verify_content("doc", n).unwrap());
}
