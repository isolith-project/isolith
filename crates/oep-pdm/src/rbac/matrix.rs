//! The role matrix: the single source of truth for role x action grants
//! (P8-M3-T02).

use std::collections::BTreeSet;

use super::{Action, Role};

/// A complete role x action grant matrix.
///
/// The matrix is total: for every (role, action) pair the entry is either
/// granted or not; there is no "maybe" cell and no overflow rule (a matrix
/// grant is the *base* decision the ABAC rule layer may then override).
///
/// Default-deny is by construction: [`RoleMatrix::allows`] answers `false`
/// for anything not explicitly granted in the set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleMatrix {
    grants: BTreeSet<(Role, Action)>,
}

impl RoleMatrix {
    /// Build a matrix from a set of explicit grants (deny everything else).
    ///
    /// Panics on duplicate... it cannot: a `BTreeSet` deduplicates. The set
    /// is simply the complete grant list.
    pub fn from_grants<I>(grants: I) -> Self
    where
        I: IntoIterator<Item = (Role, Action)>,
    {
        RoleMatrix {
            grants: grants.into_iter().collect(),
        }
    }

    /// The default staff matrix documented in `workstreams/P8-M3/p8m3t02-rbac.md`.
    ///
    /// * Designer: read + author items/revisions + submit for review.
    /// * Reviewer: read + review.
    /// * ReleaseAuthority: read + release + make obsolete.
    /// * Admin: every action.
    #[allow(clippy::unnecessary_fold)] // readability: explicit column list
    pub fn standard() -> RoleMatrix {
        let designer = [
            Action::Read,
            Action::CreateItem,
            Action::EditRevision,
            Action::SubmitForReview,
        ];
        let reviewer = [Action::Read, Action::Review];
        let release_authority = [Action::Read, Action::Release, Action::MakeObsolete];
        let all = Action::ALL;
        let mut grants = BTreeSet::new();
        for a in designer {
            grants.insert((Role::Designer, a));
        }
        for a in reviewer {
            grants.insert((Role::Reviewer, a));
        }
        for a in release_authority {
            grants.insert((Role::ReleaseAuthority, a));
        }
        for a in all {
            grants.insert((Role::Admin, a));
        }
        RoleMatrix { grants }
    }

    /// Grant `role` the `action` in a copy of this matrix.
    pub fn with_grant(mut self, role: Role, action: Action) -> Self {
        self.grants.insert((role, action));
        self
    }

    /// Revoke `role`'s `action` in a copy of this matrix.
    pub fn without_grant(mut self, role: Role, action: Action) -> Self {
        self.grants.remove(&(role, action));
        self
    }

    /// Does `role` explicitly hold `action`?
    pub fn allows(&self, role: Role, action: Action) -> bool {
        self.grants.contains(&(role, action))
    }

    /// Exactly the (role, action) pairs granted by this matrix.
    pub fn grants(&self) -> impl Iterator<Item = (Role, Action)> + '_ {
        self.grants.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_matrix_admin_is_full() {
        let m = RoleMatrix::standard();
        for action in Action::ALL {
            assert!(m.allows(Role::Admin, action));
        }
    }

    #[test]
    fn from_grants_defaults_to_deny() {
        let m = RoleMatrix::from_grants([(Role::Designer, Action::Read)]);
        assert!(m.allows(Role::Designer, Action::Read));
        assert!(!m.allows(Role::Designer, Action::Delete));
        assert!(!m.allows(Role::Admin, Action::Read));
    }

    #[test]
    fn with_and_without_grant() {
        let m = RoleMatrix::standard();
        let m = m.without_grant(Role::Admin, Action::ManageSystem);
        assert!(!m.allows(Role::Admin, Action::ManageSystem));
        let m = m.with_grant(Role::Reviewer, Action::Release);
        assert!(m.allows(Role::Reviewer, Action::Release));
    }
}
