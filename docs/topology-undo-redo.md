# P6-M3-T02: Topology Undo/Redo Stack, Serializable

Status: implemented and independently verified (Isolith wave 1, P6).

## Scope

A bounded, serializable undo/redo history for topology edits, delivered as two
modules in `crates/oep-geo`:

- `oep_geo::history::History<S>` - the generic undo/redo stack over any
  serializable state record `S: HistoryState`.
- `oep_geo::topology_state::TopologyState` - the concrete topology snapshot
  record (bodies with kind + bounding box, features with parameters) the stack
  persists in this card.

`TopologyState`, plus the whole stack, are persisted in our own documented text
format (`oep-history` v1), so an editing session's undo/redo history survives a
save/load cycle byte-identically. This is the "Topology undo/redo stack,
serializable" card (P6-M3-T02); the durable feature graph with replay is the
separate P6-M3-T03 card.

## Why std-only, no serde

Deliberately no `serde`/`serde_json` dependency (Cargo.toml untouched). The P6-M2
math card recorded the same reasoning and this card inherits it: the P6-M1 schema
card is concurrently adding serde to this very crate, so pulling the same
dependencies would collide on Cargo.toml/Cargo.lock at merge, and it re-exposes
the crates.io fetch risk on CI. The text format is simple enough to hand-roll and
it is, like the model format, "our own documented format" (deterministic, strict
decode, byte-locked golden).

## History model (single vector + cursor)

`History<S>` keeps one ordered vector of committed states plus a cursor:

| Operation | Behavior |
| --- | --- |
| `commit(state)` | Discard the redo branch, append `state`, move cursor to it. |
| `undo()` | Move cursor one state older; return the now-current state, or `None` at the oldest. |
| `redo()` | Move cursor one state newer; return the now-current state, or `None` at the newest. |
| `can_undo()` | `cursor > 0`. |
| `can_redo()` | `cursor + 1 < len`. |
| `clear()` | Drop all states. |
| capacity | `0` = unbounded; otherwise a commit that would exceed capacity drops the OLDEST states so memory stays bounded while current + recent survive. |

Classic CAD rule: a fresh `commit` after an `undo` truncates the redo branch
(you cannot redo into a new branch from a stale future).

## Serialization format (`oep-history` v1)

Deterministic text, one line per field; a state record is its own block delimited
by `state <index>` markers:

```
format oep-history
version 1
capacity 8
cursor 3
states 4
state 0
revision 1
op init
bodies 1
body oep-body-bracket solid 0.0;0.0;0.0;80.0;40.0;6.0
features 0
state 1
...
```

Record rules (both encode and strict decode enforce them):

- Topology state: `revision <n>`, `op <label>`, then counted `bodies`/`features`
  line groups.
- Body line: `body <id> <kind> <minx>;<miny>;<minz>;<maxx>;<maxy>;<maxz>`,
  kind in {`solid`, `sheet`, `wire`}, all coordinates finite, `min <= max` per
  axis.
- Feature line: `feature <id> <kind> name=value;name=value` (params optional,
  name-sorted).
- Deterministic ordering: bodies and features sorted by id, params sorted by
  name (`fmt_value` renders integral floats with `.0` so int/float is never
  ambiguous).
- `decode` returns `None` on any violation; it never panics (tolerant-failure
  doctrine). `History::from_text` rejects a state/count mismatch, a bad marker
  index, trailing junk, or an out-of-range cursor.

## Verification (real material, two-sided)

The committed golden `crates/oep-geo/tests/golden/history-bracket.oephist`
records a realistic bracket edit session: `init -> union-lip -> hole -> fillet`
then `undo x2 -> redo -> countersink` (the fresh commit drops `fillet` from the
redo branch). Final stack holds 4 states, cursor on `countersink`.

- `crates/oep-geo/tests/history_contract.rs` (13 tests): history semantics,
  redo-branch truncation, bounded-capacity drop-oldest, whole-stack
  round-trip identity, byte-lock to the committed golden, and strict-decode
  rejection cases. Regenerate with `OEP_HISTORY_REGEN=1 cargo test -p oep-geo
  --test history_contract regenerate_golden`.
- `tests/test_oep_geo_history_acceptance.py` (25 checks): independent stdlib
  re-parse of the golden plus a byte-stable re-encode; a self-consistent but
  wrong serializer cannot satisfy both Rust and Python.

Run: `cargo test --workspace` (13 integration tests) and
`python3 tests/test_oep_geo_history_acceptance.py` (or `pytest`).

## Follow-ons

- P6-M3-T01's OCCT boolean ADR names this undo/redo stack as a consumer; the
  stack is generic (`HistoryState`) so `Document`/OCCT-backed states plug in
  unchanged once P6-M1's model merges.
- P6-M3-T03: durable feature graph with replay is the separate, compatible
  card.
