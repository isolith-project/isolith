//! The authorizer: combines the role matrix and ABAC rules into decisions
//! (P8-M3-T02).

use std::collections::BTreeMap;

use super::{Action, AttributeValue, Condition, Effect, Role, RoleMatrix, Rule, Source};

/// A principal whose authorization is being evaluated.
///
/// Carries the identity name (from the P8-M3-T01 identity layer once wired),
/// the roles resolved for this session, and subject attributes used by ABAC
/// conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleSubject {
    name: String,
    roles: Vec<Role>,
    attrs: BTreeMap<String, AttributeValue>,
}

impl RoleSubject {
    /// New subject with the given roles and no attributes.
    pub fn new(name: &str, roles: Vec<Role>) -> Self {
        RoleSubject {
            name: name.to_string(),
            roles,
            attrs: BTreeMap::new(),
        }
    }

    /// Set a subject attribute, replacing any existing value.
    pub fn attr(&mut self, key: &str, value: AttributeValue) -> &mut Self {
        self.attrs.insert(key.to_string(), value);
        self
    }

    /// Principal name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Roles.
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// Subject attributes.
    pub fn attrs(&self) -> &BTreeMap<String, AttributeValue> {
        &self.attrs
    }
}

/// Why a request was allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowReason {
    /// Granted by a role held by the subject.
    Role(Role),
    /// Granted by a matching ABAC Allow rule.
    AbacRule(String),
}

/// Why a request was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// Subject holds no roles and no Allow rule matched.
    NoRole,
    /// Denied by a matching ABAC Deny rule (deny-wins).
    AbacRule(String),
    /// No role grant and no explicit Allow rule matched: default-deny.
    NotPermitted,
}

/// The result of an authorization check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Allowed, with the reason.
    Allow(AllowReason),
    /// Denied, with the reason.
    Deny(DenyReason),
}

impl Decision {
    /// True when allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow(_))
    }
}

/// Evaluates the role matrix + ABAC rule policy for a request.
#[derive(Debug, Clone)]
pub struct Authorizer {
    matrix: RoleMatrix,
    rules: Vec<Rule>,
}

impl Authorizer {
    /// New authorizer over a matrix and a rule policy.
    pub fn new(matrix: RoleMatrix, rules: Vec<Rule>) -> Self {
        Authorizer { matrix, rules }
    }

    /// The role matrix in use.
    pub fn matrix(&self) -> &RoleMatrix {
        &self.matrix
    }

    /// The rule policy in use.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Decide whether `subject` may perform `action` on `resource`.
    ///
    /// * `resource_attrs` — attributes of the item/revision being acted on
    ///   (may be empty).
    /// * `env_attrs` — environment / request context attributes (may be
    ///   empty).
    ///
    /// Evaluation order (documented in `P8-M3/p8m3t02-rbac.md`):
    ///
    /// 1. Collect the rules that match this action + subject/resource/env
    ///    attributes.
    /// 2. Any matching [`Effect::Deny`] -> `Deny(AbacRule)` (deny-wins).
    /// 3. Subject has no roles -> `Deny(NoRole)` (an Allow rule could still
    ///    grant, but the no-role + no-match case is the degenerate one).
    /// 4. Role matrix grants the action -> `Allow(Role)`.
    /// 5. Any matching [`Effect::Allow`] -> `Allow(AbacRule)`.
    /// 6. Otherwise -> `Deny(NotPermitted)` (default-deny).
    pub fn decide(
        &self,
        subject: &RoleSubject,
        resource_attrs: &[(String, AttributeValue)],
        action: &Action,
        env_attrs: &[(String, AttributeValue)],
    ) -> Decision {
        let res: BTreeMap<String, AttributeValue> = resource_attrs.iter().cloned().collect();
        let env: BTreeMap<String, AttributeValue> = env_attrs.iter().cloned().collect();

        let matched: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| {
                r.applies_to(*action)
                    && r.conditions()
                        .iter()
                        .all(|c| self.condition_holds(c, subject, &res, &env))
            })
            .collect();

        // Deny-wins.
        if let Some(r) = matched.iter().find(|r| r.effect() == Effect::Deny) {
            return Decision::Deny(DenyReason::AbacRule(r.name().to_string()));
        }

        // A subject that holds no roles at all is denied by default.
        let has_any_role = subject
            .roles()
            .iter()
            .any(|r| self.matrix.allows(*r, *action));
        if subject.roles().is_empty() && matched.is_empty() {
            return Decision::Deny(DenyReason::NoRole);
        }

        if has_any_role {
            return Decision::Allow(AllowReason::Role(
                subject
                    .roles()
                    .iter()
                    .copied()
                    .find(|r| self.matrix.allows(*r, *action))
                    .unwrap(),
            ));
        }

        if let Some(r) = matched.iter().find(|r| r.effect() == Effect::Allow) {
            return Decision::Allow(AllowReason::AbacRule(r.name().to_string()));
        }

        Decision::Deny(DenyReason::NotPermitted)
    }

    fn condition_holds(
        &self,
        c: &Condition,
        subject: &RoleSubject,
        res: &BTreeMap<String, AttributeValue>,
        env: &BTreeMap<String, AttributeValue>,
    ) -> bool {
        let attrs = match c.source() {
            Source::Subject => &subject.attrs,
            Source::Resource => res,
            Source::Environment => env,
        };
        c.matches_attrs(attrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::CmpOp;

    #[test]
    fn role_grant_is_allowed() {
        let a = Authorizer::new(RoleMatrix::standard(), vec![]);
        let s = RoleSubject::new("d", vec![Role::Designer]);
        assert_eq!(
            a.decide(&s, &[], &Action::EditRevision, &[]),
            Decision::Allow(AllowReason::Role(Role::Designer))
        );
    }

    #[test]
    fn matrix_deny_with_no_rules_is_denied() {
        let a = Authorizer::new(RoleMatrix::standard(), vec![]);
        let s = RoleSubject::new("d", vec![Role::Designer]);
        assert!(matches!(
            a.decide(&s, &[], &Action::Release, &[]),
            Decision::Deny(DenyReason::NotPermitted)
        ));
    }

    #[test]
    fn role_allow_beats_nothing_but_deny_wins_over_it() {
        let deny = Rule::new(
            "no-release-frozen",
            Effect::Deny,
            Some(vec![Action::Release]),
            vec![Condition::environment(
                "freeze",
                CmpOp::Eq,
                AttributeValue::bool_val(true),
            )],
        );
        let a = Authorizer::new(RoleMatrix::standard(), vec![deny]);
        let s = RoleSubject::new("ra", vec![Role::ReleaseAuthority]);
        assert!(matches!(
            a.decide(
                &s,
                &[],
                &Action::Release,
                &[("freeze".into(), AttributeValue::bool_val(true))]
            ),
            Decision::Deny(DenyReason::AbacRule(_))
        ));
        assert!(matches!(
            a.decide(
                &s,
                &[],
                &Action::Release,
                &[("freeze".into(), AttributeValue::bool_val(false))]
            ),
            Decision::Allow(AllowReason::Role(Role::ReleaseAuthority))
        ));
    }
}
