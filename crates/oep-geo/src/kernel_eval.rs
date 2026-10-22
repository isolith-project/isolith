//! KERNEL-REPLAY: the real kernel behind the feature-graph [`Evaluator`] seam.
//!
//! [`KernelEvaluator`] implements [`Evaluator`] with real B-Rep solids in
//! place of [`MiniBoxEvaluator`]'s bounding-box folding (P6-M3-T03 + the
//! KERNEL-BASE seam, ADR v1.3.0). A feature graph replays through it into
//! REAL solids computed by the pinned OCCT 8.0.x engine behind the oep-geo
//! kernel seam (`crate::kernel`); the value type is [`KernelValue`].
//!
//! ## Semantics (mirror the documented mini vocabulary, honestly realized)
//!
//! | kind       | effect on the resolved inputs                              |
//! |------------|------------------------------------------------------------|
//! | `Sketch`   | yields a closed planar [`KernelValue::Profile`] from the `profile` param |
//! | `Extrude`  | sweeps the profile (from a preceding `Sketch`, its own `profile` param, or the mini length/width/height fallback) by `height` along +Z |
//! | `Offset`   | rigidly translates the last solid input by `(dx, dy, dz)`  |
//! | `Union`    | exact B-Rep boolean union of all solid inputs             |
//! | `Difference` | solid input 0 minus the rest (exact B-Rep difference)    |
//! | `Trim`     | removes `dz` material from the top of the last solid input (exact boolean difference against a bounding slab) |
//! | `Mirror`   | reflects the last solid input about plane `axis` (`x`/`y`/`z`) at `at` |
//!
//! Unknown kinds and malformed / missing parameters abort the replay after
//! the completed prefix as a typed [`ReplayError`] (P2-M1-T04 tolerant-failure
//! doctrine) -- never a panic, never fabricated geometry: the kernel seam
//! returns `Result`s, and every failure maps to
//! [`ReplayError::Evaluator`].
//!
//! ## Determinism and history
//!
//! `evaluate` is deterministic: same graph x same kernel input yields the same
//! solid measurements, byte-locked via [`KernelEvaluator::canonical_trace`]
//! (the committed `.oepkrt` golden mirrors how the feature-graph v1 text
//! byte-locks its MiniBox trace). Kernel solids themselves are session-local
//! (tokens never serialize), so time navigation over real-solid states
//! re-derives the solid by replaying the state's graph (the P6-M3-T02
//! [`History`] composes a serializable graph state with this evaluator).

use crate::body::BoundingBox;
use crate::feature_graph::{Evaluator, FeatureNode, ReplayError, ReplayStep};
use crate::kernel::{
    make_box, solid_bbox, solid_boolean, solid_extrude, solid_mirror, solid_translate, sys_drop,
    sys_new, BooleanError, BooleanOp, KernelError, MirrorAxis, Point2D, Solid, SysHandle,
    Tolerance,
};

/// The output value type of [`KernelEvaluator`] replay.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelValue {
    /// Empty stock: the evaluator's start value (no material yet).
    Empty,
    /// A closed planar profile (2D polygon in the Z = 0 plane, loop closes
    /// implicitly) produced by a `Sketch` feature.
    Profile(Vec<Point2D>),
    /// A real B-Rep solid produced by the kernel.
    Solid(Solid),
}

/// The real-kernel feature-graph evaluator.
///
/// Owns one kernel session for the whole replay so every feature's solid
/// token stays live for the next feature in the same session (ADR section 5:
/// engine tokens are session-local). Drop closes the session.
pub struct KernelEvaluator {
    sys: SysHandle,
    tolerance: Tolerance,
}

/// The canonical line-oriented serialization of a kernel replay (format tag
/// spoken by [`KernelEvaluator::canonical_trace`]).
pub const TRACE_FORMAT: &str = "oep-kernel-replay-trace";
/// Version of the canonical trace text grammar.
pub const TRACE_VERSION: u64 = 1;

impl KernelEvaluator {
    /// Open a fresh kernel session with the caller-owned tolerance. Without
    /// the `kernel` feature this returns [`KernelError::EngineUnavailable`].
    pub fn new(tolerance: Tolerance) -> Result<Self, KernelError> {
        let sys = sys_new()?;
        Ok(Self { sys, tolerance })
    }

    /// The evaluator's kernel session (for measuring solids after replay).
    pub fn session(&self) -> &SysHandle {
        &self.sys
    }

    /// The caller-owned geometric tolerance used by every kernel operation.
    pub fn tolerance(&self) -> Tolerance {
        self.tolerance
    }

    /// Measure a solid's axis-aligned bounding box against the live session.
    pub fn bbox(&self, solid: &Solid) -> Result<BoundingBox, KernelError> {
        solid_bbox(&self.sys, solid, self.tolerance)
    }

    /// The deterministic, byte-stable canonical text of a replay trace:
    /// every step's feature id and kind plus, for solid steps, the measured
    /// volume, surface area, face/edge/vertex counts and bounding box.
    ///
    /// This is the "byte-locked" form of a kernel replay (the committed
    /// `.oepkrt` golden is byte-compared in the acceptance test). Nothing
    /// session-local (engine tokens, session ids) appears here, so two clean
    /// replays of the same graph serialize to identical bytes.
    pub fn canonical_trace(&self, trace: &[ReplayStep<KernelValue>]) -> String {
        let mut s = String::new();
        s.push_str(&format!("format {TRACE_FORMAT}\n"));
        s.push_str(&format!("version {TRACE_VERSION}\n"));
        s.push_str(&format!("steps {}\n", trace.len()));
        for (i, step) in trace.iter().enumerate() {
            s.push_str(&format!("step {i}\n"));
            s.push_str(&format!("id {}\n", step.id));
            s.push_str(&format!("kind {}\n", step.kind));
            match &step.output {
                KernelValue::Empty => s.push_str("value empty\n"),
                KernelValue::Profile(p) => {
                    s.push_str(&format!("value profile {}\n", p.len()));
                    for (j, pt) in p.iter().enumerate() {
                        s.push_str(&format!("point {j} {} {}\n", pt.x, pt.y));
                    }
                }
                KernelValue::Solid(sol) => {
                    let bb = self
                        .bbox(sol)
                        .map(|b| {
                            format!(
                                "{};{};{};{};{};{}",
                                b.min[0], b.min[1], b.min[2], b.max[0], b.max[1], b.max[2]
                            )
                        })
                        .unwrap_or_else(|e| format!("bbox-error:{e}"));
                    let m = &sol.measures;
                    s.push_str(&format!(
                        "solid volume={};area={};faces={};edges={};verts={};bbox={}\n",
                        m.volume, m.surface_area, m.face_count, m.edge_count, m.vertex_count, bb
                    ));
                }
            }
        }
        s
    }
}

impl Drop for KernelEvaluator {
    fn drop(&mut self) {
        sys_drop(&self.sys);
    }
}

impl Evaluator for KernelEvaluator {
    type Value = KernelValue;

    fn start_value(&mut self) -> KernelValue {
        KernelValue::Empty
    }

    fn evaluate_node(
        &mut self,
        node: &FeatureNode,
        inputs: &[KernelValue],
    ) -> Result<KernelValue, ReplayError> {
        match node.kind.as_str() {
            "Sketch" => {
                let profile = profile_from_param(node)?;
                Ok(KernelValue::Profile(profile))
            }
            "Extrude" => {
                let height = need_num(node, "height")?;
                if !height.is_finite() || height <= 0.0 {
                    return Err(eval_err(format!(
                        "Extrude '{}' height must be finite and positive, got {height}",
                        node.id
                    )));
                }
                let profile = match first_profile(inputs) {
                    Some(p) => p,
                    None => profile_from_param_or_rect(node)?,
                };
                let solid = solid_extrude(&self.sys, &profile, height, self.tolerance)
                    .map_err(kernel_err)?;
                Ok(KernelValue::Solid(solid))
            }
            "Offset" => {
                let solid = last_solid(node, inputs)?.clone();
                let dx = node.param_num("dx").unwrap_or(0.0);
                let dy = node.param_num("dy").unwrap_or(0.0);
                let dz = node.param_num("dz").unwrap_or(0.0);
                let out = solid_translate(&self.sys, &solid, dx, dy, dz, self.tolerance)
                    .map_err(kernel_err)?;
                Ok(KernelValue::Solid(out))
            }
            "Union" => {
                let solids = collect_solids(node, inputs)?;
                let out = if solids.len() == 1 {
                    solids[0].clone()
                } else {
                    solid_boolean(BooleanOp::Union, &self.sys, &solids, self.tolerance)
                        .map_err(boolean_err)?
                };
                Ok(KernelValue::Solid(out))
            }
            "Difference" => {
                let solids = collect_solids(node, inputs)?;
                let out = if solids.len() == 1 {
                    solids[0].clone()
                } else {
                    solid_boolean(BooleanOp::Difference, &self.sys, &solids, self.tolerance)
                        .map_err(boolean_err)?
                };
                Ok(KernelValue::Solid(out))
            }
            "Trim" => {
                let solid = last_solid(node, inputs)?.clone();
                let dz = need_num(node, "dz")?;
                if !dz.is_finite() || dz <= 0.0 {
                    return Err(eval_err(format!(
                        "Trim '{}' dz must be finite and positive, got {dz}",
                        node.id
                    )));
                }
                let bb = self
                    .bbox(&solid)
                    .map_err(|e| eval_err(format!("Trim '{}': {e}", node.id)))?;
                if dz >= bb.max[2] - bb.min[2] {
                    return Err(eval_err(format!(
                        "Trim '{}' would consume the whole solid (dz {dz} >= height {})",
                        node.id,
                        bb.max[2] - bb.min[2]
                    )));
                }
                // Slice the top `dz` off with an overhanging slab: the tool's
                // bottom face sits at the new top plane and its sides stick out
                // past the solid's footprint by a scale-relative margin.
                let m = trim_margin(&bb);
                let tool = make_box(
                    &self.sys,
                    bb.min[0] - m,
                    bb.min[1] - m,
                    bb.max[2] - dz,
                    bb.max[0] + m,
                    bb.max[1] + m,
                    bb.max[2] + m,
                    self.tolerance,
                )
                .map_err(kernel_err)?;
                let out = solid_boolean(
                    BooleanOp::Difference,
                    &self.sys,
                    &[solid, tool],
                    self.tolerance,
                )
                .map_err(boolean_err)?;
                Ok(KernelValue::Solid(out))
            }
            "Mirror" => {
                let solid = last_solid(node, inputs)?;
                let axis_s = node.param_str("axis").ok_or_else(|| {
                    eval_err(format!("Mirror '{}' needs an 'axis' param", node.id))
                })?;
                let axis = match axis_s {
                    "x" => MirrorAxis::X,
                    "y" => MirrorAxis::Y,
                    "z" => MirrorAxis::Z,
                    other => {
                        return Err(eval_err(format!(
                            "Mirror '{}' has unknown axis {other:?}",
                            node.id
                        )))
                    }
                };
                let at = need_num(node, "at")?;
                let out =
                    solid_mirror(&self.sys, solid, axis, at, self.tolerance).map_err(kernel_err)?;
                Ok(KernelValue::Solid(out))
            }
            other => Err(ReplayError::UnknownKind {
                feature: node.id.clone(),
                kind: other.to_string(),
            }),
        }
    }
}

/// A required numeric parameter, mapped to a typed [`ReplayError`].
fn need_num(node: &FeatureNode, key: &str) -> Result<f64, ReplayError> {
    node.param_num(key)
        .ok_or_else(|| eval_err(format!("feature '{}' is missing param '{key}'", node.id)))
}

/// A typed evaluator-level replay error (tolerant abort, never a panic).
fn eval_err(msg: String) -> ReplayError {
    ReplayError::Evaluator(msg)
}

/// Map a kernel seam failure into a replay error carrying its message.
fn kernel_err(e: KernelError) -> ReplayError {
    eval_err(e.to_string())
}

/// Map a boolean seam failure into a replay error carrying its message.
fn boolean_err(e: BooleanError) -> ReplayError {
    eval_err(e.to_string())
}

/// Parse a `profile` param (`"x1,y1;x2,y2;..."`, loop closes implicitly) into
/// a closed polygon in the Z = 0 plane. Strict: at least three points, every
/// coordinate finite. Malformed input maps to a typed error, never a panic.
fn profile_from_param(node: &FeatureNode) -> Result<Vec<Point2D>, ReplayError> {
    let raw = node.param_str("profile").ok_or_else(|| {
        eval_err(format!(
            "feature '{}' (kind {}) needs a 'profile' param",
            node.id, node.kind
        ))
    })?;
    parse_profile(raw).ok_or_else(|| {
        eval_err(format!(
            "feature '{}' has a malformed 'profile' param {raw:?}",
            node.id
        ))
    })
}

/// Parse `"x1,y1;x2,y2;..."` into points; `None` when not at least three
/// finite points or the string is structurally invalid.
pub(crate) fn parse_profile(raw: &str) -> Option<Vec<Point2D>> {
    let mut pts = Vec::new();
    for token in raw.split(';') {
        if token.trim().is_empty() {
            return None;
        }
        let (xs, ys) = token.split_once(',')?;
        let x: f64 = xs.trim().parse().ok()?;
        let y: f64 = ys.trim().parse().ok()?;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        pts.push(Point2D::new(x, y));
    }
    if pts.len() < 3 {
        return None;
    }
    Some(pts)
}

/// The first [`KernelValue::Profile`] among the resolved inputs.
fn first_profile(inputs: &[KernelValue]) -> Option<Vec<Point2D>> {
    inputs.iter().find_map(|v| match v {
        KernelValue::Profile(p) => Some(p.clone()),
        _ => None,
    })
}

/// Extrude profile resolution that also honours the mini-vocabulary rectangle
/// fallback (`length` x `width` at the origin) used by the bracket story.
fn profile_from_param_or_rect(node: &FeatureNode) -> Result<Vec<Point2D>, ReplayError> {
    if node.param_str("profile").is_some() {
        return profile_from_param(node);
    }
    let length = need_num(node, "length")?;
    let width = need_num(node, "width")?;
    if ![length, width].iter().all(|v| v.is_finite()) || length <= 0.0 || width <= 0.0 {
        return Err(eval_err(format!(
            "Extrude '{}' rectangle fallback needs finite positive length/width",
            node.id
        )));
    }
    Ok(vec![
        Point2D::new(0.0, 0.0),
        Point2D::new(length, 0.0),
        Point2D::new(length, width),
        Point2D::new(0.0, width),
    ])
}

/// The last solid among the resolved inputs (the mini semantics for the
/// single-operand ops), or a typed error when none is present.
fn last_solid<'a>(node: &FeatureNode, inputs: &'a [KernelValue]) -> Result<&'a Solid, ReplayError> {
    inputs
        .iter()
        .rev()
        .find_map(|v| match v {
            KernelValue::Solid(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| {
            eval_err(format!(
                "feature '{}' (kind {}) needs a solid input",
                node.id, node.kind
            ))
        })
}

/// Collect every solid input in declaration order for the multi-operand
/// booleans, or a typed error when any input is not a solid.
fn collect_solids(node: &FeatureNode, inputs: &[KernelValue]) -> Result<Vec<Solid>, ReplayError> {
    if inputs.is_empty() {
        return Err(eval_err(format!(
            "feature '{}' (kind {}) needs at least one input",
            node.id, node.kind
        )));
    }
    let mut out = Vec::with_capacity(inputs.len());
    for (i, v) in inputs.iter().enumerate() {
        match v {
            KernelValue::Solid(s) => out.push(s.clone()),
            other => {
                return Err(eval_err(format!(
                    "feature '{}' (kind {}) input {i} is not a solid ({})",
                    node.id,
                    node.kind,
                    value_kind(other)
                )))
            }
        }
    }
    Ok(out)
}

/// A short human label for a value's variant (error messages only).
fn value_kind(v: &KernelValue) -> &'static str {
    match v {
        KernelValue::Empty => "empty stock",
        KernelValue::Profile(_) => "a profile",
        KernelValue::Solid(_) => "a solid",
    }
}

/// A scale-relative margin so a trim slab side overhangs the solid footprint
/// deterministically (the cut plane is what matters, not the margin value).
fn trim_margin(bb: &BoundingBox) -> f64 {
    let span = (bb.max[0] - bb.min[0])
        .max(bb.max[1] - bb.min[1])
        .max(bb.max[2] - bb.min[2])
        .max(1.0);
    1e-3 * span
}
