//! The real-kernel seam for drafting: replay the `oep-geo` feature graph into
//! a solid [`Mesh`].
//!
//! [`SolidEvaluator`] implements [`oep_geo::feature_graph::Evaluator`] with
//! [`Mesh`] as the value type (the in-crate [`MiniBoxEvaluator`] reference
//! stand-in folds each feature into a *bounding box*; drafting needs actual
//! edge geometry, so this evaluator builds real triangle meshes). The
//! vocabulary is the mini feature-tree vocabulary documented in
//! `docs/geometry-feature-graph.md`:
//!
//! - `Extrude` — union the resolved inputs with a box of size
//!   `length × width × height`.
//! - `Offset` — translate the last input by `(dx, dy, dz)`.
//! - `Union` — concatenate all inputs into one mesh.
//! - `Mirror` — reflect the last input about the plane `axis` at `at`.
//! - `Trim` — a true mesh cut is NOT representable on the tessellated lane
//!   (it is a kernel-boolean op, P6-M3-T04); this evaluator returns a typed
//!   [`ReplayError::Evaluator`] rather than fabricating a cut.
//!
//! Every view projected in `oep-drafting` is computed from the mesh this
//! evaluator produces — no fabricated drawing output, per the card acceptance.

use oep_geo::feature_graph::{Evaluator, FeatureNode, ReplayError};

use crate::mesh::Mesh;

/// Replay a feature graph into a solid mesh with [`SolidEvaluator`],
/// returning the final value or the partial-trace [`ReplayError`] the
/// evaluator stopped at (tolerant-failure doctrine — never panics).
pub fn solid_from_graph(graph: &oep_geo::feature_graph::FeatureGraph) -> Result<Mesh, ReplayError> {
    let result = graph.evaluate(&mut SolidEvaluator::new());
    match result.error {
        None => result
            .trace
            .last()
            .map(|s| s.output.clone())
            .ok_or_else(|| ReplayError::Evaluator("feature graph produced no output".into())),
        Some(e) => Err(e),
    }
}

/// A mesh-producing [`Evaluator`] over the mini feature-tree vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct SolidEvaluator;

impl SolidEvaluator {
    /// A fresh solid evaluator (empty start stock).
    pub fn new() -> Self {
        Self
    }
}

impl Default for SolidEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator for SolidEvaluator {
    type Value = Mesh;

    fn start_value(&mut self) -> Mesh {
        Mesh::new()
    }

    fn evaluate_node(&mut self, node: &FeatureNode, inputs: &[Mesh]) -> Result<Mesh, ReplayError> {
        match node.kind.as_str() {
            "Extrude" => {
                let length = need(node, "length")?;
                let width = need(node, "width")?;
                let height = need(node, "height")?;
                let box_mesh = Mesh::box_solid(length, width, height);
                let mut acc = Mesh::new();
                // Union the box with the resolved inputs (envelope semantics of
                // the MiniBox reference vocabulary, at the mesh level).
                for input in inputs {
                    acc = acc.extended(input);
                }
                Ok(acc.extended(&box_mesh))
            }
            "Offset" => {
                let b = inputs
                    .last()
                    .ok_or_else(|| {
                        ReplayError::Evaluator(format!("Offset '{}' needs an input", node.id))
                    })?
                    .translated(
                        node.param_num("dx").unwrap_or(0.0),
                        node.param_num("dy").unwrap_or(0.0),
                        node.param_num("dz").unwrap_or(0.0),
                    );
                Ok(b)
            }
            "Union" => {
                if inputs.is_empty() {
                    return Err(ReplayError::Evaluator(format!(
                        "Union '{}' needs at least one input",
                        node.id
                    )));
                }
                let mut acc = Mesh::new();
                for input in inputs {
                    acc = acc.extended(input);
                }
                Ok(acc)
            }
            "Mirror" => {
                let axis = node.param_str("axis").ok_or_else(|| {
                    ReplayError::Evaluator(format!("Mirror '{}' needs an axis param", node.id))
                })?;
                let at = need(node, "at")?;
                let c = axis.chars().next().unwrap_or('\0');
                if c != 'x' && c != 'y' && c != 'z' {
                    return Err(ReplayError::Evaluator(format!(
                        "Mirror '{}' has unknown axis {:?}",
                        node.id, axis
                    )));
                }
                let b = inputs.last().ok_or_else(|| {
                    ReplayError::Evaluator(format!("Mirror '{}' needs an input", node.id))
                })?;
                Ok(b.reflected(c, at))
            }
            "Trim" => Err(ReplayError::Evaluator(format!(
                "Trim '{}' needs a kernel boolean (P6-M3-T04), not representable on the \
                 tessellated drafting lane",
                node.id
            ))),
            other => Err(ReplayError::UnknownKind {
                feature: node.id.clone(),
                kind: other.to_string(),
            }),
        }
    }
}

/// Read a required numeric parameter, mapping absence to an `Evaluator` error.
fn need(node: &FeatureNode, key: &str) -> Result<f64, ReplayError> {
    node.param_num(key).ok_or_else(|| {
        ReplayError::Evaluator(format!("feature '{}' is missing param '{key}'", node.id))
    })
}
