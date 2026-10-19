//! P8-M4-T03 -- Admin surface: role matrix, full revision view, storage quotas.
//!
//! This is the backend of the Admin UI card. It sits on top of the sibling
//! P8 cores that were stacked into this crate:
//!
//! * [`crate::rbac`]      -- role matrix + authorizer (P8-M3-T02); the admin
//!   *grants* and *revokes* roles into a live registry that the authorizer
//!   reads on every decision, so a change takes effect immediately (live),
//!   with no restart or reload.
//! * [`crate::identity`]  -- accounts (P8-M3-T01); a role grant/revoke targets
//!   a real, active account, so "who has which role" stays attached to
//!   principals the identity layer knows.
//! * [`crate::store`]     -- the persisted revision store (P8-M2-T01); the
//!   admin "see all revisions" view is a flat, newest-first projection over
//!   every item, and storage quota *usage* is measured from real blob bytes on
//!   disk.
//! * [`crate::audit`]     -- tamper-evident append-only audit log (P8-M3-T03);
//!   every admin mutation (grant, revoke, quota set/clear) appends an entry,
//!   and the log's hash chain makes the admin's footprint verifiable.
//!
//! Design notes:
//!
//! * **Live.** [`AdminService`] keeps roles and quotas behind `RwLock`s. The
//!   authorizer never caches: every [`AdminService::authorize`] call builds a
//!   fresh [`RoleSubject`](crate::rbac::RoleSubject) from the current role
//!   registry, so a grant is visible to the very next decision.
//! * **Authorization on the admin surface.** Grant/revoke require the caller
//!   to hold [`Action::ManageRoles`](crate::rbac::Action::ManageRoles);
//!   quota changes require
//!   [`Action::ManageSystem`](crate::rbac::Action::ManageSystem). Anything
//!   else is [`AdminError::Forbidden`] and leaves state untouched, so a
//!   non-admin cannot grant themselves privileges through the admin UI.
//! * **Quota accounting.** [`AdminService::usage_bytes`] sums the *on-disk*
//!   sizes of the blobs referenced by a principal's revisions (content-address
//!   => the same bytes shared by two principals count for each, a documented
//!   attribution choice; it is the conservative, over-counting direction).
//! * **Bootstrap.** [`AdminService::open_with_bootstrap`] seeds the very first
//!   [`Role::Admin`] grant (there is nobody to authorize it yet); the grant is
//!   still audit-logged so provisioning is traceable.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::audit::{self, AuditLog};
use crate::identity::{AccountStatus, IdentityService};
use crate::rbac::{Action, Authorizer, Decision, Role, RoleMatrix, RoleSubject};
use crate::revstore::{RevisionStore, StoreError, StoredRevision};
use crate::state::{Maturity, RevisionState};

/// A storage-quota failure, carrying the numbers the UI can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaError {
    /// `principal` is over (or would be over) `limit` bytes: used + requested
    /// exceeds the cap.
    Exceeded {
        /// Whose quota was hit.
        principal: String,
        /// The assigned cap in bytes.
        limit: u64,
        /// Bytes already used before this request.
        used: u64,
        /// Bytes the refused request would have added.
        requested: u64,
    },
}

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaError::Exceeded {
                principal,
                limit,
                used,
                requested,
            } => write!(
                f,
                "storage quota exceeded for {principal}: {used} used + {requested} requested > {limit} limit"
            ),
        }
    }
}

impl std::error::Error for QuotaError {}

/// Why an admin operation failed.
#[derive(Debug)]
pub enum AdminError {
    /// The acting principal is not authorized for this admin action.
    Forbidden(String),
    /// The target principal is not a known, active account.
    UnknownPrincipal(String),
    /// The principal is a known account but disabled.
    AccountDisabled(String),
    /// The revision-store layer refused the operation.
    Store(StoreError),
    /// A storage quota would be exceeded.
    Quota(QuotaError),
    /// The audit store refused to record the mutation.
    Audit(String),
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminError::Forbidden(what) => write!(f, "forbidden: {what}"),
            AdminError::UnknownPrincipal(p) => {
                write!(f, "unknown principal (not an active account): {p}")
            }
            AdminError::AccountDisabled(p) => write!(f, "account disabled: {p}"),
            AdminError::Store(e) => write!(f, "revision store: {e}"),
            AdminError::Quota(e) => write!(f, "quota: {e}"),
            AdminError::Audit(e) => write!(f, "audit store: {e}"),
        }
    }
}

impl std::error::Error for AdminError {}

impl From<StoreError> for AdminError {
    fn from(e: StoreError) -> Self {
        AdminError::Store(e)
    }
}

/// The admin backend surface (P8-M4-T03).
///
/// Thread-safe by interior mutability: roles, quotas, the audit log and the
/// revision store are shared behind `RwLock`s, so a grant/revoke/quota change
/// is live for the next call on the same running service.
pub struct AdminService {
    root: PathBuf,
    identity: IdentityService,
    authorizer: Authorizer,
    roles: RwLock<BTreeMap<String, BTreeSet<Role>>>,
    quotas: RwLock<BTreeMap<String, u64>>,
    audit: RwLock<AuditLog>,
    store: RwLock<RevisionStore>,
}

impl AdminService {
    /// Open (creating on demand) an admin service rooted at `root`, seeding
    /// `admin` as the very first [`Role::Admin`] principal.
    ///
    /// The bootstrap grant bypasses the normal authorization gate (there is
    /// nobody to authorize it yet) but is still audit-logged. All of the
    /// service state -- revision store, audit log, quota registry, role
    /// registry -- shares this root.
    pub fn open_with_bootstrap(
        root: impl AsRef<Path>,
        identity: IdentityService,
        admin: &str,
    ) -> Result<Self, AdminError> {
        let root = root.as_ref().to_path_buf();
        let store = RevisionStore::open(&root)?;
        let audit = AuditLog::open(&root).map_err(|e| AdminError::Audit(e.to_string()))?;
        let authorizer = Authorizer::new(RoleMatrix::standard(), vec![]);
        let service = AdminService {
            root,
            identity,
            authorizer,
            roles: RwLock::new(BTreeMap::new()),
            quotas: RwLock::new(BTreeMap::new()),
            audit: RwLock::new(audit),
            store: RwLock::new(store),
        };
        // The one un-authorized grant in the system: the provisioning seed.
        if service.grant_internal(admin, Role::Admin) {
            service.append_audit(admin, &format!("role:{admin}"), "bootstrap admin grant")?;
        }
        Ok(service)
    }

    /// The service root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The live role matrix the service authorizes against (role x action).
    pub fn role_matrix(&self) -> RoleMatrix {
        self.authorizer.matrix().clone()
    }

    /// The live role registry: every principal and their roles, sorted.
    pub fn grants(&self) -> BTreeMap<String, Vec<Role>> {
        self.roles
            .read()
            .expect("roles lock poisoned")
            .iter()
            .map(|(p, set)| (p.clone(), set.iter().copied().collect()))
            .collect()
    }

    /// Roles a principal currently holds (empty for a brand-new principal).
    pub fn roles_of(&self, principal: &str) -> Vec<Role> {
        self.roles
            .read()
            .expect("roles lock poisoned")
            .get(principal)
            .cloned()
            .map(|s| s.into_iter().collect())
            .unwrap_or_default()
    }

    /// Authorize `principal` for `action` against the live role registry.
    ///
    /// The role subject is rebuilt on every call, so a role grant/revoke is
    /// honored by the very next decision (live semantics).
    pub fn authorize(&self, principal: &str, action: Action) -> Decision {
        let subject = RoleSubject::new(principal, self.roles_of(principal));
        self.authorizer.decide(&subject, &[], &action, &[])
    }

    /// Grant `role` to `principal`. Returns whether the grant changed state.
    ///
    /// Requires the acting principal to hold
    /// [`Action::ManageRoles`](crate::rbac::Action::ManageRoles). The
    /// principal must be a known, active account. A successful grant is
    /// audit-logged and takes effect live.
    pub fn grant_role(&self, actor: &str, principal: &str, role: Role) -> Result<bool, AdminError> {
        self.require_allowed(actor, Action::ManageRoles)?;
        self.require_active_account(principal)?;
        let changed = self.grant_internal(principal, role);
        if changed {
            self.append_audit(
                actor,
                &format!("role:{principal}"),
                &format!("grant {role}"),
            )?;
        }
        Ok(changed)
    }

    /// Revoke `role` from `principal`. Returns whether the revoke changed state.
    ///
    /// Requires [`Action::ManageRoles`](crate::rbac::Action::ManageRoles) and
    /// a known, active target account. A successful revoke is audit-logged and
    /// takes effect live. Revoking a role the principal does not hold is a
    /// no-op (`false`), not an error.
    pub fn revoke_role(
        &self,
        actor: &str,
        principal: &str,
        role: Role,
    ) -> Result<bool, AdminError> {
        self.require_allowed(actor, Action::ManageRoles)?;
        self.require_active_account(principal)?;
        let changed = {
            let mut roles = self.roles.write().expect("roles lock poisoned");
            let some = roles.get_mut(principal);
            match some {
                Some(set) if set.contains(&role) => {
                    set.remove(&role);
                    if set.is_empty() {
                        roles.remove(principal);
                    }
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.append_audit(
                actor,
                &format!("role:{principal}"),
                &format!("revoke {role}"),
            )?;
        }
        Ok(changed)
    }

    /// See **all** revisions across every item, flattened.
    ///
    /// The ``see all revisions`` pillar of the card: a single projection over
    /// the entire vault, not a per-item drill-down. Items are walked in id
    /// order, revisions newest-first within each item.
    pub fn all_revisions(&self) -> Result<Vec<StoredRevision>, AdminError> {
        let store = self.store.read().expect("store lock poisoned");
        let mut out = Vec::new();
        for item in store.all_items()? {
            out.extend(store.list_revisions(&item.item_id)?);
        }
        Ok(out)
    }

    /// One item's revisions, newest first (the admin's per-item view).
    pub fn revisions_for(&self, item_id: &str) -> Result<Vec<StoredRevision>, AdminError> {
        let store = self.store.read().expect("store lock poisoned");
        Ok(store.list_revisions(item_id)?)
    }

    /// Convenience: create a revision as `principal` (delegates to the store).
    pub fn create_revision(
        &self,
        principal: &str,
        item_id: &str,
    ) -> Result<StoredRevision, AdminError> {
        self.require_allowed(principal, Action::CreateItem)?;
        let mut store = self.store.write().expect("store lock poisoned");
        Ok(store.create_revision(item_id, principal)?)
    }

    /// Convenience: apply one state transition as `principal`.
    ///
    /// The transition is authorized against the action the target state
    /// implies (submit-for-review / review / release / make-obsolete).
    pub fn transition_revision(
        &self,
        principal: &str,
        item_id: &str,
        revision: u32,
        to: RevisionState,
    ) -> Result<StoredRevision, AdminError> {
        let action = match to {
            RevisionState::Working => Action::Review,
            RevisionState::UnderReview => Action::SubmitForReview,
            RevisionState::Released => Action::Release,
            RevisionState::Obsolete => Action::MakeObsolete,
        };
        self.require_allowed(principal, action)?;
        let mut store = self.store.write().expect("store lock poisoned");
        Ok(store.transition(item_id, revision, to)?)
    }

    /// Convenience: set a revision's maturity as `principal` (edit-phase only).
    pub fn mature_revision(
        &self,
        principal: &str,
        item_id: &str,
        revision: u32,
        maturity: Maturity,
    ) -> Result<StoredRevision, AdminError> {
        self.require_allowed(principal, Action::EditRevision)?;
        let mut store = self.store.write().expect("store lock poisoned");
        Ok(store.set_maturity(item_id, revision, maturity)?)
    }

    /// The assigned storage cap (bytes) for `principal`, or `None` if unset.
    pub fn storage_quota(&self, principal: &str) -> Option<u64> {
        self.quotas
            .read()
            .expect("quotas lock poisoned")
            .get(principal)
            .copied()
    }

    /// Set (or, with `None`, clear) a principal's storage quota in bytes.
    ///
    /// Requires [`Action::ManageSystem`](crate::rbac::Action::ManageSystem).
    /// A change is audit-logged and takes effect immediately: the next
    /// [`AdminService::attach_under_quota`] enforces the new cap.
    pub fn set_storage_quota(
        &self,
        actor: &str,
        principal: &str,
        limit: Option<u64>,
    ) -> Result<bool, AdminError> {
        self.require_allowed(actor, Action::ManageSystem)?;
        self.require_active_account(principal)?;
        let mut quotas = self.quotas.write().expect("quotas lock poisoned");
        let (changed, detail) = match limit {
            Some(l) => match quotas.insert(principal.to_string(), l) {
                Some(prev) if prev == l => (false, format!("quota {principal} -> {l}")),
                _ => (true, format!("quota {principal} -> {l}")),
            },
            None => match quotas.remove(principal) {
                Some(_) => (true, format!("quota {principal} cleared")),
                None => (false, format!("quota {principal} cleared")),
            },
        };
        if changed {
            self.append_audit(actor, &format!("quota:{principal}"), &detail)?;
        }
        Ok(changed)
    }

    /// Real on-disk bytes stored on behalf of `principal`.
    ///
    /// Sums the file sizes of every blob referenced by a revision the
    /// principal created. Content addressing means two principals sharing one
    /// blob each count the full size -- intentionally conservative (it
    /// over-counts shared storage, never under-counts).
    pub fn usage_bytes(&self, principal: &str) -> u64 {
        let store = self.store.read().expect("store lock poisoned");
        let mut total = 0u64;
        let Ok(items) = store.all_items() else {
            return 0;
        };
        for item in items {
            let Ok(revs) = store.list_revisions(&item.item_id) else {
                continue;
            };
            for rev in revs {
                if rev.created_by != principal {
                    continue;
                }
                if let Some(sha) = &rev.blob_sha256 {
                    let path = self.root.join("blobs").join(sha);
                    if let Ok(meta) = fs::metadata(path) {
                        total += meta.len();
                    }
                }
            }
        }
        total
    }

    /// Attach content to a revision, enforced against the owner's quota.
    ///
    /// Refuses (`AdminError::Quota`) when adding `bytes` would push the
    /// owner's real usage over their assigned cap. The refusal is
    /// transactional: nothing is written. Works like
    /// [`RevisionStore::attach_content`](crate::store::RevisionStore::attach_content)
    /// otherwise.
    pub fn attach_under_quota(
        &self,
        owner: &str,
        item_id: &str,
        revision: u32,
        filename: &str,
        bytes: &[u8],
        comment: Option<&str>,
    ) -> Result<StoredRevision, AdminError> {
        self.require_allowed(owner, Action::EditRevision)?;
        if let Some(limit) = self.storage_quota(owner) {
            let used = self.usage_bytes(owner);
            let requested = bytes.len() as u64;
            if used + requested > limit {
                return Err(AdminError::Quota(QuotaError::Exceeded {
                    principal: owner.to_string(),
                    limit,
                    used,
                    requested,
                }));
            }
        }
        let mut store = self.store.write().expect("store lock poisoned");
        Ok(store.attach_content(item_id, revision, filename, bytes, comment)?)
    }

    /// Re-derive the whole audit chain from disk, detecting any tampering.
    pub fn verify_audit(&self) -> Result<Vec<audit::Entry>, audit::AuditError> {
        self.audit.read().expect("audit lock poisoned").verify()
    }

    // -- internals ---------------------------------------------------------

    /// Grant without any authorization gate (bootstrap path).
    fn grant_internal(&self, principal: &str, role: Role) -> bool {
        let mut roles = self.roles.write().expect("roles lock poisoned");
        let entry = roles.entry(principal.to_string()).or_default();
        entry.insert(role)
    }

    /// Require `actor` is allowed `action` against the *live* registry.
    fn require_allowed(&self, actor: &str, action: Action) -> Result<(), AdminError> {
        let decision = self.authorize(actor, action);
        if decision.is_allowed() {
            return Ok(());
        }
        let reason = match decision {
            Decision::Deny(_) => format!("{actor} is not authorized for {action}"),
            Decision::Allow(_) => unreachable!("is_allowed matched a deny"),
        };
        Err(AdminError::Forbidden(reason))
    }

    /// Require the principal is a known, active account.
    fn require_active_account(&self, principal: &str) -> Result<(), AdminError> {
        let accounts = self.identity.accounts();
        match accounts.get(principal) {
            None => Err(AdminError::UnknownPrincipal(principal.to_string())),
            Some(acc) if acc.status() == AccountStatus::Disabled => {
                Err(AdminError::AccountDisabled(principal.to_string()))
            }
            Some(_) => Ok(()),
        }
    }

    /// Append an audit entry; maps audit errors onto [`AdminError::Audit`].
    fn append_audit(&self, actor: &str, target: &str, detail: &str) -> Result<(), AdminError> {
        self.audit
            .write()
            .expect("audit lock poisoned")
            .append(actor, audit::Action::Change, target, Some(detail))
            .map(|_| ())
            .map_err(|e| AdminError::Audit(e.to_string()))
    }
}
