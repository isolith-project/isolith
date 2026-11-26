# UI-D4 UI-NAV: view cube, standard views, section planes — proof

Wave D card. Behaviour: an orientation cube that lands the camera on an exact
standard view when a face is clicked, four quick standard-view buttons, and a
section-plane slider that clips the model predictably. Landed on
`ui/d4-gpu-nav` (branch head after STEP 0 merge + this card's commits), merged
to `github/wave/ui-d` as `merge-UI-D4`.

## STEP 0

`git fetch github && git merge github/wave/ui-d` ran first from the worktree.
It brought the wave head (10 commits: UI-D1/D2/D3/D7 and their merges) with no
conflicts — merged as `8a9395e`. The D7 renderer correction (clip_w world-space,
empty-face guard) is in, so every standard view renders (Back included, 0px /
panic before D7).

## Owned files

- `crates/oep-viewer/src/views.rs` (new) — standard-view math (`StandardView`
  yaw/pitch, analytic `eye_for`), cube-face ↔ view mapping + round-trip hit
  test (`CubeFace::from_direction`), section-plane presets
  (`SectionAxis`, `section_plane`) and the analytic + mechanical
  visible-triangle counts (`section_triangle_count`,
  `visible_triangle_count`).
- `crates/oep-viewer/src/lib.rs` (ADDITIVE) — `pub mod views;` + re-exports.
- `crates/oep-app/src/panels/nav.rs` (new) — the nav card (cube gizmo, quick
  views, section slider) `#[path]`-included from `viewport.rs`, one
  `nav::update` call after the pick surface.
- `crates/oep-app/src/panels/viewport.rs` (ADDITIVE) — `#[path] mod nav;` +
  one `nav::update(ui, rect, scale, app)` call.
- `crates/oep-app/tests/nav.rs` (new) — the acceptance harness.
- `workstreams/modeler/ui/proof/UI-D4/` — this proof.
- `workstreams/modeler/ui/README.md` — this card's row.

The card's own camera math (`views.rs`) is unchanged since attempt 1 (40/40
oep-viewer tests green, then and now). This run fixed the two UI-D4 layout
defects that made the acceptance red.

## The gizmo defect (orchestrator-assigned) and its fix

Measured on the merged tree before the fix (live kittest rect dump): the six
face buttons were 40x20 cells on a 24 px pitch but the wrapped "Cube <face>"
labels auto-grew them to ~49-64 px tall, so the Front/Right/Top/Bottom click
rects overlapped the Back cell and a kittest click on "Cube Front" resolved to
yaw = pi (Back) — the orchestrator's measured defect. Fix in `nav.rs`:
- face labels are an explicit 10 px `RichText` with `TextWrapMode::Extend`
  (single line, can never wrap), `min_size(72x20)`;
- face cells widened to 72 px on a 76x24 grid so the full label fits one line;
- the card is drawn as one `egui::Area` fixed at the viewport corner whose
  widgets flow from the card's content (the D2 `shading.rs` idiom), so egui
  sizes the Area to exactly the card.

After the fix (transcripts/05-layout-probe.txt): every face's click rect is
72x20 (Bottom 75.6x20 — the Extend label's natural width, still one line and
20 tall), and the probe asserts `cube-face center overlaps: 0` — no face's
rect contains another face's center.

## Two collateral layout defects this run found and fixed

Both were in `nav.rs` (owned). Each reproduced with no ambiguity and each was
the cause of a previously-failing unrelated test:

1. The attempt-1 Area used `fixed_pos(Pos2::ZERO)` plus absolute `ui.put`.
   egui auto-sizes an area from its fixed origin to its content's far corner,
   so the nav Area spanned the whole top third of the screen and its
   interactable background captured the UI-B8 "Export STEP" button
   (top-left): `--test feature_from_sketch` failed `ui_step_export...` (the
   file was never written). The anchored + flow-layout card fixes it
   (`feature_from_sketch` 181/181).
2. A nav card at the viewport's top-right overlapped the D2 shading card
   (`gpu_shading` failed 4-5 tests: shading clicks landed on the nav card), and
   a card "below the shading card" overlapped the default-camera sketch plane,
   which the acceptance projects to x 788..912, y 258..333 (measured)
   (`feature_from_sketch` failed 5 dialog tests: the rectangle corner clicks
   landed on the nav card). The card is therefore anchored in the viewport's
   bottom-right corner, free of every other wave surface: D2 shading (top-right
   card), the sketch plane / sketch entry point (upper-right for the default
   camera), the B8 finish/export button (top-left), the C5 measure surface
   (bottom-left), and the feature dialog (centre). Both suites green: `--test
   feature_from_sketch` 181/181, `--test gpu_shading` 125/125.

## Acceptance (committed tree)

```
cargo test -p oep-app --test nav        -> 122 passed; 0 failed  (transcripts/01-nav-test.txt)
xvfb-run -a cargo run -p oep-app -- --smoke -> frames=8 smoke ok, exit 0  (transcripts/04-smoke.txt)
```
### 1. Each cube face lands the camera on the exact standard view

Printed by the acceptance (eye/target/up asserted to `expected_eye` within
1e-9, yaw/pitch asserted to the committed values within 1e-9, target and
distance unchanged; every one prints equal). All six:

```
face: clicked "Cube Front"  -> eye=(40.000000000000, 20.000000000000,  146.108800206649) target=(40,20,5) up=(0,1,0) | yaw=0                  pitch=0
face: clicked "Cube Back"   -> eye=(40.000000000000, 20.000000000000, -136.108800206649) target=(40,20,5) up=(0,1,0) | yaw=3.141592653590   pitch=0
face: clicked "Cube Left"   -> eye=(-101.108800206649,20.000000000000,    5.000000000000) target=(40,20,5) up=(0,1,0) | yaw=-1.570796326795  pitch=0
face: clicked "Cube Right"  -> eye=(181.108800206649, 20.000000000000,    5.000000000000) target=(40,20,5) up=(0,1,0) | yaw=1.570796326795   pitch=0
face: clicked "Cube Top"    -> eye=(40.000000000000, 161.041890732293,    9.344945847739) target=(40,20,5) up=(0,1,0) | yaw=0                  pitch=1.54
face: clicked "Cube Bottom" -> eye=(40.000000000000, -121.041890732293,   9.344945847739) target=(40,20,5) up=(0,1,0) | yaw=0                  pitch=-1.54
```

### 2. The quick Front / Top / Right / Iso buttons land exact standard views

Same assertions numerically; each prints equal to its analytic `expected_eye`:

```
button: clicked "Front" -> eye=(40.000000000000, 20.000000000000, 146.108800206649) | yaw=0  pitch=0
button: clicked "Top"   -> eye=(40.000000000000, 161.041890732293, 9.344945847739) | yaw=0  pitch=1.54
button: clicked "Right" -> eye=(181.108800206649, 20.000000000000, 5.000000000000) | yaw=1.570796326795  pitch=0
button: clicked "Iso"   -> eye=(121.469203784334, 101.469203784334, 86.469203784334) | yaw=0.785398163397  pitch=0.615479708670
```

### 3. Section slider: three committed positions, six printed numbers

The slider is placed via the AccessKit `SetValue` action (no mouse). Measured
(material-clip emulation) == analytic (closed form) == committed literal, and
non-increasing as the cut deepens:

```
section: fraction=1.000 plane_offset=10.000 visible_triangles=12 analytic_triangles=12
section: fraction=0.500 plane_offset=5.000  visible_triangles=10 analytic_triangles=10
section: fraction=0.000 plane_offset=0.000  visible_triangles=2  analytic_triangles=2
section sequence (fractions 1.0 -> 0.5 -> 0.0): visible [12, 10, 2] analytic [12, 10, 2]
```

### 4. Reachable without a mouse — accessible names

The harness finds every nav control by label in the AccessKit tree (11/11
printed): `Cube Front / Cube Back / Cube Left / Cube Right / Cube Top / Cube
Bottom`, `Front / Top / Right / Iso`, and the `Section` slider (driven by its
SetValue action, see leg 3).

## Layout evidence

transcripts/05-layout-probe.txt: every face 72x20, quick buttons 52x20,
Section slider 100x20, `cube-face center overlaps: 0`. The cube-face cells sit
on the 76x24 grid so no two faces overlap (the exact hazard the D7 mission
measured).

Real-binary screenshots (captured from DISPLAY :1 via `xwd`, decoded with PIL,
two images):

| file | sha256 |
|---|---|
| `ui-d4-nav-app-window.png` | `a1d1599555932de02a2e6d1dd233597140b00bdb742a91a19d02a608dc916d31` |
| `ui-d4-nav-cube.png` | `be6550530bbf8d7c943b317ab8dfaf03b64894f8ffab195996113e479138a524` |

`ui-d4-nav-cube.png` shows the light-theme "Orientation" card at the viewport's
bottom-right: the six face buttons in the classic cross/diamond arrangement
(Cube Top / Left / Front / Right / Back / Bottom), the 2x2 quick cluster
(Front / Top / Right / Iso), the "Section" slider and the "Triangles: 12"
readout — every label fully visible, no overlap, no clipping. The shading card
sits separately at the viewport top-right.

## Gates on the committed tree

- `cargo test -p oep-app --test nav` — 122 passed / 0 failed (01-nav-test).
- `xvfb-run -a cargo run -p oep-app -- --smoke` — `frames=8 smoke ok`, exit 0,
  renderer=wgpu viewport-backend=gpu (04).
- `cargo test -p oep-app` — 24 binaries, 3334 passed / 0 failed, exit 0 (03).
- `cargo test --workspace` — 4597 passed / 0 failed, exit 0 (06).
- `cargo test -p oep-viewer` — green (incl. views.rs 40+ tests).
- `cargo clippy -p oep-app --all-targets -- -D warnings` — clean.
- `rustfmt --check crates/oep-app/src/panels/nav.rs` — clean. (The only
  repo-wide `cargo fmt --check` complaint remains D3's `pick_gpu.rs`
  pre-existing const-block formatting, outside this card's owned list.)
- The wave head is inherently flaky under full parallel load on this host
  (the same class UI-D7 recorded as "layout-flake isolated runs"): in this
  run `layout` and `pick_gpu` each flaked once inside the big `--workspace` /
  `-p oep-app` failure paths and both pass 122/123+ isolated (see transcripts
  07-layout-isolated, 08-pickgpu-isolated). The final full `--workspace` run
  (06) was a single clean pass, 4597/0.

Target note: this worktree's `target/` is deleted after the merge read-back
(move window), per the DISK rule; it was 26 GB.
