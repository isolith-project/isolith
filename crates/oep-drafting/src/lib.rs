//! P7 -- 2D drafting workbench (EDITOR-2D-DRAFTING, decision #19 pull-forward).
//!
//! Drafting is what engineers hand to fabrication, and it is the first
//! capability pulled forward into the editor spine. This crate builds the
//! workbench on the REAL kernel and `oep-geo` feature graph:
//!
//! - standard/projected views are computed by projecting the solid mesh that
//!   [`solid::SolidEvaluator`] replays from the [`oep_geo::feature_graph`]
//!   (`solid_from_graph`); each view's projected edges come from the model,
//!   never fabricated, and are arranged on the sheet per the document's
//!   first/third-angle setting ([`layout`]);
//! - dimension annotations are wired to the `oep-sketch` variable set
//!   ([`builder::drive_bracket_drawing`] + [`render`]): a dimension's drawn
//!   value is the SAME variable-set entry that drives the model
//!   (FreeCAD-v1-variable-set behavior), so an edit to the variable updates
//!   the drawing dimension;
//! - the drawing record (sheets, views, dimensions, title block) is stored in
//!   the canonical model as the `oep-geo` [`Drawing`] class — a Class 2
//!   consumer-additive schema extension (v1 -> v2, with migration; see
//!   `docs/geometry-drawing-schema.md`);
//! - export is true vector output ([`export`]): SVG and PDF writers over one
//!   deterministic primitive model; the honest STEP/2D story is documented in
//!   `docs/drafting-step-2d-story.md` (STEP carries 3D B-Rep, drawings are
//!   exchanged as vector documents).
//!
//! Tolerant-failure doctrine throughout: typed errors ([`RenderError`],
//! `oep_sketch::solve::SketchError`, `oep_geo::feature_graph::ReplayError`),
//! never a panic, never fabricated output.
#![deny(missing_docs)]

pub mod builder;
pub mod export;
pub mod layout;
pub mod mesh;
pub mod render;
pub mod solid;
pub mod view;

pub use builder::{drive_bracket_drawing, BracketDrawing};
pub use export::pdf::write_drawing_pdf;
pub use export::svg::write_drawing_svg;
pub use mesh::Mesh;
pub use render::{render_sheet, RenderError, RenderedDimension, RenderedDrawing, RenderedView};
pub use solid::{solid_from_graph, SolidEvaluator};

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P7-DRAFTING";

/// Extract the resolved variable-set entries stored in a document's
/// `VariableSet` features (the FreeCAD-v1 parameter table that drives the
/// model and the drawing dimensions).
pub fn extract_variables(doc: &oep_geo::Document) -> std::collections::BTreeMap<String, f64> {
    let mut out = std::collections::BTreeMap::new();
    for f in &doc.features {
        if f.kind == "VariableSet" {
            if let Ok(vs) = oep_sketch::features::varset_from_feature(f) {
                for (k, v) in vs.variables {
                    out.insert(k, v);
                }
            }
        }
    }
    out
}

/// Render the first drawing/sheet of `document`, projecting the solid the
/// `graph` replays and resolving dimensions from the document's variable set.
pub fn render_document(
    doc: &oep_geo::Document,
    graph: &oep_geo::feature_graph::FeatureGraph,
) -> Result<RenderedDrawing, RenderError> {
    let vars = extract_variables(doc);
    let mesh = solid_from_graph(graph).map_err(|e| RenderError::NoGeometry(format!("{e:?}")))?;
    let drawing = doc.drawings.first().ok_or(RenderError::NoViews)?;
    let sheet = drawing.sheets.first().ok_or(RenderError::NoViews)?;
    render_sheet(sheet, &mesh, &vars)
}
