# P8-M3-T03 -- Audit log: tamper-evident append-only store

Status: implemented. Owner: P8 PDM lane. Version: 1 (2026-10-17).

## 1. Purpose

Records *who* did *what* to *which* vault object. Because this is a
sovereignty-first PDM, **reads are audited like mutations**: reading a vault
object is an access event and leaves a footprint, so `record_read` is a
first-class operation and `Action::Read` is chained into the log exactly like
`Change` or `Release`.

The store is **append-only at the storage layer** and **tamper-evident**:
every entry chains a SHA-256 over the previous entry's hash, so any
modification of the on-disk log -- an edited payload, a reordered entry, a
deleted entry, a truncated tail -- is detected when the store is reopened or
when `verify()` is called.

## 2. The model

    Action       = Read | Change | Release        (read logged per sovereignty)
    Entry        = { seq, actor, action, target, detail, at_unix,
                     prev_hash, hash }
    HeadState    = { seq, hash, count }           (persisted anchor)
    AuditLog     = append-only JSON-lines store rooted at <root>

Rules the store enforces without any external machinery:

- **Append-only.** The log file is only ever opened with `O_APPEND`; the
  public surface has `append`/`record_read`/`record_change`/`record_release`
  for writing and `verify`/`entries` for reading. There is no update or
  delete operation anywhere. An append only ever grows the file -- the
  acceptance test asserts prior bytes are byte-for-byte untouched.
- **Chained hashes.** The first entry chains from 64 hex zeros (genesis);
  every later entry stores the previous entry's `hash` as `prev_hash` and its
  own `hash` is SHA-256 over `prev_hash` plus the canonical payload bytes
  (exact layout in section 3). Editing one field breaks either that entry's
  own hash or the next entry's link.
- **Verification is real, not cosmetic.** `AuditLog::open` fully re-derives
  the chain from disk before returning a store, and `verify()` re-derives it
  on demand, so an operator or watchdog can guard a live log. Any break
  surfaces as `AuditError::TamperDetected` (chain/seq/head) or
  `AuditError::CorruptRecord` (unparseable line).
- **Empty actor/target rejected** with `AuditError::EmptyField`; nothing is
  written for a rejected append.
- **The clock is advisory.** On-disk order is the sequence number and the
  chain; `at_unix` is informational and never trusted for ordering.

## 3. On-disk layout and the chain bytes

    <root>/audit.log    JSON-lines: one Entry per line, appended only
    <root>/audit.head   head checkpoint {seq, hash, count}, committed atomically

Sample line (one entry, compact JSON):

    {"seq":1,"actor":"hal","action":"change","target":"bracket/rev:1","detail":null,
     "at_unix":1792240000,"prev_hash":"0000...0000","hash":"2f7e...c1a9"}

`entry.hash` is exactly the SHA-256 of the following canonical bytes (this
byte layout is the versioned contract; it is independent of the JSON line so
verification recomputes from parsed fields alone):

    SHA-256( prev(32B) || seq(8B BE) || actor_len(4B BE) || actor
           || action_tag(1B)        || target_len(4B BE) || target
           || at_unix(8B BE)        || detail_flag(1B)
           || [ detail_len(4B BE) || detail ]        only when detail set )

where `action_tag` is 0=read, 1=change, 2=release and `prev` is the raw
decoded 32 bytes of the previous entry's `hash` (genesis for entry 1).

### What tampering looks like and how each is caught

| Attack on the log         | Detected by                                        |
|---------------------------|----------------------------------------------------|
| Edit a payload field      | entry's own recomputed hash no longer matches      |
| Edit `hash` or `prev_hash`| next entry's link breaks, or prev-link check fails |
| Delete a middle entry     | sequence number jumps (expected 1..N contiguous)   |
| Reorder entries           | sequence out of order + link mismatch              |
| Truncate the tail         | chain ends short of the persisted head checkpoint  |
| Delete the head file      | chain non-genesis vs genesis anchor mismatch       |
| Append a forged tail      | head checkpoint (last legitimate write) diverges   |

### Honest boundary: tamper-evident, not tamper-proof

SHA-256 chaining makes every modification **detected**, but a complete
adversary who can rewrite `audit.log` *and* `audit.head` *and* recompute the
whole chain could forge a self-consistent log. Detection therefore assumes
the head checkpoint is not fully in an attacker's hands. Hardening paths
(explicitly out of scope for this card):

- Push each new head hash to an external anchor (a separate WORM/SIEM sink,
  a signed seal) so even a full rewrite shows up as a forked chain; and
- ship keys for an authenticated MAC on the head when trust boundaries are
  formalized in the P8-M3 identity/RBAC work.

## 4. The sovereignty stance on reads

Under the program's sovereignty principle (P0-M5-T02), telemetry/observability
is off by default, but inside the vault the *access record* is a core data
obligation: it is the tenant's own evidence of who touched their data. Reads
are therefore `Action::Read` entries like any other -- there is no "read
doesn't count" special case, and read events are covered by the same chain
and the same tamper detection. `crates/oep-pdm/tests/audit_log.rs`
`read_access_events_are_logged` asserts reads are logged and survive reopen.

## 5. Where things live

    crates/oep-pdm/src/audit.rs                the store (serde + sha2 only)
    crates/oep-pdm/src/lib.rs                  pub mod audit; + crate docs
    crates/oep-pdm/tests/audit_log.rs          10-acceptance / 4-unit tests (RED)
    workstreams/P8-M3/audit-log.md                         this note

`Cargo.toml` adds `serde`/`serde_json`/`sha2 = "0.10"` -- the same minimal
set, at the same versions, that the workstreams/P8-M1/P8-M2 sibling cards already place
in this crate, so the eventual merge keeps identical dependency lines.

## 6. Explicit non-goals (later cards)

- **P8-M3-T01 identity** -- `actor` is a plain identifier string today. When
  local accounts/OIDC land, callers pass the authenticated principal's
  identifier; this log's shape does not change.
- **P8-M3-T02 RBAC** -- who *may* read/change/release is RBAC's job; this log
  records what *actually happened*, regardless of allowance/denial decisions.
  Denied attempts can be appended as their own entries by the authorizer.
- **P8-M4-T02 approvals/e-signatures** -- approval decisions show up here only
  as whatever action the approval flow records through this API.
- **External anchor / cryptographic seals** -- see "Honest boundary" above.
- **REST projection and retention policy** -- P8-M6-T01 projects this log;
  rotation/archival is an operations concern, not a store concern.
- **Concurrent writers** -- the store assumes one writer per log file
  (the PDM service). Multi-writer logs and their merge semantics are a
  P8-M5 sync/air-gap question.

## 7. Verify

    cargo test -p oep-pdm                      # 4 unit + 10 acceptance tests
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all -- --check
    python3 -m pytest -q                       # repo acceptance (contract CI)

All green on the delivery branch; the full precommit gate
(`scripts/gate/precommit-gate.sh`) reports PASS.
