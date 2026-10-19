//! P8-M3-T01 acceptance contract (RED): OIDC is OPTIONAL and OFF BY DEFAULT.
//!
//! Sovereignty principle (P0-M5-T02, applied to identity): the PDM works fully
//! with local accounts only. Federated sign-in via OpenID Connect is an
//! explicit opt-in; the default configuration carries no OIDC tenant, and no
//! external identity resolution is attempted when OIDC is unconfigured.

use oep_pdm::identity::{IdentityConfig, OidcConfig};

#[test]
fn accept_default_config_has_no_oidc_provider() {
    let cfg = IdentityConfig::default();
    assert!(
        cfg.oidc().is_none(),
        "default identity config must have NO OIDC provider (OIDC off by default)"
    );
    assert!(
        !cfg.oidc_enabled(),
        "default identity config reports OIDC disabled"
    );
    assert!(
        cfg.local_accounts_enabled(),
        "local accounts are the sovereign default"
    );
}

#[test]
fn accept_oidc_requires_explicit_opt_in() {
    // A tenant only appears by explicit construction.
    let mut cfg = IdentityConfig::default();
    assert!(cfg.oidc().is_none());

    cfg.set_oidc(OidcConfig::new(
        "example-co",
        "https://issuer.example.test",
        "oep-pdm-client",
        "https://issuer.example.test/.well-known/openid-configuration",
        None,
    ));

    assert!(
        cfg.oidc_enabled(),
        "OIDC enables only when the operator opts in"
    );
    let oidc = cfg.oidc().expect("explicitly configured tenant present");
    assert_eq!(oidc.client_id(), "oep-pdm-client");
    assert_eq!(oidc.scopes(), &["openid", "profile"]);
}

#[test]
fn accept_local_auth_path_is_independent_of_oidc() {
    // With OIDC off (default), the local account path is the only path and it
    // works with no discovery, no issuer, no external dependency.
    let mut svc = oep_pdm::identity::IdentityService::new(IdentityConfig::default());
    svc.accounts_mut()
        .create(
            "erin",
            "Erin Engineer",
            "a fine local passphrase !!",
            &oep_pdm::identity::PasswordPolicy::default(),
        )
        .expect("local account created with OIDC unconfigured");
    let passport = svc
        .issue_passport("erin", "a fine local passphrase !!")
        .expect("local login succeeds without any OIDC configuration");
    assert!(passport.provider_is_local());
}
