//! Principal resolution for the auth gate (P11-T03).
//!
//! Turns the identity headers the request presents into a [`RoleSubject`]
//! for the authorizer. Today the headers are client-declared; the P8-M3-T01
//! identity layer will replace this by verifying a passport and stamping the
//! same principal context before the gate runs. The decision path below is
//! what the suite pins and what identity-landing must keep intact.
//!
//! ### Headers
//!
//! * `X-Oep-User`   - principal name (required; missing = unauthenticated).
//! * `X-Oep-Roles`  - comma-separated RBAC role names (see [`Role::from_name`]).
//! * `X-Oep-Tenant` - owning tenant for the principal (subject attribute).
//!
//! An unknown role name is a hard failure (`BadRole`): per the P8-M3 RBAC
//! contract a caller that cannot be parsed into a known role is denied, never
//! granted.

use axum::http::HeaderMap;

use crate::rbac::{AttributeValue, Role, RoleSubject};

use super::policy::TENANT_ATTR;

/// Principal identity header.
pub const HEADER_USER: &str = "x-oep-user";
/// Role list header.
pub const HEADER_ROLES: &str = "x-oep-roles";
/// Tenant header.
pub const HEADER_TENANT: &str = "x-oep-tenant";

/// A resolved principal: roles held plus optional owning tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Principal name (arbitrary identifier for now).
    pub name: String,
    /// Roles resolved from the role header.
    pub roles: Vec<Role>,
    /// Owning tenant, when declared.
    pub tenant: Option<String>,
}

/// Why a principal could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrincipalError {
    /// No `X-Oep-User` (or empty): the request carries no identity.
    Missing,
    /// A declared role name is not a known RBAC role.
    BadRole(String),
}

impl std::fmt::Display for PrincipalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalError::Missing => write!(f, "no principal presented (missing X-Oep-User)"),
            PrincipalError::BadRole(r) => write!(f, "unknown role {r:?} in X-Oep-Roles"),
        }
    }
}

impl Principal {
    /// Resolve a principal from the request headers.
    pub fn from_headers(headers: &HeaderMap) -> Result<Principal, PrincipalError> {
        let name = headers
            .get(HEADER_USER)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let name = match name {
            Some(n) => n,
            None => return Err(PrincipalError::Missing),
        };

        let tenant = headers
            .get(HEADER_TENANT)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let mut roles = Vec::new();
        if let Some(raw) = headers.get(HEADER_ROLES).and_then(|v| v.to_str().ok()) {
            for part in raw.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                match Role::from_name(part) {
                    Some(role) => roles.push(role),
                    None => return Err(PrincipalError::BadRole(part.to_string())),
                }
            }
        }

        Ok(Principal {
            name,
            roles,
            tenant,
        })
    }

    /// Build the RBAC subject carrying this principal's tenant attribute.
    pub fn to_subject(&self) -> RoleSubject {
        let mut subject = RoleSubject::new(&self.name, self.roles.clone());
        if let Some(t) = &self.tenant {
            subject.attr(TENANT_ATTR, AttributeValue::str_val(t));
        }
        subject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::HeaderName;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        map
    }

    #[test]
    fn missing_user_is_unauthenticated() {
        assert_eq!(
            Principal::from_headers(&headers(&[])),
            Err(PrincipalError::Missing)
        );
        assert_eq!(
            Principal::from_headers(&headers(&[("x-oep-roles", "designer")])),
            Err(PrincipalError::Missing)
        );
    }

    #[test]
    fn parses_roles_and_tenant() {
        let p = Principal::from_headers(&headers(&[
            ("x-oep-user", "alice"),
            ("x-oep-roles", "designer, reviewer"),
            ("x-oep-tenant", "Tenant-A"),
        ]))
        .unwrap();
        assert_eq!(p.name, "alice");
        assert_eq!(p.roles, vec![Role::Designer, Role::Reviewer]);
        assert_eq!(p.tenant.as_deref(), Some("Tenant-A"));
        assert_eq!(p.to_subject().roles(), &[Role::Designer, Role::Reviewer]);
    }

    #[test]
    fn unknown_role_is_rejected() {
        assert_eq!(
            Principal::from_headers(&headers(&[
                ("x-oep-user", "mallory"),
                ("x-oep-roles", "designer, superuser"),
            ])),
            Err(PrincipalError::BadRole("superuser".to_string()))
        );
    }

    #[test]
    fn empty_roles_yield_no_grants() {
        let p = Principal::from_headers(&headers(&[("x-oep-user", "nobody")])).unwrap();
        assert!(p.roles.is_empty());
        assert!(p.to_subject().roles().is_empty());
    }
}
