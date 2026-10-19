//! Roles and actions (P8-M3-T02).

use core::fmt;

/// The four static roles of the PDM vault service.
///
/// * [`Designer`](Role::Designer): creates and edits items/revisions,
///   submits work for review.
/// * [`Reviewer`](Role::Reviewer): reviews (approves/rejects) submitted
///   revisions.
/// * [`ReleaseAuthority`](Role::ReleaseAuthority): releases and obsoletes
///   revisions.
/// * [`Admin`](Role::Admin): full access, including user/role/system
///   administration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Authors and edits items/revisions; submits work for review.
    Designer,
    /// Reviews (approves/rejects) submitted revisions.
    Reviewer,
    /// Releases and obsoletes revisions.
    ReleaseAuthority,
    /// Full access, including user/role/system administration.
    Admin,
}

impl Role {
    /// Every role, in declaration order (used by exhaustive sweeps).
    pub const ALL: [Role; 4] = [
        Role::Designer,
        Role::Reviewer,
        Role::ReleaseAuthority,
        Role::Admin,
    ];

    /// Parse a role from its canonical lowercase name.
    ///
    /// Accepted spellings: `designer`, `reviewer`, `release_authority`,
    /// `release-authority`, `admin`. Unknown names return `None` (a caller
    /// should then deny, never default-warrenty-grant).
    pub fn from_name(s: &str) -> Option<Role> {
        match s {
            "designer" => Some(Role::Designer),
            "reviewer" => Some(Role::Reviewer),
            "release_authority" | "release-authority" | "releaseauthority" => {
                Some(Role::ReleaseAuthority)
            }
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    /// Canonical lowercase name of this role.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Designer => "designer",
            Role::Reviewer => "reviewer",
            Role::ReleaseAuthority => "release_authority",
            Role::Admin => "admin",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Actions a principal can perform on/against the PDM vault.
///
/// The set is fixed and enumerated so the role matrix can be exhaustive for
/// every (action, role) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// View any item/revision/file metadata or content.
    Read,
    /// Create a vault item.
    CreateItem,
    /// Create or modify a working revision.
    EditRevision,
    /// Promote working -> under-review.
    SubmitForReview,
    /// Approve or reject an under-review revision.
    Review,
    /// Promote under-review -> released.
    Release,
    /// Mark a released (or working) revision obsolete.
    MakeObsolete,
    /// Purge an item/revision.
    Delete,
    /// Account lifecycle (create/disable/reset).
    ManageUsers,
    /// Change role grants / matrix.
    ManageRoles,
    /// Configuration, quotas.
    ManageSystem,
}

impl Action {
    /// Every action, in declaration order.
    pub const ALL: [Action; 11] = [
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

    /// Parse an action from its canonical lowercase name.
    pub fn from_name(s: &str) -> Option<Action> {
        Some(match s {
            "read" => Action::Read,
            "create_item" | "create-item" => Action::CreateItem,
            "edit_revision" | "edit-revision" => Action::EditRevision,
            "submit_for_review" | "submit-for-review" => Action::SubmitForReview,
            "review" => Action::Review,
            "release" => Action::Release,
            "make_obsolete" | "make-obsolete" | "obsolete" => Action::MakeObsolete,
            "delete" => Action::Delete,
            "manage_users" | "manage-users" => Action::ManageUsers,
            "manage_roles" | "manage-roles" => Action::ManageRoles,
            "manage_system" | "manage-system" => Action::ManageSystem,
            _ => return None,
        })
    }

    /// Canonical lowercase name of this action.
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Read => "read",
            Action::CreateItem => "create_item",
            Action::EditRevision => "edit_revision",
            Action::SubmitForReview => "submit_for_review",
            Action::Review => "review",
            Action::Release => "release",
            Action::MakeObsolete => "make_obsolete",
            Action::Delete => "delete",
            Action::ManageUsers => "manage_users",
            Action::ManageRoles => "manage_roles",
            Action::ManageSystem => "manage_system",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_round_trip() {
        for role in Role::ALL {
            assert_eq!(Role::from_name(role.as_str()), Some(role));
        }
        assert_eq!(Role::from_name("REVIEWER"), None);
        assert_eq!(Role::from_name("root"), None);
    }

    #[test]
    fn action_names_round_trip() {
        for action in Action::ALL {
            assert_eq!(Action::from_name(action.as_str()), Some(action));
        }
        assert_eq!(Action::from_name("SUPERUSER"), None);
    }

    #[test]
    fn all_role_and_action_counts() {
        assert_eq!(Role::ALL.len(), 4);
        assert_eq!(Action::ALL.len(), 11);
    }
}
