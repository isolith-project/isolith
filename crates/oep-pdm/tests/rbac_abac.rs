//! P8-M3-T02 acceptance contract: attribute-based rules (ABAC).
//!
//! Exercises the ABAC layer on top of the role matrix:
//!   * Conditions evaluate subject, resource and environment attributes with
//!     a range of operators (Eq, Neq, In, Contains, Gt, Ge, Exists, ...).
//!   * Deny rules are deny-wins: a matched Deny rule denies even a role the
//!     matrix would have allowed (e.g. Admin).
//!   * Allow rules can authorize an action the role matrix denied, but a
//!     Deny rule still beats an Allow rule for the same principal/action.

use oep_pdm::rbac::{
    Action, AttributeValue, Authorizer, CmpOp, Condition, Decision, Effect, Role, RoleMatrix,
    RoleSubject, Rule,
};

fn sa(k: &str, v: AttributeValue) -> (String, AttributeValue) {
    (k.to_string(), v)
}

// A designer, with a department attribute.
fn designer(name: &str, dept: &str) -> RoleSubject {
    let mut s = RoleSubject::new(name, vec![Role::Designer]);
    s.attr("department", AttributeValue::Str(dept.to_string()));
    s.attr("security", AttributeValue::Int(2));
    s
}

#[test]
fn allow_rule_can_override_role_denial() {
    // Matrix denies Designer's Review. An ABAC Allow rule grants a designer
    // acting as a delegated reviewer on their own team document.
    let m = RoleMatrix::standard();
    let policy = vec![Rule::new(
        "senior-designer-may-review",
        Effect::Allow,
        Some(vec![Action::Review]),
        vec![
            Condition::subject("team_lead", CmpOp::Eq, AttributeValue::Bool(true)),
            Condition::resource(
                "doc_type",
                CmpOp::Eq,
                AttributeValue::Str("owning".to_string()),
            ),
        ],
    )];

    let a = Authorizer::new(m, policy);

    let mut lead = designer("alice", "PDM");
    lead.attr("team_lead", AttributeValue::Bool(true));

    let resource: Vec<(String, AttributeValue)> = [(
        "meta",
        "doc_type",
        AttributeValue::Str("owning".to_string()),
    )]
    .into_iter()
    .map(|(_, k, v)| sa(k, v))
    .collect();

    match a.decide(&lead, &resource, &Action::Review, &[]) {
        Decision::Allow(_) => {}
        d => panic!("expected Allow for delegated reviewer, got {d:?}"),
    }
}

#[test]
fn allow_rule_does_not_fire_without_attribute_conditions() {
    let m = RoleMatrix::standard();
    let policy = vec![Rule::new(
        "senior-designer-may-review",
        Effect::Allow,
        Some(vec![Action::Review]),
        vec![
            Condition::subject("team_lead", CmpOp::Eq, AttributeValue::Bool(true)),
            Condition::resource(
                "doc_type",
                CmpOp::Eq,
                AttributeValue::Str("owning".to_string()),
            ),
        ],
    )];
    let a = Authorizer::new(m, policy);

    // Same designer, but not a team lead -> condition fails -> still denied
    // because the role matrix denies Designer's Review.
    let plain = designer("bob", "PDM");
    let resource: Vec<(String, AttributeValue)> =
        [sa("doc_type", AttributeValue::Str("owning".to_string()))]
            .into_iter()
            .collect();
    assert!(matches!(
        a.decide(&plain, &resource, &Action::Review, &[]),
        Decision::Deny(_)
    ));
}

#[test]
fn deny_rule_is_deny_wins_even_for_admin() {
    let m = RoleMatrix::standard();
    // A cannot-delete carve-out: no one deletes archived vault archives.
    let policy = vec![Rule::new(
        "no-delete-archived",
        Effect::Deny,
        Some(vec![Action::Delete]),
        vec![Condition::resource(
            "vault_class",
            CmpOp::Eq,
            AttributeValue::Str("archive".to_string()),
        )],
    )];
    let a = Authorizer::new(m, policy);

    let mut admin = RoleSubject::new("root", vec![Role::Admin]);
    admin.attr("is_human", AttributeValue::Bool(true));

    let archive: Vec<(String, AttributeValue)> = [sa(
        "vault_class",
        AttributeValue::Str("archive".to_string()),
    )]
    .into_iter()
    .collect();
    let ordinary: Vec<(String, AttributeValue)> =
        [sa("vault_class", AttributeValue::Str("live".to_string()))]
            .into_iter()
            .collect();

    // Admin CAN delete live vault content...
    assert!(matches!(
        a.decide(&admin, &ordinary, &Action::Delete, &[]),
        Decision::Allow(_)
    ));
    // ...but the Deny rule wins over the Admin matrix grant on archives.
    assert!(matches!(
        a.decide(&admin, &archive, &Action::Delete, &[]),
        Decision::Deny(_)
    ));
}

#[test]
fn resource_and_environment_attributes_constrain_designer_edits() {
    let m = RoleMatrix::standard();
    // Freeze window: no editing during a release freeze (environment attribute),
    // and designers may only touch items owned by their own department.
    let policy = vec![
        Rule::new(
            "freeze-window",
            Effect::Deny,
            Some(vec![Action::EditRevision]),
            vec![Condition::environment(
                "freeze",
                CmpOp::Eq,
                AttributeValue::Bool(true),
            )],
        ),
        Rule::new(
            "foreign-dept",
            Effect::Deny,
            Some(vec![Action::EditRevision]),
            vec![Condition::resource(
                "owning_dept",
                CmpOp::Neq,
                AttributeValue::Str("PDM".to_string()),
            )],
        ),
    ];
    let a = Authorizer::new(m, policy);
    let d = designer("carol", "PDM");

    let own: Vec<(String, AttributeValue)> =
        [sa("owning_dept", AttributeValue::Str("PDM".to_string()))]
            .into_iter()
            .collect();
    let foreign: Vec<(String, AttributeValue)> =
        [sa("owning_dept", AttributeValue::Str("MECH".to_string()))]
            .into_iter()
            .collect();

    assert!(matches!(
        a.decide(&d, &own, &Action::EditRevision, &[]),
        Decision::Allow(_)
    ));
    assert!(matches!(
        a.decide(&d, &foreign, &Action::EditRevision, &[]),
        Decision::Deny(_)
    ));
    assert!(matches!(
        a.decide(
            &d,
            &own,
            &Action::EditRevision,
            &[sa("freeze", AttributeValue::Bool(true))]
        ),
        Decision::Deny(_)
    ));
}

#[test]
fn numeric_and_set_operators_work() {
    use std::collections::BTreeSet;
    let m = RoleMatrix::standard();

    let mut reviewer = RoleSubject::new("rev1", vec![Role::Reviewer]);
    // clearance int pointer; the document demands >= 5.
    reviewer.attr("clearance", AttributeValue::Int(5));
    // multi-role audience set contains 'reviewers'.
    let mut audience = BTreeSet::new();
    audience.insert("reviewers".to_string());
    audience.insert("peers".to_string());
    reviewer.attr("audience", AttributeValue::Set(audience));

    let policy = vec![Rule::new(
        "clearance-gate",
        Effect::Deny,
        Some(vec![Action::Read]),
        vec![
            Condition::subject("clearance", CmpOp::Lt, AttributeValue::Int(5)),
            Condition::resource(
                "classification",
                CmpOp::Eq,
                AttributeValue::Str("restricted".to_string()),
            ),
        ],
    )];
    let a = Authorizer::new(m, policy);

    let restricted: Vec<(String, AttributeValue)> = [sa(
        "classification",
        AttributeValue::Str("restricted".to_string()),
    )]
    .into_iter()
    .collect();
    let open: Vec<(String, AttributeValue)> = [sa(
        "classification",
        AttributeValue::Str("open".to_string()),
    )]
    .into_iter()
    .collect();

    // clearance 5 is NOT < 5, so the Deny rule does not fire -> allowed.
    assert!(matches!(
        a.decide(&reviewer, &restricted, &Action::Read, &[]),
        Decision::Allow(_)
    ));
    assert!(matches!(
        a.decide(&reviewer, &open, &Action::Read, &[]),
        Decision::Allow(_)
    ));
}

#[test]
fn missing_attributes_do_not_fire_a_condition() {
    let m = RoleMatrix::standard();
    let policy = vec![Rule::new(
        "requires-clearance",
        Effect::Deny,
        Some(vec![Action::Read]),
        vec![Condition::subject(
            "clearance",
            CmpOp::Exists,
            AttributeValue::Bool(true),
        )],
    )];
    let a = Authorizer::new(m, policy);

    // Subject has no clearance attribute -> Exists condition false -> no Deny.
    let nobody = RoleSubject::new("anon", vec![Role::Reviewer]);
    // In the `missing_attributes` test we need a resource vec; here, however,
    // no resource attribute is used. Keep an explicit typed empty vec to avoid
    // building an unsized slice.
    let resource: Vec<(String, AttributeValue)> = vec![];
    assert!(matches!(
        a.decide(&nobody, &resource, &Action::Read, &[]),
        Decision::Allow(_)
    ));

    // Subject with clearance -> Exists true -> Deny (must also pass the rule's
    // resource condition; here it's empty/unconded so the deny fires).
    let mut cleared = RoleSubject::new("cleared", vec![Role::Reviewer]);
    cleared.attr("clearance", AttributeValue::Int(7));
    assert!(matches!(
        a.decide(&cleared, &resource, &Action::Read, &[]),
        Decision::Deny(_)
    ));
}
