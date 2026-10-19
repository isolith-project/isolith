//! The `oep-rules` knowledge set: parameters, formulas, checks and rules.
//!
//! This is the knowledge-light engine the P7-M4-T02 card calls for. It is
//! documented as NOT parity with CATIA V5 Knowledgeware (see
//! `P7-M4/rules-checks-sandbox.md`): parameters are flat `f64`/boolean/string
//! values (no typed physical quantities, no units engine), formulas are
//! evaluated on demand (not keep-alive), checks are evaluated on demand (not
//! event-driven), and rules are single-pass if-then-else that can only SET
//! knowledge parameters (they never re-generate geometry).
//!
//! ## Evaluation order (deterministic)
//!
//! 1. Static parameters (number/boolean/string) and formulas are resolved with
//!    dependency ordering; a cycle is a typed [`RulesError::CyclicFormula`].
//! 2. Rules run in declaration order over fixpoint rounds. A rule's condition
//!    and action expressions see the values as of the previous round; a rule
//!    that changes a parameter takes effect in the following round. The loop
//!    stops when a round changes nothing or the documented safety cap is hit.
//! 3. Checks are evaluated against the final (post-rule) values, in
//!    declaration order, and their message templates are rendered.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{LoadError, RulesError, ValueMismatch};
use crate::expr::{parse, PathSeg, Value};
use crate::part::PartContext;

/// The `format` header of the knowledge JSON format.
pub const FORMAT: &str = "oep-knowledge";
/// The supported knowledge schema version.
pub const VERSION: u32 = 1;
/// The `format` header of a generated evaluation report.
pub const REPORT_FORMAT: &str = "oep-rules-report";
/// The report schema version.
pub const REPORT_VERSION: u32 = 1;
/// Safety cap on the rule fixpoint loop (documented behavior on exhaustion).
const MAX_RULE_ROUNDS: u32 = 64;

/// A parameter declaration: a static value or a formula expression.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Parameter {
    /// Parameter name (used as an identifier in expressions).
    pub name: String,
    /// How the parameter's value is produced.
    #[serde(flatten)]
    pub source: ParameterSource,
}

/// The flat sources a parameter can take (knowledge-light, our own model).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParameterSource {
    /// A fixed number.
    Number {
        /// The value.
        value: f64,
    },
    /// A fixed boolean.
    Boolean {
        /// The value.
        value: bool,
    },
    /// A fixed string.
    String {
        /// The value.
        value: String,
    },
    /// An expression evaluated over other parameters and the part.
    Formula {
        /// The expression text (documented grammar).
        expr: String,
    },
}

/// A check: a boolean condition plus a human message template.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Check {
    /// Check name.
    pub name: String,
    /// Boolean condition expression.
    pub condition: String,
    /// Message template, rendered with `{param}` placeholders.
    pub message: String,
}

/// A rule: an if-then-else over actions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleDef {
    /// Rule name.
    pub name: String,
    /// Boolean condition expression.
    #[serde(rename = "if")]
    pub if_expr: String,
    /// Actions run when the condition is true.
    #[serde(default)]
    pub then: Vec<RuleAction>,
    /// Actions run when the condition is false.
    #[serde(default, rename = "else")]
    pub else_: Vec<RuleAction>,
}

/// A single rule action.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RuleAction {
    /// Set an existing knowledge parameter to the result of an expression.
    Set {
        /// Parameter to set.
        parameter: String,
        /// Expression producing the new value.
        value: String,
    },
    /// Emit a message (an expression, usually a string or concatenation).
    Message {
        /// Expression producing the message text.
        text: String,
    },
}

/// A complete knowledge set in our own `oep-knowledge` v1 JSON format.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct KnowledgeSet {
    /// Format tag, always [`FORMAT`].
    pub format: String,
    /// Schema version, always [`VERSION`].
    pub version: u32,
    /// Document unit of measure (informational, e.g. `"mm"`).
    pub units: String,
    /// Parameter/checks/rules collections, in declaration order.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Check declarations.
    #[serde(default)]
    pub checks: Vec<Check>,
    /// Rule declarations.
    #[serde(default)]
    pub rules: Vec<RuleDef>,
}

impl KnowledgeSet {
    /// Parse and validate a knowledge set from our JSON format.
    ///
    /// Validation covers the format header, version, duplicate names within
    /// each collection, and known parameter source types. Expression syntax is
    /// checked lazily at evaluation (typed [`RulesError::Parse`]).
    pub fn from_json(text: &str) -> Result<Self, LoadError> {
        let ks: KnowledgeSet = serde_json::from_str(text)?;
        if ks.format != FORMAT {
            return Err(LoadError::BadFormat(ks.format.clone()));
        }
        if ks.version != VERSION {
            return Err(LoadError::BadVersion(ks.version));
        }
        ks.validate_names()?;
        Ok(ks)
    }

    fn validate_names(&self) -> Result<(), LoadError> {
        let mut seen: BTreeMap<&'static str, BTreeMap<String, ()>> = BTreeMap::new();
        for name in self.parameters.iter().map(|p| p.name.as_str()) {
            if seen
                .entry("parameter")
                .or_default()
                .insert(name.to_string(), ())
                .is_some()
            {
                return Err(LoadError::DuplicateName {
                    kind: "parameter",
                    name: name.to_string(),
                });
            }
        }
        for name in self.checks.iter().map(|c| c.name.as_str()) {
            if seen
                .entry("check")
                .or_default()
                .insert(name.to_string(), ())
                .is_some()
            {
                return Err(LoadError::DuplicateName {
                    kind: "check",
                    name: name.to_string(),
                });
            }
        }
        for name in self.rules.iter().map(|r| r.name.as_str()) {
            if seen
                .entry("rule")
                .or_default()
                .insert(name.to_string(), ())
                .is_some()
            {
                return Err(LoadError::DuplicateName {
                    kind: "rule",
                    name: name.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Evaluate this knowledge set against a real part.
    pub fn evaluate(&self, part: &PartContext) -> Result<Report, RulesError> {
        // ---- fixpoint rule rounds (single-pass, deterministic) ----
        let mut overrides: BTreeMap<String, (Value, String)> = BTreeMap::new();
        let mut last_round: Vec<RuleOutcome> = Vec::new();
        let mut first_changed_outcomes: Option<Vec<RuleOutcome>> = None;
        let mut quiet = false;

        for _round in 0..MAX_RULE_ROUNDS {
            let memo = RefCell::new(BTreeMap::<String, Value>::new());
            let stack = RefCell::new(Vec::<String>::new());
            let mut pending: Vec<(String, Value, String)> = Vec::new();
            let mut outcomes = Vec::with_capacity(self.rules.len());

            let mut changed_this_round = false;
            for rule in &self.rules {
                let cond_expr = parse(&rule.if_expr)?;
                let cond_val = cond_expr.eval(&|segs: &[PathSeg]| {
                    self.resolve_path(segs, part, &overrides, &memo, &stack)
                })?;
                let cond_bool = match cond_val.as_bool() {
                    Some(b) => b,
                    None => {
                        return Err(RulesError::RuleConditionNotBoolean {
                            name: rule.name.clone(),
                            mismatch: ValueMismatch {
                                expr: rule.if_expr.clone(),
                                context: "rule condition",
                                got: cond_val,
                                wanted: "boolean",
                            },
                        })
                    }
                };
                let actions = if cond_bool { &rule.then } else { &rule.else_ };

                let mut outcome = RuleOutcome {
                    name: rule.name.clone(),
                    branch: if cond_bool { "then" } else { "else" }.to_string(),
                    actions: Vec::new(),
                };
                for action in actions {
                    match action {
                        RuleAction::Set { parameter, value } => {
                            let expr = parse(value).map_err(|e| RulesError::RuleActionError {
                                name: rule.name.clone(),
                                cause: Box::new(e),
                            })?;
                            let v = expr
                                .eval(&|segs: &[PathSeg]| {
                                    self.resolve_path(segs, part, &overrides, &memo, &stack)
                                })
                                .map_err(|e| RulesError::RuleActionError {
                                    name: rule.name.clone(),
                                    cause: Box::new(e),
                                })?;
                            // The parameter being set must exist.
                            let current = self
                                .resolve_path(
                                    &[PathSeg::Field(parameter.clone())],
                                    part,
                                    &overrides,
                                    &memo,
                                    &stack,
                                )
                                .map_err(|e| RulesError::RuleActionError {
                                    name: rule.name.clone(),
                                    cause: Box::new(e),
                                })?;
                            let is_changed = current != v;
                            outcome.actions.push(AppliedAction {
                                op: "set".to_string(),
                                parameter: Some(parameter.clone()),
                                value: v.clone(),
                                changed: Some(is_changed),
                            });
                            if is_changed {
                                pending.push((parameter.clone(), v, rule.name.clone()));
                                changed_this_round = true;
                            }
                        }
                        RuleAction::Message { text } => {
                            let expr = parse(text).map_err(|e| RulesError::RuleActionError {
                                name: rule.name.clone(),
                                cause: Box::new(e),
                            })?;
                            let v = expr
                                .eval(&|segs: &[PathSeg]| {
                                    self.resolve_path(segs, part, &overrides, &memo, &stack)
                                })
                                .map_err(|e| RulesError::RuleActionError {
                                    name: rule.name.clone(),
                                    cause: Box::new(e),
                                })?;
                            outcome.actions.push(AppliedAction {
                                op: "message".to_string(),
                                parameter: None,
                                value: v,
                                changed: None,
                            });
                        }
                    }
                }
                outcomes.push(outcome);
            }

            // prefer the outcomes of the first round that actually changed a value,
            // so the report's rule trace is the firing that produced the change
            // (not the later quiet round where everything was already stable).
            last_round = outcomes;
            if changed_this_round && first_changed_outcomes.is_none() {
                first_changed_outcomes = Some(last_round.clone());
            }
            if !changed_this_round {
                quiet = true;
                break;
            }
            for (name, value, rule) in pending {
                overrides.insert(name, (value, rule));
            }
        }
        if !quiet {
            return Err(RulesError::RuleLoopLimit {
                rounds: MAX_RULE_ROUNDS,
            });
        }
        let rule_outcomes = first_changed_outcomes.unwrap_or(last_round);

        // ---- checks (against final values) ----------------------------------
        let memo = RefCell::new(BTreeMap::<String, Value>::new());
        let stack = RefCell::new(Vec::<String>::new());
        let lookup_ = |segs: &[PathSeg]| -> Result<Value, RulesError> {
            self.resolve_path(segs, part, &overrides, &memo, &stack)
        };

        let mut check_outcomes = Vec::with_capacity(self.checks.len());
        for c in &self.checks {
            let expr = parse(&c.condition)?;
            let v = expr.eval(&lookup_)?;
            let passed = match v.as_bool() {
                Some(b) => b,
                None => {
                    return Err(RulesError::CheckNotBoolean {
                        name: c.name.clone(),
                        mismatch: ValueMismatch {
                            expr: c.condition.clone(),
                            context: "check condition",
                            got: v,
                            wanted: "boolean",
                        },
                    })
                }
            };
            let message = render_message(&c.message, &lookup_)?;
            check_outcomes.push(CheckOutcome {
                name: c.name.clone(),
                passed,
                message,
            });
        }

        // ---- report ------------------------------------------------------------
        let mut params_report = Vec::with_capacity(self.parameters.len());
        for p in &self.parameters {
            let v = lookup_(&[PathSeg::Field(p.name.clone())])?;
            let source = if overrides.contains_key(&p.name) {
                "rule"
            } else if matches!(p.source, ParameterSource::Formula { .. }) {
                "formula"
            } else {
                "literal"
            };
            params_report.push(ParameterValue {
                name: p.name.clone(),
                value: v,
                source: source.to_string(),
                rule: overrides.get(&p.name).map(|(_, r)| r.clone()),
            });
        }

        let units = self.units.clone();
        let part_number = part.part_number.clone();
        Ok(Report {
            format: REPORT_FORMAT.to_string(),
            version: REPORT_VERSION,
            units,
            part: PartInfo {
                id: part.id.clone(),
                name: part.name.clone(),
                part_number,
            },
            parameters: params_report,
            checks: check_outcomes,
            rules: rule_outcomes,
        })
    }

    /// Resolve a member path against the part and parameters.
    ///
    /// A single-field path is a parameter name; a `part.*` path is resolved by
    /// the [`PartContext`]. Formula evaluation is memoized per resolution pass
    /// and cycle-checked via the call stack.
    fn resolve_path(
        &self,
        segs: &[PathSeg],
        part: &PartContext,
        overrides: &BTreeMap<String, (Value, String)>,
        memo: &RefCell<BTreeMap<String, Value>>,
        stack: &RefCell<Vec<String>>,
    ) -> Result<Value, RulesError> {
        match segs.first() {
            Some(PathSeg::Field(first)) if first == "part" => part.resolve_member(segs),
            Some(PathSeg::Field(name)) if segs.len() == 1 => {
                self.resolve_parameter(name, part, overrides, memo, stack)
            }
            _ => Err(RulesError::InvalidMemberPath(format!("{segs:?}"))),
        }
    }

    fn resolve_parameter(
        &self,
        name: &str,
        part: &PartContext,
        overrides: &BTreeMap<String, (Value, String)>,
        memo: &RefCell<BTreeMap<String, Value>>,
        stack: &RefCell<Vec<String>>,
    ) -> Result<Value, RulesError> {
        if let Some((v, _)) = overrides.get(name) {
            return Ok(v.clone());
        }
        if let Some(v) = memo.borrow().get(name) {
            return Ok(v.clone());
        }
        let p = self
            .parameters
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| RulesError::UndefinedParameter(name.to_string()))?;
        let value = match &p.source {
            ParameterSource::Number { value } => Value::Number(*value),
            ParameterSource::Boolean { value } => Value::Bool(*value),
            ParameterSource::String { value } => Value::Str(value.clone()),
            ParameterSource::Formula { expr } => {
                let in_cycle = stack.borrow().iter().any(|n| n == name);
                if in_cycle {
                    let mut path = stack.borrow().clone();
                    path.push(name.to_string());
                    return Err(RulesError::CyclicFormula(path));
                }
                stack.borrow_mut().push(name.to_string());
                let parsed = parse(expr)?;
                let v = parsed.eval(&|segs: &[PathSeg]| {
                    self.resolve_path(segs, part, overrides, memo, stack)
                })?;
                stack.borrow_mut().pop();
                v
            }
        };
        memo.borrow_mut().insert(name.to_string(), value.clone());
        Ok(value)
    }
}

/// Render a `{param}` message template against the current values.
fn render_message(
    template: &str,
    lookup: &dyn Fn(&[PathSeg]) -> Result<Value, RulesError>,
) -> Result<String, RulesError> {
    let bytes: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '{' {
            // {{ is an escaped literal brace.
            if i + 1 < bytes.len() && bytes[i + 1] == '{' {
                out.push('{');
                i += 2;
                continue;
            }
            // Scan for a closing brace; the content is one identifier.
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && (bytes[j].is_alphanumeric() || bytes[j] == '_') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '}' && j > start {
                let name: String = bytes[start..j].iter().collect();
                let v = lookup(&[PathSeg::Field(name.clone())])
                    .map_err(|_| RulesError::UnknownMessagePlaceholder(name.clone()))?;
                out.push_str(&v.to_string());
                i = j + 1;
                continue;
            }
            // Not a well-formed placeholder: emit the brace literally.
            out.push(c);
            i += 1;
        } else if c == '}' {
            if i + 1 < bytes.len() && bytes[i + 1] == '}' {
                out.push('}');
                i += 2;
                continue;
            }
            out.push(c);
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Identity of the part a report was evaluated against.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PartInfo {
    /// Part id.
    pub id: String,
    /// Part name.
    pub name: String,
    /// Optional part number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
}

/// One resolved parameter in a report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ParameterValue {
    /// Parameter name.
    pub name: String,
    /// Final (post-rule) value.
    pub value: Value,
    /// Provenance: `"literal"`, `"formula"` or `"rule"`.
    pub source: String,
    /// The rule that overrode this parameter, when `source == "rule"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

/// One evaluated check in a report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CheckOutcome {
    /// Check name.
    pub name: String,
    /// Whether the condition held.
    pub passed: bool,
    /// The rendered message.
    pub message: String,
}

/// One applied action recorded in a rule outcome.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppliedAction {
    /// `"set"` or `"message"`.
    pub op: String,
    /// The target parameter for `set` actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The value produced (for `message`, the rendered text).
    pub value: Value,
    /// For `set`, whether the value actually changed; absent for messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
}

/// One rule's outcome in a report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleOutcome {
    /// Rule name.
    pub name: String,
    /// Which branch ran: `"then"` or `"else"`.
    pub branch: String,
    /// The actions recorded for that branch.
    pub actions: Vec<AppliedAction>,
}

/// The deterministic evaluation report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Report {
    /// Report format tag ([`REPORT_FORMAT`]).
    pub format: String,
    /// Report schema version.
    pub version: u32,
    /// Document unit of measure.
    pub units: String,
    /// The part evaluated.
    pub part: PartInfo,
    /// Resolved parameters, in declaration order.
    pub parameters: Vec<ParameterValue>,
    /// Check outcomes, in declaration order.
    pub checks: Vec<CheckOutcome>,
    /// Rule outcomes, in declaration order.
    pub rules: Vec<RuleOutcome>,
}

impl Report {
    /// The final value of a parameter, if declared.
    pub fn value_of(&self, name: &str) -> Option<&Value> {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    }

    /// The provenance of a parameter (`"literal"`, `"formula"`, `"rule"`).
    pub fn source_of(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.source.as_str())
    }

    /// The outcome of a check by name.
    pub fn check(&self, name: &str) -> Option<&CheckOutcome> {
        self.checks.iter().find(|c| c.name == name)
    }

    /// The outcome of a rule by name.
    pub fn rule(&self, name: &str) -> Option<&RuleOutcome> {
        self.rules.iter().find(|r| r.name == name)
    }

    /// Serialize to our canonical pretty JSON (deterministic, trailing LF).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Serialize to compact JSON (used by tests and the demo runner).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("report serializes to JSON")
    }
}
