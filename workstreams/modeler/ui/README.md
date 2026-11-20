# Isolith modeler UI workstream (editor parity program, wave A)

One row per wave-A card. Proof lives under `proof/<card>/`. Acceptance for each
card is the real command output captured in its proof file, never a summary.

| Card | Title | Behaviour | Status | Proof |
|---|---|---|---|---|
| UI-A1 | UI-DEPS-BASELINE: add eframe/egui 0.37, create `crates/oep-app` | `isolith` binary opens one 1280x800 window whose title bar names product and version and exits cleanly on close; `--smoke` runs a bounded number of frames headless and exits 0 | done | `proof/ui-a1/` |
| UI-A3 | UI-THEME: `oep-ui-theme` semantic tokens, light default | Semantic tokens live in `crates/oep-ui-theme` (surface, surface_raised, text_primary, text_muted, accent, border, focus_ring, danger; light + dark value each); app installs them as the egui `Visuals` at startup with light default; contrast proven 4.5:1 text / 3:1 non-text in both modes; end-to-end app test falls apart on raw hex colours | done | `proof/ui-a3/` |
