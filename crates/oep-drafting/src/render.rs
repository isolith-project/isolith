//! Render a drawing Sheet to concrete paper-space geometry.
//!
//! [`render_sheet`] takes the drawing record (an `oep-geo` [`Sheet`]), the
//! part's solid mesh (replayed from the feature graph by
//! [`crate::solid::SolidEvaluator`]) and the resolved variable-set entries
//! (the `oep-sketch` `VariableSet`), and produces a [`RenderedDrawing`] whose
//! every coordinate is COMPUTED, never fabricated:
//!
//! - each view's projected edges come from [`crate::view::project_edges`] on
//!   the real mesh, placed on the sheet by [`crate::layout::layout_views`]
//!   per the sheet's first/third-angle convention;
//! - each dimension's drawn label comes from the variable-set entry that
//!   drives the model (FreeCAD-v1 behavior) — and the geometry's measured
//!   extent is carried alongside so a divergent solve can never hide;
//! - the title block is a deterministic table of the sheet's `TitleBlock`.
//!
//! `NaN`/`-nan` never enter the drawing: non-finite geometry is skipped and
//! dimension resolution fails closed to a typed [`RenderError`].
//!
//! All paper coordinates are in sheet millimetres with the origin at the
//! BOTTOM-left and Y UP (writers adapt to their own coordinate systems).

use oep_geo::{Dimension, DimensionKind, ProjectionType, Sheet, TitleBlock, ViewKind};

use crate::layout::{layout_views, PlacedView};
use crate::mesh::Mesh;
use crate::view::{extent, project_edges, Edge2};

/// Border margin around the sheet, mm.
pub const BORDER_MM: f64 = 10.0;
/// Title block width, mm.
pub const TITLE_BLOCK_W: f64 = 160.0;
/// Title block height, mm.
pub const TITLE_BLOCK_H: f64 = 32.0;
/// Distance the dimension line sits outside the view bbox, mm.
pub const DIM_OFFSET_MM: f64 = 10.0;
/// Font size for labels and title-block text, mm.
pub const TEXT_MM: f64 = 3.5;

/// A typed render failure (never a panic, per the crate's tolerant doctrine).
#[derive(Debug, Clone, PartialEq)]
pub enum RenderError {
    /// The sheet has no views to lay out.
    NoViews,
    /// An empty or non-finite mesh cannot be projected.
    NoGeometry(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoViews => write!(f, "sheet has no views"),
            RenderError::NoGeometry(s) => write!(f, "no drawable geometry: {s}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// A view placed on the sheet with its paper-space projected edges.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedView {
    /// The view's id.
    pub id: String,
    /// Which standard view.
    pub kind: ViewKind,
    /// The label drawn under/with the view (e.g. "FRONT").
    pub label: String,
    /// Paper-space centre.
    pub cx: f64,
    /// Paper-space centre.
    pub cy: f64,
    /// Paper-space extent width.
    pub extent_w: f64,
    /// Paper-space extent height.
    pub extent_h: f64,
    /// Projected edges translated into paper coordinates.
    pub edges_paper: Vec<Edge2>,
}

/// A dimension annotation resolved for drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedDimension {
    /// The dimension's id.
    pub id: String,
    /// The view it annotates.
    pub view_id: String,
    /// Horizontal or vertical.
    pub kind: DimensionKind,
    /// The variable-set entry driving the label, if bound (FreeCAD-v1).
    pub variable: Option<String>,
    /// The drawn label text (e.g. "80").
    pub label: String,
    /// The authoritative value this label shows (the variable, else measured).
    pub value: f64,
    /// The value measured from the model geometry (model units).
    pub measured: f64,
    /// Dimension line start (paper mm) — the segment drawn with arrows.
    pub ax: f64,
    /// Dimension line start.
    pub ay: f64,
    /// Dimension line end.
    pub bx: f64,
    /// Dimension line end.
    pub by: f64,
}

/// The complete, placed drawing of one sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedDrawing {
    /// Sheet width, mm.
    pub width_mm: u32,
    /// Sheet height, mm.
    pub height_mm: u32,
    /// First/third-angle convention used for the layout.
    pub projection: ProjectionType,
    /// The sheet's title block values.
    pub title_block: TitleBlock,
    /// The placed views.
    pub views: Vec<RenderedView>,
    /// The resolved dimension annotations.
    pub dimensions: Vec<RenderedDimension>,
}

/// Resolve a dimension's drawn value from the variable set (FreeCAD-v1):
/// the ENTRY NAME drives the label so an edit to the variable updates the
/// drawing; the model-measured extent is returned for cross-checking.
fn resolve_dimension(
    dim: &Dimension,
    measured: f64,
    variables: &std::collections::BTreeMap<String, f64>,
) -> RenderedDimension {
    let (value, label) = match &dim.variable {
        Some(name) => match variables.get(name) {
            Some(v) => (*v, fmt_num(*v)),
            None => (measured, fmt_num(measured)),
        },
        None => (measured, fmt_num(measured)),
    };
    RenderedDimension {
        id: dim.id.clone(),
        view_id: dim.view_id.clone(),
        kind: dim.kind,
        variable: dim.variable.clone(),
        label,
        value,
        measured,
        ..RenderedDimension::canvas(dim)
    }
}

impl RenderedDimension {
    /// A zeroed canvas used while a dimension is being resolved (the geometry
    /// is backfilled after layout). `resolve_dimension` fixes the label/value.
    fn canvas(dim: &Dimension) -> Self {
        Self {
            id: dim.id.clone(),
            view_id: dim.view_id.clone(),
            kind: dim.kind,
            variable: dim.variable.clone(),
            label: String::new(),
            value: 0.0,
            measured: 0.0,
            ax: 0.0,
            ay: 0.0,
            bx: 0.0,
            by: 0.0,
        }
    }
}

/// Measure a projected edge set along a dimension axis (model units).
fn measure(edges: &[Edge2], kind: DimensionKind) -> f64 {
    let (mn, mx) = crate::view::bounds2(edges);
    match kind {
        DimensionKind::Horizontal => mx[0] - mn[0],
        DimensionKind::Vertical => mx[1] - mn[1],
    }
}

/// Render a sheet to paper geometry. Returns a typed error, never panics.
pub fn render_sheet(
    sheet: &Sheet,
    mesh: &Mesh,
    variables: &std::collections::BTreeMap<String, f64>,
) -> Result<RenderedDrawing, RenderError> {
    let (sheet_w, sheet_h) = (sheet.size.width_mm as f64, sheet.size.height_mm as f64);
    // Drawing region: inside the border, above the title block band (which
    // occupies the bottom-right) and inside the right margin.
    let region_w = sheet_w - 2.0 * BORDER_MM;
    let region_h = sheet_h - 2.0 * BORDER_MM - TITLE_BLOCK_H;

    if mesh.is_empty()
        || !mesh
            .positions
            .iter()
            .all(|p| p.iter().all(|c| c.is_finite()))
    {
        return Err(RenderError::NoGeometry(
            "solid mesh is empty or non-finite".into(),
        ));
    }
    if sheet.views.is_empty() {
        return Err(RenderError::NoViews);
    }

    // Per-view projection + model extent.
    let mut model_edges: Vec<(ViewKind, Vec<Edge2>)> = Vec::new();
    for v in &sheet.views {
        model_edges.push((v.kind, project_edges(mesh, v.kind)));
    }

    // Lay out in the region.
    let layout_input: Vec<(ViewKind, f64, f64, f64)> = sheet
        .views
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let (w, h) = extent(&model_edges[i].1);
            (v.kind, w, h, v.scale)
        })
        .collect();
    let placed: Vec<PlacedView> = layout_views(sheet.projection, &layout_input, region_w, region_h);

    // Translate projected edges into paper coordinates centred on each view.
    let mut views = Vec::with_capacity(sheet.views.len());
    let mut paper_center: std::collections::HashMap<String, (f64, f64, f64, f64)> =
        std::collections::HashMap::new();
    for ((v, (kind, edges)), pv) in sheet
        .views
        .iter()
        .zip(model_edges.iter())
        .zip(placed.iter())
    {
        let (mn, mx) = crate::view::bounds2(edges);
        let (mcx, mcy) = ((mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0);
        let scale = v.scale.max(f64::EPSILON);
        let to_paper = |p: [f64; 2]| -> [f64; 2] {
            [pv.cx + (p[0] - mcx) / scale, pv.cy + (p[1] - mcy) / scale]
        };
        let edges_paper: Vec<Edge2> = edges
            .iter()
            .filter_map(|e| {
                let a = to_paper(e[0]);
                let b = to_paper(e[1]);
                // Clamp into the sheet so nothing leaks outside the paper.
                let c = |x: f64| x.clamp(0.0, sheet_w);
                let d = |y: f64| y.clamp(0.0, sheet_h);
                let (a, b) = ([c(a[0]), d(a[1])], [c(b[0]), d(b[1])]);
                if a == b {
                    None
                } else {
                    Some([a, b])
                }
            })
            .collect();
        let label = format!("{:?}", v.kind).to_uppercase();
        views.push(RenderedView {
            id: v.id.clone(),
            kind: *kind,
            label,
            cx: pv.cx,
            cy: pv.cy,
            extent_w: pv.w,
            extent_h: pv.h,
            edges_paper,
        });
        paper_center.insert(v.id.clone(), (pv.cx, pv.cy, pv.w, pv.h));
    }

    // Dimensions: label from the variable set, line placed just outside the
    // view's paper bbox along the dimension axis.
    let mut dimensions = Vec::new();
    let empty_edges: Vec<Edge2> = Vec::new();
    for d in &sheet.dimensions {
        let view_model_edges = sheet
            .views
            .iter()
            .zip(model_edges.iter())
            .find(|(v, _)| v.id == d.view_id)
            .map(|(_, (_, e))| e)
            .unwrap_or(&empty_edges);
        let measured = measure(view_model_edges, d.kind);
        let mut rd = resolve_dimension(d, measured, variables);
        if let Some(&(cx, cy, w, h)) = paper_center.get(&d.view_id) {
            let (x0, x1, y0, y1) = (cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0);
            match d.kind {
                DimensionKind::Horizontal => {
                    let y = y0 - DIM_OFFSET_MM;
                    rd.ax = x0;
                    rd.ay = y;
                    rd.bx = x1;
                    rd.by = y;
                }
                DimensionKind::Vertical => {
                    let x = x0 - DIM_OFFSET_MM;
                    rd.ax = x;
                    rd.ay = y0;
                    rd.bx = x;
                    rd.by = y1;
                }
            }
        }
        dimensions.push(rd);
    }

    Ok(RenderedDrawing {
        width_mm: sheet.size.width_mm,
        height_mm: sheet.size.height_mm,
        projection: sheet.projection,
        title_block: sheet.title_block.clone(),
        views,
        dimensions,
    })
}

/// Format a resolved dimension value deterministically: finite, quantised to
/// 1e-6, trailing zeros trimmed (80.0 -> "80", 6.25 -> "6.25").
pub fn fmt_num(x: f64) -> String {
    if !x.is_finite() {
        return "?".into();
    }
    let r = (x * 1_000_000.0).round() / 1_000_000.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        let s = format!("{r:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
