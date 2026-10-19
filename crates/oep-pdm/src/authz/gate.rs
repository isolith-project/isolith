//! The axum gate: turns an RBAC decision into an HTTP response (P11-T03).
//!
//! `authz_middleware` runs in front of the contract handlers on the
//! authorized service (`PdmService::start_with_authorizer`, see
//! [`crate::service`]). Per request it:
//!
//! 1. classifies the endpoint ([`super::endpoints::classify`]),
//! 2. resolves the principal from identity headers
//!    ([`super::principal::Principal::from_headers`]),
//! 3. calls [`decide`] to ask the P8-M3 authorizer whether the action may
//!    proceed on the (tenant-scoped) object,
//! 4. maps the outcome:
//!    * `Allow`        -> the handler runs unchanged,
//!    * `Forbidden`    -> `403` (unauthenticated or cross-role),
//!    * `NotFoundHidden` -> `404` (cross-tenant object, IDOR-masked; an
//!      object in another tenant is indistinguishable from a missing one).
//!
//! Only the two routes whose decision needs the request body (lifecycle
//! transition, sync push) have their body buffered (bounded by
//! [`GATE_BODY_CAP`]); every other request passes its body through untouched.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::{error_code, ErrorEnvelope};
use crate::rbac::{AttributeValue, Authorizer, Decision, DenyReason, RoleSubject};
use crate::service::Store;
use crate::store::{StoreError, VaultStore};

use super::endpoints::{self, Classified, Target};
use super::policy::{CROSS_TENANT_ENV, RULE_CROSS_TENANT, TENANT_ATTR};
use super::principal::{Principal, PrincipalError};
use super::resource::item_tenant;

/// Cap for request bodies the gate buffers for a decision (transition /
/// sync-push targets). Real CAD uploads do not route through this branch.
pub const GATE_BODY_CAP: usize = 4 * 1024 * 1024;

/// State the gate needs: the same store the handlers serve plus the
/// authorizer that answers every decision.
#[derive(Clone)]
pub struct AuthState {
    /// The P8-M3 RBAC authorizer backing this boundary.
    pub authorizer: Arc<Authorizer>,
    /// The vault store (shared with the handlers below the gate).
    pub store: Store,
}

/// The gate's verdict for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    /// Permit: forward to the handler.
    Allow,
    /// Deny visibly: the caller is not permitted (`403 Forbidden`).
    Forbidden,
    /// Deny silently: the object is not visible to the caller and must not
    /// be revealed as existing (`404 Not Found`, IDOR masking).
    NotFoundHidden,
}

/// The pure decision: may `subject` perform `target.action` on `target`'s
/// object? Shared by the middleware and the unit/integration suites.
///
/// * Object-level targets: the item's tenant is resolved; when both the
///   subject and the object carry a tenant and they differ, the request is
///   marked cross-tenant and the strict policy denies it (masked `404`).
///   A genuinely absent object returns `Allow` so the handler's own `404`
///   (same observable code as the mask) answers.
/// * A store error fails closed (`Forbidden`).
pub fn decide(
    authorizer: &Authorizer,
    subject: &RoleSubject,
    store: &VaultStore,
    target: &Target,
) -> AuthOutcome {
    let mut resource_attrs: Vec<(String, AttributeValue)> = Vec::new();
    let mut cross_tenant = false;

    if let Some(id) = &target.item_id {
        match store.get_item(id) {
            Ok(item) => {
                if let Some(tenant) = item_tenant(&item) {
                    resource_attrs
                        .push((TENANT_ATTR.to_string(), AttributeValue::str_val(&tenant)));
                    if let Some(AttributeValue::Str(subject_tenant)) =
                        subject.attrs().get(TENANT_ATTR)
                    {
                        cross_tenant = !subject_tenant.is_empty() && subject_tenant != &tenant;
                    }
                }
            }
            // Absent object: let the handler's own 404 answer (no oracle).
            Err(StoreError::NotFound(..)) => return AuthOutcome::Allow,
            // Store unusable: fail closed rather than admit.
            Err(_) => return AuthOutcome::Forbidden,
        }
    }

    let env_attrs: Vec<(String, AttributeValue)> = vec![(
        CROSS_TENANT_ENV.to_string(),
        AttributeValue::bool_val(cross_tenant),
    )];

    match authorizer.decide(subject, &resource_attrs, &target.action, &env_attrs) {
        Decision::Allow(_) => AuthOutcome::Allow,
        // The strict policy's cross-tenant deny-wins rule: the object exists
        // but belongs to another tenant, so mask it as missing.
        Decision::Deny(DenyReason::AbacRule(name)) if name == RULE_CROSS_TENANT && cross_tenant => {
            AuthOutcome::NotFoundHidden
        }
        // Every other deny (role matrix, no role, other rule) is a visible
        // 403: the request itself is not permitted.
        Decision::Deny(_) => AuthOutcome::Forbidden,
    }
}

/// The gate middleware (see module docs for the flow).
pub async fn authz_middleware(
    State(state): State<Arc<AuthState>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_ascii_uppercase();
    let path = req.uri().path().to_string();
    let need_body = endpoints::needs_body(&method, &path);

    let (parts, body) = req.into_parts();
    let (req, body_bytes) = if need_body {
        match to_bytes(body, GATE_BODY_CAP).await {
            Ok(bytes) => (
                Request::from_parts(parts.clone(), Body::from(bytes.clone())),
                Some(bytes),
            ),
            Err(_) => {
                return json_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    error_code::BAD_REQUEST,
                    "request body too large for authorization decision",
                );
            }
        }
    } else {
        (Request::from_parts(parts.clone(), body), None)
    };

    match endpoints::classify(&method, &path, body_bytes.as_deref()) {
        Classified::Open => next.run(req).await,
        Classified::Unknown => json_response(
            StatusCode::NOT_FOUND,
            error_code::NOT_FOUND,
            "endpoint not found",
        ),
        Classified::Protected(target) => {
            let principal = match Principal::from_headers(&parts.headers) {
                Ok(p) => p,
                Err(PrincipalError::Missing) => {
                    return json_response(
                        StatusCode::FORBIDDEN,
                        "forbidden",
                        "authentication required",
                    );
                }
                Err(PrincipalError::BadRole(role)) => {
                    return json_response(
                        StatusCode::FORBIDDEN,
                        "forbidden",
                        format!("unknown role {role:?} in X-Oep-Roles"),
                    );
                }
            };
            let subject = principal.to_subject();
            let outcome = match state.store.lock() {
                Ok(guard) => decide(&state.authorizer, &subject, &guard, &target),
                Err(_) => {
                    return json_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        error_code::INTERNAL,
                        "store lock poisoned",
                    );
                }
            };
            match outcome {
                AuthOutcome::Allow => next.run(req).await,
                AuthOutcome::Forbidden => json_response(
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    "caller is not permitted to perform this action",
                ),
                AuthOutcome::NotFoundHidden => json_response(
                    StatusCode::NOT_FOUND,
                    error_code::NOT_FOUND,
                    "object not found",
                ),
            }
        }
    }
}

/// A contract error-envelope response (`{"error":{"code","message"}}`).
fn json_response(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    let envelope = ErrorEnvelope::new(code, message);
    let body = serde_json::to_vec(&envelope).unwrap_or_else(|_| {
        b"{\"error\":{\"code\":\"internal_error\",\"message\":\"serialize\"}}".to_vec()
    });
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::RoleSubject;
    use crate::rbac::{AttributeValue, Role};
    use crate::store::VaultStore;

    fn subject(name: &str, role: Role, tenant: &str) -> RoleSubject {
        let mut s = RoleSubject::new(name, vec![role]);
        s.attr(TENANT_ATTR, AttributeValue::str_val(tenant));
        s
    }

    fn target_owned(store: &mut VaultStore, owner: &str) -> Target {
        use crate::api::CreateItemRequest;
        let item = store
            .create_item(&CreateItemRequest {
                name: format!("owned-{owner}"),
                kind: "part".to_string(),
                description: None,
                owner: Some(owner.to_string()),
                context: None,
            })
            .expect("seed item");
        Target {
            action: crate::rbac::Action::Read,
            item_id: Some(item.id),
        }
    }

    #[test]
    fn absent_object_defers_to_handler() {
        let store = VaultStore::new();
        let a = super::super::policy::strict_boundary_authorizer();
        let d = subject("d", Role::Designer, "Tenant-A");
        let t = Target {
            action: crate::rbac::Action::Read,
            item_id: Some("ghost".into()),
        };
        assert_eq!(decide(&a, &d, &store, &t), AuthOutcome::Allow);
    }

    #[test]
    fn cross_tenant_read_is_masked_not_found() {
        let mut store = VaultStore::new();
        let a = super::super::policy::strict_boundary_authorizer();
        let t = target_owned(&mut store, "Tenant-A");
        let attacker = subject("dan", Role::Designer, "Tenant-B");
        assert_eq!(
            decide(&a, &attacker, &store, &t),
            AuthOutcome::NotFoundHidden
        );
    }

    #[test]
    fn same_tenant_read_is_allowed_for_designer() {
        let mut store = VaultStore::new();
        let a = super::super::policy::strict_boundary_authorizer();
        let t = target_owned(&mut store, "Tenant-A");
        let alice = subject("alice", Role::Designer, "Tenant-A");
        assert_eq!(decide(&a, &alice, &store, &t), AuthOutcome::Allow);
    }
}
