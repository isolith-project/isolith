//! Typed error model for the `oep-rules` rules/checks sandbox.
//!
//! Everything fallible returns a typed error; nothing panics on bad input
//! (tolerant-failure doctrine, P2-M1-T04). [`RulesError`] covers parse, type,
//! resolution and evaluation failures; [`LoadError`] covers knowledge-set load
//! validation.

use std::fmt;

use crate::value::Value;

/// A description of the value a failing expression produced where a specific
/// type was required.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueMismatch {
    /// The expression text that produced the wrong value.
    pub expr: String,
    /// The role the expression was evaluated for (e.g. "check condition").
    pub context: &'static str,
    /// The value actually produced.
    pub got: Value,
    /// The type that was required.
    pub wanted: &'static str,
}

/// Evaluation / resolution failure in the rules/checks sandbox.
#[derive(Clone, Debug, PartialEq)]
pub enum RulesError {
    /// Expression could not be parsed (syntax error in the documented grammar).
    Parse {
        /// The source expression text.
        expr: String,
        /// Human-readable explanation.
        message: String,
    },
    /// A parameter name referenced by a formula/check/rule does not exist.
    UndefinedParameter(String),
    /// A `part.*` member path does not exist on the selected part.
    InvalidMemberPath(String),
    /// The selected part has no part number but a formula needs one.
    MissingPartNumber,
    /// No part matched the requested id (or the document is empty).
    NoPart(String),
    /// A referenced feature id does not exist on the part.
    UnknownFeature(String),
    /// A referenced feature parameter key does not exist on the feature.
    UnknownFeatureParameter {
        /// Feature id.
        feature: String,
        /// Parameter key.
        key: String,
    },
    /// An index was given with the wrong kind (e.g. a name where a number was required).
    BadIndex(String),
    /// Type mismatch inside an expression.
    TypeMismatch(ValueMismatch),
    /// Division by zero at runtime.
    DivisionByZero {
        /// The expression text.
        expr: String,
    },
    /// `sqrt` of a negative number at runtime.
    NegativeSqrt {
        /// The expression text.
        expr: String,
    },
    /// `Cycle` of mutually-referencing formulas.
    ///
    /// The vector lists the parameter names on the cycle path.
    CyclicFormula(Vec<String>),
    /// A check condition did not evaluate to a boolean.
    CheckNotBoolean {
        /// Check name.
        name: String,
        /// Why it isn't a boolean.
        mismatch: ValueMismatch,
    },
    /// A rule condition did not evaluate to a boolean.
    RuleConditionNotBoolean {
        /// Rule name.
        name: String,
        /// Why it isn't a boolean.
        mismatch: ValueMismatch,
    },
    /// A rule action `value`/`text` expression failed to evaluate.
    RuleActionError {
        /// Rule name.
        name: String,
        /// The underlying error.
        cause: Box<RulesError>,
    },
    /// A rule loop failed to stabilize. This is the documented single-pass
    /// safety valve: rules that change state forever are stopped.
    RuleLoopLimit {
        /// How many fixpoint iterations were attempted.
        rounds: u32,
    },
    /// A message template referenced an unknown parameter.
    UnknownMessagePlaceholder(String),
    /// A rule restricted how many times a message action may repeat.
    Internal(&'static str),
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RulesError::Parse { expr, message } => {
                write!(f, "parse error in expression {expr:?}: {message}")
            }
            RulesError::UndefinedParameter(name) => {
                write!(f, "undefined parameter {name:?}")
            }
            RulesError::InvalidMemberPath(path) => {
                write!(f, "invalid part member path {path:?}")
            }
            RulesError::MissingPartNumber => write!(f, "part has no part number"),
            RulesError::NoPart(hint) => write!(f, "no part to evaluate against: {hint}"),
            RulesError::UnknownFeature(id) => {
                write!(f, "feature {id:?} not found on the part")
            }
            RulesError::UnknownFeatureParameter { feature, key } => {
                write!(f, "feature {feature:?} has no parameter {key:?}")
            }
            RulesError::BadIndex(w) => write!(f, "bad index in member path: {w}"),
            RulesError::TypeMismatch(m) => write!(
                f,
                "type mismatch in {context} of {expr:?}: wanted {wanted}, got {got}",
                context = m.context,
                expr = m.expr,
                wanted = m.wanted,
                got = m.got,
            ),
            RulesError::DivisionByZero { expr } => {
                write!(f, "division by zero in expression {expr:?}")
            }
            RulesError::NegativeSqrt { expr } => {
                write!(f, "sqrt of a negative number in expression {expr:?}")
            }
            RulesError::CyclicFormula(path) => {
                write!(f, "cyclic formula reference: {}", path.join(" -> "))
            }
            RulesError::CheckNotBoolean { name, mismatch } => write!(
                f,
                "check {name:?} condition is not a boolean ({context} of {expr:?}): got {got}",
                context = mismatch.context,
                expr = mismatch.expr,
                got = mismatch.got,
            ),
            RulesError::RuleConditionNotBoolean { name, mismatch } => write!(
                f,
                "rule {name:?} condition is not a boolean ({context} of {expr:?}): got {got}",
                context = mismatch.context,
                expr = mismatch.expr,
                got = mismatch.got,
            ),
            RulesError::RuleActionError { name, cause } => {
                write!(f, "rule {name:?} action failed: {cause}")
            }
            RulesError::RuleLoopLimit { rounds } => {
                write!(f, "rule evaluation did not stabilize after {rounds} rounds")
            }
            RulesError::UnknownMessagePlaceholder(name) => {
                write!(f, "message template references unknown parameter {name:?}")
            }
            RulesError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for RulesError {}

/// Knowledge-set load validation failure (format header, duplicates, types).
#[derive(Debug)]
pub enum LoadError {
    /// The JSON could not be parsed at all.
    Json(serde_json::Error),
    /// The `format` header is not `"oep-knowledge"`.
    BadFormat(String),
    /// The `version` is not the supported version (currently 1).
    BadVersion(u32),
    /// Duplicate name within one collection (parameters/checks/rules).
    DuplicateName {
        /// Which collection ("parameter", "check", "rule").
        kind: &'static str,
        /// The duplicate name.
        name: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Json(e) => write!(f, "invalid knowledge JSON: {e}"),
            LoadError::BadFormat(got) => {
                write!(f, "bad format header {got:?} (expected \"oep-knowledge\")")
            }
            LoadError::BadVersion(v) => {
                write!(f, "unsupported knowledge version {v} (supported: 1)")
            }
            LoadError::DuplicateName { kind, name } => {
                write!(f, "duplicate {kind} name {name:?}")
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError::Json(e)
    }
}
