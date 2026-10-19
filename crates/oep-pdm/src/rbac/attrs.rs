//! Attribute values, sources and comparison operators for ABAC rules
//! (P8-M3-T02).

use std::collections::BTreeMap;

/// A typed value carried by a principal, resource or environment attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// String value (e.g. a department name, classification label).
    Str(String),
    /// 64-bit integer (e.g. a clearance level).
    Int(i64),
    /// Boolean flag (e.g. `freeze`, `team_lead`).
    Bool(bool),
    /// Set of strings (e.g. an audience / group membership list).
    Set(std::collections::BTreeSet<String>),
}

impl AttributeValue {
    /// Build a [`AttributeValue::Str`] wrapper (convenience).
    pub fn str_val(s: impl Into<String>) -> Self {
        AttributeValue::Str(s.into())
    }

    /// Build a [`AttributeValue::Int`] wrapper.
    pub fn int_val(i: i64) -> Self {
        AttributeValue::Int(i)
    }

    /// Build a [`AttributeValue::Bool`] wrapper.
    pub fn bool_val(b: bool) -> Self {
        AttributeValue::Bool(b)
    }

    /// Build a [`AttributeValue::Set`] wrapper from pieces.
    pub fn set_val<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        AttributeValue::Set(items.into_iter().map(Into::into).collect())
    }

    /// A coarse compatibility guard used before comparing: `Eq`/`Neq` compare
    /// values of the same variant, numeric operators only apply to `Int`.
    fn variant_eq(&self, other: &AttributeValue) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Where an attribute lives: on the principal (caller), the resource (the
/// item/revision being acted on) or the environment (context of the request).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Attribute of the principal (caller).
    Subject,
    /// Attribute of the resource (item/revision being acted on).
    Resource,
    /// Attribute of the environment (request context).
    Environment,
}

/// Comparison operators usable in a rule condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    /// Attribute exists at all (expected value ignored).
    Exists,
    /// Equal (same value and variant).
    Eq,
    /// Not equal.
    Neq,
    /// For Set attributes: contains the given string. For Str: string
    /// containment (case-sensitive). Ignored otherwise.
    Contains,
    /// For Set attributes: set membership of the given string.
    In,
    /// Numeric greater-than (Int only).
    Gt,
    /// Numeric greater-than-or-equal (Int only).
    Ge,
    /// Numeric less-than (Int only).
    Lt,
    /// Numeric less-than-or-equal (Int only).
    Le,
}

/// A single condition: `source.key <op> expected`.
///
/// A condition is conjunctive within a rule (all conditions of a rule must
/// hold for the rule to match).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    source: Source,
    key: String,
    op: CmpOp,
    expected: AttributeValue,
}

impl Condition {
    /// New condition on the given attribute source.
    pub fn new(source: Source, key: &str, op: CmpOp, expected: AttributeValue) -> Self {
        Condition {
            source,
            key: key.to_string(),
            op,
            expected,
        }
    }

    /// Condition over a subject attribute.
    pub fn subject(key: &str, op: CmpOp, expected: AttributeValue) -> Self {
        Self::new(Source::Subject, key, op, expected)
    }

    /// Condition over a resource attribute.
    pub fn resource(key: &str, op: CmpOp, expected: AttributeValue) -> Self {
        Self::new(Source::Resource, key, op, expected)
    }

    /// Condition over an environment attribute.
    pub fn environment(key: &str, op: CmpOp, expected: AttributeValue) -> Self {
        Self::new(Source::Environment, key, op, expected)
    }

    /// The attribute source this condition reads.
    pub fn source(&self) -> Source {
        self.source
    }

    /// The attribute key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The operator.
    pub fn op(&self) -> CmpOp {
        self.op
    }

    /// The expected value (ignored for [`CmpOp::Exists`]).
    pub fn expected(&self) -> &AttributeValue {
        &self.expected
    }

    /// Does `attrs` satisfy this condition?
    ///
    /// Missing attributes never satisfy a condition (they read as "unknown",
    /// which is a non-match): a rule with a condition on a missing attribute
    /// simply does not fire. This is the conservative, default-deny-friendly
    /// reading.
    pub fn matches_attrs(&self, attrs: &BTreeMap<String, AttributeValue>) -> bool {
        let actual = match attrs.get(&self.key) {
            Some(v) => v,
            None => return false,
        };

        match self.op {
            CmpOp::Exists => true,
            CmpOp::Eq => actual.variant_eq(&self.expected) && actual == &self.expected,
            CmpOp::Neq => actual != &self.expected,
            CmpOp::Contains => match (actual, &self.expected) {
                (AttributeValue::Set(set), AttributeValue::Str(needle)) => {
                    set.iter().any(|s| s == needle)
                }
                (AttributeValue::Str(hay), AttributeValue::Str(needle)) => hay.contains(needle),
                _ => false,
            },
            CmpOp::In => match (actual, &self.expected) {
                (AttributeValue::Set(set), AttributeValue::Str(needle)) => set.contains(needle),
                _ => false,
            },
            CmpOp::Gt | CmpOp::Ge | CmpOp::Lt | CmpOp::Le => {
                let (a, b) = match (actual, &self.expected) {
                    (AttributeValue::Int(a), AttributeValue::Int(b)) => (a, b),
                    _ => return false,
                };
                match self.op {
                    CmpOp::Gt => a > b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Lt => a < b,
                    CmpOp::Le => a <= b,
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, AttributeValue)]) -> BTreeMap<String, AttributeValue> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn eq_and_neq() {
        let a = attrs(&[("dept", AttributeValue::str_val("PDM"))]);
        assert!(
            Condition::subject("dept", CmpOp::Eq, AttributeValue::str_val("PDM")).matches_attrs(&a)
        );
        assert!(
            !Condition::subject("dept", CmpOp::Eq, AttributeValue::str_val("MECH"))
                .matches_attrs(&a)
        );
        assert!(
            Condition::subject("dept", CmpOp::Neq, AttributeValue::str_val("MECH"))
                .matches_attrs(&a)
        );
    }

    #[test]
    fn missing_attribute_is_false() {
        let a = attrs(&[]);
        assert!(
            !Condition::subject("dept", CmpOp::Eq, AttributeValue::str_val("PDM"))
                .matches_attrs(&a)
        );
        assert!(
            !Condition::subject("dept", CmpOp::Exists, AttributeValue::bool_val(true))
                .matches_attrs(&a)
        );
    }

    #[test]
    fn set_membership_and_containment() {
        let a = attrs(&[("audience", AttributeValue::set_val(["reviewers", "peers"]))]);
        assert!(
            Condition::subject("audience", CmpOp::In, AttributeValue::str_val("peers"))
                .matches_attrs(&a)
        );
        assert!(
            !Condition::subject("audience", CmpOp::In, AttributeValue::str_val("admins"))
                .matches_attrs(&a)
        );
        assert!(Condition::subject(
            "audience",
            CmpOp::Contains,
            AttributeValue::str_val("reviewers")
        )
        .matches_attrs(&a));
    }

    #[test]
    fn numeric_ops() {
        let a = attrs(&[("clearance", AttributeValue::int_val(5))]);
        assert!(
            Condition::subject("clearance", CmpOp::Gt, AttributeValue::int_val(4))
                .matches_attrs(&a)
        );
        assert!(
            !Condition::subject("clearance", CmpOp::Gt, AttributeValue::int_val(5))
                .matches_attrs(&a)
        );
        assert!(
            Condition::subject("clearance", CmpOp::Ge, AttributeValue::int_val(5))
                .matches_attrs(&a)
        );
        assert!(
            Condition::subject("clearance", CmpOp::Lt, AttributeValue::int_val(6))
                .matches_attrs(&a)
        );
        assert!(
            !Condition::subject("clearance", CmpOp::Lt, AttributeValue::int_val(5))
                .matches_attrs(&a)
        );
    }
}
