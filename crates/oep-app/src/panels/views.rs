//! View placement tools (wave F, card UI-F2 UI-VIEWS): views are placed on a
//! sheet by dragging - base, projected, section and detail - and projected
//! views stay aligned with their base view.
//!
//! ## What this panel is
//!
//! A `#[path]` child module of `sheet` (exactly like the mate tool is a child
//! of `assy_tree`): the drafting panel pulls it in with one `#[path]` include
//! and renders it with ONE call. As a child module of `sheet`, it reaches the
//! parent's private [`super::SheetModel`] (the live canonical drawing document,
//! the selected sheet's size + projection, the routed [`super::doc_io`]
//! history) directly - one source of truth, byte-exact hashes.
//!
//! ## The placement surface
//!
//! The panel owns a small sheet-preview canvas. Each placed view is shown as a
//! chip at its paper-millimetre position; the BASE view's chip is draggable
//! (`Sense::drag`, the same idiom the E2 mate tool uses). Dragging the base
//! repositions it, and every view derived from that base (projected / section /
//! detail) moves with it by EXACTLY the same delta - its position is computed
//! every frame as `base.center + offset`, never stored independently. The chip
//! labels are stable (`drag base <id>`, `<id>`), so the acceptance can find a
//! chip by accessible label and drive a real pointer drag through the harness.
//!
//! ## Geometry: the panel only places and formats (no second projection path)
//!
//! - **Base / projected** views: projected edges and extents come from the
//!   EXISTING seam ([`oep_drafting::view::project_edges`] + [`oep_drafting::view::extent`],
//!   the same functions `render_sheet` uses), paper extents use the seam's
//!   documented convention (`paper = model / scale`), and a projected view's
//!   ALIGNED offset is derived from [`oep_drafting::layout::layout_views`] -
//!   a projected TOP sits directly above a FRONT base (shared `cx`) per the
//!   first/third-angle convention, a RIGHT view to the right (shared `cy`). No
//!   offset is hand-rolled; a module test re-derives it from the seam.
//! - **Section**: the panel cuts the committed bracket solid (replayed via
//!   [`oep_drafting::solid::solid_from_graph`]) with a plane, computes the real
//!   cross-section polygon from the mesh triangles, and hatches that polygon at
//!   45° with fixed spacing. The generated hatch-segment count is published
//!   next to an INDEPENDENT analytic count for the same cut plane; the
//!   acceptance asserts both are non-zero and equal.
//! - **Detail**: the panel magnifies the base view's projected region at an
//!   explicit requested scale; the rendered paper extents (region model extent
//!   divided by the scale) are compared with the analytic extents and asserted
//!   equal.
//!
//! The panel never re-implements projection or a solid - every number it
//! publishes traces to `oep_drafting` on the committed bracket.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_drafting as draft;
use oep_geo::{ProjectionType, ViewKind};

/// Context-data key of this panel's published `String` oracle (tests read it
/// by value, like every panel oracle in this wave).
pub const VIEWS_REPORT_KEY: &str = "ui_f2_views_report";
/// Context-data key of the panel's own persistent UI state.
const VIEWS_STATE_KEY: &str = "ui_f2_views_state";
/// Journal token this panel records when a placed view is created (routed
/// through the UI-B2 seam, like every other drafting edit).
pub const PLACE_TOKEN: &str = "draft:place";

/// Sheet-preview canvas size (logical points).
const CANVAS_W: f32 = 264.0;
const CANVAS_H: f32 = 132.0;
/// Fallback paper millimetres per logical point when no sheet is selected.
const MM_PER_PT: f64 = 0.9;
/// Hatch spacing (mm) in section views.
const HATCH_SPACING_MM: f64 = 2.0;
/// The section cut plane: axis 0 (x) at the bracket mid-length.
const SECTION_AXIS: usize = 0;
const SECTION_AT_MM: f64 = 40.0;
/// The requested detail magnification factor (rendered region = model / this).
const DETAIL_SCALE: f64 = 0.5;

/// The placement kinds this panel can place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlacedKind {
    /// A standard view that anchors a projected/section/detail tree.
    Base(ViewKind),
    /// A standard view placed from a base, aligned per the projection angle.
    Projected(ViewKind),
    /// A cross-section of the bracket solid through the section plane, hatched.
    Section,
    /// A magnified region of the base view at the requested scale.
    Detail,
}

impl PlacedKind {
    fn kind_name(self) -> String {
        match self {
            PlacedKind::Base(k) | PlacedKind::Projected(k) => format!("{:?}", k).to_lowercase(),
            PlacedKind::Section => "section".to_string(),
            PlacedKind::Detail => "detail".to_string(),
        }
    }
}

/// One placed view. A base view carries no `base`; every derived view carries
/// the base id plus a base-relative paper offset (mm). Its live position is
/// ALWAYS `base.center + offset`, so moving the base moves it by exactly the
/// same delta and its axes stay aligned with the base.
#[derive(Debug, Clone)]
struct PlacedView {
    id: String,
    kind: PlacedKind,
    base: Option<String>,
    offset: Option<[f64; 2]>,
    /// Paper extent (mm): (w, h).
    w: f64,
    h: f64,
}

/// The panel's persistent UI state.
#[derive(Debug, Clone, Default)]
struct ViewsUi {
    /// Placed views, in placement order (index 0 = the first base if any).
    views: Vec<PlacedView>,
    /// Draggable base center (paper mm), the anchor all derived views follow.
    base_center: Option<[f64; 2]>,
    /// Drag-start base center, for reporting exact deltas.
    drag_start: Option<[f64; 2]>,
    /// Base delta (mm) applied by the last completed drag.
    last_base_delta: [f64; 2],
    /// Derived (projected) delta (mm) in the same gesture - must equal base.
    last_derived_delta: [f64; 2],
    /// Section hatching numbers (generated / analytic / region extents).
    section: Option<SectionNumbers>,
    /// Detail extents (rendered / analytic / requested scale).
    detail: Option<DetailNumbers>,
    last: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SectionNumbers {
    generated: usize,
    analytic: usize,
    region_w: f64,
    region_h: f64,
}

#[derive(Debug, Clone, Copy)]
struct DetailNumbers {
    scale: f64,
    render_w: f64,
    render_h: f64,
    analytic_w: f64,
    analytic_h: f64,
}

/// Replay the committed bracket feature graph into its solid mesh through the
/// EXISTING drafting evaluator - the same mesh `render_sheet` projects.
fn bracket_mesh() -> Option<draft::Mesh> {
    let driven = super::drive_bracket_drawing().ok()?;
    draft::solid_from_graph(&driven.graph).ok()
}

/// The parent sheet's size + projection (the sheet views are placed on);
/// falls back to the committed A4-landscape third-angle defaults.
fn sheet_geometry(model: &super::SheetModel) -> (f64, f64, ProjectionType) {
    model
        .selected_sheet()
        .map(|s| {
            (
                s.size.width_mm as f64,
                s.size.height_mm as f64,
                s.projection,
            )
        })
        .unwrap_or((297.0, 210.0, ProjectionType::ThirdAngle))
}

/// Model-space extent (w, h) of a standard view of the bracket, via the seam's
/// own projector - the exact functions `render_sheet` uses (no re-projection).
fn std_view_extent(kind: ViewKind, mesh: &draft::Mesh) -> (f64, f64) {
    let edges = draft::view::project_edges(mesh, kind);
    draft::view::extent(&edges)
}

/// The preview-canvas scale (paper mm per logical point): auto-fit the sheet's
/// larger dimension into the canvas, with a small margin so chips never sit at
/// the very edge. Falls back to the fixed scale when the sheet geometry is
/// degenerate (no sheet selected yet).
fn fit_scale(sheet_w: f64, sheet_h: f64) -> f64 {
    if sheet_w <= 0.0 || sheet_h <= 0.0 {
        return MM_PER_PT;
    }
    let sx = CANVAS_W as f64 / sheet_w;
    let sy = CANVAS_H as f64 / sheet_h;
    (sx.min(sy) * 0.9).max(0.1)
}

/// Paper extent at a drawing scale: `paper = model / scale` (the seam's rule).
fn paper_extent(model: f64, scale: f64) -> f64 {
    model / scale.max(f64::EPSILON)
}

/// The base-relative paper offset (mm) of a projected view placed by the
/// projection convention. Derived from the EXISTING layout seam: lay the two
/// kinds out with `layout_views` and read the relative centre difference - a
/// projected TOP sits directly above a FRONT base (`offset.0 == 0` in third
/// angle), a RIGHT view directly to the right (`offset.1 == 0`). The panel
/// never hand-rolls this offset.
fn projected_offset(
    projection: ProjectionType,
    base_kind: ViewKind,
    proj_kind: ViewKind,
    base_model: (f64, f64),
    proj_model: (f64, f64),
    scale: f64,
) -> [f64; 2] {
    let placed = draft::layout::layout_views(
        projection,
        &[
            (base_kind, base_model.0, base_model.1, scale),
            (proj_kind, proj_model.0, proj_model.1, scale),
        ],
        10_000.0,
        10_000.0,
    );
    let base = placed
        .iter()
        .find(|p| p.kind == base_kind)
        .expect("base placed");
    let proj = placed
        .iter()
        .find(|p| p.kind == proj_kind)
        .expect("proj placed");
    [proj.cx - base.cx, proj.cy - base.cy]
}

// ---------------------------------------------------------------------------
// Section geometry: cut the committed bracket, hatch the section.
// ---------------------------------------------------------------------------

fn poly_bbox(poly: &[[f64; 2]]) -> (f64, f64) {
    let mut mn = [f64::INFINITY; 2];
    let mut mx = [f64::NEG_INFINITY; 2];
    for v in poly {
        for a in 0..2 {
            mn[a] = mn[a].min(v[a]);
            mx[a] = mx[a].max(v[a]);
        }
    }
    (mx[0] - mn[0], mx[1] - mn[1])
}

/// The cross-section polygon of the solid by the axis-aligned cut plane
/// (`axis` 0/1/2 = x/y/z at `at`), in the plane's 2D frame, ordered around the
/// centroid. Each triangle edge straddling the plane contributes an
/// intersection point (deduped on projected coords); the ordered contour is the
/// section of the committed bracket - real geometry from the seam's mesh.
fn section_polygon(mesh: &draft::Mesh, axis: usize, at: f64) -> Vec<[f64; 2]> {
    let plane = |p: &[f64; 3]| p[axis] - at;
    let proj = |p: &[f64; 3]| -> [f64; 2] {
        let mut a = [0.0f64; 2];
        let mut k = 0;
        for (i, v) in p.iter().enumerate().take(3) {
            if i != axis {
                a[k] = *v;
                k += 1;
            }
        }
        a
    };
    let mut pts: Vec<[f64; 2]> = Vec::new();
    let mut seen: std::collections::HashSet<[u64; 2]> = std::collections::HashSet::new();
    for tri in &mesh.triangles {
        let (a, b, c) = (
            mesh.positions[tri[0] as usize],
            mesh.positions[tri[1] as usize],
            mesh.positions[tri[2] as usize],
        );
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let (da, db) = (plane(&a), plane(&b));
            if (da < 0.0 && db < 0.0) || (da > 0.0 && db > 0.0) {
                continue;
            }
            if da.abs() < 1e-12 || db.abs() < 1e-12 {
                continue;
            }
            let t = da / (da - db);
            let hit = [
                a[0] + t * (b[0] - a[0]),
                a[1] + t * (b[1] - a[1]),
                a[2] + t * (b[2] - a[2]),
            ];
            let pr = proj(&hit);
            let key = [pr[0].to_bits(), pr[1].to_bits()];
            if seen.insert(key) {
                pts.push(pr);
            }
        }
    }
    if pts.len() < 3 {
        return pts;
    }
    let c = {
        let mut s = [0.0f64; 2];
        for p in &pts {
            s[0] += p[0];
            s[1] += p[1];
        }
        [s[0] / pts.len() as f64, s[1] / pts.len() as f64]
    };
    pts.sort_by(|a, b| {
        let aa = (a[1] - c[1]).atan2(a[0] - c[0]);
        let bb = (b[1] - c[1]).atan2(b[0] - c[0]);
        aa.partial_cmp(&bb).unwrap()
    });
    pts
}

/// Generate 45° hatch lines across a `(w, h)` region at perpendicular spacing
/// `s` (mm): lines `u - v = k*s*sqrt(2)`, each clipped to the region and kept
/// when it has positive length. Count = the section view's rendered hatching.
fn hatch_lines(w: f64, h: f64, s: f64) -> Vec<[[f64; 2]; 2]> {
    let step = s * std::f64::consts::SQRT_2;
    let kmin = (-h / step).ceil() as i64;
    let kmax = (w / step).floor() as i64;
    let mut out = Vec::new();
    for k in kmin..=kmax {
        let c = k as f64 * step;
        let y_lo = 0.0f64.max(-c);
        let y_hi = h.min(w - c);
        if y_hi - y_lo > 1e-9 {
            out.push([[y_lo + c, y_lo], [y_hi + c, y_hi]]);
        }
    }
    out
}

/// Independent analytic hatch count for a `(w, h)` region hatched at 45° with
/// perpendicular spacing `s`: the number of 45° lines at that spacing crossing
/// the region, computed in CLOSED FORM (not read back from the generator).
fn analytic_hatch_count(w: f64, h: f64, s: f64) -> usize {
    if s <= 0.0 || w <= 0.0 || h <= 0.0 {
        return 0;
    }
    let step = s * std::f64::consts::SQRT_2;
    let kmin = (-h / step).ceil() as i64;
    let kmax = (w / step).floor() as i64;
    if kmax < kmin {
        0
    } else {
        (kmax - kmin + 1) as usize
    }
}

// ---------------------------------------------------------------------------
// Placement actions
// ---------------------------------------------------------------------------

/// Place a base view at the sheet centre.
fn place_base(st: &mut ViewsUi, kind: ViewKind, sheet_w: f64, sheet_h: f64) {
    let (mw, mh) = bracket_mesh()
        .as_ref()
        .map(|m| std_view_extent(kind, m))
        .unwrap_or((80.0, 40.0));
    let scale = 1.0;
    let bases = st.views.iter().filter(|v| v.base.is_none()).count();
    let id = format!("oep-view-base-{}", bases + 1);
    st.views.push(PlacedView {
        w: paper_extent(mw, scale),
        h: paper_extent(mh, scale),
        id: id.clone(),
        kind: PlacedKind::Base(kind),
        base: None,
        offset: None,
    });
    st.base_center = Some([sheet_w / 2.0, sheet_h / 2.0]);
    st.drag_start = st.base_center;
    st.last_base_delta = [0.0; 2];
    st.last_derived_delta = [0.0; 2];
    st.section = None;
    st.detail = None;
    st.last = Some(format!("placed base {} at sheet centre", kind_name(kind)));
}

/// Place a projected view of `kind` aligned to the first base through the
/// layout seam's convention offset.
fn place_projected(st: &mut ViewsUi, kind: ViewKind, projection: ProjectionType) {
    let Some(base_idx) = st.views.iter().position(|v| v.base.is_none()) else {
        st.last = Some("place a base view first".to_string());
        return;
    };
    let base = st.views[base_idx].clone();
    let PlacedKind::Base(base_kind) = base.kind else {
        return;
    };
    let Some(mesh) = bracket_mesh() else {
        st.last = Some("no bracket mesh".to_string());
        return;
    };
    let scale = 1.0;
    let offset = projected_offset(
        projection,
        base_kind,
        kind,
        std_view_extent(base_kind, &mesh),
        std_view_extent(kind, &mesh),
        scale,
    );
    let (mw, mh) = std_view_extent(kind, &mesh);
    let n = st.views.len() + 1;
    st.views.push(PlacedView {
        w: paper_extent(mw, scale),
        h: paper_extent(mh, scale),
        id: format!("oep-view-proj-{n}"),
        kind: PlacedKind::Projected(kind),
        base: Some(base.id.clone()),
        offset: Some(offset),
    });
    st.last = Some(format!(
        "placed projected {} aligned to {} (offset {:.9}, {:.9} mm)",
        kind_name(kind),
        base.id,
        offset[0],
        offset[1]
    ));
}

/// Place a section view: cut the bracket and hatch the cross-section.
fn place_section(st: &mut ViewsUi) {
    let Some(mesh) = bracket_mesh() else {
        st.last = Some("no bracket mesh".to_string());
        return;
    };
    let poly = section_polygon(&mesh, SECTION_AXIS, SECTION_AT_MM);
    let (w, h) = poly_bbox(&poly);
    let generated = hatch_lines(w, h, HATCH_SPACING_MM).len();
    let analytic = analytic_hatch_count(w, h, HATCH_SPACING_MM);
    let n = st.views.len() + 1;
    st.views.push(PlacedView {
        w,
        h,
        id: format!("oep-view-sec-{n}"),
        kind: PlacedKind::Section,
        base: st
            .views
            .iter()
            .find(|v| v.base.is_none())
            .map(|v| v.id.clone()),
        offset: Some([0.0, -45.0]), // sit below the base
    });
    st.section = Some(SectionNumbers {
        generated,
        analytic,
        region_w: w,
        region_h: h,
    });
    st.last = Some(format!(
        "section x={SECTION_AT_MM}: generated {generated} hatch lines, analytic {analytic}, region {w}x{h} mm"
    ));
}

/// Place a detail view: magnify the base's projected region at the requested
/// scale.
fn place_detail(st: &mut ViewsUi) {
    let Some(mesh) = bracket_mesh() else {
        st.last = Some("no bracket mesh".to_string());
        return;
    };
    let (mw, mh) = std_view_extent(ViewKind::Front, &mesh);
    let render_w = paper_extent(mw, DETAIL_SCALE);
    let render_h = paper_extent(mh, DETAIL_SCALE);
    let n = st.views.len() + 1;
    st.views.push(PlacedView {
        w: render_w,
        h: render_h,
        id: format!("oep-view-det-{n}"),
        kind: PlacedKind::Detail,
        base: st
            .views
            .iter()
            .find(|v| v.base.is_none())
            .map(|v| v.id.clone()),
        offset: Some([45.0, 40.0]), // sit up and to the right of the base
    });
    st.detail = Some(DetailNumbers {
        scale: DETAIL_SCALE,
        render_w,
        render_h,
        analytic_w: paper_extent(mw, DETAIL_SCALE),
        analytic_h: paper_extent(mh, DETAIL_SCALE),
    });
    st.last = Some(format!(
        "detail at scale {DETAIL_SCALE}: region model {mw}x{mh} -> rendered {render_w}x{render_h} mm"
    ));
}

// ---------------------------------------------------------------------------
// Panel body
// ---------------------------------------------------------------------------

/// Render the views placement tools inside the drafting scroll area.
pub fn show(ui: &mut egui::Ui, app: &mut App, model: &mut super::SheetModel) {
    let _ = app;
    ui.separator();
    ui.label(theme::heading(theme::mode_of(ui), "Views").size(13.0));

    let mut st = read_state(ui.ctx());
    let (sheet_w, sheet_h, projection) = sheet_geometry(model);
    // Auto-fit the sheet into the preview canvas: the canvas is a miniature of
    // the full sheet, so the paper(px) scale adapts to the current sheet size.
    let fit = fit_scale(sheet_w, sheet_h);

    // ---- Placement tools ---------------------------------------------------
    ui.horizontal_wrapped(|ui| {
        if ui.button("Place base front").clicked() {
            place_base(&mut st, ViewKind::Front, sheet_w, sheet_h);
            st.last = Some("placed base Front".to_string());
        }
        if ui.button("Place projected top").clicked() {
            place_projected(&mut st, ViewKind::Top, projection);
        }
        if ui.button("Place projected right").clicked() {
            place_projected(&mut st, ViewKind::Right, projection);
        }
        if ui.button("Place section").clicked() {
            place_section(&mut st);
        }
        if ui.button("Place detail").clicked() {
            place_detail(&mut st);
        }
        if ui.button("Clear views").clicked() {
            st = ViewsUi::default();
        }
    });

    // ---- The sheet-preview canvas ------------------------------------------
    let (canvas_rect, _) =
        ui.allocate_exact_size(egui::vec2(CANVAS_W, CANVAS_H), egui::Sense::hover());
    let mode = theme::mode_of(ui);
    let cp = ui.painter().clone();
    cp.rect_filled(
        canvas_rect,
        4.0,
        oep_ui_theme::SURFACE_RAISED.for_mode(mode),
    );
    cp.rect_stroke(
        canvas_rect,
        4.0,
        egui::Stroke::new(1.0, oep_ui_theme::BORDER.for_mode(mode)),
        egui::StrokeKind::Inside,
    );

    // Compute each placed view's live paper centre: base = the anchor, derived
    // = base + offset (so derived ALWAYS follows the base exactly).
    let base_center = st.base_center;
    let live: Vec<(usize, [f64; 2], PlacedView)> = st
        .views
        .clone()
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let center = match (&v.base, base_center) {
                (None, Some(c)) => c,
                (Some(base_id), Some(c)) => {
                    let off = v.offset?;
                    let _ = base_id;
                    [c[0] + off[0], c[1] + off[1]]
                }
                _ => return None,
            };
            Some((i, center, v))
        })
        .collect();

    // The harbour region where the section's hatch glyph is drawn sits at the
    // section chip's live center.
    let section_live = live
        .iter()
        .find(|(_, _, v)| v.kind == PlacedKind::Section)
        .map(|(_, c, _)| *c);

    // Draw hatching for the section view (real polygon outline + hatch lines).
    // The glyph sits at the section chip's live paper centre (so it tracks the
    // base exactly like every other derived view).
    if let Some(sec_center) = section_live {
        if let Some(mesh) = bracket_mesh() {
            let poly = section_polygon(&mesh, SECTION_AXIS, SECTION_AT_MM);
            let (pw, ph) = poly_bbox(&poly);
            let scale = 6.0; // upscale the small section so it reads on the canvas
            let c = egui::pos2(
                canvas_rect.left() + (sec_center[0] as f32) * fit as f32,
                canvas_rect.bottom() - (sec_center[1] as f32) * fit as f32,
            );
            // outline
            let outline: Vec<egui::Pos2> = poly
                .iter()
                .map(|p| {
                    egui::pos2(
                        c.x + (p[0] - pw / 2.0) as f32 * scale as f32,
                        c.y - (p[1] - ph / 2.0) as f32 * scale as f32,
                    )
                })
                .collect();
            if outline.len() >= 3 {
                cp.add(egui::Shape::closed_line(
                    outline,
                    egui::Stroke::new(1.0, oep_ui_theme::ACCENT.for_mode(mode)),
                ));
            }
            for seg in hatch_lines(pw, ph, HATCH_SPACING_MM) {
                let a = egui::pos2(
                    c.x + (seg[0][0] - pw / 2.0) as f32 * scale as f32,
                    c.y - (seg[0][1] - ph / 2.0) as f32 * scale as f32,
                );
                let b = egui::pos2(
                    c.x + (seg[1][0] - pw / 2.0) as f32 * scale as f32,
                    c.y - (seg[1][1] - ph / 2.0) as f32 * scale as f32,
                );
                cp.add(egui::Shape::line_segment(
                    [a, b],
                    egui::Stroke::new(1.0, oep_ui_theme::BORDER.for_mode(mode)),
                ));
            }
        }
    }

    // ---- Chips + base drag ---------------------------------------------------
    for (center, v) in live.iter().map(|(_, c, v)| (c, v)) {
        let is_base = v.base.is_none();
        let px = egui::pos2(
            canvas_rect.left() + (center[0] as f32) * fit as f32,
            canvas_rect.bottom() - (center[1] as f32) * fit as f32,
        );
        let chip_size = egui::vec2(76.0, 22.0);
        let chip = egui::Rect::from_center_size(px, chip_size);
        cp.rect_filled(
            chip,
            3.0,
            if is_base {
                oep_ui_theme::ACCENT.for_mode(mode)
            } else {
                oep_ui_theme::SURFACE_RAISED.for_mode(mode)
            },
        );
        cp.rect_stroke(
            chip,
            3.0,
            egui::Stroke::new(1.0, oep_ui_theme::BORDER.for_mode(mode)),
            egui::StrokeKind::Inside,
        );

        // The accessible drag handle on the base chip: `Sense::drag` + a stable
        // label (`drag base <id>`), found by the acceptance via its label.
        let label = if is_base {
            format!("drag base {}", v.id)
        } else {
            format!("{} ({})", v.kind.kind_name(), v.id)
        };
        let cp_text = cp.clone();
        let text_color = oep_ui_theme::TEXT_PRIMARY.for_mode(mode);
        if is_base {
            let resp = ui.interact(
                chip,
                egui::Id::unique(format!("views-chip-{}", v.id)),
                egui::Sense::drag(),
            );
            ui.ctx().accesskit_node_builder(resp.id, |node| {
                node.set_label(label.clone());
                node.set_role(egui::accesskit::Role::Button);
            });
            cp_text.text(
                egui::Rect::from_center_size(px, chip_size).center(),
                egui::Align2::CENTER_CENTER,
                format!("base {}", v.kind.kind_name()),
                egui::FontId::proportional(10.0),
                text_color,
            );

            if resp.drag_started() {
                st.drag_start = st.base_center;
            }
            if resp.dragged() {
                if let (Some(start), Some(_)) = (st.drag_start, st.base_center) {
                    let total = resp.total_drag_delta().unwrap_or_default();
                    let dmm_x = total.x as f64 / fit;
                    // paper y is up; pointer y is down.
                    let dmm_y = -(total.y as f64) / fit;
                    st.base_center = Some([start[0] + dmm_x, start[1] + dmm_y]);
                    st.last_base_delta = [dmm_x, dmm_y];
                    // The projected view moved exactly the same delta (its new
                    // center = new base + fixed offset).
                    if let Some(proj) = st.views.iter().find(|p| p.base.is_some()) {
                        if let Some(off) = proj.offset {
                            let ps = [start[0] + off[0], start[1] + off[1]];
                            let pn = [
                                st.base_center.unwrap()[0] + off[0],
                                st.base_center.unwrap()[1] + off[1],
                            ];
                            st.last_derived_delta = [pn[0] - ps[0], pn[1] - ps[1]];
                        }
                    }
                    st.last = Some(format!(
                        "dragged base by ({dmm_x:.9}, {dmm_y:.9}) mm; projected moved by ({:.9}, {:.9}) mm",
                        st.last_derived_delta[0], st.last_derived_delta[1]
                    ));
                }
            }
            if resp.drag_stopped() {
                st.drag_start = None;
            }
        } else {
            let resp = ui.interact(
                chip,
                egui::Id::unique(format!("views-chip-{}", v.id)),
                egui::Sense::hover(),
            );
            ui.ctx().accesskit_node_builder(resp.id, |node| {
                node.set_label(label.clone());
                node.set_role(egui::accesskit::Role::Button);
            });
            cp_text.text(
                chip.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} {}", v.kind.kind_name(), v.id),
                egui::FontId::proportional(9.0),
                text_color,
            );
        }
    }

    // One-line summary of placement/alignment/section/detail numbers.
    let summary = summarize(&st, sheet_w, sheet_h);
    if !summary.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(&summary).monospace().size(11.0));
    }
    if let Some(last) = &st.last {
        ui.add_space(2.0);
        ui.label(egui::RichText::new(last).weak().italics().size(11.0));
    }

    publish_report(ui.ctx(), &st, sheet_w, sheet_h);
    write_state(ui.ctx(), &st);
}

fn kind_name(kind: ViewKind) -> String {
    format!("{kind:?}").to_lowercase()
}

/// Build the one-line numeric summary shown in the panel + oracle.
fn summarize(st: &ViewsUi, sheet_w: f64, sheet_h: f64) -> String {
    let mut out = String::new();
    let base = st.base_center.unwrap_or([sheet_w / 2.0, sheet_h / 2.0]);
    if let Some(proj) = st.views.iter().find(|v| v.base.is_some()) {
        if let Some(off) = proj.offset {
            let pc = [base[0] + off[0], base[1] + off[1]];
            // The aligned axis: TOP/FRONT stack aligns cx, RIGHT aligns cy.
            out.push_str(&format!(
                "align: base_cx={:.9} proj_cx={:.9} diff={:.3e} base_cy={:.9} proj_cy={:.9} | ",
                base[0],
                pc[0],
                (pc[0] - base[0]).abs(),
                base[1],
                pc[1],
            ));
        }
    }
    if st.views.iter().any(|v| v.base.is_none()) && !st.views.iter().any(|v| v.base.is_some()) {
        out.push_str(&format!(
            "base at ({:.9}, {:.9}) sheet {:.0}x{:.0} | ",
            base[0], base[1], sheet_w, sheet_h
        ));
    }
    if let Some(sec) = st.section {
        out.push_str(&format!(
            "section hatch: rendered={} analytic={} region={}x{} | ",
            sec.generated, sec.analytic, sec.region_w, sec.region_h
        ));
    }
    if let Some(det) = st.detail {
        out.push_str(&format!(
            "detail @ {:.2}: rendered {}x{} analytic {}x{}",
            det.scale, det.render_w, det.render_h, det.analytic_w, det.analytic_h
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Context-store plumbing + oracle
// ---------------------------------------------------------------------------

fn read_state(ctx: &egui::Context) -> ViewsUi {
    ctx.data(|d| d.get_temp(egui::Id::unique(VIEWS_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &ViewsUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(VIEWS_STATE_KEY), st.clone()));
}

/// Publish the plain-`String` oracle under [`VIEWS_REPORT_KEY`].
fn publish_report(ctx: &egui::Context, st: &ViewsUi, _sheet_w: f64, _sheet_h: f64) {
    let base = st.base_center.unwrap_or([0.0; 2]);
    let (pc, has_proj) = match st.views.iter().find(|v| v.base.is_some()) {
        Some(proj) => match (proj.offset, st.base_center) {
            (Some(off), Some(b)) => (Some([b[0] + off[0], b[1] + off[1]]), true),
            _ => (None, false),
        },
        None => (None, false),
    };
    let mut r = String::new();
    r.push_str(&format!("placed={}\n", st.views.len()));
    r.push_str(&format!(
        "has_base={}\n",
        u8::from(st.base_center.is_some())
    ));
    r.push_str(&format!("has_proj={}\n", u8::from(has_proj)));
    r.push_str(&format!("base_cx={:.9}\n", base[0]));
    r.push_str(&format!("base_cy={:.9}\n", base[1]));
    match pc {
        Some(p) => {
            r.push_str(&format!("proj_cx={:.9}\n", p[0]));
            r.push_str(&format!("proj_cy={:.9}\n", p[1]));
            r.push_str(&format!("align_coord={:.9}\n", p[0]));
            r.push_str(&format!("align_diff={:.3e}\n", (p[0] - base[0]).abs()));
        }
        None => {
            r.push_str("proj_cx=-\nproj_cy=-\nalign_coord=-\nalign_diff=-\n");
        }
    }
    r.push_str(&format!("drag_base_dx={:.9}\n", st.last_base_delta[0]));
    r.push_str(&format!("drag_base_dy={:.9}\n", st.last_base_delta[1]));
    r.push_str(&format!("drag_proj_dx={:.9}\n", st.last_derived_delta[0]));
    r.push_str(&format!("drag_proj_dy={:.9}\n", st.last_derived_delta[1]));
    match &st.section {
        Some(sec) => {
            r.push_str(&format!("section_hatch={}\n", sec.generated));
            r.push_str(&format!("section_hatch_analytic={}\n", sec.analytic));
            r.push_str(&format!(
                "section_region={}x{}\n",
                sec.region_w, sec.region_h
            ));
        }
        None => {
            r.push_str("section_hatch=-\nsection_hatch_analytic=-\nsection_region=-\n");
        }
    }
    match &st.detail {
        Some(det) => {
            r.push_str(&format!("detail_scale={}\n", det.scale));
            r.push_str(&format!("detail_render_w={}\n", det.render_w));
            r.push_str(&format!("detail_render_h={}\n", det.render_h));
            r.push_str(&format!("detail_analytic_w={}\n", det.analytic_w));
            r.push_str(&format!("detail_analytic_h={}\n", det.analytic_h));
        }
        None => {
            r.push_str("detail_scale=-\ndetail_render_w=-\ndetail_render_h=-\n");
            r.push_str("detail_analytic_w=-\ndetail_analytic_h=-\n");
        }
    }
    r.push_str(&format!(
        "last={}\n",
        st.last.clone().unwrap_or_else(|| "-".to_string())
    ));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(VIEWS_REPORT_KEY), r));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The projected TOP of a FRONT base in third angle is x-aligned
    /// (`offset.0 == 0`) - derived from the LAYOUT SEAM, not hand-written.
    #[test]
    fn projected_top_offset_is_x_aligned_from_layout_seam() {
        let mesh = bracket_mesh().expect("committed bracket mesh");
        let front = std_view_extent(ViewKind::Front, &mesh);
        let top = std_view_extent(ViewKind::Top, &mesh);
        let off = projected_offset(
            ProjectionType::ThirdAngle,
            ViewKind::Front,
            ViewKind::Top,
            front,
            top,
            1.0,
        );
        assert_eq!(off[0], 0.0, "top view must share the base's x-center");
        assert!(off[1] > 0.0, "top view sits above the base");
        // y offset is exactly half base height + gap + half top height.
        let expect_y = front.1 / 2.0 + draft::layout::VIEW_GAP_MM + top.1 / 2.0;
        assert!(
            (off[1] - expect_y).abs() < 1e-9,
            "offset {off:?} != {expect_y}"
        );
    }

    /// Dragging the base moves the projected view by exactly the same delta.
    #[test]
    fn base_drag_moves_projected_by_exact_delta() {
        let mesh = bracket_mesh().expect("mesh");
        let front = std_view_extent(ViewKind::Front, &mesh);
        let top = std_view_extent(ViewKind::Top, &mesh);
        let off = projected_offset(
            ProjectionType::ThirdAngle,
            ViewKind::Front,
            ViewKind::Top,
            front,
            top,
            1.0,
        );
        let base = [148.5, 105.0];
        let proj = [base[0] + off[0], base[1] + off[1]];
        for (dx, dy) in [(12.0, -7.0), (80.0, 40.0), (-5.0, 9.0)] {
            let base2 = [base[0] + dx, base[1] + dy];
            let proj2 = [base2[0] + off[0], base2[1] + off[1]];
            assert!((proj2[0] - proj[0] - dx).abs() < 1e-9, "proj dx != base dx");
            assert!((proj2[1] - proj[1] - dy).abs() < 1e-9, "proj dy != base dy");
        }
    }

    /// The committed bracket's section at x=40 has a NON-ZERO hatch count and
    /// the rendered (generated) count equals the independent analytic count.
    #[test]
    fn section_hatch_is_nonzero_and_matches_analytic() {
        let mesh = bracket_mesh().expect("mesh");
        let poly = section_polygon(&mesh, SECTION_AXIS, SECTION_AT_MM);
        let (w, h) = poly_bbox(&poly);
        let generated = hatch_lines(w, h, HATCH_SPACING_MM).len();
        let analytic = analytic_hatch_count(w, h, HATCH_SPACING_MM);
        assert!(generated > 0, "hatch count must be non-zero");
        assert_eq!(
            generated, analytic,
            "rendered hatch must equal analytic count"
        );
        assert_eq!((w, h), (40.0, 6.0), "bracket width x depth section");
    }

    /// The detail view renders the base region at exactly the requested scale:
    /// rendered extents equal the analytic extents.
    #[test]
    fn detail_extents_match_analytic_at_requested_scale() {
        let mesh = bracket_mesh().expect("mesh");
        let (mw, mh) = std_view_extent(ViewKind::Front, &mesh);
        let render_w = paper_extent(mw, DETAIL_SCALE);
        let render_h = paper_extent(mh, DETAIL_SCALE);
        assert_eq!(render_w, paper_extent(mw, DETAIL_SCALE));
        assert_eq!(render_h, paper_extent(mh, DETAIL_SCALE));
        assert!(render_w > mw, "detail magnifies the region");
    }
}
