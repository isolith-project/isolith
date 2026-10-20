# P6-M3-T03: Feature Graph (History) Model with Replay — Mini Feature Tree

Status: implemented and verified (Keelworks wave 1, P6), PR review pending.

## Scope

A **feature graph**: the design-history model of a native OEP part, stored as a
rooted **mini feature tree** with deterministic **replay**. The graph is the
thing that turns "a list of edits someone made" into "the model you can
recompute from nothing". This card delivers:

- `oep_geo::feature_graph::FeatureGraph` — the tree + construction/validation.
- `oep_geo::feature_graph::evaluate` — deterministic, tolerant replay through a
  pluggable `Evaluator` trait (kernel-agnostic seam).
- `oep_geo::feature_graph::MiniBoxEvaluator` — an in-crate reference evaluator
  (bbox stand-in) that makes replay testable **today**, before the real
  geometry kernel (P6-M3-T01 Open CASCADE binding) exists.
- Own-format serialization (`oep-feature-graph` v1): graph + canonical replay
  trace, byte-locked to a committed golden and independently re-derived by a
  stdlib Python acceptance test.

Relationship to siblings: P6-M1-T01 defines the `Feature` *record* (id, name,
open `kind`, parameter map). This card is its *executable* counterpart: the
graph structure and the replay engine that consume feature records. P6-M3-T02
is the orthogonal **undo/redo stack** (time navigation of whole states); this
card is the **dependency graph + recompute** model. They compose: an edit
appends to the graph, then replay re-derives the state, then the undo stack
remembers the snapshot.

## What "not CATIA semantics" means (locked decision)

The name of the card is deliberate. This is **not** an attempt to replicate
CATIA's feature tree:

- **No knowledgeware step.** No parametric constraint closure, no solver feed.
  A feature's effect is what its `kind` + params say when replayed, full stop.
- **No hidden solver state.** The graph is the complete, self-contained record;
  replay is repeatable byte-for-byte from the serialized text.
- **Open operation vocabulary.** `kind` is an open PascalCase vocabulary
  (`Extrude`, `Offset`, `Union`, `Trim`, `Mirror`, …). Readers and kernels
  negotiate by name; unknown kinds never crash decode or validate — replay
  aborts after the completed prefix and reports the unknown kind (tolerant-
  failure doctrine, P2-M1-T04).
- **Explicit dependencies.** Inputs are declared (`refs`), not inferred. A
  feature receives exactly the outputs of its refs, else its parent's output,
  else the evaluator's `start_value` for roots.

## Design

### Graph model

- `FeatureGraph` owns `nodes` in **creation order**; a node's position is its
  replay index. No topological sort is ever needed: the strictly-backward
  `refs` invariant guarantees the graph is a DAG and replay order is trivially
  creation order.
- `FeatureNode { id, name, kind, parent, refs, params }` — `parent` gives the
  tree shape; `refs` are the explicit input (dependency) edges; `params` are
  `(name, ParamValue)` pairs (`Num(f64)` | `Str(short-id)`), kept **sorted by
  key** so serialization is deterministic (double everywhere, P6-M1-T03).
- `validate()` returns a list of `GraphError`s: duplicate ids, unknown
  parent/ref, forward refs. Unknown `kind`s are *not* validation errors (open
  vocabulary).

### Replay

`evaluate(engine)` walks nodes in creation order; for each node resolves
inputs — refs' outputs in declaration order, else `[parent_output]`, else
`[start_value]` — and calls `Evaluator::evaluate_node(node, inputs)`. The
result is `ReplayResult { trace, error }`:

- On success `error == None` and `trace` holds every node's output.
- On failure (unknown kind, missing/forward dependency, evaluator error) the
  returned trace is the **completed prefix** and `error` names the cause —
  the process never panics on data problems.
- Determinism is a hard property: same graph × same evaluator ⇒ identical
  trace (locked by a dedicated test).

The `Evaluator` trait is the kernel seam. Until P6-M3-T01 lands the real
kernel, `MiniBoxEvaluator` folds each op into a bbox `[f64;6]`:

| kind | effect (inputs already resolved) |
|------|----------------------------------|
| `Extrude` | union of inputs with a box `length × width × height` at origin |
| `Offset`  | shift last input by `(dx, dy, dz)` |
| `Union`   | bounding-box envelope of all inputs |
| `Trim`    | `maxz -= dz` on the last input |
| `Mirror`  | reflect last input about plane `axis` at coordinate `at` |

These are documented **stand-in semantics** for a native mini modeler, not a
claim about any kernel's behaviour.

### Own format (`oep-feature-graph` v1)

Deterministic line-oriented text. The golden for the bracket story:

```
format oep-feature-graph
version 1
nodes 5
node 0
id oep-feature-bracket-body
name bracket-body
kind Extrude
parent -
refs 0
params 3
height=6
length=80
width=40
node 1
id oep-feature-side-shelf
…
trace 5
step 0
id oep-feature-bracket-body
kind Extrude
box 0;0;0;80;40;6
…
```

- Params are written sorted by key; values are `f64` or `str:<id>`.
- The `trace` section is computed **live** by `to_text` via `MiniBoxEvaluator`,
  locking structure and behaviour into the same bytes.
- `from_text` decodes strictly and never panics: wrong format/version, bad
  counts, missing/non-sequential markers, non-finite numbers, empty ids or
  param names, blank/trailing lines → `None`. Unknown kinds and unlinked ids
  *do* decode (semantic checks live in `validate` / replay).
- Round-trip is a fixpoint: `to_text(from_text(t)) == t` for any canonical `t`
  (trace re-derives deterministically), and `from_text(to_text(g)) == g`.

### Decision: std-only, no serde

Like the P6-M2-T01 vendored-math card, this card deliberately adds **no
dependencies** and does not touch `Cargo.toml`/`Cargo.lock`. Rationale:

- The concurrently-running P6-M1-T01 schema card adds `serde`/`serde_json` to
  this same crate; a second Cargo.toml/Cargo.lock edit would collide at merge.
- Simplifies CI (no crates.io fetch risk for the new crate surface).
- The text format is small, auditable line-by-line, and independently
  re-parsable in Python stdlib — which is exactly what the two-sided golden
  verification needs.

## Verification (real material)

The golden `bracket-feature-graph.oepfg` is a native (non-CATIA) mini feature
tree for a bracket part: Extrude → Offset → Union → Trim → Mirror, with
closed-form bbox outputs. Two independent sides must agree:

1. **Rust contract test** (`crates/oep-geo/tests/feature_graph_contract.rs`,
   15 tests): structure/validation rules, replay closed-form outputs,
   determinism, parent-input propagation, unknown-kind partial-trace tolerance,
   golden byte-lock, round-trip identity, strict-decode rejection cases, and a
   custom `Evaluator` proving the seam is kernel-agnostic.
2. **Python acceptance** (`tests/test_oep_geo_feature_graph_acceptance.py`,
   stdlib only): re-derives every replay output from the committed graph with
   plain arithmetic and cross-checks the committed trace within 1e-9.

A self-consistent but wrong evaluator cannot pass both.

## Regenerate the golden

```bash
cargo run -p oep-geo --example gen_feature_graph_golden -- \
  crates/oep-geo/tests/golden
```

(Rust test regen path: `OEP_FEATURE_GRAPH_REGEN=1 cargo test -p oep-geo
--test feature_graph_contract`.)

## Remaining seams (later cards)

- Real geometry: `MiniBoxEvaluator` is the stand-in until the P6-M3-T01 Open
  CASCADE binding provides a `Evaluator` over actual B-Reps.
- PMI association: feature ids can later link to the P6-M1-T02 `oep-pmi`
  annotations (open vocabulary already tolerates any `kind`).
- Importer hub (P6-M4-T02): readers map foreign histories (3DXML/CGR/STEP)
  onto this graph rather than losing their edit-friendliness.
