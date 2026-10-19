//! P8-M3-T02 acceptance contract: exhaustive role x action matrix.
//!
//! The role matrix is the single source of truth for what a role may do.
//! This test enumerates EVERY cell of the matrix (actions x roles) and
//! asserts both the allowed and the denied side explicitly, so the matrix is
//! exhaustive, not sampled. It also asserts the standard matrix cannot grant
//! anything beyond its documented cells (default-deny is verified here at the
//! matrix level and again end-to-end in tests/rbac_default_deny.rs).

use oep_pdm::rbac::{Action, Role, RoleMatrix};

/// Index helper: declared action set for the exhaustive sweep.
const ALL_ACTIONS: [Action; 11] = [
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

const ALL_ROLES: [Role; 4] = [
    Role::Designer,
    Role::Reviewer,
    Role::ReleaseAuthority,
    Role::Admin,
];

/// Expected standard matrix, rows = roles, columns = ALL_ACTIONS (same order).
/// true = the (role, action) pair is explicitly granted; false = denied.
const EXPECTED: [[bool; 11]; 4] = [
    // Designer
    [
        true,  // Read
        true,  // CreateItem
        true,  // EditRevision
        true,  // SubmitForReview
        false, // Review
        false, // Release
        false, // MakeObsolete
        false, // Delete
        false, // ManageUsers
        false, // ManageRoles
        false, // ManageSystem
    ],
    // Reviewer
    [
        true,  // Read
        false, // CreateItem
        false, // EditRevision
        false, // SubmitForReview
        true,  // Review
        false, // Release
        false, // MakeObsolete
        false, // Delete
        false, // ManageUsers
        false, // ManageRoles
        false, // ManageSystem
    ],
    // ReleaseAuthority
    [
        true,  // Read
        false, // CreateItem
        false, // EditRevision
        false, // SubmitForReview
        false, // Review
        true,  // Release
        true,  // MakeObsolete
        false, // Delete
        false, // ManageUsers
        false, // ManageRoles
        false, // ManageSystem
    ],
    // Admin
    [
        true, true, true, true, true, true, true, true, true, true, true,
    ],
];

fn allowed(m: &RoleMatrix, role: Role, action: Action) -> bool {
    m.allows(role, action)
}

#[test]
fn matrix_is_exhaustive_for_actions_x_roles() {
    let m = RoleMatrix::standard();
    for (ri, role) in ALL_ROLES.iter().enumerate() {
        for (ai, action) in ALL_ACTIONS.iter().enumerate() {
            let name = format!("{:?} x {:?}", role, action);
            assert_eq!(
                allowed(&m, *role, *action),
                EXPECTED[ri][ai],
                "matrix cell {} must be {}",
                name,
                if EXPECTED[ri][ai] {
                    "granted"
                } else {
                    "denied"
                }
            );
        }
    }
    // Sweep really did cover every cell.
    assert_eq!(ALL_ACTIONS.len() * ALL_ROLES.len(), 44);
}

#[test]
fn standard_matrix_covers_every_declared_action() {
    let m = RoleMatrix::standard();
    for action in &ALL_ACTIONS {
        // Admin is the full-access role; every action must be reachable by at
        // least one role, and default-deny means non-Admin is gated per cell
        // (asserted exhaustively above).
        assert!(
            m.allows(Role::Admin, *action),
            "admin may always do {action:?}"
        );
    }
}

#[test]
fn constrained_roles_are_denied_privileged_domains() {
    let m = RoleMatrix::standard();
    // Separation of duties: Reviewer and ReleaseAuthority never edit.
    for role in [Role::Reviewer, Role::ReleaseAuthority] {
        assert!(
            !m.allows(role, Action::EditRevision),
            "{role:?} must not edit"
        );
        assert!(
            !m.allows(role, Action::SubmitForReview),
            "{role:?} must not submit"
        );
    }
    // Only Admin can administer people/roles/system.
    for role in [Role::Designer, Role::Reviewer, Role::ReleaseAuthority] {
        for action in [
            Action::ManageUsers,
            Action::ManageRoles,
            Action::ManageSystem,
        ] {
            assert!(!m.allows(role, action), "{role:?} must not {action:?}");
        }
    }
    // Release is 3-way separated: Designer (creates) and Reviewer (approves)
    // must not release; ReleaseAuthority must not review its own gate.
    assert!(!m.allows(Role::Designer, Action::Release));
    assert!(!m.allows(Role::Reviewer, Action::Release));
    assert!(!m.allows(Role::Reviewer, Action::MakeObsolete));
    assert!(m.allows(Role::ReleaseAuthority, Action::Release));
}
