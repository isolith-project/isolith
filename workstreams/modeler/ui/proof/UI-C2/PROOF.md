# UI-C2 UI-PROPERTY-PANEL: per-feature parameter editor — proof

Card: UI-C2, wave C of the editor parity program.
Files owned: `crates/oep-app/src/panels/properties.rs`,
`crates/oep-app/tests/property_panel.rs` (new), plus this proof and the README
row.
Branch: `ui/c2-property-panel`; wave head merged as STEP 0 (see
`transcripts/00-step0-merge.txt`: NOT empty — while this card ran, UI-C3 landed
on the wave and its merge `merge-UI-C3` was pulled into the local tree, so the
gate was re-run against the merged head).

## Behaviour delivered

The properties panel (right column) edits the SELECTED feature's parameters —
depth, angle, direction, symmetric — with a live preview, refuses invalid
values, and separates Apply from Cancel:

- **Selection is the one shared id.** The panel reads the tree panel's shared
  selection (`isolith.ui.tree.selection`); selecting an `Extrude`/`Revolve`
  feature loads its committed parameters into four editable fields (a body or
  part row honestly shows no editable parameters).
- **Live preview through the UI-B8 lane.** The preview reuses the B8
  derivation verbatim — this file's own `feature_preview` `#[path]` copy of
  `features.rs` — so the committed sketch profile from the sketch oracle plus
  the feature kind and the edited amount produce a marked-preview document
  clone whose volume / face count / rasterized frame hash are published every
  frame. Editing the depth field therefore moves the volume predictably
  (fixed profile area × depth: asserting exactly that is the acceptance),
  while the document hash and journal stay untouched (preview is not a
  commit).
- **Invalid values are refused with the field + reason named.** A negative,
  non-finite or non-numeric depth/angle, or a direction outside the axis
  tokens, sets `error_field`/`error` on the oracle and leaves the document
  (hash) and journal byte-identical.
- **Apply and Cancel differ by effect.** Apply validates, then runs the
  UI-B2 routed-mutation seam (`doc_io::EditHistory::apply`, token
  `feat:param`) writing the four parameters into the feature — exactly ONE
  journal entry and a moving `doc_hash`. Cancel restores the committed set and
  leaves the journal length and document hash untouched.

`--test property_panel` drives the REAL `app::App` (seeded `.oepdoc` with an
Extrude feature, a drawn closed profile for the preview, the shared-selection
click, `set_text_value_accesskit` into the depth field and rect clicks on
Apply/Cancel) and asserts: depth 10 → 20 keeps volume identical to the
document-derived `build_clone` result AND doubles it predictably; invalid depth
and direction refuse naming field + reason with hash/journal untouched; Apply
commits exactly one journal entry and moves the hash; Cancel restores draft +
committed set with journal/hash unchanged; a body row is not editable.

## Acceptance (real command output in `transcripts/`)

| Command | Result |
|---|---|
| `cargo test -p oep-app --test property_panel` | 210 passed / 0 failed (`03`) |
| `cargo test -p oep-app` | all 19 targets ok / 0 failed (`04`) |
| `cargo test --workspace` | 215 targets ok / 0 failed, WS_EXIT=0 (`05`) |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 (`06`) |
| `cargo fmt --all -- --check` | clean (`01`) |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean (`02`) |

## Screenshots (two launches each, byte-identical; sha256 in `08-captures.txt`)

- `empty.png` `a7594b124ac8ea49c95ef49d2c25106bf37cf309ca808d652e9943bb417586ef`
  — default launch: the properties panel shows the B7 dimension editor plus the
  C2 section's honest empty state ("Select a feature to edit its parameters").
- `importstep.png` `0f5688066374a933aadde412ed24ce4310fa094d480d216538a781721f1e90c3`
  — the demo document's `step-import (ImportStep)` row selected: the panel
  honestly reports "No editable parameters for ImportStep" (a feature whose
  kind this editor does not model), and the status bar reads "1 selected".

The shipped demo document contains only an `ImportStep` feature, so the
reachable real-binary frames show the honest empty/non-editable states; the
full editor (four fields, live preview volume, Apply/Cancel) is proven by the
kittest acceptance which seeds an `Extrude` feature. (The wave's own preview
card produced its dialog screenshot the same way.)

Click coordinates for `importstep` (kittest node rects at 1280x800, ppp=1.0):
feature row (94,179) — the same row UI-C1's capture clicks.

## Design decisions and honesty notes

**Preview "uses the UI-B8 preview path".** This file includes `features.rs` as
its own `feature_preview` `#[path]` module (the same allowance every B-wave
test makes) and calls only its pure derivation functions — `committed_geometry`
(parse the sketch oracle profile) and `build_clone(sk, kind, amount)` (derive
the marked-preview document clone with the feature's kind and the edited
amount). The volume/faces are the derived document's own mass properties and
the frame hash uses the same `render` + `backend::frame_hash` path B8's
`preview_frame` uses, so "a changed depth moves the volume predictably" is the
B8 lane's own math, not a second preview mechanism. The panel never touches
B8's typed dialog state (it keeps a fresh module compile), so no cross-copy
typed value is shared.

**"Edits are commands through the router and journal, not direct mutation."**
Apply is the only document-write path and it runs entirely inside
`doc_io::EditHistory::apply` — the UI-B2 routed-mutation seam with the
bounded canonical-snapshot store — writing the four `Feature.parameters` keys
with the before/after hashes recorded under `feat:param`. The panel owns its
own live document copy + `EditHistory`, seeded from the same source the tree
uses (`ui_c1_tree_seed` `.oepdoc` JSON, else the demo STEP through the app's
own `doc_io::import_step`), exactly as UI-C1's tree panel owns its own — the
shared surface is the plain-`String` selection id and the plain-`String` C2
oracle (`ui_c2_property_report`). The UI-B1 mutation-guard test stays green and
unmodified. The `feat:param` token is this panel's editing-language token
recorded by the journal (like `tree:rename`/`tree:delete`); `oep-shell` has no
parameter-edit command and that crate is not an owned file.

**"Invalid values leave the document untouched and name the field and
reason."** `validate_params` returns `(field, reason)` for a non-positive or
non-numeric amount and a non-axis direction; the oracle publishes
`error_field`/`error` and neither the document hash nor the journal moves
(asserted by hash equality for `-5`, `abc` and `sideways`).

**Apply vs Cancel.** Apply validates and journals exactly one entry
(`journal` 0 → 1, `doc_hash` moves); a further frame stays at exactly 1 (no
double commit). Cancel restores drafts from `applied` and leaves both
`journal` and `doc_hash` unchanged. Distinguishable by effect, as the card
requires.

**Intentional `shot.sh` golden drift.** The default launch frame now carries
the C2 empty-state line in the properties panel, so the committed ui-a2 golden
(`2f20383c…1047`) drifts to the deterministic `a7594b12…586ef` (measured
byte-identical across two independent launches by `shot.sh`, recorded in
`transcripts/07-shot.txt`). The golden refresh belongs to the wave-C integrate
card which owns those files, exactly as UI-C1 recorded its tree-row drift.
