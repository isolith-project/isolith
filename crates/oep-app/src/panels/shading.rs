//! Shading panel (wave D, card UI-D2): runtime view-mode selector +
//! transparency slider + oracle, overlaid on the viewport.
//!
//! The GPU viewport's shading is runtime-selectable — the five proven states
//! (Shaded / Edges / Flat / Smooth / 50% transparent) are each reachable from
//! this card:
//!
//! - a **view-mode selector** (four `selectable_value` buttons, accessible via
//!   their labels in the kittest harness) setting `ViewMode`;
//! - a **transparency slider** (`0..=1`, shown as a percentage) setting the
//!   alpha; 50% is the accepted transparent state;
//! - an **oracle**: a plain `String` report published under
//!   [`SHADING_ORACLE_KEY`] (same cross-`#[path]`-safe pattern as the measure
//!   panel) so the acceptance harness can read the live selection back.
//!
//! Wiring: `viewport.rs` includes this module via `#[path]` and calls
//! [`draw`] once, exactly like `features::draw` / `measure::draw`. The card
//! writes its chosen state into `oep_viewer::gpu::shading` (the process-wide
//! state the GPU backend reads at every render) and, when the selection
//! changes, forces the shared viewport backend to re-render by calling its
//! existing `set_camera(camera())` — a no-op camera move through the seam
//! trait that triggers a fresh `render_frame` (the seam owns the backend in
//! the same egui store `viewport.rs` uses, keyed by `VIEWPORT_STATE_KEY`).
//!
//! The card draws only token colours (no raw hex: the UI-A3 guard scans
//! `src/`), paints no document state (the UI-B1 mutation guard is a
//! no-op here), and consumes no keys, so the viewport / palette / sketch
//! panels are untouched.

use eframe::egui;

use oep_viewer::gpu::shading::{set_state as shading_set, ShadingState, ViewMode};

/// Context-data key under which the panel stores its own UI state (the last
/// selection the user made; falls back to the GPU backend's current state).
pub const SHADING_STATE_KEY: &str = "isolith.ui.shading.panel";
/// Context-data key under which the panel publishes its `String` oracle every
/// frame: `mode=<label>\nalpha=<0..1>`.
pub const SHADING_ORACLE_KEY: &str = "isolith.ui.shading.oracle";

/// The panel UI state: the selected view mode and the transparency `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadingPanelState {
    /// Selected view mode.
    pub mode: ViewMode,
    /// Transparency `0..=1` (`1.0` fully opaque).
    pub alpha: f32,
}

impl Default for ShadingPanelState {
    fn default() -> Self {
        // The studio default offered at startup: Shaded with edges, opaque.
        Self {
            mode: ViewMode::Shaded,
            alpha: 1.0,
        }
    }
}

impl From<ShadingPanelState> for ShadingState {
    fn from(p: ShadingPanelState) -> Self {
        ShadingState {
            mode: p.mode,
            alpha: p.alpha,
        }
    }
}

/// The rectangle the shading card occupies for a given viewport (top-right,
/// clear of the bottom-left measure card and the top feature/sketch surfaces).
///
/// `card` is in SCREEN coordinates (where the Area is anchored); every other
/// rect is LOCAL to the card (`card.min` as origin), so the `egui::Area` is
/// sized exactly to the card and never intercepts clicks outside it (egui
/// Areas with `fixed_pos(Pos2::ZERO)` span from the origin to their widget
/// extent and would swallow the feature surface's clicks).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadingRects {
    /// The card background (screen coords).
    pub card: egui::Rect,
    /// The transparency slider (local to the card).
    pub slider: egui::Rect,
    /// One rect per view mode, in [`ViewMode::ALL`] order (local to card).
    pub mode_rows: [egui::Rect; 4],
}

/// Deterministic layout of the shading card inside the viewport. The card is
/// anchored top-right; the inner rects are offsets from `card.min`.
pub fn shading_rects(viewport: egui::Rect) -> ShadingRects {
    let w = 148.0;
    let row_h = 22.0;
    let card_h = 26.0 + 4.0 * row_h + 28.0; // heading + 4 rows + slider row
    let x = viewport.max.x - w - 8.0;
    let y = viewport.min.y + 8.0;
    let card = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, card_h));
    let mut mode_rows = [egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO); 4];
    for (i, slot) in mode_rows.iter_mut().enumerate() {
        let ry = 26.0 + i as f32 * row_h;
        *slot = egui::Rect::from_min_size(egui::pos2(6.0, ry), egui::vec2(w - 12.0, row_h - 2.0));
    }
    let slider =
        egui::Rect::from_min_size(egui::pos2(6.0, card_h - 26.0), egui::vec2(w - 12.0, 20.0));
    ShadingRects {
        card,
        slider,
        mode_rows,
    }
}

/// Read the panel UI state. Falls back to [`ShadingPanelState::default`]
/// (`Shaded`, opaque) when the user has not touched it yet, so the studio
/// look is what the app offers first.
fn read_state(ctx: &egui::Context) -> ShadingPanelState {
    ctx.data(|d| {
        d.get_temp::<ShadingPanelState>(egui::Id::unique(SHADING_STATE_KEY))
            .unwrap_or_default()
    })
}

/// Apply the panel selection to the runtime GPU backend.
///
/// The process-global `oep_viewer::gpu::shading` state is what the backend
/// renders each frame; this keeps it bound to the panel. Only when the
/// selection actually CHANGED do we also force the shared viewport backend to
/// re-render: a no-op camera move through the seam (`set_camera(camera())`)
/// triggers a fresh `render_frame` with the new state (the backend store is
/// `Arc<Mutex<Box<dyn ViewportBackend>>>` under the viewport's
/// `VIEWPORT_STATE_KEY`, created on the first viewport frame).
fn apply_state(ctx: &egui::Context, state: &ShadingPanelState) {
    let previous = oep_viewer::gpu::shading::state();
    let next: ShadingState = (*state).into();
    let changed = next != previous;
    shading_set(next);
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(SHADING_STATE_KEY), *state));
    if changed {
        ctx.data_mut(|d| {
            if let Some(arc) = d.get_temp::<std::sync::Arc<
                std::sync::Mutex<Box<dyn oep_viewer::ViewportBackend>>,
            >>(egui::Id::unique(super::VIEWPORT_STATE_KEY))
            {
                if let Ok(mut guard) = arc.lock() {
                    let cam = guard.camera();
                    guard.set_camera(cam);
                }
            }
        });
        ctx.request_repaint();
    }
}

/// Publish the `String` oracle (`mode=` / `alpha=`), same pattern as the
/// measure panel, so the kittest acceptance can read the live selection.
fn publish_oracle(ctx: &egui::Context, state: &ShadingPanelState) {
    let text = format!("mode={}\nalpha={:.2}\n", state.mode.label(), state.alpha);
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(SHADING_ORACLE_KEY), text);
    });
}

/// Draw the shading card over the viewport and apply any selection change to
/// the GPU backend. One call from `viewport.rs`, alongside `features::draw`
/// and `measure::draw`.
pub fn draw(ctx: &egui::Context, viewport: egui::Rect) {
    let mut state = read_state(ctx);
    let p = palette(ctx);
    let rects = shading_rects(viewport);

    // Anchor the card at the VIEWPORT's top-right-corner minus margin via
    // fixed_pos + normal sequential ui layout. egui sizes the Area to exactly
    // its content, so its click-interception rect is the card and nothing
    // else — anchored to the viewport, not the screen (the screen's right
    // edge is where the properties panel lives). (The naive
    // `fixed_pos(Pos2::ZERO)` + absolute `ui.put` combo makes the Area
    // auto-size to a huge rect that swallows other panels' clicks.)
    egui::Area::new(egui::Id::unique("isolith_shading_panel"))
        .order(egui::Order::Foreground)
        .fixed_pos(rects.card.min)
        .interactable(true)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(p.surface_raised)
                .corner_radius(egui::CornerRadius::same(4))
                .stroke(egui::Stroke::new(1.0, p.border));
            frame.show(ui, |ui| {
                ui.set_width(rects.card.width().max(140.0));
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Shading")
                            .size(12.0)
                            .color(p.text_primary),
                    );
                });

                for mode in ViewMode::ALL {
                    let selected = state.mode == mode;
                    let text = egui::RichText::new(mode.label()).size(11.0);
                    let mut button = egui::Button::new(text)
                        .min_size(egui::vec2(rects.card.width() - 10.0, 18.0));
                    if selected {
                        // No `RichText::strong` (the UI-A2 guard owns that):
                        // the selected row is shown by a filled button.
                        button = button.selected(false).fill(
                            oep_ui_theme::Palette::resolve(
                                if ctx.global_style().visuals.dark_mode {
                                    oep_ui_theme::Mode::Dark
                                } else {
                                    oep_ui_theme::Mode::Light
                                },
                            )
                            .accent,
                        );
                    }
                    if ui.add(button).clicked() {
                        state.mode = mode;
                    }
                }

                let mut alpha = state.alpha;
                let slider = egui::Slider::new(&mut alpha, 0.0..=1.0)
                    .text("transparency")
                    .show_value(false);
                if ui.add(slider).changed() {
                    state.alpha = (alpha * 100.0).round() / 100.0;
                }

                apply_state(ctx, &state);
                publish_oracle(ctx, &state);
            });
        });
}

/// Palette resolved for the installed theme mode (same derivation as the
/// sibling panels).
fn palette(ctx: &egui::Context) -> oep_ui_theme::Palette {
    use oep_ui_theme::Mode;
    let mode = if ctx.global_style().visuals.dark_mode {
        Mode::Dark
    } else {
        Mode::Light
    };
    oep_ui_theme::Palette::resolve(mode)
}
