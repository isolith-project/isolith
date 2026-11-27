# UI-E4 UI-ASSY-EXPLODE: exploded view control — proof

Wave E of the editor parity program. Card: `t_efb1fb62` — an explode slider
with per-component offsets that moves components predictably and is saved with
the document, so it round-trips through save and reopen.

## Behaviour

A new `assy_explode` panel (a `#[path]` child module of `assy_tree`, exactly
the E2/E3 additive pattern) renders in the assembly region:

- an **explode factor slider** (`explode factor`, 0.0..=2.0, driven through the
  real AccessKit `SetValue` path in the acceptance harness);
- **per-component x/y/z offset editors** (`explode offset <id> <axis>`, one
  DragValue per axis per component inside a short bounded ScrollArea);
- each component moves along **its own direction** by `factor × offset` (offsets
  default to radial directions from the assembly centroid when a document has no
  committed offsets, so every component explodes outward even untouched);
- every factor/offset change is **one journaled write** (`assy:explode` token
  via `model.history.apply`) so the panel participates in the same undo/redo as
  the tree and mate tool;
- the **explode state (factor + per-component offsets + base placements) is
  embedded in the saved document** (an ignored top-level `explode` membership in
  the canonical oep-geo JSON) so save → reopen restores the exact exploded
  positions and the controls, byte-identically.

Wiring is additive and intentional, exactly as the card reads:

```
crates/oep-app/src/panels/assy_explode.rs   (new)
crates/oep-app/src/panels/assy_tree.rs      (one #[path] include + ONE show call)
crates/oep-app/tests/assy_explode.rs        (new)
workstreams/modeler/ui/proof/UI-E4/          (this proof)
workstreams/modeler/ui/README.md             (this card's row)
```

No other file is touched. No new dependency: the saved-payload splice and the
explode-state encoding are hand-rolled (`text_hash` uses the same FNV-1a-64
scheme as `doc_io::document_hash`; the offset/state text format is a plain
`|`-joined token string embedded as one JSON string membership, so no JSON
crate and no escaping is needed — `serde_json` is not a dependency of
`oep-app`, per the card's constraint).

## Acceptance clauses (real numbers, committed output in `transcripts/`)

### 1. Factor 0 leaves every transform byte-identical to the unexploded document

Driving the slider to 1 and back to 0 through the real input path must return
the exact unexploded bytes. At factor 0 the panel writes the **base placements
verbatim** (never `base + 0 × offset`, which could flip a `-0.0`) and
canonicalises the document exactly like `assy_mates::canonicalize_doc`, so the
document hash and every per-component transform hash are identical:

```
unexploded doc hash = 15168000965944691752
unexploded base thash = 8678518526205813477  cap thash = 126298874090400601
factor 1 doc hash     = 6997398794052828193
factor 0 doc hash     = 15168000965944691752
factor 0 base thash = 8678518526205813477  cap thash = 126298874090400601
unexploded JSON == factor-0 JSON : true
```

Asserted: `factor-0 hash == unexploded hash`, `factor-0 JSON == unexploded
JSON` (byte-identical), and both per-component transform hashes return. (The
serde_json shortest round-trip of a freshly-computed `f64` can differ by one
ulp from the parsed-back doc — e.g. `o:base:`'s `-18.843366572035794` — which
is why the panel canonicalises after every write; the round-trip clause below
proves the mechanism end to end.)

### 2. Factor 1 and factor 2 move each component along its own direction by exactly factor × its offset

The actual document placement (`pos_`) is printed against `before + factor ×
offset` (`exp_`), with the offset used printed for both components at both
factors:

```
factor 1:
  base: before (0.000000,0.000000,0.000000) after (-18.843367,-9.421683,-13.459548) moved (-18.843367,-9.421683,-13.459548) offset (-18.843,-9.422,-13.460)
  cap:  before (28.000000,14.000000,20.000000) after (46.843367,23.421683,33.459548) moved (18.843367,9.421683,13.459548) offset (18.843,9.422,13.460)
factor 2:
  base: before (0.000000,0.000000,0.000000) after (-37.686733,-18.843367,-26.919095) moved (-37.686733,-18.843367,-26.919095) offset (-18.843,-9.422,-13.460)
  cap:  before (28.000000,14.000000,20.000000) after (65.686733,32.843367,46.919095) moved (37.686733,18.843367,26.919095) offset (18.843,9.422,13.460)
```

Asserted for every component at both factors: `pos == before + factor × offset`
exactly, and each component moves along **its own** direction (base along
`(-18.8, -9.4, -13.5)`, cap along `(+18.8, +9.4, +13.5)` — opposite radial
directions from the centroid, both scaled by factor).

### 3. A per-component override moves only that component

Overriding cap's x offset to 40 at factor 1 must move cap alone; base's
position **and** transform hash must be unchanged:

```
override cap offset.x = 40 (factor 1):
  base before (-18.843,-9.422,-13.460) after (-18.843,-9.422,-13.460)  moved exactly? true
  cap  before (46.843,23.422,33.460) after (68.000,23.422,33.460)  x moved by 21.157
  base transform hash unchanged?  1480940897015207264 -> 1480940897015207264
  cap  transform hash changed?    12622972043842813462 -> 6508680276428737152
  doc  hash                       6997398794052828193 -> 16355051259319296173
```

Asserted: `base` position unchanged (`moved exactly? true`) **and** base's
16-double transform hash identical before/after; `cap` moved by exactly its new
offset × factor; the document hash changed (only cap moved).

### 4. The exploded state round-trips: save, reopen, byte-identical

The full saved payload (canonical document + embedded `explode` member) is
captured, reopened in a fresh harness seeded with it, and compared:
`pos_cap = base + 2 × 30 = 88` after the cap x override to 30 at factor 2:

```
saved payload hash (factor 2 + cap override) = 2783414007677588141
saved encode       = v1|factor2|o:base:-18.843366572035794,-9.421683286017897,-13.459547551454138|o:cap:30,9.421683286017897,13.459547551454138|b:base:0,0,0|b:cap:28,14,20
factor = 2  base pos = -37.68673314407159,-18.84336657203579,-26.919095102908276  cap pos = 88,32.84336657203579,46.91909510290827
reopened payload hash = 2783414007677588141
reopened encode      = v1|factor2|o:base:-18.843366572035794,-9.421683286017897,-13.459547551454138|o:cap:30,9.421683286017897,13.459547551454138|b:base:0,0,0|b:cap:28,14,20
saved payload == reopened payload (byte-identical): true
factor reopened = 2  base pos = -37.68673314407159,-18.84336657203579,-26.919095102908276  cap pos = 88,32.84336657203579,46.91909510290827
```

Asserted byte-for-byte: `saved payload == reopened payload`, `saved encode ==
reopened encode`, the reopened document is byte-identical to the saved one, and
the reopened exploded positions (`pos_base`, `pos_cap`) and factor equal the
saved side. The saved payload is also proven to still load as a canonical
oep-geo document (parsed without error) — the `explode` member is ignored by
plain readers, so an exploded save is still a valid `.oepdoc`.

## Layout note: why the explode panel renders AFTER the mate tool

The assembly region is the fixed lower 48% of the model-tree column (only 240px
wide; at the shipped 1280x800 window its height is ~350px). The E3 proof
already documents that this fixed region cannot hold three full wave-E surfaces
without the mate tool's lower list scrolling. This card adds a fourth surface.
The explode panel therefore renders as the **last** in-flow section, directly
after the mate tool, so:

- the E2/E3 pinned rows — the mate tool's `+ Add mate` and `Delete 0` — keep
  exactly the layout E2/E3 shipped, and the E3 dof acceptance (which drives
  those exact buttons through real clicks) is untouched (verified: it stayed
  green through this card's development, including the fix below);
- the explode section's per-axis DragValues sit one-per-line inside a short
  bounded ScrollArea, so the section stays shallow and **never** individually
  overflows the 240px column (a prefixed DragValue is ~177px wide; three-per-row
  overflowed the column and — measured in a debug probe — pushed the mate
  tool's pinned `+ Add mate` to `x ∈ [211.5, 294.3]`, i.e. partly outside the
  region clip, which is exactly why the E3 dof test started failing when the
  panel was first placed before the mate tool).

The regression this card had to fix (part of the acceptance, since
`assy_diagnostics.rs` must not be touched): with an earlier in-development
layout the wide offset rows shifted the mate tool's pinned rows outside the
240px region column and `dof after mate added` stayed `0` instead of hitting
`3`. Root cause (confirmed by a geometry probe): the DragValue prefix text made
each editor ~177px wide, three-per-row ≈ 600px, overflowing the column and
pushing `+ Add mate` / `Delete 0` outside the region's clip rect where egui
drops interaction. The fix keeps explode after mates and bounds the offset
editors, so `+ Add mate` returns to `[[0.0 1326.5]-[82.8 1346.5]]` (fully
inside the clipped region, as E3 shipped it) and the dof acceptance passes with
no change to `assy_diagnostics.rs`.

## Gates (on the committed tree)

- `cargo test -p oep-app --test assy_explode`: **142 passed / 0 failed, exit
  0** — `transcripts/10-acceptance-nocapture.txt` (all numbers printed,
  `--test-threads=1` so the four wave tests run interleaved with their proofs).
- Full `assy` regression lane: `--test assy_diagnostics` **143 / 0** (the card
  forbids touching this test; it passes unchanged with the panel present),
  `--test assy_mates` (E2) **143 / 0**, `--test assy_tree` (E1) **142 / 0** —
  `transcripts/15-regressions.txt`.
- `xvfb-run -a cargo run -p oep-app -- --smoke`: **exit 0**, `frames=8 smoke
  ok` — `transcripts/20-smoke.txt`.
- `cargo test -p oep-app` (all targets): **30 targets, 4519 passed / 0
  failed, exit 0** — `transcripts/30-oep-app-full.txt`. (The documented
  pre-existing egui_kittest exit-time SIGSEGV flake under full parallel load
  hit three unrelated kittest-heavy targets on earlier runs; each re-ran
  clean in isolation and the committed oep-app transcript is the green run.)
- `cargo test --workspace`: **230 suites, 5793 passed / 0 failed, exit 0** —
  `transcripts/31-workspace.txt` (run with `--test-threads=1` so the
  documented exit-time kittest SIGSEGV cannot hit; the +1 suite / +142 tests
  vs UI-E3's 229 / 5651 is this card's new `assy_explode` target).
- Mutation guard green: `no_module_mutates_the_document_outside_doc_io_rs`
  and `the_scanner_flags_planted_direct_document_mutation_and_passes_reads`
  both pass unchanged — the explode panel writes only through
  `model.history.apply` (its `assy:explode` token), never mutating the
  document directly (in `transcripts/30-oep-app-full.txt`).
- clippy `-D warnings` all targets: exit 0 — `transcripts/40-clippy.txt`.
  fmt `--all -- --check`: exit 0 — `transcripts/41-fmt.txt`.
  Licensing: 6/6, 34 crates / 535 deps unchanged — `transcripts/42-licensing.txt`.

## Screenshots

No screenshots are committed for this card: the four acceptance clauses are
proven with byte-exact oracle numbers and the real command output above, and
the shipped-window rendering of all four wave-E surfaces in one fixed-height
region is the UI-E5 integrate card's scope (as the E3 proof already notes for
its own screenshot). The explode controls are exercised through the real input
path (AccessKit `SetValue` on both the slider and the offset DragValues), which
is strictly stronger than a screenshot for the behaviour this card owns.

## STEP 0 note

STEP 0 was run first from this worktree: `git fetch github && git merge
github/wave/ui-e`. This branch was cut from `main` at `6275787` and carries the
wave-E tree, mates and diagnostics; the merge brings NOTHING here because the
branch head is already exactly `9339aa8` (the remote wave/ui-e head, which
brought UI-E1's assembly tree, UI-E2's mate tool and UI-E3's diagnostics
panel). `git merge github/wave/ui-e` reports "Already up to date." — an empty
merge, stated as such per the card. No rebase, no reset.
