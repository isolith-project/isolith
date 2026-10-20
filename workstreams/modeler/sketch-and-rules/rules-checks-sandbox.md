# P7-M4-T02: Rules/checks sandbox (Knowledgeware-lite, documented not parity)

Status: implemented (Isolith wave 2, P7 modeler lane, milestone P7.4 Sketcher/Parametrics).
Deliverable: `crates/oep-rules`, a knowledge-light engine (parameters, formulas,
checks, single-pass rules) evaluated against real `oep-geo` parts, plus a rules
demo on the committed canonical bracket part.

## TL;DR

The sandbox matches the *basics* of CATIA V5 Knowledgeware in the knowledge-light
sense this program can legally and honestly deliver: a part carries user
parameters, formulas compute derived values from the part's real data, checks
assert conditions with human messages, and if-then-else rules react by setting
knowledge parameters. It is documented as **not parity** with V5 Knowledgeware:
the scope is a deliberate subset, the expression grammar is our own, there is no
units engine, no reactive event model, no design tables, and no geometry
regeneration. See the not-parity section below for the exact list.

This keeps the program's clean-room invariant (no vendor binaries, no copied
source) and its ratification that "no feature-for-feature CATIA parity" is a
goal's non-goal. Knowledgeware, CATIA and related marks belong to Dassault
Systemes; this deliverable references the concept from public knowledge only and
contains no Dassault artifacts.

## Scope

A knowledge set is a JSON document in our own format (`format = "oep-knowledge"`,
`version = 1`, spec below) with three collections:

- **Parameters.** Flat values typed as number (`f64`, in the document's unit of
  measure), boolean, or string, declared with a literal value; or **formulas**
  declared as an expression in our own documented grammar (see below).
- **Checks.** A named boolean condition plus a message template rendered with
  `{parameter}` placeholders. Evaluation reports pass/fail with the rendered
  message.
- **Rules.** A named if-then-else: a boolean condition plus then/else action
  lists. Two action ops exist: `set` (assign a knowledge parameter, from an
  expression) and `message` (emit a text built by an expression).

Evaluation produces a deterministic report (`format = "oep-rules-report"`,
`version = 1`): the part identity, every parameter's final value with provenance
(`literal` / `formula` / `rule`), every check outcome, and every rule's trace
(branch taken and actions applied). The report serializes to pretty JSON with a
stable byte layout, which the acceptance suite locks to a committed golden.

## Evaluation order (deterministic)

1. Static parameters and formulas are resolved in dependency order. A formula
   may reference other parameters and `part.*` members of the real part. A
   cycle of mutually-referencing formulas is a typed error (`CyclicFormula`).
2. Rules run in declaration order over fixed-point rounds. A rule's condition
   and action expressions see parameter values as of the previous round; a rule
   that changes a value takes effect in the following round. The loop stops when
   a round changes nothing, with a documented safety cap of 64 rounds
   (exceeding it is a typed error, never a hang).
3. Checks are evaluated against the final (post-rule) values, in declaration
   order, and their messages are rendered.

Everything is deterministic: no wall clocks, no hashes over unordered maps, no
randomness. The same knowledge set over the same part yields byte-identical
reports.

## Our own expression grammar (documented, NOT CATIA formula syntax)

Formulas, check conditions, rule conditions and rule action values use one small
recursive-descent language implemented in `crates/oep-rules/src/expr.rs`:

```text
expression   := or
or           := and      ( "or"  and      )*
and          := equality ( "and" equality )*
equality     := comparison ( "==" | "!=" ) comparison )*
comparison   := additive ( "<=" | ">=" | "<" | ">" ) additive )*
additive     := unary    ( "+" | "-" ) unary )*
unary        := ( "-" | "not" ) unary | postfix
postfix      := primary ( "." ident | "[" ( number | string ) "]" )*
primary      := number | string | "true" | "false"
              | ident | ident "(" expr ( "," expr )* ")"
              | "(" expr ")"
```

- Numbers are plain decimal `f64`. Strings are double-quoted with `\"` `\\`
  escape support. Identifiers are `[A-Za-z_][A-Za-z0-9_]*`; `true`/`false`/
  `and`/`or`/`not` are reserved.
- Functions: `abs`, `round`, `floor`, `ceil`, `min`, `max`, `sqrt`, `pow`.
- `+` is arithmetic for two numbers and concatenation when either operand is a
  string. `and`/`or` short-circuit. Comparison works on two numbers or two
  strings; equality works on any two values of the same type (cross-type `==`
  is false). Division by zero and `sqrt` of a negative are typed errors.
- Part member paths (the "real part" boundary): `part.name`, `part.part_number`,
  `part.bbox.width|length|depth`, `part.bbox.min[i]|max[i]` (i = 0..2), and
  `part.features["<feature_id>"].parameters["<key>"]`. Feature parameters are
  the values stored on the part's features in the canonical oep-geo document.
  Unknown parameters, features, or keys are typed errors, never silent zeros.

This is our own grammar on purpose. It is not a compatible dialect of CATIA's
formula/knowledge syntax and makes no claim to parse V5 files or expressions.

## Message templates

A check message is a template string where `{parameter}` placeholders are
replaced with the final value of the named parameter; `{{` and `}}` escape
literal braces. Unknown placeholders are typed errors. Messages are rendered
deterministically and appear verbatim in the report.

## Evaluation to real material (the rules demo)

The acceptance runs the `demo_rules` example against the committed canonical
bracket part (`crates/oep-geo/tests/golden/example-bracket.oepgeo.json`: 80 x
40 x 6 mm envelope, fillet radius 2 mm, mount hole diameter 6 mm and depth
6 mm) with the committed knowledge set
(`crates/oep-rules/tests/golden/kw-bracket.json`). On that real part:

- formulas read the part: `sheet_thickness = 6`, `sheet_area = 3200`,
  `edge_fillet = 2`, `hole_diameter = 6`, `hole_depth = 6`;
- the rule `thick_sheet_uses_tight_factor` tightens `safe_factor` from 2.0 to
  1.5 (the part is a "thick" 6 mm sheet), so `max_safe_hole` becomes 9 mm;
- checks: four pass (`sheet_min_thickness`, `sheet_max_thickness`,
  `mount_hole_safe`, `through_hole`);
- the check `edge_fillet_deburr` FAILS (fillet 2 mm below the required 3 mm),
  and the rule `auto_deburr_escalate` reacts by setting `deburr_status = ACT`,
  which the report records.

That is the "rules demo on a real part" acceptance: real committed material, no
mocks, verified twice (the crate's own byte-locked report golden, plus an
independent stdlib Python re-computation of every number from the part's JSON).

## Not-parity scope (the explicit statement)

The sandbox is **not parity** with CATIA V5 Knowledgeware, and is documented
here as such instead of pretending otherwise. Concretely, the following V5
Knowledgeware capabilities are OUT of scope and intentionally absent:

- **Design tables.** No spreadsheet-driven parameter sets.
- **Reactive / event-driven evaluation.** V5 "checks on update", reactions and
  the events agent re-run knowledge when the part changes; here everything is
  evaluated on demand in one explicit pass. There is no geometry-change event.
- **Geometry regeneration.** A V5 rule can drive a dimension and the part
  re-generates; here rules only SET knowledge parameters. Re-driving geometry
  requires the P6-M3-T03 feature replay engine and is future work, not this
  card.
- **Typed physical quantities and units engine.** V5 has length/angle/mass
  quantities with dimension checking and conversion; here all numbers are plain
  `f64` in the document unit (typically mm) and there is no unit conversion.
- **User-defined features (UDF), powerCopy, generics.** No reusable knowledge
  feature templates.
- **Knowledge Advisor / Knowledge Expert libraries.** No central rule/check
  libraries with applicability trees.
- **Part families / catalog browser / parameter browser UI.** No parameter
  browser workbench; the sandbox is a headless engine plus a CLI example.
- **Formula editor / design table UI.** No interactive editor; the knowledge set
  is authored as JSON in our own format.
- **Full V5 formula language.** No parse of, or compatibility with, CATIA
  expression syntax; the grammar above is ours.
- **Persistence inside the part / incremental keep-alive formulas.** The
  knowledge set is a separate JSON document; oep-geo itself is unchanged.

Additionally, several *basics* are deliberately shallow: parameters are flat
(no hierarchy), formulas compute but do not store history, checks are static
conditions (no severity levels, no batch reports), and rules run to a fixpoint
with a safety cap rather than a full reactive engine.

Everything inside the lines above is implemented and tested; everything outside
is documented as not parity rather than being half-built.

## API (Rust)

| Type | Purpose |
| --- | --- |
| `KnowledgeSet::from_json(text)` | parse + validate our `oep-knowledge` v1 JSON (header, version, duplicate names) |
| `KnowledgeSet::evaluate(&PartContext)` | run the deterministic evaluation, returns `Report` |
| `PartContext::from_document(&Document, part_id: Option<&str>)` | extract the read-only real-part view |
| `PartContext::features` | feature id -> parameter map transcribed from the canonical model |
| `Report::value_of / source_of / check / rule` | typed accessors for tests and tooling |
| `Report::to_json_pretty` | canonical deterministic serialization |
| `expr::parse(src)` | parse an expression into an `Expr` (typed parse errors) |
| `Expr::eval(lookup)` | evaluate against a path resolver |

All fallible paths return typed errors (`RulesError` / `LoadError`); nothing
panics on bad input.

## File map

- `crates/oep-rules/Cargo.toml` - workspace member, depends on `oep-geo`
- `crates/oep-rules/src/lib.rs` - crate docs, module wiring, re-exports
- `crates/oep-rules/src/error.rs` - typed error model
- `crates/oep-rules/src/value.rs` - number/boolean/string value model
- `crates/oep-rules/src/expr.rs` - our own expression grammar (parser/evaluator)
- `crates/oep-rules/src/part.rs` - `PartContext` over the canonical model
- `crates/oep-rules/src/knowledge.rs` - knowledge set, evaluation, report
- `crates/oep-rules/examples/demo_rules.rs` - the real-part rules demo
- `crates/oep-rules/tests/knowledge_contract.rs` - Rust acceptance contract
- `crates/oep-rules/tests/golden/kw-bracket.json` - the committed knowledge set
- `crates/oep-rules/tests/golden/bracket-report.golden.json` - byte-locked report
- `tests/test_p7m4t02_rules_acceptance.py` - independent stdlib verification
- `docs/layout.md` - registered as the P7 oep-rules workstream crate

## Verification

- `cargo test -p oep-rules` runs the contract: expression grammar, real-part
  evaluation, rule trace, cycle and duplicate detection, and the byte-locked
  report golden (regenerate with `OEP_RULES_REGEN=1` after an intentional
  change).
- `tests/test_p7m4t02_rules_acceptance.py` re-reads the same real-part JSON in
  Python, independently recomputes every expected number and check outcome, runs
  the actual demo binary, and byte-compares its stdout to the committed report
  golden. A self-consistent-but-wrong engine cannot pass.
- CI: workspace lint (rustfmt, clippy -D warnings), unit, integration, and the
  precommit gate (pytest + cargo test) all green on the merged state.

## Linkage

- P7-M4-T01 (sketcher/parametrics integration): the same `f64` parameter and
  var-set semantics are the sketcher's target; variable sets store as oep-geo
  feature parameters this sandbox already reads.
- P6-M3-T03 (feature replay): rules that drive dimensions would consume replay
  to re-generate; explicitly out of scope here (see not parity).
- P8 oep-pdm: a knowledge set is a small JSON document and can ride alongside a
  revision as metadata; no schema change to oep-geo was made.
- P12 QA: this crate follows the workspace pattern (typed errors, goldens,
  independent Python verification).
