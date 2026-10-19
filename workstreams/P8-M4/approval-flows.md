# P8-M4-T02 -- Approval flows, reviewer comments, and the e-signature-equivalent (intent-affirmation, not a legal seal)

Status: implemented. Owner: P8 PDM lane (Web UI wave). Version: 1 (2026-10-17).

## 1. Purpose

The Web UI half of approval handling for the P8 PDM lane: a change ticket
(ECR/ECO, from P8-M2-T02) is submitted by a designer, reviewed by a
reviewer, and either approved or rejected, with reviewer comments persisting
with the ticket and the decision recorded as our own
**e-signature-equivalent** -- an *intent-affirmation*, explicitly documented
not-parity with a legal electronic signature.

It builds directly on the two sibling cards that had to land first:

- the lite change-management ticket model (P8-M2-T02): `ChangeTicket`,
  `TicketStatus`, tracked `history`, revision-set link;
- the RBAC role matrix (P8-M3-T02): `RoleMatrix::standard()`,
  `RoleSubject`, default-deny `Authorizer`.

## 2. The flow

    Designer (SubmitForReview)   Reviewer (Review)
          |                           |
    Draft -> Open -> UnderReview -> Approved | Rejected

- A ticket is created in `Draft`; the designer links its revision set.
- `submit_for_review` (gated on `Action::SubmitForReview`, granted to the
  Designer role) advances the ticket Draft -> Open -> UnderReview. Both
  transitions are tracked in `history`, exactly as the change model tracks
  every real status change.
- `add_reviewer_comment` (gated on `Action::Read`) appends a `ReviewComment`
  that persists with the ticket, in insertion order, retrievable alongside
  the ticket after the decision is recorded.
- `decide` (gated on `Action::Review`, granted to the Reviewer role) records
  an `ApprovalDecision` (Approve -> `Approved`, Reject -> `Rejected`) and a
  `SigningIntent`. Both target transitions are validated by the change
  model, so a ticket that is not UnderReview cannot be decided, and terminal
  states are absorbing.
- Everything is default-deny: a principal whose role does not grant the
  action is refused with a message naming the reason, and no signature is
  recorded on a denied decision.

## 3. Reviewer comments persist with the ticket

The acceptance calls for reviewer comments that persist with the ticket.
The board keeps a per-ticket comment thread keyed by ticket id; the ticket
and its comments round-trip through the same store and are both retrievable
after an approve/reject decision. The web detail page renders the thread on
the ticket page, so a reviewer can follow the conversation when deciding.
No on-disk persistence yet -- the in-memory store deliberately mirrors the
lite posture of P8-M2-T02 (persistence is a P8-M1-T02/P8-M6-T01 concern).

## 4. The e-signature-equivalent: intent-affirmation, not a legal seal

Our own signing concept ("our own 3Dpassport-equivalent" framing per the
program board): a decision is recorded as an **intent-affirmation** by the
acting principal. Every `SigningIntent` carries:

- `decision` (Approve | Reject) and the ticket id;
- `by` -- the acting principal (their name, per the P8-M3-T01 identity layer
  once wired);
- `affirm` -- a free-text affirmation statement the reviewer asserts (e.g.
  "I affirm I reviewed this ticket and approve the change."). The board
  refuses a decision with an empty affirmation, so every recorded signature
  is a real, stated intent;
- `at` -- RFC 3339 UTC timestamp;
- `nature == SigningNature::IntentAffirmation`.

`SigningNature::LegalSeal` exists in the type only as the documented
non-parity bound: this concept deliberately produces **no legal seal**.
It is not X.509/PAdES, it is not qualified/generic legal signature material,
it carries no identity proof beyond the asserted principal, and it has no
legal effect. That is the point of "documented not-parity." The e-signature
concept here is an auditable record of stated intent for the change board,
not a substitute for a legal signature. The acceptance test asserts both
halves: a recorded signature is always `IntentAffirmation`, and it is never
`LegalSeal`.

## 5. The web surface

`crates/oep-pdm/src/web.rs` is a std-only HTTP/1.1 surface that drives the
board over real sockets -- the same no-framework transport posture the P7
stub used before axum was ratified (workstreams/P8-M1/web-stack-adr.md keeps the std-only
seam for tooling/contract-test clients). It deliberately adds no web
framework so this card does not race the axum service skeleton
(P8-M1-T01, landed in parallel).

Routes under `/approval`:

- `GET /approval` -- ticket list
- `GET /approval/{id}` -- detail: status, comments, decide form, signing
  records
- `POST /approval/new` -- create (kind + title)
- `POST /approval/{id}/submit` -- submit for review (Designer)
- `POST /approval/{id}/comment` -- add reviewer comment
- `POST /approval/{id}/decide` -- approve/reject with an intent-affirmation
  statement; 303 to the detail page, 403 on default-deny, and the detail
  page always documents that records are "intent-affirmation" only.

The embedding principal (`by` + `role`) travels in the form until the
P8-M3-T01 identity layer and P8-M6-T01 REST surface bind real sessions; the
board itself is transport-agnostic so those layers can mount the same model.

## 6. Where things live

    crates/oep-pdm/src/approval.rs          board + comments + signing intent (std-only)
    crates/oep-pdm/src/web.rs               std-only HTTP surface for the flow
    crates/oep-pdm/src/lib.rs               pub mod approval; pub mod web;
    crates/oep-pdm/tests/approval_flows.rs  domain acceptance (RED)
    crates/oep-pdm/tests/approval_web_flows.rs  end-to-end over real sockets (RED)
    workstreams/P8-M4/approval-flows.md                 this note
    tests/test_p8_m4_approval_flows.py      acceptance test (doc + module + flow)

## 7. Explicit non-goals (later cards)

- **P8-M3-T01** identity binding; **P8-M6-T01** REST surface and persistence;
  the axum-mounted multi-page app (**P8-M4-T01**).
- A legal/qualified electronic signature in the X.509/PAdES sense: out of
  scope by design, documented above.
