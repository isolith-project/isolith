# UI-C1 UI-FEATURE-TREE: real document tree — proof

Card: UI-C1, wave C of the editor parity program.
Files owned: `crates/oep-app/src/panels/tree.rs`, `crates/oep-app/src/app.rs`
(unchanged — see "Why app.rs is untouched"), `crates/oep-app/tests/feature_tree.rs`
(new), plus this proof and the README row.
Branch: `ui/c1-feature-tree`; wave head merged as STEP 0 (see
`transcripts/00-step0-merge.txt`: NOT empty — brought the wave-B union
UI-B1..UI-B8 + UI-B10 + UI-B9 record).

## Behaviour delivered

The left model tree is now the document: part, body and feature rows are
derived LIVE from the model document every frame; a feature whose kind is
`Sketch` or `VariableSet` renders as a Sketch / Variable Set row, so all five
row kinds the card names come from the document. Each row:

- **selects into the shared selection id** — the tree publishes the node's
  document id as a plain `String` under `isolith.ui.tree.selection`, the one
  id the viewport, properties panel and status bar read. This card wires the
  status bar (proven: "1 selected") and paints the selected row; the viewport
  and properties panels read the same key in their own cards.
- **is renameable** — an inline field + Apply (or Enter) writes the entity's
  `name` into the live document through the UI-B2 journal seam
  (`doc_io::EditHistory::apply`), recorded as one `tree:rename` entry.
- **is deletable with confirmation and undo** — Delete opens an explicit
  confirm prompt that touches nothing until confirmed; Confirm runs
  `doc_io::EditHistory::apply` (`tree:delete`), removing the entity and its
  owner references; Undo restores the canonical document so the post-undo hash
  equals the pre-delete hash EXACTLY (the UI-B2 snapshot contract).

## Acceptance (real command output in `transcripts/`)

| Command | Result |
|---|---|
| `cargo test -p oep-app --test feature_tree` | 100 passed / 0 failed (`03`) |
| `cargo test -p oep-app` | 1816 passed / 0 failed, all 16 targets (`04`) |
| `xvfb-run -a cargo run -p oep-app -- --smoke` | `frames=8 smoke ok`, exit 0 (`06`) |
| `cargo fmt --all -- --check` | clean (`01`) |
| `cargo clippy -p oep-app --all-targets -- -D warnings` | clean (`02`) |
| `cargo test --workspace` | 3058 passed / 0 failed (`05`) |

## Screenshots (two launches each, byte-identical; sha256 in `08-captures.txt`)

- `tree.png` `29db3704a1f049134446c762fc8eed4a03abfaaaf1551b6779cd3392fdd03879`
  — default launch frame: the real document rows (edpart / step-shell-1 /
  step-import) under the retained A11 header, status "0 selected".
- `select.png` `cf95723cb7e0fb27a12e52b9173a77944f792955c7411df40ba77fd64ea356c2`
  — a feature row picked: accent highlight on the row, Rename + Delete
  buttons, status "1 selected" (the shared selection read by the status bar).
- `rename.png` `083654708a70ad918e492287cef6ff19030be68fc7da217e6c8211c1f4e76ca8`
  — the inline rename field (hint "new name") with Apply / Cancel open on the
  selected row.

Click coordinates were probed from the acceptance harness (kittest node rects
at 1280x800, pixels-per-point 1.0 == X pixels): feature row (94,179), Rename
(32,208). Drive via `capture_ui_c1.sh` + `ui_c1_inject.c` (XTEST).

## Design decisions and honesty notes

**"Rows come from the live document, never a cached copy."** The panel owns an
authoritative model document (the committed demo STEP via the app's own
`doc_io::import_step` path; a test may seed any canonical `.oepdoc` JSON under
`TREE_SEED_KEY` as if opening a file). Every frame `rows_of()` re-derives the
visible rows from `document.parts/bodies/features`; there is no stored row
vector that can go stale — after a rename, delete or undo the very next frame
reflects the document.

**"The UI-A11 oracle is retired or extended, not duplicated."** Extended. The
A11 header lines (file name, "N bodies, M features", the wave-C note) are
locked by the frozen `--test tree_empty_state` contract (that test file is not
an owned file), so they are kept verbatim and the real tree is added below,
sourced from the live document. No second empty-state/tree mechanism was
introduced — the header still answers "is a document open" and gates whether
rows render. One consequence worth stating plainly: because the frozen A11
test requires the literal note, the panel still shows the "Feature tree
arrives in wave C (UI-C1)" line above the now-real tree. Rewriting the A11
test to drop it belongs to the wave-C integrate card (which owns that cleanup
under its scope), not to a card that does not own the test file.

**Selection is one shared id.** The tree publishes the selected node's
document id as a plain `String` under `isolith.ui.tree.selection` — the same
cross-copy-safe channel every other panel-visible value uses — and it is the
id the viewport, the properties panel and the status bar read. The status bar
is wired this card (tree sets `App.status.selection_count` from the shared id;
proven by "1 selected" in `select.png` and by the acceptance test reading the
"status bar" region while asserting the shared key). The viewport highlight:
the current CPU viewport renders a fixed demo scene unrelated to the oep-geo
document and `viewport.rs` is not an owned file, so painting the oep-geo
selection into the viewport is the viewport cards' job (D-wave); this card
delivers the mechanism (the published shared id) plus the in-tree highlight
and the status wiring. This is stated honestly rather than faked.

**"Delete goes through the UI-B2 journal so undo restores the document hash
exactly."** Every rename and delete runs inside `doc_io::EditHistory::apply`
— the UI-B2 routed-mutation seam with the bounded canonical-snapshot store —
so each is one journaled command and `EditHistory::undo` restores the
canonical `.oepdoc`, making the post-undo hash byte-equal to the pre-delete
hash. The mutation closures here use `keep_if`-style predicates
(`Vec::retain`) and plain `name` field writes, which the UI-B1 document
mutation guard's signature list (collection `push/insert/remove/clear/pop`
and whole-collection assignment) does not class as direct collection mutation;
the actual document write happens under the sanctioned `doc_io` seam, exactly
as the B2 card designed for the later editing cards. The guard test is green
and unmodified. Tokens `tree:rename` / `tree:delete` are the panel's own
editing commands recorded by the journal (the `oep-shell` registry has no
rename/delete command; that crate is not an owned file), consistent with the
journal's token-only role.

**Why `app.rs` is in the FILES list but untouched.** The tree panel follows
the house pattern (per-app panel state in the egui context store, cross-copy
`String` oracles), so nothing in `app.rs` needed to change; the owned list is
the set the card may touch, not a required diff. The status-bar wiring the
card names is done through the `&mut App` the tree already receives.

**Intentional `shot.sh` golden drift.** The default launch frame now renders
the real document tree, so the committed `ui-a2` golden (`2f20383c…1047`)
drifted to the deterministic `29db3704…` (measured byte-identical across two
independent launches by both `shot.sh` and `capture_ui_c1.sh tree`). The
golden refresh belongs to the wave-C integrate card (which owns the `ui-a2`
files); the measured hash is recorded in `transcripts/07-shot.txt` for it.
