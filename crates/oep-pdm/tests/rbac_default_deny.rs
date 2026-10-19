//! P8-M3-T02 acceptance contract: default-deny, end to end.
//!
//! default-deny is the invariant: an authorization never grants access unless
//! the role matrix OR an explicit ABAC Allow rule says so, and a Deny rule
//! always wins. This test drives the full Authorizer (matrix + rules) with
//! hostile inputs: no roles, empty rules, unknown actions/attributes, and
//! cross-checks every denied matrix cell from the role-matrix test through
//! the authorizer (not just the matrix primitive).

use oep_pdm::rbac::{
    Action, AttributeValue, Authorizer, CmpOp, Condition, Decision, Effect, Role, RoleMatrix,
    RoleSubject, Rule,
};

#[test]
fn subject_with_no_roles_is_denied_everything() {
    let a = Authorizer::new(RoleMatrix::standard(), vec![]);
    let nobody = RoleSubject::new("ghost", vec![]);
    let resource: Vec<(String, AttributeValue)> = vec![];
    for action in [
        Action::Read,
        Action::CreateItem,
        Action::EditRevision,
        Action::SubmitForReview,
        Action::Review,
        Action::Release,
        Action::MakeObsolete,
        Action::Delete,
        Action::ManageUsers,
        Action::ManageRoles,
        Action::ManageSystem,
    ] {
        assert!(
            matches!(
                a.decide(&nobody, &resource, &action, &[]),
                Decision::Deny(_)
            ),
            "no-role subject must be denied {action:?}"
        );
    }
}

#[test]
fn every_role_is_denied_an_action_not_on_its_row() {
    let a = Authorizer::new(RoleMatrix::standard(), vec![]);
    let resource: Vec<(String, AttributeValue)> = vec![];
    let cases = [
        (Role::Designer, Action::Review),
        (Role::Designer, Action::Release),
        (Role::Designer, Action::ManageUsers),
        (Role::Reviewer, Action::EditRevision),
        (Role::Reviewer, Action::Release),
        (Role::ReleaseAuthority, Action::EditRevision),
        (Role::ReleaseAuthority, Action::Delete),
    ];
    for (role, action) in cases {
        let s = RoleSubject::new("user", vec![role]);
        assert!(
            matches!(a.decide(&s, &resource, &action, &[]), Decision::Deny(_)),
            "{role:?} must be denied {action:?}"
        );
    }
}

#[test]
fn empty_policy_rules_never_deny_a_matrix_grant_but_never_grant_a_denial() {
    let a = Authorizer::new(RoleMatrix::standard(), vec![]);
    let resource: Vec<(String, AttributeValue)> = vec![];
    let d = RoleSubject::new("designer", vec![Role::Designer]);
    assert!(matches!(
        a.decide(&d, &resource, &Action::EditRevision, &[]),
        Decision::Allow(_)
    ));
    let r = RoleSubject::new("reviewer", vec![Role::Reviewer]);
    assert!(matches!(
        a.decide(&r, &resource, &Action::Review, &[]),
        Decision::Allow(_)
    ));
}

#[test]
fn unknown_or_unset_attributes_are_handled_as_false_not_granted() {
    // An Allow rule whose conditions can never be satisfied must not grant.
    use std::collections::BTreeSet;
    let m = RoleMatrix::standard();
    let mut audience = BTreeSet::new();
    audience.insert("nobody".to_string()); // subject's set doesn't contain 'admins'
    let mut s = RoleSubject::new("x", vec![Role::Reviewer]);
    let audience_v = AttributeValue::Set(audience.clone());
    s.attr("audience", audience_v.clone());

    let policy = vec![Rule::new(
        "impossible-allow",
        Effect::Allow,
        Some(vec![Action::ManageSystem]),
        vec![Condition::subject(
            "audience",
            CmpOp::In,
            AttributeValue::Str("admins".to_string()),
        )],
    )];
    let a = Authorizer::new(m, policy);
    let resource: Vec<(String, AttributeValue)> = vec![];
    // Reviewer + impossible allow -> still denied (not granted).
    assert!(matches!(
        a.decide(&s, &resource, &Action::ManageSystem, &[]),
        Decision::Deny(_)
    ));
}

#[test]
fn deny_rule_beats_allow_rule_for_same_principal() {
    // Same principal triggers both an Allow and a Deny for the same action:
    // deny-wins must hold at the authorizer level.
    let m = RoleMatrix::standard();
    let allow = Rule::new(
        "try-grant",
        Effect::Allow,
        Some(vec![Action::Delete]),
        vec![Condition::subject(
            "safety_off",
            CmpOp::Eq,
            AttributeValue::Bool(true),
        )],
    );
    let deny = Rule::new(
        "safety-always-on",
        Effect::Deny,
        Some(vec![Action::Delete]),
        vec![Condition::subject(
            "safety_off",
            CmpOp::Eq,
            AttributeValue::Bool(true),
        )],
    );
    let a = Authorizer::new(m, vec![allow, deny]);
    let mut s = RoleSubject::new("root", vec![Role::Admin]);
    s.attr("safety_off", AttributeValue::Bool(true));
    let resource: Vec<(String, AttributeValue)> = vec![];
    assert!(matches!(
        a.decide(&s, &resource, &Action::Delete, &[]),
        Decision::Deny(_)
    ));
}

#[test]
fn authorizer_is_consistent_with_the_role_matrix_test() {
    // The exhaustive role-matrix (tests/rbac_role_matrix.rs) and the
    // end-to-end authorizer must agree on every cell.
    let a = Authorizer::new(RoleMatrix::standard(), vec![]);
    let resource: Vec<(String, AttributeValue)> = vec![];
    let all_actions = [
        Action::Read,
        Action::CreateItem,
        Action::EditRevision,
        Action::SubmitForReview,
        Action::Review,
        Action::Release,
        Action::MakeObsolete,
        Action::Delete,
        Action::ManageUsers,
        Action::ManageRoles,
        Action::ManageSystem,
    ];
    let all_roles = [
        Role::Designer,
        Role::Reviewer,
        Role::ReleaseAuthority,
        Role::Admin,
    ];
    for role in all_roles {
        for action in all_actions {
            let s = RoleSubject::new("u", vec![role]);
            let decision = a.decide(&s, &resource, &action, &[]);
            match decision {
                Decision::Allow(_) => {
                    assert!(
                        RoleMatrix::standard().allows(role, action),
                        "authorizer allowed {role:?} {action:?} but matrix denies it"
                    );
                }
                Decision::Deny(_) => {
                    assert!(
                        !RoleMatrix::standard().allows(role, action),
                        "authorizer denied {role:?} {action:?} but matrix grants it (no rules)"
                    );
                }
            }
        }
    }
}
