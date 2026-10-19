//! Driving a MODEL dimension from a solved sketch (the card acceptance).
//!
//! [`drive_sketch_to_document`] solves a sketch against a variable set and
//! materializes the result as a canonical `oep-geo` [`Document`]: a part
//! owning a solid body whose width (bbox) equals the solved sketch dimension
//! named by `dimension`, plus the `Sketch` / `VariableSet` features in the
//! part's history. Editing a variable and re-driving updates the model
//! dimension: the "sketch parametrically drives a model dimension" acceptance
//! of P7-M4-T01, in the chosen shell's own semantics.

use crate::features::{sketch_to_feature, varset_to_feature, SKETCH_KIND};
use crate::sketch::Sketch;
use crate::solve::{solve_sketch, SketchError};
use crate::varset::VariableSet;
use oep_geo::{Body, BodyKind, BoundingBox, Document, Feature, Part};

/// Solve a sketch + variable set, then drive a model body width from the solved
/// dimension `dimension` (a line id whose length is dimensioned).
///
/// Produces a canonical oep-geo document: one part, one solid body whose width
/// equals the solved dimension, and the `Sketch` + `VariableSet` features in
/// the part history. Returns a typed error when the sketch is not solvable.
pub fn drive_sketch_to_document(
    part_id: &str,
    part_name: &str,
    sketch: &Sketch,
    vars: &VariableSet,
    dimension: &str,
) -> Result<Document, SketchError> {
    let outcome = solve_sketch(sketch, vars)?;
    // The dimension is addressed by its VARIABLE name first (FreeCAD-v1
    // semantics: "width" is the variable-set entry the length references),
    // falling back to a raw line id for literal-length sketches.
    let width = outcome
        .driven(dimension)
        .or_else(|| outcome.length_of(dimension))
        .ok_or_else(|| {
            SketchError::Message(format!(
                "dimension '{dimension}' is neither a variable-set entry nor a line id"
            ))
        })?;
    // Model height from the solved top-right corner (p2 = (width, height)).
    let height = outcome.point("p2").map(|(_, y)| y).unwrap_or(1.0);

    let sketch_feat = sketch_to_feature(sketch);
    let vars_feat = varset_to_feature(vars);

    let body_id = format!("oep-body-{}", sanitize(part_id));
    let body = Body::new(&body_id, format!("{part_name} solid"), BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [width, height, 1.0]))
        .with_feature(&sketch_feat.id)
        .with_feature(&vars_feat.id);

    let part = Part::new(part_id, part_name)
        .with_body(&body_id)
        .with_feature(&sketch_feat.id)
        .with_feature(&vars_feat.id);

    let mut doc = Document::new("mm");
    doc.push_feature(sketch_feat);
    doc.push_feature(vars_feat);
    doc.push_body(body);
    doc.push_part(part);
    Ok(doc)
}

/// The width of the model body driven by a sketch dimension.
///
/// `part_id` is the part id passed to [`drive_sketch_to_document`]; the body
/// id is derived from it the same way that function derives it.
pub fn body_width(doc: &Document, part_id: &str) -> f64 {
    let body_id = format!("oep-body-{}", sanitize(part_id));
    doc.bodies
        .iter()
        .find(|b| b.id == body_id)
        .and_then(|b| b.bbox)
        .map(|bb| bb.max[0] - bb.min[0])
        .unwrap_or(f64::NAN)
}

/// Materialize the driven dimension as an `Extrude` feature parameter, so the
/// "model dimension" is a first-class oep-geo feature parameter (not only the
/// body bbox). Returns the feature for the caller to attach to a part.
pub fn driven_feature(
    sketch: &Sketch,
    vars: &VariableSet,
    dimension: &str,
) -> Result<Feature, SketchError> {
    let outcome = solve_sketch(sketch, vars)?;
    let width = outcome
        .driven(dimension)
        .or_else(|| outcome.length_of(dimension))
        .ok_or_else(|| {
            SketchError::Message(format!(
                "dimension '{dimension}' is neither a variable-set entry nor a line id"
            ))
        })?;
    Ok(Feature::new(
        format!("oep-feature-drive-{dimension}"),
        format!("sketch-drive-{dimension}"),
        "Extrude",
    )
    .with_param("driven_by", format!("{SKETCH_KIND}:{}", sketch.id))
    .with_param("dimension", dimension)
    .with_param("extent", width))
}

/// Rebuses an id into a body id component (keeps the oep-geo id discipline:
/// alphanumeric plus hyphen only).
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
