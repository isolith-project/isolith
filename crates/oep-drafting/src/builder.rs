//! The acceptance scenario: drive a solid model AND its drawing from one
//! parametric variable set.
//!
//! [`drive_bracket_drawing`] is the EDITOR-2D-DRAFTING acceptance driver: it
//! solves an `oep-sketch` [`Sketch`] against a [`VariableSet`] (FreeCAD-v1
//! semantics), materializes BOTH the 3D model (a canonical `oep-geo`
//! [`Document`] with part/body + `Sketch`/`VariableSet`/`Extrude` features)
//! and the drawing record (an `oep-geo` [`Drawing`] bound to that part), and
//! builds the [`FeatureGraph`] the drafting workbench replays into a solid
//! mesh. An edit to a variable re-solves, re-drives the model, and the redrawn
//! dimension follows — the card acceptance.
//!
//! The drawing's dimensions reference the variable-set entries BY NAME; the
//! drawn value is resolved from the same entries that drive the model.

use oep_geo::feature_graph::{FeatureGraph, FeatureNode};
use oep_geo::{
    Body, BodyKind, BoundingBox, Dimension, DimensionKind, Document, Drawing, Feature,
    ProjectionType, Sheet, SheetSize, TitleBlock, View, ViewKind,
};
use oep_sketch::features::{sketch_to_feature, varset_to_feature};
use oep_sketch::sketch::Sketch;
use oep_sketch::solve::{solve_sketch, SketchError};
use oep_sketch::varset::VariableSet;

/// A driven model + its drawing + the solid-producing feature graph.
#[derive(Debug, Clone)]
pub struct BracketDrawing {
    /// The canonical model document (part, body, features, drawing).
    pub document: Document,
    /// The feature graph the workbench replays into a solid mesh.
    pub graph: FeatureGraph,
}

/// Resolve one spine variable to the value that drives the model.
///
/// FreeCAD-v1 semantics: the variable-set entry IS the model dimension, so the
/// variable is authoritative (exact, e.g. `width = 80`); the solved sketch
/// geometry is the fallback and the renderer cross-checks it. Order: variable
/// set first (the same parameter the drawing dimension labels), then the
/// solved sketch dimension.
fn resolve_spine(
    outcome: &oep_sketch::solve::SolveOutcome,
    vars: &VariableSet,
    name: &str,
) -> Result<f64, SketchError> {
    if let Ok(v) = vars.get(name) {
        return Ok(v);
    }
    outcome
        .driven(name)
        .or_else(|| outcome.length_of(name))
        .ok_or_else(|| {
            SketchError::Message(format!(
                "spine '{name}' is neither a variable-set entry nor a solved sketch dimension"
            ))
        })
}

/// Solve `sketch` against `vars`, then drive a solid box model (spines named
/// by `length_var`/`width_var`/`depth_var`) plus a three-view third-angle
/// drawing with variable-driven dimensions.
///
/// The drawing sheet carries:
/// - a front/top/right view of the part,
/// - a horizontal dimension on the TOP view driven by `length_var`,
/// - a vertical dimension on the TOP view driven by `width_var`,
/// - a vertical dimension on the FRONT view driven by `depth_var`.
///
/// The card-acceptance driver: many named inputs by design (it is the single
/// documented entry point that builds a model + drawing from one variable set,
/// not a hot path; clippy's arg threshold is waived with rationale).
#[allow(clippy::too_many_arguments)]
pub fn drive_bracket_drawing(
    part_id: &str,
    part_name: &str,
    sketch: &Sketch,
    vars: &VariableSet,
    length_var: &str,
    width_var: &str,
    depth_var: &str,
    title: &str,
    part_number: Option<&str>,
    material: Option<&str>,
    revision: Option<&str>,
    drawn_by: Option<&str>,
    date: Option<&str>,
    company: Option<&str>,
) -> Result<BracketDrawing, SketchError> {
    let outcome = solve_sketch(sketch, vars)?;
    let length = resolve_spine(&outcome, vars, length_var)?;
    let width = resolve_spine(&outcome, vars, width_var)?;
    let depth = resolve_spine(&outcome, vars, depth_var)?;
    if length <= 0.0 || width <= 0.0 || depth <= 0.0 {
        return Err(SketchError::InvalidDimension(
            [length, width, depth]
                .into_iter()
                .fold(0.0f64, f64::min)
                .min(0.0),
        ));
    }

    let sketch_feat = sketch_to_feature(sketch);
    let vars_feat = varset_to_feature(vars);
    let extrude_feat = Feature::new(
        format!("oep-feature-{}-extrude", sanitize(part_id)),
        format!("extrude-{part_name}"),
        "Extrude",
    )
    .with_param(
        "driven_by",
        format!("{}:{}", sketch_feat.kind, sketch_feat.id),
    )
    .with_param("extent", depth)
    .with_body(format!("oep-body-{}", sanitize(part_id)));

    // The solid-producing feature graph (replayed by the workbench).
    let graph_root = FeatureNode::new(
        format!("oep-feature-{}-extrude", sanitize(part_id)),
        format!("extrude-{part_name}"),
        "Extrude",
    )
    .num("length", length)
    .num("width", width)
    .num("height", depth);
    let mut graph = FeatureGraph::new();
    graph
        .add_root(graph_root)
        .expect("fresh graph, unique root id");

    let body_id = format!("oep-body-{}", sanitize(part_id));
    let body = Body::new(&body_id, format!("{part_name} solid"), BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [length, width, depth]))
        .with_feature(&sketch_feat.id)
        .with_feature(&vars_feat.id)
        .with_feature(&extrude_feat.id);

    let part = oep_geo::Part::new(part_id, part_name)
        .with_part_number(part_number.unwrap_or(part_id))
        .with_body(&body_id)
        .with_feature(&sketch_feat.id)
        .with_feature(&vars_feat.id)
        .with_feature(&extrude_feat.id);

    // The drawing record (schema v2, Class 2 extension).
    let drawing = Drawing::new(
        format!("oep-drawing-{}", sanitize(part_id)),
        format!("{part_name} drawing"),
    )
    .with_sheet(
        Sheet::new("oep-sheet-1", "Sheet 1")
            .with_size(SheetSize::A4_LANDSCAPE)
            .with_projection(ProjectionType::ThirdAngle)
            .with_title_block(
                TitleBlock::new(title)
                    .with_part_number(part_number.unwrap_or(part_id))
                    .with_material(material.unwrap_or(""))
                    .with_scale("1:1")
                    .with_revision(revision.unwrap_or("A"))
                    .with_drawn_by(drawn_by.unwrap_or("oep-drafting"))
                    .with_date(date.unwrap_or(""))
                    .with_sheet("1 of 1")
                    .with_company(company.unwrap_or("Isolith")),
            )
            .with_view(View::new("oep-view-front", "Front", ViewKind::Front).with_part(part_id))
            .with_view(View::new("oep-view-top", "Top", ViewKind::Top).with_part(part_id))
            .with_view(View::new("oep-view-right", "Right", ViewKind::Right).with_part(part_id))
            .with_dimension(
                Dimension::new(
                    "oep-dim-length",
                    format!("{part_name} length"),
                    "oep-view-top",
                    DimensionKind::Horizontal,
                )
                .with_variable(length_var),
            )
            .with_dimension(
                Dimension::new(
                    "oep-dim-width",
                    format!("{part_name} width"),
                    "oep-view-top",
                    DimensionKind::Vertical,
                )
                .with_variable(width_var),
            )
            .with_dimension(
                Dimension::new(
                    "oep-dim-depth",
                    format!("{part_name} depth"),
                    "oep-view-front",
                    DimensionKind::Vertical,
                )
                .with_variable(depth_var),
            ),
    );

    let mut doc = Document::new("mm");
    doc.push_feature(sketch_feat);
    doc.push_feature(vars_feat);
    doc.push_feature(extrude_feat);
    doc.push_body(body);
    doc.push_part(part);
    doc.push_drawing(drawing);

    let errors = doc.validate();
    assert!(
        errors.is_empty(),
        "driven document must validate: {errors:?}"
    );

    Ok(BracketDrawing {
        document: doc,
        graph,
    })
}

/// Rebuses an id into an id-safe component (oep-geo id discipline).
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
