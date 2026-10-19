# P8-M3-T02 -- RBAC: roles + attribute-based rules (ABAC)

Status: RED-first acceptance contract (implementation follows as GREEN).

## Scope

Native authorization for the oep-pdm vault service, `crates/oep-pdm::rbac`.
Independent, self-contained module: no wiring into the REST layer yet
(P8-M6-T01 owns that seam).

## Roles

Static roles: `Designer`, `Reviewer`, `ReleaseAuthority`, `Admin`.
A subject carries one or more roles. The matrix is the single source of truth
for what a role may do; everything not granted by the matrix or an ABAC rule is
denied (default-deny).

## Actions

The action set covers the PDM object lifecycle (P8-M1/M2 verbs) plus system
administration:

| Action | Meaning |
|---|---|
| Read | view any item/revision/file metadata or content |
| CreateItem | create a vault item |
| EditRevision | create/modify a working revision |
| SubmitForReview | promote working -> under-review |
| Review | approve/reject an under-review revision |
| Release | promote under-review -> released |
| MakeObsolete | mark a released/working revision obsolete |
| Delete | purge an item/revision |
| ManageUsers | account lifecycle (create/disable/reset) |
| ManageRoles | edit role grants / matrix |
| ManageSystem | configuration, quotas |

## Role x Action matrix (can)

| Action | Designer | Reviewer | ReleaseAuthority | Admin |
|---|---|---|---|---|
| Read | yes | yes | yes | yes |
| CreateItem | yes | no | no | yes |
| EditRevision | yes | no | no | yes |
| SubmitForReview | yes | no | no | yes |
| Review | no | yes | no | yes |
| Release | no | no | yes | yes |
| MakeObsolete | no | no | yes | yes |
| Delete | no | no | no | yes |
| ManageUsers | no | no | no | yes |
| ManageRoles | no | no | no | yes |
| ManageSystem | no | no | no | yes |

Every (role, action) cell is enumerated by the acceptance test
`tests/rbac_role_matrix.rs`, asserting both the allowed and the denied side
(the matrix is exhaustive, not sampled).

## ABAC rules

Beyond the role matrix, decisions may be conditioned on attributes of the
subject (caller), the resource (item/revision), and the environment.

* `Condition`: source { Subject, Resource, Environment } + key + operator
  (Eq, Neq, Contains, In, Gt, Ge, Lt, Le, Exists, NotExists) + optional value
  (string / int / bool / string-set / number).
* `Rule`: named; effect { Allow, Deny }; optional action set (None = any
  action); condition list (all must match).
* Evaluation: **deny-wins**. Any matched Deny rule denies immediately (a Deny
  rule beats even Admin unless configured otherwise by the same policy). A
  matched Allow rule can authorize an action the role matrix denied.

Semantics of `Authorizer::decide(subject, resource, action, env)`:

1. subject carries no roles and no Allow rule matches -> Deny(NoRole)
2. any matching Deny rule -> Deny(DenyRule)
3. role matrix grants the action -> Allow(Role)
4. any matching Allow rule -> Allow(AbacRule)
5. otherwise -> Deny(NotPermitted)

## Default-deny

Verified by `tests/rbac_default_deny.rs`: no role + any action is denied;
an action not on the matrix is denied for every role; a subject with no roles
is denied everything; an unknown role/action lookups never grant.

## Integration points (future)

P8-M6-T01 wires `Authorizer` into the REST layer (subject from identity
P8-M3-T01, resource attributes from the vault object model P8-M1-T02).
P8-M4-T03 admin UI reads the matrix and add/removes rules through
`RoleMatrix`/`Policy` constructors. This module stays independent so those
lanes can merge without coupling.
