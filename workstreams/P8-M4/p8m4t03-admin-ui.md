# P8-M4-T03 -- Admin UI backend: role matrix, revisions, storage quotas

Status: GREEN, acceptance tested (`crates/oep-pdm/tests/admin_roles_live.rs` and
`admin_revisions_quotas.rs`), contract lockstep
(`tests/test_p8m4t03_admin_ui.py`).

## Scope

The backend of the Admin UI card: `crates/oep-pdm::admin` (an `AdminService`
facade). It delivers the three acceptance pillars on top of the sibling P8
cores that were stacked into this crate:

| Pillar | What the admin can do | Core it uses |
|---|---|---|
| Role matrix | grant/revoke roles live; see the role x action matrix | `rbac` (P8-M3-T02) |
| Revisions | see ALL revisions across every item | `store`/`state` (P8-M2-T01) |
| Storage quotas | set/clear per-principal byte caps, enforced on writes | `store` blobs (P8-M2-T01) |
| (cross-cutting) | every admin mutation is audit-logged, tamper-evident | `audit` (P8-M3-T03) |

This card deliberately does NOT build a pixel-level web page. The REST wire-up
(P8-M6-T01) and any HTML/CLI skin (P8-M4 siblings) call this module; the module
is the tested, audit-logged admin behavior.

## Live semantics

Roles and quotas live behind `RwLock`s inside `AdminService`. Every
`authorize()` call rebuilds the `RoleSubject` from the current role registry,
so a grant/revoke is honored by the very next decision on the same running
service: no restart, no reload, no cache invalidation. Quota changes are read
at every `attach_under_quota` call.

## Authorization on the admin surface

Grant/revoke require `Action::ManageRoles`; quota set/clear require
`Action::ManageSystem`. Both are admin-only in the standard matrix
(P8-M3-T02). A non-admin caller gets `AdminError::Forbidden` and state is left
untouched, so a designer cannot grant themselves a role through the admin UI.
The target of a grant/revoke/quota must be a known, active account (identity,
P8-M3-T01); unknown or disabled principals are refused.

## Bootstrap

`AdminService::open_with_bootstrap(root, identity, admin)` seeds the very first
`Admin` role -- there is nobody to authorize it yet. The grant is still
audit-logged, so provisioning is traceable.

## Quota accounting

`usage_bytes(principal)` sums the on-disk sizes of every blob referenced by a
revision the principal created. Content addressing means two principals
sharing one blob each count the full size: this is deliberately conservative
(it over-counts shared storage, never under-counts, and it is honest about
attribution). Enforcement (`attach_under_quota`) is transactional: a refused
attach writes nothing.

## Audit trail

`grant_role`, `revoke_role` and `set_storage_quota` each append a
`Change` entry to the tamper-evident `audit` log (P8-M3-T03), keyed by the
involved principal (`role:<name>`, `quota:<name>`). `verify_audit()` re-derives
the hash chain from disk, so an operator can prove the admin's footprint is
complete and untampered.

## Files

- `crates/oep-pdm/src/admin.rs` -- the module
- `crates/oep-pdm/src/lib.rs` -- `pub mod admin;`
- `crates/oep-pdm/tests/admin_roles_live.rs` -- live grant/revoke + matrix + audit
- `crates/oep-pdm/tests/admin_revisions_quotas.rs` -- revisions view + quotas
- `tests/test_p8m4t03_admin_ui.py` -- contract lockstep guard

## Verification

```text
cargo test --workspace                # incl. the 9 new admin acceptance tests
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
python3 -m pytest tests/ -q           # contract lockstep guard
```
