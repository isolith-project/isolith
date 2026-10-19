//! Role-based access control for the P8 PDM service (P8-M3-T02).
//!
//! A self-contained authorization core: a static [`RoleMatrix`] (the single
//! source of truth for what a role may do) plus an attribute-based rule layer
//! ([`Rule`]) that can further restrict (Deny, deny-wins) or, via explicit
//! Allow rules, authorize an action the role matrix would deny.
//!
//! The one invariant, tested exhaustively (`tests/rbac_default_deny.rs`):
//! **default-deny**. Nothing is granted unless the matrices/rules say so; a
//! Deny rule beats an Allow rule for the same subject and action.
//!
//! This module is deliberately dependency-free (std only) so the REST wiring
//! (P8-M6-T01) and the admin UI (P8-M4-T03) can depend on it without pulling
//! the rest of the PDM stack.
//!
//! # Workflow
//!
//! 1. Build an [`Authorizer`] from a matrix (e.g. [`RoleMatrix::standard`])
//!    and a policy (`Vec<[`Rule`]>`).
//! 2. For each request, build a [`RoleSubject`] (whose identity/roles come
//!    from the P8-M3-T01 identity layer) and the resource attribute map
//!    (from the P8-M1-T02 vault object model), then call
//!    [`Authorizer::decide`].
//! 3. Anything that is not explicitly allowed yields [`Decision::Deny`].

mod attrs;
mod authorizer;
mod matrix;
mod policy;
mod roles;

pub use attrs::{AttributeValue, CmpOp, Condition, Source};
pub use authorizer::{AllowReason, Authorizer, Decision, DenyReason, RoleSubject};
pub use matrix::RoleMatrix;
pub use policy::{Effect, Rule};
pub use roles::{Action, Role};
