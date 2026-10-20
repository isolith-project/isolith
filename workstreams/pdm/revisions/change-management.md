# P8-M2-T02 -- Change management: ECR/ECO-style tickets linked to revisions (lite)

Status: implemented (model only). Owner: P8 PDM lane. Version: 1 (2026-10-17).

## 1. Purpose

Change control over the vault: an engineering-change ticket -- an ECR
(engineering change request, proposes a change) or an ECO (engineering change
order, authorizes and drives one) -- captures *what* the change is for, *who*
owns it, *what status* it is in, and *which vault revisions* it touches
(its **revision set**).

Scope is deliberately lite, per the card acceptance: a ticket model, a link
to a revision set, tracked statuses, and unit tests. There is **no workflow
engine**: statuses are a fixed enumerated lifecycle, transitions are validated
by the model itself, and every real transition is appended to a per-ticket
history. Approval flows, e-signatures, RBAC/audit, persistence and the REST
surface are later P8 cards and are explicitly out of scope here.

## 2. The model

    TicketKind    = Ecr | Eco
    TicketStatus  = Draft -> Open -> UnderReview -> {Approved | Rejected | Cancelled}
                  = Approved -> Implemented | Cancelled
                  = terminal: Rejected | Implemented | Cancelled   (absorbing)
    StatusChange  = { from, to, at, by, note }   (append-only history)
    RevisionRef   = { item_id, revision_number } (one vault revision)
    RevisionSet   = set of RevisionRef            (the ticket's affected set)
    ChangeTicket  = { id, kind, title, description, status, history,
                      revision_set, created_by, created_at }
    ChangeStore   = in-memory store; create/get/ticket_mut + queries
                    by status and by affected item

Rules the model enforces without any workflow engine:

- A fresh ticket starts in `Draft`; the revision set starts empty.
- `set_status` validates every transition against `can_transition_to`;
  an illegal jump (e.g. Draft -> Approved, or any move out of a terminal
  status) is rejected with `ChangeError::InvalidTransition` and records
  nothing. Same-status calls are idempotent no-ops.
- Every real transition appends a `StatusChange` to `history` with the
  acting principal and an RFC 3339 UTC timestamp -- status changes are
  *tracked*.
- The revision set is a set: inserting an already-linked revision is a
  no-op. Unlinking is supported.

## 3. Where things live

    crates/oep-pdm/src/change.rs                the model (pure std, no deps)
    crates/oep-pdm/src/lib.rs                   pub mod change; + crate docs
    crates/oep-pdm/tests/change_management.rs   public-API acceptance (RED)
    workstreams/pdm/revisions/change-management.md                  this note

Deliberately dependency-free (no serde, no db): the module compiles with
the std library only, so this card does not touch `Cargo.toml`/`Cargo.lock`
while sibling P8 cards land their own pieces of the same crate concurrently.

## 4. The revision-set link (the seam)

A ticket's revision set references revisions by `(item_id, revision_number)`
strings, exactly the identity the P7-M2 save-revisions contract and the
P8-M1-T02 vault object model use (revision numbers are server-assigned
strings, newest last; item ids are opaque url-safe strings).

The change model validates only that a reference is well-formed (non-empty
fields). **Existence of the referenced revision in the vault is the
P8-M1-T02 vault model's job** -- those cards land independently, so this
card treats a revision reference as a claim, not a foreign-key. When the
vault object model and revision state machine (P8-M2-T01) merge, a ticket
can be surfaced with a live status per referenced revision (e.g. block
linking to an already-released revision) without changing this model's
shape.

## 5. Explicit non-goals (later cards)

- **P8-M2-T01** revision state machine on the *revisions* themselves
  (the four working/under_review/released/obsolete states are the vault's
  domain, not the ticket's).
- **P8-M2-T03** where-used / impact on the relationship graph.
- **P8-M4-T02** approval flows + reviewer comments + e-signature; this card
  only records `Approved`/`Rejected` as statuses with the acting principal.
- **P8-M3** RBAC/identity/audit log.
- **P8-M6-T01** REST API v1 + JSON projection (the ticket model here is the
  in-memory truth that API will project).
- Persistence: `ChangeStore` is in-memory like the vault stub; durable
  storage arrives with the real vault store.

## 6. Verify

    cargo test -p oep-pdm                      # 11 unit + 6 acceptance tests
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all -- --check
    python3 -m pytest -q                       # repo acceptance (contract CI)

All four are green on the delivery branch; the full precommit gate
(`scripts/gate/precommit-gate.sh`) reports PASS.
