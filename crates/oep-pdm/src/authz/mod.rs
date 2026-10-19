//! P11-T03: the auth boundary -- RBAC enforcement over the REST v1 contract.
//!
//! This module is the enforcement layer the threat model's M-RBAC closure
//! (P11/auth-boundary.md) stands on. It sits in front of the real contract
//! handlers and answers one question per request: may this principal perform
//! the action this endpoint requires, on this vault object, in this tenant?
//!
//! The decision core is the P8-M3-T02 RBAC implementation
//! ([`crate::rbac`]): a static role matrix plus an attribute-based (ABAC)
//! rule layer, evaluated by the default-deny [`crate::rbac::Authorizer`].
//! This module contributes the REST wiring the RBAC design note always
//! anticipated (P8-M3/p8m3t02-rbac.md): endpoint to action mapping
//! ([`endpoints`]), header principal resolution ([`principal`]), tenant
//! scoping of vault objects ([`resource`]), the strict boundary policy
//! ([`policy`]) and the axum gate that turns decisions into HTTP statuses
//! ([`gate`]).
//!
//! ## Denial contract
//!
//! * Unauthenticated (no principal) or cross-role attempts: `403 Forbidden`.
//! * Cross-tenant access to a vault object: `404 Not Found`, so an object in
//!   another tenant is indistinguishable from one that does not exist (no
//!   IDOR existence oracle). This is the "cross-role 403, cross-tenant 404"
//!   split the card's acceptance names as "403/404".
//! * Endpoints outside the mapped contract surface: `404` (no oracle either).
//!
//! ## Opt-in wiring
//!
//! The boundary is intentionally opt-in. `PdmService::start` / the shipped
//! binaries keep today's open behavior; the security suite and the future
//! identity-landing path use `PdmService::start_with_authorizer`, which layers
//! the same handlers behind this gate. When the P8-M3-T01 identity layer
//! lands, the header-carried principal here is replaced by a verified
//! passport; the decision path stays exactly the same.

pub mod endpoints;
pub mod gate;
pub mod policy;
pub mod principal;
pub mod resource;

pub use endpoints::{classify, Classified, Target};
pub use gate::{authz_middleware, decide, AuthOutcome, AuthState, GATE_BODY_CAP};
pub use policy::{
    permissive_authorizer, strict_boundary_authorizer, strict_boundary_rules, CROSS_TENANT_ENV,
    RULE_CROSS_TENANT, TENANT_ATTR,
};
pub use principal::{Principal, PrincipalError};
pub use resource::item_tenant;
