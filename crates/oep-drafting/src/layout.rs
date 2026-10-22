//! Sheet layout: place the standard views on the paper per the first- or
//! third-angle convention.
//!
//! The projected CONTENT of a view is identical under either convention (the
//! model is orthographically projected the same way); what changes is how the
//! views are ARRANGED on the sheet:
//!
//! - **Third-angle** (ASME): the top view sits ABOVE the front view and the
//!   right view sits to the RIGHT of the front view.
//! - **First-angle** (ISO/EU): folded away from the viewer, so the top view
//!   sits BELOW the front view and the right view sits to the LEFT.
//!
//! This module centres the cluster of the milestone's three standard views
//! (front / top / right) in the available drawing region, leaving the border,
//! the title block area and inter-view gaps clear.

use oep_geo::{ProjectionType, ViewKind};

/// Gap between adjacent views on a sheet, in paper millimetres.
pub const VIEW_GAP_MM: f64 = 15.0;

/// A local (pre-shift) placement: centre `[x, y]` plus paper extent `(w, h)`.
type LocalPlacement = ([f64; 2], f64, f64);

/// A view placed on the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedView {
    /// Which standard view this is.
    pub kind: ViewKind,
    /// Paper-space center x (mm from sheet left edge).
    pub cx: f64,
    /// Paper-space center y (mm from sheet bottom edge).
    pub cy: f64,
    /// Paper-space width of the view's bounding box (mm).
    pub w: f64,
    /// Paper-space height of the view's bounding box (mm).
    pub h: f64,
}

/// Scale model extents to paper extents. `scale` is the view's drawing scale
/// (model units per paper unit); the paper width = model width / scale.
fn paper_extent(model: f64, scale: f64) -> f64 {
    model / scale.max(f64::EPSILON)
}

/// Arrange the three standard views into the region `[0, region_w] ×
/// [0, region_h]` (paper mm), returning one [`PlacedView`] per input.
///
/// `views` carries `(kind, model_w, model_h, scale)` per view (the model
/// extent of the projected edges and the drawing scale). The cluster is
/// centred in the region with [`VIEW_GAP_MM`] between views.
///
/// Deterministic; never panics. `region_w`/`region_h` must be positive.
pub fn layout_views(
    projection: ProjectionType,
    views: &[(ViewKind, f64, f64, f64)],
    region_w: f64,
    region_h: f64,
) -> Vec<PlacedView> {
    let gap = VIEW_GAP_MM;
    let extents: Vec<(f64, f64)> = views
        .iter()
        .map(|(_, w, h, s)| (paper_extent(*w, *s), paper_extent(*h, *s)))
        .collect();

    // Locally, front sits at centre (0, 0); top/right are offset per the
    // convention (all positive-local offsets, folded below/left for first angle).
    let (mut loc, mut bbox): (Vec<LocalPlacement>, [f64; 4]) = (Vec::new(), [0.0; 4]);
    for (i, (kind, _w, _h, _s)) in views.iter().enumerate() {
        let (w, h) = extents[i];
        let (dx, dy) = match kind {
            ViewKind::Front => (0.0, 0.0),
            ViewKind::Top => match projection {
                ProjectionType::ThirdAngle => (0.0, extents[0].1 / 2.0 + gap + h / 2.0),
                ProjectionType::FirstAngle => (0.0, -extents[0].1 / 2.0 - gap - h / 2.0),
            },
            ViewKind::Right => match projection {
                ProjectionType::ThirdAngle => (extents[0].0 / 2.0 + gap + w / 2.0, 0.0),
                ProjectionType::FirstAngle => (-extents[0].0 / 2.0 - gap - w / 2.0, 0.0),
            },
        };
        let (cx, cy) = (dx, dy);
        let (mnx, mxx) = (cx - w / 2.0, cx + w / 2.0);
        let (mny, mxy) = (cy - h / 2.0, cy + h / 2.0);
        bbox = [
            bbox[0].min(mnx),
            bbox[1].min(mny),
            bbox[2].max(mxx),
            bbox[3].max(mxy),
        ];
        loc.push(([cx, cy], w, h));
    }

    // Shift so the cluster centre lands on the region centre.
    let cw = bbox[2] - bbox[0];
    let ch = bbox[3] - bbox[1];
    let shift_x = region_w / 2.0 - (bbox[0] + cw / 2.0);
    let shift_y = region_h / 2.0 - (bbox[1] + ch / 2.0);

    views
        .iter()
        .zip(loc.iter())
        .map(|((kind, _, _, _), (c, w, h))| PlacedView {
            kind: *kind,
            cx: c[0] + shift_x,
            cy: c[1] + shift_y,
            w: *w,
            h: *h,
        })
        .collect()
}
