//! The auth-boundary policy: strict and permissive authorizers (P11-T03).
//!
//! The strict boundary is the P8-M3-T02 [`RoleMatrix::standard`] plus one
//! ABAC deny-wins rule that forbids cross-tenant access. The permissive
//! authorizer is the default-permissive stub the card must *not* be green
//! against (see `tests/auth_boundary.rs`, `strict_mode_denies_what_
//! permissive_allows`): giving every role every action and dropping the
//! tenant rule is exactly the kind of "open by default" boundary this suite
//! exists to catch.

use crate::rbac::{
    Action, AttributeValue, Authorizer, CmpOp, Condition, Effect, Role, RoleMatrix, Rule,
};

/// The ABAC attribute key for the tenant on a subject (principal) and on a
/// resource (vault object).
pub const TENANT_ATTR: &str = "tenant";

/// The environment attribute the gate sets to `true` when the request
/// crosses a tenant boundary (computed from subject vs resource attributes,
/// which the P8-M3 engine cannot compare directly).
pub const CROSS_TENANT_ENV: &str = "cross_tenant";

/// Name of the deny-wins cross-tenant rule (shown in deny reasons).
pub const RULE_CROSS_TENANT: &str = "no-cross-tenant";

/// The rule closure the strict boundary enforces: any request whose
/// `cross_tenant` environment attribute is `true` is denied, deny-wins,
/// before any role matrix or Allow rule is consulted.
///
/// It applies to every action (`None` scope), so no role, not even Admin,
/// may cross a tenant boundary through this gate. This is decision D1 in
/// `workstreams/security/auth-boundary.md`: tenant isolation is absolute here; a scoped
/// cross-tenant grant for the support role would be added as an explicit
/// Allow rule once the identity layer lands.
pub fn strict_boundary_rules() -> Vec<Rule> {
    vec![Rule::new(
        RULE_CROSS_TENANT,
        Effect::Deny,
        None,
        vec![Condition::environment(
            CROSS_TENANT_ENV,
            CmpOp::Eq,
            AttributeValue::bool_val(true),
        )],
    )]
}

/// The strict boundary authorizer: standard staff matrix plus the
/// no-cross-tenant deny-wins rule. This is "the current RBAC implementation"
/// before a principal is admitted.
pub fn strict_boundary_authorizer() -> Authorizer {
    Authorizer::new(RoleMatrix::standard(), strict_boundary_rules())
}

/// The default-permissive stub: every role holds every action and there are
/// no rules. Used ONLY by the mutation-style proof to show the strict suite
/// genuinely asserts (the same requests the strict boundary denies must not
/// be denied here). Never wired into a serving path.
pub fn permissive_authorizer() -> Authorizer {
    let grants: Vec<(Role, Action)> = Role::ALL
        .iter()
        .flat_map(|r| Action::ALL.iter().map(move |a| (*r, *a)))
        .collect();
    Authorizer::new(RoleMatrix::from_grants(grants), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::{Decision, DenyReason, RoleSubject};

    fn tenant_subject(name: &str, roles: Vec<Role>, tenant: &str) -> RoleSubject {
        let mut s = RoleSubject::new(name, roles);
        s.attr(TENANT_ATTR, AttributeValue::str_val(tenant));
        s
    }

    fn cross_env(cross: bool) -> Vec<(String, AttributeValue)> {
        vec![(
            CROSS_TENANT_ENV.to_string(),
            AttributeValue::bool_val(cross),
        )]
    }

    #[test]
    fn strict_boundary_denies_when_cross_tenant_env_is_true() {
        let a = strict_boundary_authorizer();
        let admin = tenant_subject("root", vec![Role::Admin], "Tenant-A");
        // Even Admin is denied the moment the gate marks the request cross-tenant.
        assert!(matches!(
            a.decide(&admin, &[], &Action::Read, &cross_env(true)),
            Decision::Deny(DenyReason::AbacRule(_))
        ));
        // Same-tenant (cross_tenant=false) proceeds to the matrix.
        assert!(a
            .decide(&admin, &[], &Action::Read, &cross_env(false))
            .is_allowed());
    }

    #[test]
    fn permissive_authorizer_grants_every_union_to_every_role() {
        let a = permissive_authorizer();
        let actor = tenant_subject("someone", vec![Role::Reviewer], "Tenant-A");
        for action in Action::ALL {
            assert!(
                a.decide(&actor, &[], &action, &cross_env(true))
                    .is_allowed(),
                "permissive stub must not deny {action:?} even cross-tenant"
            );
        }
    }

    #[test]
    fn standard_matrix_still_denies_without_rule_engine() {
        // The matrix alone keeps role discipline: a Designer is not a
        // ReleaseAuthority regardless of rules.
        let a = Authorizer::new(RoleMatrix::standard(), strict_boundary_rules());
        let d = tenant_subject("d", vec![Role::Designer], "Tenant-A");
        assert!(matches!(
            a.decide(&d, &[], &Action::Release, &cross_env(false)),
            Decision::Deny(_)
        ));
    }
}
