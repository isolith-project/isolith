---
title: Keelworks P7-M5-T02 Accessibility Checklist (viewer and workbench surfaces)
version: 1.0.0
date: 2026-10-16
status: Ratified for the P7-M5-T02 headless surfaces
owner: Keelworks program governance, P7 modeler lane (P7-M5-T02)
applies_to: the oep-viewer surface (P6-M5-T02 preview viewer) and the oep-shell workbench command surface (P7-M2-T01), and the gated items that land when the P7-M5 immediate-mode interactive panel is delivered
supersedes: nothing
---

# Keelworks P7-M5-T02 Accessibility Checklist

## Scope and honesty note

Both surfaces named by the card are, at this milestone, headless and
keyboard/CLI-first: the viewer renders to pixels on demand and the workbench
is a typed command surface. "Accessibility" here means the surfaces are
discoverable, operable by text/keyboard alone, free of color-only-encoded
information, and rendered against a theme that clears WCAG contrast floors.
Interactive-GUI-only accessibility (focus indication, screen-reader DOM
integration, GUI zoom range, click-target sizes) cannot pass until the
immediate-mode UI panel exists; those items are listed as DEFERRED with their
landing gate rather than claimed early.

Status values: PASS (asserted by the named automated test on real material),
DEFERRED (gated on a named later card or surface), N-A (does not apply to
this headless surface).

## Viewer surface (oep-viewer)

| ID | Criterion | Status | Verified by |
| --- | --- | --- | --- |
| VW-A11Y-01 | Selection is accessible without vision: pick returns full structured identity (part id, body id, instance chain) and never requires reading pixels or colors. | PASS | crates/oep-viewer/tests/accessibility_theme.rs (structural_identity_is_not_color_only), on the committed golden bracket document |
| VW-A11Y-02 | No color-only encoding: identity and state live in structured data, never in pixel color; part color is a stable reference cue only. | PASS | crates/oep-viewer/tests/accessibility_theme.rs (structural_identity_is_not_color_only; Selection carries no color fields by construction) |
| VW-A11Y-03 | Photosensitivity and no auto-motion: render() is a pure deterministic single-frame function; no animation loop, no flashing, no auto-play. | PASS | crates/oep-viewer/tests/accessibility_theme.rs (render_is_deterministic_single_frame, byte-identical re-render) |
| VW-A11Y-04 | Non-text contrast (WCAG 1.4.11): every part palette color keeps at least 3.0:1 against the theme background; measured worst case is 3.4:1 (blue hue). | PASS | crates/oep-viewer/tests/accessibility_theme.rs (worst_part_contrast_meets_wcag_floor + every_part_color_across_hue_meets_the_floor) |
| VW-A11Y-05 | Predictable framing: the initial camera frames and centers the model bounds, so the first view is stable and predictable. | PASS | crates/oep-viewer/tests/viewer_contract.rs (framed_camera_projects_bracket_centroid_near_view_centre) |
| VW-A11Y-06 | Theme baseline is a single source: background and part colors come from oep_viewer::theme, and the renderer consumes those constants. | PASS | crates/oep-viewer/tests/accessibility_theme.rs (renderer_uses_theme_constants) + crates/oep-viewer/src/render.rs |
| VW-A11Y-07 | Interactive viewport accessibility (keyboard orbit/zoom/reset with visible focus, GUI zoom range, click-target sizes) | DEFERRED | gated on the P7-M5 immediate-mode UI card that first embeds the viewer in a windowed surface |
| VW-A11Y-08 | Shading is decorative only: depth shading dims faces but is never the semantic channel; identity is always the structured pick result, never a shade or color. The shading dim factor itself is not held to the contrast floor because it carries no information. | PASS | documented in crates/oep-viewer/src/render.rs shading note; identity channel asserted by VW-A11Y-01 |

## Workbench surface (oep-shell)

| ID | Criterion | Status | Verified by |
| --- | --- | --- | --- |
| WB-A11Y-01 | Every command is discoverable from help: `help` prints the full command index, and every dispatchable command (load:cgr, load:3dxml, load:step, open-pdm, save-pdm, save, journal, help) exists in the registry. | PASS | crates/oep-shell/tests/help_index.rs (registry_covers_every_dispatchable_command); tests/test_p7m5t02_a11y_help.py cross-checks the committed index against the metadata |
| WB-A11Y-02 | Per-command docs exist for every command and are GENERATED from command metadata (usage, arguments, description, examples); the committed index is byte-identical to a fresh render (drift-guarded). | PASS | crates/oep-shell/tests/help_index.rs (every_command_is_documented + help_index_generated_from_metadata_is_committed) |
| WB-A11Y-03 | No color-only encoding: help and CLI output are plain text with no ANSI color escapes; nothing is conveyed by color alone. | PASS | crates/oep-shell/tests/help_index.rs (rendered_help_has_no_ansi_color_escapes) |
| WB-A11Y-04 | Predictable syntax and actionable errors: consistent command:arg grammar; an unknown command fails with a message naming the token and pointing at `help`. | PASS | crates/oep-shell/tests/help_index.rs (unknown_command_error_points_to_help) + oep-shell/src/commands.rs validate_token |
| WB-A11Y-05 | Help is machine-readable: the index serializes to JSON covering every command, stable byte-for-byte, for assistive tooling and agent automation. | PASS | crates/oep-shell/tests/help_index.rs (json_index_covers_all_commands_and_is_stable) |
| WB-A11Y-06 | Help exit codes are defined and measured: `help` and `help:NAME` exit 0; an empty invocation and an unknown command exit 1 with guidance (measured on the built oep-workbench binary). | PASS | measured in the P7-M5-T02 RE note (oep-workbench binary exit codes captured on the rebuilt example) |
| WB-A11Y-07 | Keyboard/pointer independence: the entire surface is text-driven; no operation requires a pointer, a display, or a GUI (headless by construction, CI-safe). | PASS | headless by construction; exercised by the board's CI jobs (no display, no GUI deps) |
| WB-A11Y-08 | Interactive panel accessibility (focus ring, keyboard focus order, screen-reader integration, GUI contrast themes) | DEFERRED | gated on the P7-M5 immediate-mode UI card |

## Theme baseline (measured 2026-10-16)

The viewer theme is dark-neutral by default:

- Background: RGB (28, 30, 34).
- Part palette: per-part stable hue at saturation 0.65, value 1.0. The
  palette was tuned in this card from value 0.9 because the previous default
  dropped the blue hue to about 2.9:1 against the background, below the WCAG
  1.4.11 floor; the tuned palette measures a worst case of 3.4:1 (blue).
- No theme state is conveyed by color alone: selection identity is the
  structured pick result, and help output is plain text with no ANSI colors.

## Gated items (deferred with a named gate)

These items cannot legitimately pass on a headless surface, so they are
DEFERRED to the card that delivers the interactive surface rather than
claimed here: VW-A11Y-07 and WB-A11Y-08. The P7-M5-T02 checklist is re-run
at that gate with the interactive items asserted for real.

## Change log

- 1.0.0 (2026-10-16): Initial ratification for P7-M5-T02. Passes the
  headless-surface accessibility items on the viewer (oep-viewer) and
  workbench (oep-shell) surfaces with automated tests and a tuned theme that
  clears the WCAG 1.4.11 contrast floor, and records the interactive-only
  items as DEFERRED to the immediate-mode UI gate.
