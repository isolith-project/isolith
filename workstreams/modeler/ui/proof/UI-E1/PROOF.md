# UI-E1 UI-ASSY-TREE: components, insert, ground, isolate — proof

Wave E card. An assembly document is visible and editable in the assembly tree:
insert two parts, ground one, isolate the other, and the document matches
byte-for-byte what the CLI builds for the same operations.

## Files owned by this card

- `crates/oep-app/src/panels/assy_tree.rs` (new: assembly tree panel)
- `crates/oep-app/src/panels/mod.rs` (ADDITIVE: `pub mod assy_tree;` + RegionKind::Assembly)
- `crates/oep-app/src/app.rs` (ADDITIVE: Assembly region rendered after the model tree)
- `crates/oep-app/src/config.rs` (ADDITIVE: the region's rect in the shell layout)
- `crates/oep-app/tests/assy_tree.rs` (new: acceptance harness)
- `workstreams/modeler/ui/proof/UI-E1/` (this proof)

No manifest change: the panel builds on the EXISTING assembly seam
(`oep_shell::assem::AssemblySession`) which `oep-app` already links via
`oep-shell`. `oep-assem` types are never named; `parse_mate("fixed", ...)`
constructs the `Fixed` mate through the shell's own public helper, so grounding
goes through the seam's vocabulary without `oep-app` depending on `oep-assem`.

## Acceptance

```
cargo test -p oep-app --test assy_tree   -> green (4 tests / 130 total)
xvfb-run -a cargo run -p oep-app -- --smoke -> frames=8 smoke ok, exit 0
```

Transcripts: `transcripts/01-tests-assy-tree.txt`, `transcripts/02-smoke.txt`.

### Insert two parts; UI document == CLI document byte-for-byte

The panel seeds from the committed two-part fixture (the engine's golden for
"two parts inserted"); the tree shows both instances with their ids. The test
calls BOTH paths and asserts byte equality, printing both hashes:

```
test ui_document_matches_cli_for_same_two_inserts ... ok
ui  two-part doc sha256 = 036cf2654c1ca325c994b42f921b1b22c323e3377b9af883067cfc8417985e9b
cli two-part doc sha256 = 036cf2654c1ca325c994b42f921b1b22c323e3377b9af883067cfc8417985e9b
ui  reported doc  hash  = 15168000965944691752
```

The insert action path is also exercised: from an EMPTY assembly document the
"Insert oep-part-base" / "Insert oep-part-cap" buttons place the two committed
parts as instances and the resulting document still equals the CLI path's:

```
test insert_two_parts_from_empty_matches_cli ... ok
ui  two-instances doc sha256 = 036cf2654c1ca325c994b42f921b1b22c323e3377b9af883067cfc8417985e9b
cli two-instances doc sha256 = 036cf2654c1ca325c994b42f921b1b22c323e3377b9af883067cfc8417985e9b
```

### Ground one, isolate the other: sets exactly as expected

```
test ground_one_isolate_other_document_hash_roundtrips ... ok
pre-isolate hash = 15168000965944691752  sha256 036cf2654c1ca325c994b42f921b1b22c323e3377b9af883067cfc8417985e9b
grounded set = [base]
visible set  = [base,cap]
after isolate: visible set = [cap]
after isolate: rows = [1]
isolated doc sha256 = a3b1a169ba68a30aa4ed733f494e4d4ef464840964ff7f76fbfb779d45ee0384
after un-isolate: visible set = [base,cap]
after un-isolate: hash = 15168000965944691752
```

Grounded set = `{base}` (base pinned by a `Fixed` mate; the grounded set is read
from the seam via `moving_ids()` — instances NOT moving), visible set =
`{base,cap}`. Isolate keeps only `cap` visible and hides base's row AND its
geometry note (`rows=1`); un-isolate restores both and the document hash
returns exactly to the pre-isolate value, and the restored canonical document
is byte-identical to the pre-isolate document.

### A11y: every row has an accessible name; keyboard reachable

```
test rows_have_accessible_names_and_region_is_keyboard_reachable ... ok
keyboard-selected row = base
```

Each tree row is an AccessKit node with a non-empty label; the region is
labelled "assembly tree"; a row focused and activated with Space (keyboard
only, no mouse) selects it (asserted via the panel oracle).

## Guards and full gates (on the committed tree)

- Mutation guard green: `no_module_mutates_the_document_outside_doc_io_rs`
  (the panel's only document writes go through the `doc_io::EditHistory` seam;
  no banned signature appears in `src/`) — `transcripts/03-mutation-guard.txt`.
- Layout contract green: region geometry tiles without overlap/clipping and
  the five locked regions remain reachable at both resolutions (the Assembly
  region is nested INSIDE the model-tree column, so the five rects are
  unchanged) — `transcripts/04-layout.txt`.
- clippy `-D warnings` (all targets): exit 0 — `transcripts/05-clippy.txt`.
- fmt `--check`: exit 0 — `transcripts/06-fmt.txt`.
- `cargo test --workspace`: **4922 passed / 0 failed**, exit 0 —
  `transcripts/07-workspace.txt`. (Note: `pick_gpu` flaked once under full
  parallel load — the documented host X/GPU flake — and passes isolated and
  in the clean full run; logged to be honest.)

## Screenshots (deterministic: two independent captures byte-identical)

- `tree.png` — default: the assembly tree shows `base` and `cap` with their
  instance ids, Insert/Ground/Isolate buttons.
- `grounded.png` — `base` row carries the `· grounded` marker (Fixed mate via
  the seam); `cap` selected.
- `isolated.png` — only `cap` is visible (base row and its geometry hidden);
  the button reads `Un-isolate`.

sha256 per image in `*.sha256`:

```
tree.png       f43b62d8e51acd15520754798fe1fb2048176e5569ef046848497e7ef6b5a24e
grounded.png   b14cc194bf0bb2b23424a3d656b91e09b246586b16cf0db2edea288db90ef9b7
isolated.png   5970642f48ddf51bf68382a2dd94b960e7e282e141fa735d8b1e5d8aae51df80
```
