# UI-A11 UI-TREE-EMPTY-STATE proof

Card: UI-A11 UI-TREE-EMPTY-STATE - the model tree must stop claiming "No model
open" when a document is loaded.
Branch: `ui/a11-tree-empty-state`. Wave branch: `wave/ui-a` (remote `github`).

Behaviour: the left panel tells the truth about document state. With no
document open it says "No model open" (kept). With a document open it names the
document, reports what the document actually contains (body/feature counts
taken from the document itself), and says plainly that the real feature tree
arrives in wave C (UI-C1) instead of pretending to be one. It never fabricates
tree rows.

The gate contradiction this fixes: the status bar reads `edpart.step *` (a
document is loaded, dirty), the viewport renders its geometry, and the left
panel still said "No model open" because `panels/tree.rs` was UI-A4's hardcoded
stub. Now the same screen reads: `edpart.step *`, `1 body, 1 feature`, and
`Feature tree arrives in wave C (UI-C1)`.

## What landed

- `crates/oep-app/src/panels/tree.rs` - the honest tree panel. It reads the
  live document through the handle UI-A7 put on the app: every frame the doc
  panel publishes a plain-`String` oracle under `DOC_REPORT_KEY` (in the egui
  context store) carrying the display name and - new for this card - the
  body/feature counts taken from the document itself. Because the oracle is a
  `String`, its type identity is shared across all `#[path]` copies, so this
  panel reads exactly what the `--test doc_io` oracle reads and stays on the
  current frame's truth (no restart, no cached state, no file I/O of its own).
  Rendering: heading "Model Tree" kept, region accessible name / geometry /
  panel frames untouched. No document -> "No model open" (weak italic). Document
  -> file name (strong), `N body/bodies, M feature/features`, and the weak
  italic "Feature tree arrives in wave C (UI-C1)" line. No raw hex, no
  `Color32::from_*`: only the token styles egui already applies (UI-A3 guard
  green, proven in the bin unit target).
- `crates/oep-app/src/panels/doc.rs` - the doc panel's report oracle now also
  carries `bodies=`/`features=` lines derived from the live document. UI-A7
  designed this `String` report as the cross-`#[path]`-copy oracle; the two new
  lines are what make a truthful count available to the tree panel without the
  tree opening files or caching stale state. No behaviour change for the doc
  panel itself: existing `doc_io` tests stay green (the report parser ignores
  unknown lines), and no test is weakened.
- `crates/oep-app/tests/tree_empty_state.rs` - the acceptance target,
  `cargo test -p oep-app --test tree_empty_state`. Drives the real
  `app::App` via `egui_kittest` and asserts what the tree region actually
  renders through the AccessKit tree:
  1. no-doc text: with the doc panel's own empty-session oracle
     (`file=Untitled`, no contents) the tree keeps "No model open" and
     fabricates nothing;
  2. doc text: once the demo document is open the tree names it (`edpart.step`),
     reports `1 body, 1 feature` (the truthful counts, cross-checked against
     the import hub), and shows the wave-C note;
  3. update-on-open: the same running app flips from "No model open" to the
     named document on the next frame, purely by re-reading the per-frame
     oracle - no restart.

## Files owned vs touched

Card-owned files touched: `crates/oep-app/src/panels/tree.rs`,
`crates/oep-app/tests/tree_empty_state.rs`, `workstreams/modeler/ui/README.md`
(row added), `workstreams/modeler/ui/proof/ui-a11/` (this proof),
`workstreams/modeler/ui/proof/ui-a2/frame.png` + `.sha256` (deliberate golden
refresh, below).

Out-of-list file, with reason: `crates/oep-app/src/panels/doc.rs`. The card
DETAIL says "Read the document through the handle UI-A7 put on `App`
(`doc_io`)". UI-A7 landed that handle as the `String` report under
`DOC_REPORT_KEY` in the egui context store (the only cross-module-readable
channel - a private `DocState`'s type identity is `#[path]`-copy-specific, a
`String`'s is shared). That report carried file/dirty/hash but not counts, and
the card requires a body/feature count "taken from the document itself". The
minimal change that makes the count reachable through UI-A7's own oracle seam
is two lines in that report. The document layer itself (doc_io.rs, the import
hub, the model) is untouched; no new dependency; no geometry/layout/frame change.

## Acceptance (all real output under `transcripts/`)

- `cargo test -p oep-app --test tree_empty_state` -> 8 passed (the three new
  tree tests + the independent import cross-check + pre-existing unit tests
  pulled in via `#[path]`). [01]
- `cargo test -p oep-app` -> all targets green: bin 6, doc_io 10, harness 7,
  layout 8, palette 9, tree_empty_state 8, viewport 7. [02]
- `cargo fmt -p oep-app --check` exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0. [03]
- `xvfb-run -a cargo run -p oep-app -- --smoke` -> `frames=8 smoke ok`, exit 0.
  [04]
- Deliberate golden refresh (the frame changes because the panel text changes):
  old golden `e60b86dc…9a9b` preserved as `ui-a2-golden-before-a11.png`; new
  golden `1798df38…673fe` recorded; two independent `bash scripts/ui/shot.sh`
  runs byte-identical. [05]
- Gate pytest legs green (unaffected): `test_brand_identity.py` 5/5,
  `test_contributor_rules_decision.py` PASS. [06]

### Old and new golden hashes, side by side

```
old (pre-A11): e60b86dcf4e29e599497f783ed4f8a38e4af9d7e72d478a9e63f201d5e94a9b4
new (A11)    : 1798df384f4bf05fb76df0d2caf96c140e6d1098e52a3e1247120d37533673fe
```

`frame.png` in this proof dir === `proof/ui-a2/frame.png` (sha256
`1798df38…673fe`), the same golden shot.sh records.
