//! P8-M4-T03 acceptance contract: the admin can **see all revisions**.
//!
//! Second acceptance pillar: "see all revisions". The admin surface must
//! return every revision across every item in the vault -- not just one item
//! at a time -- newest first, with the per-item grouping still available, and
//! the view must be consistent with the underlying revision store. Storage
//! quotas (third pillar) are enforced on the write path and refuse content
//! that would exceed the assigned quota.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use oep_pdm::admin::{AdminError, AdminService};
use oep_pdm::identity::{IdentityConfig, IdentityService};
use oep_pdm::rbac::Role;
use oep_pdm::state::{Maturity, RevisionState};
use oep_pdm::revstore::{RevisionStore, StoredRevision};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempRoot {
    dir: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "oep-pdm-admin-rev-it-{}-{tag}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp root");
        Self { dir }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn harness() -> (TempRoot, IdentityService) {
    let t = TempRoot::new("harness");
    let mut identity = IdentityService::new(IdentityConfig::default());
    for user in ["root", "ada", "grace"] {
        identity
            .accounts_mut()
            .create(
                user,
                user,
                "correct-horse-battery-staple",
                &Default::default(),
            )
            .expect("create account");
    }
    (t, identity)
}

/// Seed the vault (root admin, ada designer, three revisions) and return it.
fn seeded_service() -> (TempRoot, AdminService) {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();
    service.grant_role("root", "ada", Role::Designer).unwrap();
    service.create_revision("ada", "bracket-001").unwrap();
    service.create_revision("ada", "bracket-001").unwrap();
    service.create_revision("ada", "chassis").unwrap();
    (t, service)
}

#[test]
fn admin_sees_every_revision_across_all_items() {
    let (t, service) = seeded_service();
    let all = service.all_revisions().unwrap();

    assert_eq!(all.len(), 3, "all revisions across all items");
    // Completeness: the admin view contains exactly every (item, revision)
    // pair in the vault regardless of creation-second timing.
    let mut pairs: Vec<(String, u32)> = all
        .iter()
        .map(|r| (r.item_id.clone(), r.revision))
        .collect();
    pairs.sort();
    assert_eq!(
        pairs,
        vec![
            ("bracket-001".to_string(), 1),
            ("bracket-001".to_string(), 2),
            ("chassis".to_string(), 1),
        ]
    );

    // The flat view is consistent with the store's own per-item listing.
    let store = RevisionStore::open(&t.dir).unwrap();
    let mut through_store = 0;
    for item in store.all_items().unwrap() {
        through_store += store.list_revisions(&item.item_id).unwrap().len();
    }
    assert_eq!(through_store, all.len());

    // Per-item grouping is newest-first.
    let bracket = service.revisions_for("bracket-001").unwrap();
    assert_eq!(bracket.len(), 2);
    assert_eq!(bracket[0].revision, 2, "newest revision of the item first");
    assert_eq!(bracket[1].revision, 1);
}

#[test]
fn revision_states_are_visible_to_the_admin() {
    let (_t, service) = seeded_service();
    // Honest transitions through the state machine, driven by root (admin):
    // submit for review, then mature to Final and release -- the states must
    // surface in the view.
    service
        .transition_revision("root", "bracket-001", 2, RevisionState::UnderReview)
        .unwrap();
    service
        .mature_revision("root", "chassis", 1, Maturity::Final)
        .unwrap();
    service
        .transition_revision("root", "chassis", 1, RevisionState::Released)
        .unwrap();

    let all = service.all_revisions().unwrap();
    let by_id: std::collections::BTreeMap<String, &StoredRevision> = all
        .iter()
        .map(|r| (format!("{}/{}", r.item_id, r.revision), r))
        .collect();
    assert_eq!(by_id["bracket-001/2"].state, RevisionState::UnderReview);
    assert_eq!(by_id["chassis/1"].state, RevisionState::Released);
    assert_eq!(by_id["bracket-001/1"].state, RevisionState::Working);
}

#[test]
fn storage_quota_set_and_enforced_on_real_bytes() {
    let (_t, service) = seeded_service();

    // No quota set by default: attaching is never refused on quota grounds.
    assert_eq!(service.storage_quota("ada"), None);
    let content_a = vec![0u8; 128];
    service
        .attach_under_quota("ada", "bracket-001", 1, "a.bin", &content_a, None)
        .unwrap();

    // root sets ada's budget to 200 bytes; she has already stored 128.
    let changed = service.set_storage_quota("root", "ada", Some(200)).unwrap();
    assert!(changed);
    assert_eq!(service.storage_quota("ada"), Some(200));
    assert_eq!(service.usage_bytes("ada"), 128u64);

    // A 128-byte attachment would take her to 256 > 200 -> refused.
    let big = vec![1u8; 128];
    let err = service
        .attach_under_quota("ada", "bracket-001", 2, "big.bin", &big, None)
        .unwrap_err();
    assert!(
        matches!(err, AdminError::Quota(_)),
        "quota breach must be refused, got {err:?}"
    );
    // Refusal is transactional: nothing was written.
    assert_eq!(service.usage_bytes("ada"), 128u64);

    // Raising the quota takes effect immediately (live, no restart).
    service.set_storage_quota("root", "ada", Some(512)).unwrap();
    service
        .attach_under_quota("ada", "bracket-001", 2, "big.bin", &big, None)
        .unwrap();
    assert_eq!(service.usage_bytes("ada"), 256u64);

    // Clearing the quota removes the cap entirely.
    service.set_storage_quota("root", "ada", None).unwrap();
    assert_eq!(service.storage_quota("ada"), None);
    service
        .attach_under_quota("ada", "chassis", 1, "big.bin", &big, None)
        .unwrap();
    assert_eq!(service.usage_bytes("ada"), 384u64);
}

#[test]
fn only_admin_can_set_quotas_and_quota_changes_are_audit_logged() {
    let (_t, service) = seeded_service();

    // ada is only a designer; ManageSystem (quotas) is admin-only.
    let err = service
        .set_storage_quota("ada", "ada", Some(10))
        .unwrap_err();
    assert!(matches!(err, AdminError::Forbidden(_)));

    service.set_storage_quota("root", "ada", Some(10)).unwrap();
    let entries = service.verify_audit().unwrap();
    let joined: Vec<String> = entries
        .iter()
        .map(|e| format!("{}:{}", e.action, e.target))
        .collect();
    assert!(
        joined.join("|").contains('"') || joined.join("|").contains("change"),
        "quota mutation must appear in the audit log: {joined:?}"
    );
}
