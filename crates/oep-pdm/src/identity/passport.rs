//! The verified principal: our own take on the "passport" concept
//! (P8-M3-T01).
//!
//! The incumbent platform's commercial identity product is not referenced or
//! recreated here. We adopt only the abstract idea - *one verified identity
//! that travels with the user across the engineering platform* - and define it
//! ourselves: a small, explicit [`IdentityPassport`] carrying the subject, a
//! human display name, and the provider that vouches for it. RBAC (P8-M3-T02)
//! converts this into a `RoleSubject` for authorization decisions.

use serde::{Deserialize, Serialize};

/// Which provider vouches for a passport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassportProvider {
    /// Vouched for by a local account in this storage.
    Local,
    /// Vouched for by an OIDC tenant (federated, opt-in only). Carries the
    /// configured provider name.
    Oidc(String),
}

/// The verified principal after a login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityPassport {
    subject: String,
    display_name: String,
    provider: PassportProvider,
}

impl IdentityPassport {
    /// The login name / subject.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Human display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Provider that vouched for this identity.
    pub fn provider(&self) -> &PassportProvider {
        &self.provider
    }

    /// True when vouched for by a local account.
    pub fn provider_is_local(&self) -> bool {
        matches!(self.provider, PassportProvider::Local)
    }

    /// Issue a local-account passport (called by the identity service after a
    /// verified local login).
    pub(crate) fn local(subject: String, display_name: String) -> Self {
        IdentityPassport {
            subject,
            display_name,
            provider: PassportProvider::Local,
        }
    }
}
