//! P8-M4-T03 acceptance contract: the admin can grant/revoke roles **live**.
//!
//! The first acceptance pillar: "admin can grant/revoke roles live; changes
//! take effect". A role grant must be immediately visible to the next
//! authorization decision on the SAME running service (no restart, no reload),
//! and a revoke must be immediately visible too. Each grant/revoke is an
//! admin mutation, so it is also audit-logged (tamper-evident store). Only a
//! principal that already holds an admin-grade role may grant/revoke; the
//! authorization gate itself must read the live registry.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use oep_pdm::admin::{AdminError, AdminService};
use oep_pdm::identity::{IdentityConfig, IdentityService};
use oep_pdm::rbac::{Action, AllowReason, Decision, Role};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique, throwaway temp directory that cleans up after itself.
struct TempRoot {
    dir: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "oep-pdm-admin-role-it-{}-{tag}-{n}",
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

/// Identity service with a handful of accounts, ready to receive roles.
fn harness() -> (TempRoot, IdentityService) {
    let t = TempRoot::new("harness");
    let mut identity = IdentityService::new(IdentityConfig::default());
    for user in ["root", "ada", "grace", "alice", "mallory"] {
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

#[test]
fn granted_role_takes_effect_live_without_restart() {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();

    // Before any grant, ada holds no roles -> every decision is default-deny.
    let before = service.authorize("ada", Action::EditRevision);
    assert!(!before.is_allowed(), "fresh principal must start denied");

    // root (bootstrap admin) grants ada the designer role.
    let changed = service
        .grant_role("root", "ada", Role::Designer)
        .expect("admin grant succeeds");
    assert!(changed, "grant of a new role must report changed=true");

    // Same running service, no reload: the very next decision sees it.
    let after = service.authorize("ada", Action::EditRevision);
    assert!(
        after.is_allowed(),
        "role grant must take effect live, got {after:?}"
    );
    assert!(
        !service.authorize("ada", Action::Release).is_allowed(),
        "designer must not gain release-authority actions"
    );
}

#[test]
fn revoked_role_takes_effect_live() {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();
    service.grant_role("root", "ada", Role::Reviewer).unwrap();
    assert!(service.authorize("ada", Action::Review).is_allowed());

    let changed = service.revoke_role("root", "ada", Role::Reviewer).unwrap();
    assert!(changed, "revoke of an existing role reports changed=true");
    assert!(
        !service.authorize("ada", Action::Review).is_allowed(),
        "role revoke must take effect live"
    );
    // Revoking a role ada never had is a no-op (false), not an error.
    let noop = service.revoke_role("root", "ada", Role::Reviewer).unwrap();
    assert!(!noop);
}

#[test]
fn only_an_admin_grade_principal_can_grant_or_revoke() {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();
    service.grant_role("root", "ada", Role::Designer).unwrap();

    // ada is only a designer: she must NOT be allowed to manage roles.
    let err = service
        .grant_role("ada", "grace", Role::Reviewer)
        .unwrap_err();
    assert!(
        matches!(err, AdminError::Forbidden(_)),
        "non-admin grant refused, got {err:?}"
    );
    assert!(
        !service.authorize("grace", Action::Review).is_allowed(),
        "refused grant must not take effect"
    );
    let err = service
        .revoke_role("ada", "ada", Role::Designer)
        .unwrap_err();
    assert!(matches!(err, AdminError::Forbidden(_)));
    assert!(service.authorize("ada", Action::EditRevision).is_allowed());

    // A principal with NO roles at all is also refused.
    let err = service
        .grant_role("mallory", "grace", Role::Reviewer)
        .unwrap_err();
    assert!(matches!(err, AdminError::Forbidden(_)));
}

#[test]
fn role_matrix_asserts_documented_standard_grants() {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();
    service.grant_role("root", "ada", Role::Designer).unwrap();
    service
        .grant_role("root", "grace", Role::ReleaseAuthority)
        .unwrap();
    service.grant_role("root", "alice", Role::Reviewer).unwrap();

    // Matrix-level (role x action) assertions mirror the RBAC card's standard
    // matrix: the admin surface exposes the role matrix live.
    let matrix = service.role_matrix();
    for action in Action::ALL {
        assert!(matrix.allows(Role::Admin, action), "admin: {action}");
    }
    assert!(matrix.allows(Role::Designer, Action::CreateItem));
    assert!(matrix.allows(Role::Designer, Action::SubmitForReview));
    assert!(!matrix.allows(Role::Designer, Action::Release));
    assert!(matrix.allows(Role::Reviewer, Action::Review));
    assert!(!matrix.allows(Role::Reviewer, Action::Release));
    assert!(matrix.allows(Role::ReleaseAuthority, Action::Release));

    // The grants registry reports who holds what.
    let grants = service.grants();
    assert_eq!(grants["ada"].as_slice(), &[Role::Designer]);
    assert_eq!(grants["grace"].as_slice(), &[Role::ReleaseAuthority]);
    assert_eq!(grants["alice"].as_slice(), &[Role::Reviewer]);
    assert_eq!(
        service.authorize("ada", Action::SubmitForReview),
        Decision::Allow(AllowReason::Role(Role::Designer))
    );
}

#[test]
fn grant_and_revoke_are_audit_logged_tamper_evidently() {
    let (t, identity) = harness();
    let service = AdminService::open_with_bootstrap(&t.dir, identity, "root").unwrap();

    service.grant_role("root", "ada", Role::Designer).unwrap();
    service.revoke_role("root", "ada", Role::Designer).unwrap();
    service.grant_role("root", "grace", Role::Reviewer).unwrap();

    // The audit trail is the durable record: verify() re-derives the chain.
    let entries = service.verify_audit().expect("chain verifies");
    let bodies: Vec<String> = entries
        .iter()
        .map(|e| format!("{}:{}:{}", e.actor, e.action, e.target))
        .collect();
    let joined = bodies.join("|");

    // Bootstrapping root + every grant/revoke must be recorded and keyed by
    // the involved principal target ("role:ada", "role:grace").
    assert!(
        joined.contains("ada") && joined.contains("grace"),
        "role mutations must be audit-logged, got {joined}"
    );
    assert!(
        joined.matches("root").count() >= 4,
        "every admin mutation leaves an entry (boot/grant/revoke/grant), got {joined}"
    );
    // The chain still verifies after a reopen, i.e. the log is tamper-evident.
    drop(service);
    let reopened = AdminService::open_with_bootstrap(&t.dir, harness().1, "root").unwrap();
    let again = reopened.verify_audit().expect("reopened chain verifies");
    assert!(again.len() >= entries.len());
}
