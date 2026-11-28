# UI-F2 UI-VIEWS: place base, projected, section and detail views — proof

Wave F card. Views are placed on a sheet by dragging: base, projected, section
and detail - and projected views stay aligned with their base view (move by
exactly the same delta when the base is dragged).

## Files owned by this card

- `crates/oep-app/src/panels/views.rs` (new: the view placement tools - base /
  projected / section / detail placement, the draggable base chip, derived-view
  alignment, the sheet-preview canvas, the hatch + detail geometry, and the
  `ui_f2_views_report` oracle)
- `crates/oep-app/src/panels/sheet.rs` (ADDITIVE: one `#[path]` include of
  `views.rs` next to the existing `doc_io` include, and one `views::show(...)`
  call at the end of the drafting scroll-area content. The F1 sheet controls'
  node rects are unchanged - verified: F1 `--test sheet` still 159 passed.)
- `crates/oep-app/tests/views.rs` (new: acceptance harness, drives the REAL
  `app::App` through egui_kittest with the E2-precedent tall window
  `[1280.0, 1800.0]`)
- `workstreams/modeler/ui/proof/UI-F2/` (this proof)
- `workstreams/modeler/ui/README.md` (row updated in the same commit)

The panel only places and formats. Geometry comes from the EXISTING drafting
seam: `oep_drafting::view::project_edges` + `extent` and
`oep_drafting::layout::layout_views` (projected offsets re-derived, not
hand-rolled), `oep_drafting::solid::solid_from_graph` (section of the committed
bracket), and `oep_drafting::builder::drive_bracket_drawing` (fixture). There
is no second projection path.

## Acceptance

```
cargo test -p oep-app --test views            -> green (4/4, 158 through the
                                                 #[path] include)
cargo test -p oep-app --test sheet            -> green (159 passed, F1 nodes untouched)
xvfb-run -a cargo run -p oep-app -- --smoke   -> frames=8 smoke ok, exit 0
```

Transcripts: `transcripts/01-views-acceptance.txt`, `transcripts/02-smoke.txt`.

### 1. Base + projected: axes stay aligned (within 1e-6, both printed)

Place `base front`, then `projected top` on the committed sheet. The projected
top's offset is re-derived from `oep_drafting::layout::layout_views` (a TOP
sits directly above a FRONT base, shared `cx`). The acceptance reads the oracle
and prints both coordinates and their difference:

```
ALIGN: base_cx = 148.5  proj_cx = 148.5
```

`align_diff = 0.000e0` (oracle field), asserted `< 1e-6`. A module test
(`projected_top_offset_is_x_aligned_from_layout_seam`) re-derives the offset
from the seam so it can never drift.

### 2. Dragging the base moves the projected view by exactly the same delta

A real pointer drag is driven through the harness on the base chip (`Sense::drag`
+ `drag_delta`, chip id via `egui::Id::unique`, the E2 mate-tool idiom; the chip
is found by its stable AccessKit label `drag base oep-view-base-1`). The base
chip is dragged 80px right + 30px down. Every derived view's live position is
computed EACH FRAME as `base.center + offset` (never stored independently), so
the projected view moves with the base by exactly the same paper delta:

```
DRAG DELTA: base moved (141.414141414, -53.030303030) mm, projected moved (141.414141414, -53.030303030) mm
```

`assert |base - proj| < 1e-6` for both components, `base` moved non-zero, and
the alignment coordinate still equal after the drag.

### 3. Section view of the committed bracket shows hatching

`Place section` cuts the committed bracket solid with the X-axis-aligned plane
at `SECTION_AT_MM = 40.0` (mid-length, via `solid_from_graph`), computes the
real cross-section polygon from the mesh triangles, and hatches it at 45° with
spacing 2 mm. The generated hatch-segment count is published next to an
INDEPENDENT closed-form analytic count for the same `(w, h)` region:

```
SECTION HATCH: rendered = 17  analytic = 17
section region (mm) = 40x6
```

`assert rendered > 0 && rendered == analytic`. A module test
(`section_hatch_is_nonzero_and_matches_analytic`) locks the same contract.

### 4. Detail view magnifies at exactly the requested scale

`Place detail` magnifies the base view's projected region at
`DETAIL_SCALE = 0.5`; the rendered paper extents (`model / scale`) are compared
with the analytic extents and asserted equal:

```
DETAIL: requested scale=0.5 rendered=(160, 12) analytic=(160, 12)
```

(front of the bracket is 80 x 6 model mm; at scale 0.5 the paper extent is
160 x 12 mm). A module test (`detail_extents_match_analytic_at_requested_scale`)
locks the same invariant.

## Guards and full gates

- Mutation guard green: every placed view journals through the routed
  `doc_io` seam token `draft:place` (`PLACE_TOKEN`); no direct document write
  appears in `src/` (`transcripts/01-views-acceptance.txt`).
- clippy `-D warnings` (all targets, workspace): exit 0 - `transcripts/04-clippy.txt`.
- fmt `--check`: exit 0 - `transcripts/03-fmt.txt`.
- licensing 6/6 (34 crates / 535 deps, unchanged - `oep-drafting` was already
  a dependency from F1, `Cargo.lock` untouched): exit 0 - `transcripts/05-licensing.txt`.
- `cargo test --workspace`: **232 suites / 6558 passed / 0 failed**, exit 0 -
  `transcripts/06-workspace.txt`. The documented egui_kittest teardown SIGSEGV
  under full parallel load (a pre-existing flake, see UI-F1/D4 metadata) hit
  `measure` during capture; it passes standalone (167/0) and the clean run is
  green at `-j 2`.
- smoke: `frames=8 smoke ok`, exit 0 - `transcripts/02-smoke.txt`.

## Screenshot (deterministic: two independent captures byte-identical)

- `views.png` - the real merged-head binary at the default 1280x800 window.
  The Views placement tools sit BELOW the drafting fold inside the drafting
  scroll area (F1 content fills the region; the probe locked F1 bottom=733,
  region bottom=774, 41pt slack - so the F1 layout is untouched and the views
  section is reached by scrolling, exactly like the E-wave "list" capture).
  The capture wheel-scrolls the drafting scroll area to its bottom clamp, then
  clicks the four placement tools (base front, projected top, section, detail)
  at their post-scroll screen coordinates (derived from a same-code kittest
  probe at the real 1280x800 window). The frame shows the placed views inside
  the sheet-preview canvas: the draggable accent `base front` chip, the
  `projected top` chip aligned above it, the hatched section outline (the 45°
  hatch lines at 2 mm spacing, 17 lines) and the `detail` chip - the canvas
  auto-fits the full sheet so every chip is visible.

sha256 in `views.png.sha256`:

```
c4ad34d66932cbc59ac4821e675a5908652f117a847f402036924aca76ca61fc
```

Captured via `capture_f2.sh` (drives the real `isolith` binary under xvfb
through the XTEST injector `ui_f2_inject.c`, which supports move/click/wheel),
two independent launches, byte-identical.
