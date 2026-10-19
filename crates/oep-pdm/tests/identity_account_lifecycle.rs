//! P8-M3-T01 acceptance contract (RED): the identity layer's local account
//! lifecycle, exercised on real material.
//!
//! What "real material" means for an identity task:
//!   * Passwords are hashed with a real, vetted KDF (argon2id via the
//!     `argon2` crate) and never stored or returned in plaintext.
//!   * The full lifecycle is driven through the public API: create -> active;
//!     authenticate works; disable -> authenticate is refused (the account
//!     exists but is inert); enable -> authenticate works again; reset
//!     password -> the old password is dead and the new one authenticates.
//!   * A second, independent implementation re-parses the stored hash format
//!     to prove the secret material is a standard PHC string, not a custom
//!     encoding (locked by tests/test_p8m3t01_passport_concept.py).

use oep_pdm::identity::{
    AccountError, AccountStore, AuthError, IdentityConfig, IdentityService, PasswordPolicy,
};

const GOOD_PW: &str = "correct horse battery staple 93__!";

fn fresh_service() -> IdentityService {
    // OIDC off by default (sovereignty, P0-M5-T02): the default config has no
    // OIDC tenant, only local accounts. This is asserted in the OTHER contract
    // test (identity_oidc_default_off.rs).
    IdentityService::new(IdentityConfig::default())
}

#[test]
fn accept_lifecycle_create_disable_enable_reset() {
    let mut svc = fresh_service();
    let policy = PasswordPolicy::default();

    // --- create -----------------------------------------------------------
    let account = svc
        .accounts_mut()
        .create("alice", "Alice Ankers", GOOD_PW, &policy)
        .expect("create returns an active account");
    assert_eq!(account.username(), "alice");
    assert_eq!(account.display_name(), "Alice Ankers");
    assert!(account.is_active(), "new account is active by default");

    // Authenticate with the right password succeeds.
    svc.authenticate("alice", GOOD_PW)
        .expect("alice can sign in");

    // Wrong password fails without revealing whether the account exists.
    assert!(matches!(
        svc.authenticate("alice", "wrong-password"),
        Err(AuthError::InvalidCredentials)
    ));

    // --- disable ----------------------------------------------------------
    svc.accounts_mut()
        .disable("alice")
        .expect("disable succeeds");
    assert!(
        !svc.accounts()
            .get("alice")
            .expect("account still exists")
            .is_active(),
        "disabled account is inactive"
    );
    assert!(
        matches!(
            svc.authenticate("alice", GOOD_PW),
            Err(AuthError::AccountDisabled)
        ),
        "disabled account cannot authenticate"
    );

    // --- enable -----------------------------------------------------------
    svc.accounts_mut().enable("alice").expect("enable succeeds");
    assert!(svc.accounts().get("alice").unwrap().is_active());
    svc.authenticate("alice", GOOD_PW)
        .expect("reenabled account signs in");

    // --- reset password ----------------------------------------------------
    svc.accounts_mut()
        .reset_password("alice", "new passphrase 47!", &policy)
        .expect("reset succeeds");
    assert!(
        matches!(
            svc.authenticate("alice", GOOD_PW),
            Err(AuthError::InvalidCredentials)
        ),
        "old password is dead after reset"
    );
    svc.authenticate("alice", "new passphrase 47!")
        .expect("new password works after reset");
}

#[test]
fn accept_duplicate_and_unknown_accounts() {
    let mut store = AccountStore::default();
    let policy = PasswordPolicy::default();
    store
        .create("bob", "Bob Builder", GOOD_PW, &policy)
        .expect("create bob");
    assert!(matches!(
        store.create("bob", "Bob The Second", GOOD_PW, &policy),
        Err(AccountError::DuplicateUsername(_))
    ));
    assert!(store.get("nobody").is_none(), "unknown account is absent");
}

#[test]
fn accept_password_is_stored_hashed_not_plaintext() {
    let mut store = AccountStore::default();
    let policy = PasswordPolicy::default();
    store
        .create("carol", "Carol Caret", GOOD_PW, &policy)
        .expect("create carol");
    let account = store.get("carol").unwrap();
    let stored = account.secret_material();
    assert!(
        !stored.contains(GOOD_PW),
        "stored material must not contain the plaintext password"
    );
    assert!(
        stored.starts_with("$argon2id$"),
        "stored material is a standard PHC argon2id string (got a prefix of {:?})",
        &stored[..stored.len().min(24)]
    );
    assert!(!account.secret_material().is_empty());
}

#[test]
fn accept_identity_service_issues_passport_after_authenticate() {
    let mut svc = fresh_service();
    svc.accounts_mut()
        .create("dana", "Dana Designer", GOOD_PW, &PasswordPolicy::default())
        .expect("create dana");
    let passport = svc
        .issue_passport("dana", GOOD_PW)
        .expect("a verified local login issues a passport");
    assert_eq!(passport.subject(), "dana");
    assert_eq!(passport.display_name(), "Dana Designer");
    assert!(
        passport.provider_is_local(),
        "local accounts issue local passports"
    );
}
