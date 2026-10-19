//! P7-M4-T02 -- Rules/checks sandbox (`oep-rules`): knowledge-light formulas
//! and checks over `oep-geo` parts, documented as NOT parity with CATIA V5
//! Knowledgeware.
//!
//! This is the P7 modeler lane's knowledge-light engine. A [`KnowledgeSet`]
//! (parameters + formulas + checks + single-pass rules, serialized in our own
//! `oep-knowledge` v1 JSON) is evaluated against a real part ([`PartContext`]
//! extracted from the canonical [`oep_geo::Document`] model) and produces a
//! deterministic [`Report`].
//!
//! Deliberate scope border (full statement in `workstreams/P7-M4/rules-checks-sandbox.md`):
//! parameters, formulas, checks, and single-pass if-then-else rules only. No
//! design tables, no reactive/event-driven management, no geometry
//! regeneration, no units engine, no UDF/powerCopy/generics, no Knowledge
//! Advisor/Expert libraries. See the not-parity section of the doc.
//!
//! Modules:
//! - [`error`]: typed error model ([`RulesError`], [`LoadError`]).
//! - [`value`]: the dynamic `[Value]` (number/boolean/string).
//! - [`expr`]: our own documented expression grammar (parser + evaluator).
//! - [`part`]: [`PartContext`], the read-only view of a real part.
//! - [`knowledge`]: [`KnowledgeSet`], its evaluation, and the [`Report`].
#![deny(missing_docs)]

pub mod error;
pub mod expr;
pub mod knowledge;
pub mod part;
pub mod value;

/// The sandbox value type (number/boolean/string).
pub use value::Value;

/// The read-only view of a real part used as formula/check/rule context.
pub use part::PartContext;

/// The knowledge-light model: parameters, formulas, checks, rules.
pub use knowledge::{KnowledgeSet, Report};

/// The primary typed error type for evaluation and resolution.
pub use error::RulesError;

/// Workstream identifier string for this crate.
pub const WORKSTREAM: &str = "P7-RULES";
