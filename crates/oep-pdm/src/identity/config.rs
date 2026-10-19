//! Identity configuration: local accounts + optional OIDC (P8-M3-T01).
//!
//! Sovereignty principle (P0-M5-T02, applied to identity): the PDM signs you
//! in with local accounts and forwards NOTHING to any third party unless an
//! operator explicitly opts in. Therefore the default [`IdentityConfig`] has:
//!
//! * `local_accounts_enabled: true` - local accounts are always available;
//! * `oidc: None` - **no OIDC tenant by default**. Federated sign-in appears
//!   only when code constructs an [`OidcConfig`] and calls [`set_oidc`].
//!
//! This mirrors the telemetry decision: *defaults are the product*, and the
//! default must be sovereign.

/// An OpenID Connect relying-party tenant (optional, off by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    /// Human/provider name, e.g. the company SSO.
    provider_name: String,
    /// OIDC issuer URL (the `iss` value tokens must carry).
    issuer: String,
    /// This deployment's client id at the issuer.
    client_id: String,
    /// OIDC discovery document URL (`/.well-known/openid-configuration`).
    discovery_url: String,
    /// Scopes requested at the authorization endpoint.
    scopes: Vec<String>,
}

impl OidcConfig {
    /// Build a tenant with the required fields. Scopes default to
    /// `["openid", "profile"]`; pass `scopes: Some(&[])` to keep only
    /// `openid`.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        provider_name: impl Into<String>,
        issuer: impl Into<String>,
        client_id: impl Into<String>,
        discovery_url: impl Into<String>,
        scopes: Option<&[&str]>,
    ) -> Self {
        OidcConfig {
            provider_name: provider_name.into(),
            issuer: issuer.into(),
            client_id: client_id.into(),
            discovery_url: discovery_url.into(),
            scopes: scopes
                .map(|s| s.iter().map(|x| x.to_string()).collect())
                .unwrap_or_else(|| vec!["openid".to_string(), "profile".to_string()]),
        }
    }

    /// Provider name.
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// Issuer URL.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Client id.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// OIDC discovery URL.
    pub fn discovery_url(&self) -> &str {
        &self.discovery_url
    }

    /// Requested scopes.
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
}

/// Identity configuration for the PDM service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityConfig {
    local_accounts_enabled: bool,
    oidc: Option<OidcConfig>,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        // Sovereignty default: local accounts on, OIDC off.
        IdentityConfig {
            local_accounts_enabled: true,
            oidc: None,
        }
    }
}

impl IdentityConfig {
    /// Local accounts are on by default and can be turned off only for
    /// federated-only deployments (rare, explicit).
    pub fn local_accounts_enabled(&self) -> bool {
        self.local_accounts_enabled
    }

    /// The configured OIDC tenant, if any.
    pub fn oidc(&self) -> Option<&OidcConfig> {
        self.oidc.as_ref()
    }

    /// True when an OIDC tenant is configured (i.e. federated sign-in is
    /// available as an opt-in).
    pub fn oidc_enabled(&self) -> bool {
        self.oidc.is_some()
    }

    /// Explicitly opt in to an OIDC tenant. This is the ONLY way OIDC turns
    /// on; the default never has one.
    pub fn set_oidc(&mut self, tenant: impl Into<Option<OidcConfig>>) {
        self.oidc = tenant.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_sovereign() {
        let cfg = IdentityConfig::default();
        assert!(cfg.local_accounts_enabled());
        assert!(cfg.oidc().is_none());
        assert!(!cfg.oidc_enabled());
    }
}
