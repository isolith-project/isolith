//! ABAC rule definitions (P8-M3-T02).

use super::{Action, Condition};

/// Effect of an evaluated rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Permit the action (used to grant an action the role matrix denies,
    /// subject to conditions).
    Allow,
    /// Forbid the action (deny-wins, above any matrix/Allow grant).
    Deny,
}

/// A named, effect-bearing, optionally action-scoped, condition-guarded rule.
///
/// A rule matches a request when:
///
/// * its action scope is unrestricted (`None`) or contains the action, AND
/// * every condition holds.
///
/// Matching rules are applied by [`super::Authorizer`] with deny-wins
/// semantics: a single matching [`Effect::Deny`] denies regardless of matrix
/// or Allow rules; otherwise a matching [`Effect::Allow`] grants even where
/// the role matrix denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    name: String,
    effect: Effect,
    actions: Option<std::collections::BTreeSet<Action>>,
    conditions: Vec<Condition>,
}

impl Rule {
    /// New rule.
    ///
    /// * `name` — stable identifier (shown in deny/allow reasons).
    /// * `effect` — Allow or Deny.
    /// * `actions` — `Some(set)` restricts the rule to those actions,
    ///   `None` applies it to every action.
    /// * `conditions` — conjunctive feature list; all must hold to match.
    pub fn new(
        name: &str,
        effect: Effect,
        actions: Option<Vec<Action>>,
        conditions: Vec<Condition>,
    ) -> Self {
        Rule {
            name: name.to_string(),
            effect,
            actions: actions.map(|v| v.into_iter().collect()),
            conditions,
        }
    }

    /// Rule name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rule effect.
    pub fn effect(&self) -> Effect {
        self.effect
    }

    /// Action scope (None = any action).
    pub fn actions(&self) -> Option<impl Iterator<Item = Action> + '_> {
        self.actions.as_ref().map(|s| s.iter().copied())
    }

    /// Conditions of this rule.
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// Does the rule's action scope cover `action`?
    pub fn applies_to(&self, action: Action) -> bool {
        match &self.actions {
            None => true,
            Some(set) => set.contains(&action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::{AttributeValue, CmpOp, Condition};

    #[test]
    fn applies_to_scope() {
        let scoped = Rule::new("x", Effect::Deny, Some(vec![Action::Delete]), vec![]);
        assert!(scoped.applies_to(Action::Delete));
        assert!(!scoped.applies_to(Action::Read));

        let any = Rule::new("y", Effect::Deny, None, vec![]);
        assert!(any.applies_to(Action::Read));
        assert!(any.applies_to(Action::ManageSystem));
    }

    #[test]
    fn conditions_exposed() {
        let c = Condition::subject("dept", CmpOp::Eq, AttributeValue::str_val("PDM"));
        let r = Rule::new("r", Effect::Allow, None, vec![c.clone()]);
        assert_eq!(r.conditions().len(), 1);
        assert_eq!(r.conditions()[0].key(), "dept");
    }
}
