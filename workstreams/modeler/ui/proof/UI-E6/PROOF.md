# UI-E6 UI-NAV-CUBE: 3D orientation cube beside the existing button panel - proof

Wave E card (out-of-band, Don requested 2026-11-26 after the wave-D screenshots).
Branch: `wave/ui-e` worktree `ui-e6`. Behaviour: a real 3D orientation cube in
the viewport's bottom-right corner beside the UI-D4 button panel, tracking the
camera live, with its click/drag-to-align interaction behind the
`nav_cube_click_to_align` setting (DEFAULT OFF for patent conservatism).

## Owned files (exactly the card's list)

- `crates/oep-app/src/settings.rs` (new) - the settings surface; `Settings` +
  `nav_cube_click_to_align` (default off), `read`/`write` over the egui context
  store, `NAV_CUBE_CLICK_TO_ALIGN_HELP` one-line help, `SETTINGS_KEY`.
- `crates/oep-app/src/panels/nav_cube.rs` (new) - the cube module: `NavTarget`
  (Face/Edge/Corner over the SHARED `CubeFace` table), `cube_camera`,
  `project_cube`, convex `face_corner_quads`, the analytic current-face oracle,
  the `PassCache` (re-rasterizes only when yaw/pitch/highlight/size change),
  the axis triad, `cube_rect`, `NavCubeScalars`, `update()`, 9 module tests.
- `crates/oep-app/src/panels/mod.rs` - registers `pub mod nav_cube;` and
  `#[path = "../settings.rs"] pub mod settings;`.
- `crates/oep-app/src/panels/nav.rs` - APPENDS one `nav_cube::update(...)` call
  at the end of `nav::update`. The button panel's behaviour is untouched.
- `crates/oep-app/tests/nav_cube.rs` (new) - the acceptance harness (drives the
  REAL `app::App` through `egui_kittest`).
- `crates/oep-app/tests/nav.rs` - NOT touched (see acceptance leg 6).
- `workstreams/modeler/ui/README.md` - this card's row.
- `workstreams/modeler/ui/proof/UI-E6/` - this proof + transcripts + capture
  script + screenshots.

`viewport.rs` is NOT touched: the cube is driven by the one `nav_cube::update`
call appended in `nav.rs`, drawn through the SAME renderer the viewport uses
(`oep_viewer::render::render` into a 96 px pass; a second small draw pass,
never a second scene submission), per the card.

## Legal posture (why the interaction ships behind a flag)

The interaction (click face/edge/corner to reorient, drag the cube to orbit
with snap) reads on Autodesk's US7782319B2 while it is in force. Per the
orchestrator addendum (Don's in-session decision, verbatim "just make it not
selectable by default and we'll make it selectable in a few months"), the cube
ships as a live orientation indicator with `nav_cube_click_to_align` DEFAULT
OFF, and the interactive path is fully implemented and tested behind the flag.
The patent and decision are recorded in the vault at `Topics/Isolith.md`,
decision #24. The re-check and the flip are Don's for April 2027. Nothing in
this card enables the flag; its default is asserted by a module test
(`nav_cube_click_to_align_defaults_off`). The name "orientation cube" / "nav
cube" is used everywhere; the trademark "ViewCube" appears nowhere.

## Design notes (decisions made here, all conforming to the card)

- Shared face-name table: the cube composes its accessible labels and face
  names from the SAME `CubeFace`/view-name table the UI-D4 button panel uses
  (`oep_viewer::views::CubeFace`, `label()`/`view()`); the button panel
  composes "Cube <face>" from the same base names. No second copy of the view
  names exists; supplied labels ("Orientation cube ..."/"Orientation cube
  faces: ...") stay unique in the accessibility tree so no existing
  `get_by_label` in `tests/nav.rs` collides.
- The interaction is a widget handled LAST in the frame on the viewport layer
  (`ui.interact` at the cube rect, `Sense::click_and_drag()`), so while the
  flag is ON a press on the cube is claimed by the cube (the viewport's
  datum-plane pick and pick surface never see it: no accidental plane swing),
  and while the flag is OFF nothing is claimed and the pointer passes straight
  through to the viewport orbit/pick lanes (the card's "must not swallow orbit
  or pick" contract). The cube itself does NOT orbit from its own drag: the
  viewport's single raw-input orbit lane moves the camera during a flag-on
  cube drag (one 1x orbit, never doubled; the cube contributes click-to-align
  plus the drag-release snap). The a11y labels live in a separate ALWAYS
  non-interactive `Area` at the cube rect.
- Orientation math is f64 end-to-end in the projection/orientation path so the
  1e-9 quaternion assertions are achievable. The camera basis is built exactly
  as the orbit camera builds it (look direction `f`, right `f x up`, up
  `s x f`); the target quaternion is extracted from the proper rotation
  `(s, u, -f)` (right, up, back), the SAME view basis the renderer uses, so
  every committed view is a genuine rotation (this fixed a first-draft
  left-handed-reflection matrix that collapsed several 45-degree targets onto
  Front; see the git history of `look_quaternion`).
- PassCache: `(yaw, pitch, pass_px, highlight_face)` is the dirty key; the
  resident six-quad scene is re-rasterized only when it changes and the cached
  texture is re-shown in steady state ("with zero cube work").
- Cross-include discipline: context-store values crossing the app and test
  compilations are plain std scalars/tuples/Strings only (`NavCubeScalars`,
  the settings `bool`) - the same pattern `NavSnapshot` established; a typed
  struct gets a different `TypeId` across the two compilations.
- Placement: `cube_rect` is right-aligned in the viewport, inset `CUBE_INSET`
  (8 px), sized 96 px at scale 1.0 (DPI-scaled), sitting directly above the
  D4 nav card and CLEAR of the D2 shading card's committed rect (top-right).
  Proven by the module test `cube_rect_is_clear_of_shading_card` and by the
  layout probe in the kittest runs; the 1600x1000 screenshots show it landed.

## Acceptance legs (all real command output; per-target numbers printed)

Transcripts of the printed acceptance numbers:
`transcripts/20-acceptance-numbers.txt` (analytic table + flag-on click table +
flag-off numbers + a11y line). Full suite: `transcripts/21-nav-cube-full.txt`.

### 1. Analytic current-face marking (6 standard views + 4 isometric corners)

`current_face_matches_literal_standard_views` and
`current_face_at_four_isometric_corners` print yaw/pitch/current/expected for
every case; all `match=true`. Representative:

```
analytic: view=Front  yaw=0.000000000000       pitch=0.000000000000  current=Front  expected=Front  match=true
analytic: view=Back   yaw=3.141592653590       pitch=0.000000000000  current=Back   expected=Back   match=true
analytic: view=Top    yaw=0.000000000000       pitch=1.540000000000  current=Top    expected=Top    match=true
analytic-corner: dir=(0.577350, 0.577350, 0.577350)  yaw=0.785398163397  pitch=0.615479708670  current=Top     expected=Top     match=true
analytic-corner: dir=(-0.577350, -0.577350, 0.577350) yaw=-0.785398163397 pitch=-0.615479708670 current=Bottom  expected=Bottom  match=true
```

The kittest leg (`cube_exposes_face_names_and_orientation_readout`) drives the
real button panel and reads the live cube report: the current-orientation
readout changes when a quick view button lands the camera (numbers in the
transcript).

### 2. Flag-off pass-through: no swallowed orbit or pick

`flag_off_drag_inside_cube_reaches_viewport_orbit` (flag OFF, the default)
drags 60 px across the cube rect; the CAMERA orbits by exactly that delta times
`ORBIT_RAD_PER_PX` = 0.005:

```
flag-off: drag [878.0 472.0] -> [938.0 472.0] orbited yaw 0.700000000000 -> 1.000000000000 (delta 0.300000000000 == 60 * 0.005)
flag-off: click at cube [908.0 472.0] passed through (camera pose unchanged), report.click_to_align=false
```

A clean click on the cube passes through untouched (pose unchanged, flag
reported false): the cube claims nothing while the flag is off.

### 3. Flag-on click: each face, edge and corner lands the documented view to 1e-9

`flag_on_click_each_face_edge_corner_lands_documented_orientation` clicks all
26 targets (6 faces, 12 edges, 8 corners), each from its own nudged base pose,
and asserts the landed camera quaternion equals the target's documented
orientation within 1e-9 (the UI-D4 assertion style). Every one prints
`match=true`:

```
flag-on click: target=Front       pos=(902.2,468.3) landed=(0.000000000000,0.000000000000)          quat=(1.000000000000,0.000000000000,-0.000000000000,-0.000000000000) match=true
flag-on click: target=Front-Right pos=(899.7,466.7) landed=(0.785398163397,0.000000000000)          quat=(0.923879532511,0.000000000000,0.382683432365,0.000000000000) match=true
flag-on click: target=Front-Top   pos=(902.1,466.3) landed=(0.000000000000,0.785398163397)          quat=(0.923879532511,-0.382683432365,-0.000000000000,-0.000000000000) match=true
flag-on click: target=Back-Left-Top pos=(899.7,465.0) landed=(-2.356194490192,0.615479708670)       quat=(-0.364705199631,0.115916895959,0.880476239217,0.279848142333) match=true
```

All 26 in the transcript; the pure self-resolution leg
(`each_target_resolves_at_its_own_pose`) proves each target resolves at its own
projected point at a general pose, so the kittest leg is the end-to-end click
proof.

### 4. Flag-on drag with snap

`flag_on_drag_orbits_and_release_snaps` drags the cube (press inside, move,
release); the camera moves during the drag and on release snaps onto the
nearest of the 26 documented orientations:

```
flag-on drag: id pos [898.0 462.0] -> [922.0 480.0], snapped to Front-Right-Top at angle 0.000000 rad; landed=(0.785398163397,0.615479708670)
```

### 5. Reachable without a mouse (a11y)

The cube exposes accessible labels for the six face names (from the SHARED face
table) and the current-orientation readout; they stay unique against the
existing nav suite:

```
a11y: orientation cube labels present; nav labels stay singular
```

### 6. Existing nav suite and the rest of the workspace

`crates/oep-app/tests/nav.rs` is UNTOUCHED (git status: no change). Its test
binary reports 142 passed / 0 failed on the merged wave tree (incl. UI-E1) =
the UI-D4 122 nav assertions PLUS the module tests that now also compile
inside every `mod app` binary (9 nav_cube + 3 settings + the modules UI-E1
adds; the card requires "the existing 122 assertions must all still pass",
which they do). Full `cargo test --workspace`: 227 suites, 5365 passed / 0
failed (transcript `30-workspace.txt`).

## Flagships: deterministic 1600x1000 screenshots

`capture_e6.sh` runs the REAL merged-branch binary under `xvfb-run -s
"-screen 0 1600x1000x24"` via the XTEST injector (`ui_e6_inject.c`, the same
injector UI-D6 shipped). The 1280x800 app window maps at the origin of the
1600x1000 frame (no WM, NorthWest gravity), so the UI-D4 layout probe
coordinates are valid and the captured frame is 1600x1000 as the card requires.
Two independent launches per mode MUST be byte-identical or the script exits
non-zero; the `.sha256` is the determinism oracle.

Mode `standard`: clicks the D4 "Front" quick button (center 749,665 from the
UI-D4 05-layout-probe) so the camera lands the exact Front view and the cube's
current face is Front. Mode `iso`: clicks "Iso" (809,688) so the camera lands
the exact isometric view (yaw 0.7854, pitch 0.6155) and the cube shows its
isometric readout. The cube region in the two frames is pixel-diffed and
DIFFERENT (standard: green Front face dominant; iso: three faces with the
highlighted current face), proving the cube tracks the camera live.

| file | sha256 |
|---|---|
| `standard.png` | `c599196c0d39e84d05fbfe4074a102bd818cdd4edcf0e877eec29e75bd738add` |
| `iso.png` | `9db64a9ef3d2e5d9c9eb023515b9a103c77d7dfb02d8c4f5b6ada7d547e71964` |

Both byte-identical across two independent launches (the capture script hard
fails otherwise); pixel scan of the cube region shows the rendered cube, face
labels, and the triad, and confirms the two modes differ (standard: the single
highlighted front face; iso: three faces with the highlighted current face).

The captures are made from the FINAL committed binary (the merged tree incl.
UI-E1). The first captures were made before the UI-E1 merge and their sha256
drifted when UI-E1's shell landing (assembly-tree panel) repainted the frame;
the committed frames + oracles above are the final merged-tree ones, re-created
and byte-identical across two launches each.

## Gates

| Leg | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) - transcript `gate-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings --keep-going` | PASS (exit 0) - transcript `gate-clippy.txt` |
| `cargo test --workspace` | PASS (exit 0) - 227 suites, 5365 passed / 0 failed - transcript `30-workspace.txt` (run with `-j 4`; see the flake note below) |
| `python3 scripts/licensing-check.py` | PASS (exit 0) - 6/6 - transcript `gate-licensing.txt` |
| `bash scripts/gate/precommit-gate.sh` | PASS - pytest 558 passed, 29 skipped; cargo test green - transcript `10-precommit.txt` (see the flake note below) |
| em-dash text guard (src AND tests, this card's files) | PASS - no em dash or en dash character in any new/edited Rust file (`settings.rs`, `nav_cube.rs`, `nav.rs`, `mod.rs`, `tests/nav_cube.rs`, `tests/nav.rs`; `grep -c` for both characters = 0), see the scan below |

### Precommit gate: environment + the documented kittest teardown flake

The precommit gate's pytest leg needed a pytest-capable interpreter: this host's
default `python3` (Hermes 3.14, no pip) and system `python3.12` (no pytest in
the env) both lack it, so the first gate runs hit `[FAIL] pytest - No module
named pytest`. The gate was re-run with `VIRTUAL_ENV=/tmp/gatevenv` (a
throwaway `python3.12` venv with `pytest 9.2.0`), giving `558 passed, 29
skipped`. The cargo-test leg also tripped the documented egui_kittest
exit-time SIGSEGV flake (`layout`/`pick_gpu`, signal 11 at process teardown
under the full parallel suite; the SAME class UI-D7/D4/D6/E1 recorded). Both
binaries pass isolated (layout 142/0, pick_gpu 143/0 on the merged tree), and
the definitive `cargo test --workspace` on this very tree passes with `-j 4`
(bounded parallelism under the flake's load threshold): 227 suites / 5365
passed / 0 failed (transcript `30-workspace.txt`). These are
environmental/known-flake, no test weakened or skipped.

### Perf

`bash scripts/ui/perf.sh`: p95 6.672 ms vs the committed 16.6 ms threshold
(wave-D reference 6.408/6.449, i.e. within noise), 1,002,528 triangles at
1600x1000 on the real NVIDIA GB10, min fps 135.9 / mean 155.4, steady-state
upload guard (64 B/frame) PASS inside the heavy leg, app smoke
`frames=8 smoke ok`. The cube is NOT on that measured path (the perf harness
drives `GpuBackend` at the seam with the 1M-triangle grid scene, not the app
shell); in the app the cube is one extra 96 px textured draw in steady state
with zero cube rasterization work (PassCache). Transcript `11-perf.txt`.

## Module tests (the pure half of the acceptance, in `nav_cube.rs`)

9 module tests: edge/corner table geometric soundness, analytic 45-degree and
isometric directions, camera-quaternion == target-orientation to 1e-9 for all
26 targets, current-face literal standard views, current-face at the four
isometric corners, self-resolution at each target's own point, projection
flush and centred, each-target-resolves-at-its-own-pose, and the shading-card
clearance. Plus 3 settings tests (default off, help text, store round trip).

```
cargo test -p oep-app --test nav_cube -> 155 passed; 0 failed  (transcript 21-nav-cube-full.txt)
```
(155 on the merged wave tree: the 9 nav_cube module tests + 3 settings tests +
the kittest acceptance, plus the module tests UI-E1's panels add to every
`mod app` binary.)
