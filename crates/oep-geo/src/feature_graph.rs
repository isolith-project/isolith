//! Feature graph (history) model with replay — a mini feature tree for native
//! open models (P6-M3-T03).
//!
//! The [`FeatureGraph`] is the design-history of a native OEP part: a rooted
//! **mini feature tree** recorded in creation order. Each [`FeatureNode`] is
//! one modelling operation with an **open** PascalCase `kind` vocabulary
//! (`Extrude`, `Offset`, `Union`, `Trim`, `Mirror`, ...) so readers and
//! kernels can negotiate without a closed enum. This deliberately is **not**
//! CATIA semantics: there is no knowledgeware step, no constraint closure, no
//! hidden solver state — the graph is an explicit, serializable, replayable
//! record of operations, exactly repeatable from its own bytes.
//!
//! ## Replay
//!
//! [`FeatureGraph::evaluate`] re-derives the model state from the graph:
//! walking nodes in creation order, resolving each node's inputs (the outputs
//! of its explicit `refs`, else its parent's output, else the evaluator's
//! `start_value` for roots) and handing them to a pluggable [`Evaluator`].
//! The strictly-backward `refs` invariant (enforced by [`FeatureGraph::validate`])
//! makes the graph a DAG and replay deterministic with no topological sort.
//! Replay is **tolerant** (P2-M1-T04 doctrine): an unknown kind, a missing or
//! forward dependency aborts *after* the completed prefix and returns the
//! partial trace alongside the error — it never panics.
//!
//! [`MiniBoxEvaluator`] is an in-crate reference evaluator that folds each
//! feature into a bounding box; it stands in for the real geometry kernel
//! until the P6-M3-T01 Open CASCADE binding lands. The committed golden
//! (`tests/golden/bracket-feature-graph.oepfg`) is produced by the
//! `gen_feature_graph_golden` example and independently re-derived by the
//! stdlib Python acceptance test — a self-consistent but wrong evaluator
//! cannot satisfy both sides.
//!
//! ## Own format (`oep-feature-graph` v1)
//!
//! Deterministic line-oriented text (std-only, no serde dependency — chosen
//! to keep this card free of `Cargo.toml`/`Cargo.lock` churn so it merges
//! cleanly with the concurrently-running P6-M1/P6-M2 cards, mirroring the
//! P6-M2-T01 vendored-math decision). The text carries the graph **and** its
//! canonical replay trace (derived live by [`FeatureGraph::to_text`] via
//! [`MiniBoxEvaluator`]), so structure and behaviour are byte-locked together.
//! [`FeatureGraph::from_text`] decodes strictly and never panics: malformed
//! structure, bad counts, non-finite numbers and empty ids yield `None`, while
//! unknown `kind`s and unlinked ids still decode (open vocabulary + tolerant
//! linking; semantic checks live in [`FeatureGraph::validate`] and replay).

use std::fmt;

/// The format tag spoken by [`FeatureGraph::to_text`] / `from_text`.
pub const FORMAT: &str = "oep-feature-graph";
/// Version of the own-format text grammar.
pub const VERSION: u64 = 1;

/// A 3D axis-aligned bounding box `[minx, miny, minz, maxx, maxy, maxz]`.
///
/// The [`MiniBoxEvaluator`] value type and the unit of the replay trace.
pub type BBox = [f64; 6];

/// One keyed parameter value of a feature: a number or a short identifier.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// Numeric parameter (f64, double-everywhere per P6-M1-T03).
    Num(f64),
    /// Short identifier string (e.g. a plane `"yz"` or a profile reference).
    Str(String),
}

impl ParamValue {
    /// The number if this value is [`ParamValue::Num`].
    pub fn num(&self) -> Option<f64> {
        match self {
            ParamValue::Num(n) => Some(*n),
            ParamValue::Str(_) => None,
        }
    }

    /// The string if this value is [`ParamValue::Str`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ParamValue::Num(_) => None,
            ParamValue::Str(s) => Some(s),
        }
    }
}

/// Errors produced by graph construction and structural validation.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// Two nodes carry the same document-wide id.
    DuplicateId(String),
    /// A node's `parent` names an id that is not in the graph.
    UnknownParent {
        /// The offending child id.
        child: String,
        /// The missing parent id.
        parent: String,
    },
    /// A node's `refs` names an id that is not in the graph.
    UnknownRef {
        /// The offending feature id.
        feature: String,
        /// The missing reference id.
        reference: String,
    },
    /// A node's `refs` points at a feature defined *later* in creation order,
    /// breaking the strictly-backward invariant (replay would be ambiguous).
    ForwardRef {
        /// The offending feature id.
        feature: String,
        /// The forward reference id.
        reference: String,
    },
    /// A numeric parameter is NaN/±inf and cannot be serialized.
    NonFiniteParam {
        /// The owning feature id.
        feature: String,
        /// The offending parameter key.
        key: String,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::DuplicateId(id) => write!(f, "duplicate id {id:?}"),
            GraphError::UnknownParent { child, parent } => {
                write!(f, "feature {child:?} has unknown parent {parent:?}")
            }
            GraphError::UnknownRef { feature, reference } => {
                write!(f, "feature {feature:?} references unknown id {reference:?}")
            }
            GraphError::ForwardRef { feature, reference } => {
                write!(f, "feature {feature:?} forward-references {reference:?}")
            }
            GraphError::NonFiniteParam { feature, key } => {
                write!(f, "feature {feature:?} has non-finite param {key:?}")
            }
        }
    }
}

/// One node of the feature tree: a single modelling operation in creation
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureNode {
    /// Unique document-wide id (`oep-feature-…` convention).
    pub id: String,
    /// Human-readable feature name.
    pub name: String,
    /// Open PascalCase operation vocabulary (e.g. `Extrude`, `Mirror`).
    /// Unknown kinds never break decode or validate; replay reports them.
    pub kind: String,
    /// Id of the parent feature, or `None` for the root of the tree.
    pub parent: Option<String>,
    /// Ids of explicit input features (dependency edges). Must be strictly
    /// earlier in creation order (see [`FeatureGraph::validate`]).
    pub refs: Vec<String>,
    /// Parameters, always kept sorted by key for deterministic output.
    pub params: Vec<(String, ParamValue)>,
}

impl FeatureNode {
    /// A leaf-with-everything-empty operation node.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            parent: None,
            refs: Vec::new(),
            params: Vec::new(),
        }
    }

    /// Builder: set the parent id (use [`FeatureGraph::add_child`] instead to
    /// keep structure verified).
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent = Some(id.into());
        self
    }

    /// Builder: add an explicit input (dependency) feature id.
    pub fn with_ref(mut self, id: impl Into<String>) -> Self {
        self.refs.push(id.into());
        self
    }

    /// Builder: add a parameter value, kept sorted by key.
    pub fn param(mut self, key: impl Into<String>, value: ParamValue) -> Self {
        self.insert_param(key.into(), value);
        self
    }

    /// Builder: add a numeric parameter.
    pub fn num(mut self, key: impl Into<String>, value: f64) -> Self {
        self.insert_param(key.into(), ParamValue::Num(value));
        self
    }

    /// Builder: add a string parameter.
    pub fn text(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.insert_param(key.into(), ParamValue::Str(value.into()));
        self
    }

    /// Numeric value of a parameter, or `None` if absent / not numeric.
    pub fn param_num(&self, key: &str) -> Option<f64> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.num())
    }

    /// String value of a parameter, or `None` if absent / not a string.
    pub fn param_str(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_str())
    }

    fn insert_param(&mut self, key: String, value: ParamValue) {
        let pos = self
            .params
            .binary_search_by(|(k, _)| k.as_str().cmp(key.as_str()))
            .unwrap_or_else(|e| e);
        self.params.insert(pos, (key, value));
    }
}

/// The mini feature tree: all nodes in creation order (`index == position`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeatureGraph {
    /// Nodes in creation order; a node's position is its replay index.
    pub nodes: Vec<FeatureNode>,
}

/// A completed replay step: one node's identity and its evaluated output.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayStep<V> {
    /// The feature id that produced this step.
    pub id: String,
    /// The feature kind that produced this step.
    pub kind: String,
    /// The evaluated output value.
    pub output: V,
}

/// Replay errors. Data-level errors abort after the completed prefix.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    /// A node's kind is not understood by the evaluator (open vocabulary).
    UnknownKind {
        /// The offending feature id.
        feature: String,
        /// The unknown kind string.
        kind: String,
    },
    /// A dependency id could not be resolved to an output.
    MissingDependency {
        /// The offending feature id.
        feature: String,
        /// The unresolvable reference id.
        reference: String,
    },
    /// A dependency points forward in creation order.
    ForwardReference {
        /// The offending feature id.
        feature: String,
        /// The forward reference id.
        reference: String,
    },
    /// An evaluator-level condition (e.g. a missing required parameter).
    Evaluator(String),
}

/// The outcome of a replay: the completed trace and, on failure, the error
/// that stopped it (partial trace retained — tolerant-failure doctrine).
#[derive(Debug)]
pub struct ReplayResult<V> {
    /// Evaluated steps in replay (creation) order; on error, only nodes that
    /// completed before the failure are present.
    pub trace: Vec<ReplayStep<V>>,
    /// `None` when the whole graph replayed; `Some(error)` when it stopped.
    pub error: Option<ReplayError>,
}

/// Kernel-agnostic replay seam: a type that turns one feature + its resolved
/// inputs into that feature's output.
///
/// The real geometry kernel (Open CASCADE bindings, P6-M3-T01) implements this
/// trait; the in-crate [`MiniBoxEvaluator`] is the reference stand-in.
pub trait Evaluator {
    /// The output value type (a bbox, a B-Rep handle, ...).
    type Value: Clone + PartialEq + fmt::Debug;

    /// The initial model state fed to roots with no refs and no parent
    /// (e.g. empty stock).
    fn start_value(&mut self) -> Self::Value;

    /// Compute one feature's output from its resolved inputs (in the order the
    /// refs were declared; a feature with no refs receives `[parent_output]`).
    fn evaluate_node(
        &mut self,
        node: &FeatureNode,
        inputs: &[Self::Value],
    ) -> Result<Self::Value, ReplayError>;
}

impl FeatureGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of features in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// `true` when the graph has no features.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate nodes in creation (replay) order.
    pub fn iter(&self) -> std::slice::Iter<'_, FeatureNode> {
        self.nodes.iter()
    }

    /// Look up a node by id.
    pub fn node(&self, id: &str) -> Option<&FeatureNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The creation-order position of a node id.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Append a root node (no parent), refusing duplicate ids and nodes that
    /// declare a parent.
    pub fn add_root(&mut self, node: FeatureNode) -> Result<(), GraphError> {
        if self.position(&node.id).is_some() {
            return Err(GraphError::DuplicateId(node.id));
        }
        if node.parent.is_some() {
            return Err(GraphError::UnknownParent {
                child: node.id,
                parent: node.parent.unwrap_or_default(),
            });
        }
        self.nodes.push(node);
        Ok(())
    }

    /// Append a node as a child of an existing feature, setting its parent and
    /// refusing duplicate ids and unknown parents.
    pub fn add_child(&mut self, parent_id: &str, mut node: FeatureNode) -> Result<(), GraphError> {
        if self.position(&node.id).is_some() {
            return Err(GraphError::DuplicateId(node.id));
        }
        if self.position(parent_id).is_none() {
            return Err(GraphError::UnknownParent {
                child: node.id,
                parent: parent_id.to_string(),
            });
        }
        node.parent = Some(parent_id.to_string());
        self.nodes.push(node);
        Ok(())
    }

    /// Structural validation: unique ids, known parents, known refs, and the
    /// strictly-backward refs invariant (⇒ DAG, deterministic replay).
    pub fn validate(&self) -> Result<(), Vec<GraphError>> {
        let mut errs = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            if self.nodes[..i].iter().any(|m| m.id == n.id) {
                errs.push(GraphError::DuplicateId(n.id.clone()));
            }
            if let Some(p) = &n.parent {
                if self.position(p).is_none() {
                    errs.push(GraphError::UnknownParent {
                        child: n.id.clone(),
                        parent: p.clone(),
                    });
                }
            }
            for r in &n.refs {
                match self.position(r) {
                    None => errs.push(GraphError::UnknownRef {
                        feature: n.id.clone(),
                        reference: r.clone(),
                    }),
                    Some(j) if j >= i => errs.push(GraphError::ForwardRef {
                        feature: n.id.clone(),
                        reference: r.clone(),
                    }),
                    Some(_) => {}
                }
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// Deterministic replay: evaluate every feature in creation order through
    /// the given [`Evaluator`], feeding each node the outputs of its refs
    /// (else parent's output, else `start_value`). On error, the returned
    /// [`ReplayResult`] carries the completed prefix trace, never panicking.
    pub fn evaluate<E: Evaluator>(&self, engine: &mut E) -> ReplayResult<E::Value> {
        let mut outputs: Vec<Option<E::Value>> = vec![None; self.nodes.len()];
        let mut trace: Vec<ReplayStep<E::Value>> = Vec::with_capacity(self.nodes.len());

        for (i, n) in self.nodes.iter().enumerate() {
            let inputs = if !n.refs.is_empty() {
                let mut v = Vec::with_capacity(n.refs.len());
                for r in &n.refs {
                    let j = match self.position(r) {
                        None => {
                            return ReplayResult {
                                trace,
                                error: Some(ReplayError::MissingDependency {
                                    feature: n.id.clone(),
                                    reference: r.clone(),
                                }),
                            };
                        }
                        Some(j) => j,
                    };
                    if j >= i {
                        return ReplayResult {
                            trace,
                            error: Some(ReplayError::ForwardReference {
                                feature: n.id.clone(),
                                reference: r.clone(),
                            }),
                        };
                    }
                    match &outputs[j] {
                        Some(o) => v.push(o.clone()),
                        None => {
                            return ReplayResult {
                                trace,
                                error: Some(ReplayError::MissingDependency {
                                    feature: n.id.clone(),
                                    reference: r.clone(),
                                }),
                            };
                        }
                    }
                }
                v
            } else if let Some(p) = &n.parent {
                let j = match self.position(p) {
                    None => {
                        return ReplayResult {
                            trace,
                            error: Some(ReplayError::MissingDependency {
                                feature: n.id.clone(),
                                reference: p.clone(),
                            }),
                        };
                    }
                    Some(j) => j,
                };
                match &outputs[j] {
                    Some(o) => vec![o.clone()],
                    None => vec![engine.start_value()],
                }
            } else {
                vec![engine.start_value()]
            };

            let out = match engine.evaluate_node(n, &inputs) {
                Ok(v) => v,
                Err(e) => {
                    return ReplayResult {
                        trace,
                        error: Some(e),
                    };
                }
            };
            outputs[i] = Some(out.clone());
            trace.push(ReplayStep {
                id: n.id.clone(),
                kind: n.kind.clone(),
                output: out,
            });
        }

        ReplayResult { trace, error: None }
    }

    /// Serialize the graph and its canonical replay trace (via
    /// [`MiniBoxEvaluator`]) to the deterministic `oep-feature-graph` v1 text.
    ///
    /// Fails with [`GraphError::NonFiniteParam`] if any numeric parameter is
    /// not finite.
    pub fn to_text(&self) -> Result<String, GraphError> {
        let mut s = String::new();
        s.push_str("format ");
        s.push_str(FORMAT);
        s.push('\n');
        s.push_str(&format!("version {VERSION}\n"));
        s.push_str(&format!("nodes {}\n", self.nodes.len()));

        for (i, n) in self.nodes.iter().enumerate() {
            // Finite check first so a bad graph fails before writing anything.
            for (k, v) in &n.params {
                if let ParamValue::Num(x) = v {
                    if !x.is_finite() {
                        return Err(GraphError::NonFiniteParam {
                            feature: n.id.clone(),
                            key: k.clone(),
                        });
                    }
                }
            }
            s.push_str(&format!("node {i}\n"));
            s.push_str(&format!("id {}\n", n.id));
            s.push_str(&format!("name {}\n", n.name));
            s.push_str(&format!("kind {}\n", n.kind));
            match &n.parent {
                Some(p) => s.push_str(&format!("parent {p}\n")),
                None => s.push_str("parent -\n"),
            }
            s.push_str(&format!("refs {}\n", n.refs.len()));
            for r in &n.refs {
                s.push_str(r);
                s.push('\n');
            }
            s.push_str(&format!("params {}\n", n.params.len()));
            for (k, v) in &n.params {
                s.push_str(k);
                s.push('=');
                match v {
                    ParamValue::Num(x) => s.push_str(&format!("{x}")),
                    ParamValue::Str(x) => {
                        s.push_str("str:");
                        s.push_str(x);
                    }
                }
                s.push('\n');
            }
        }

        let result = self.evaluate(&mut MiniBoxEvaluator::new());
        s.push_str(&format!("trace {}\n", result.trace.len()));
        for (j, step) in result.trace.iter().enumerate() {
            s.push_str(&format!("step {j}\n"));
            s.push_str(&format!("id {}\n", step.id));
            s.push_str(&format!("kind {}\n", step.kind));
            let b = step.output;
            s.push_str(&format!(
                "box {};{};{};{};{};{}\n",
                b[0], b[1], b[2], b[3], b[4], b[5]
            ));
        }
        Ok(s)
    }

    /// Strictly parse the `oep-feature-graph` v1 text back into a graph.
    ///
    /// `None` on any structural violation (wrong format/version, bad counts,
    /// missing/duplicate/non-sequential markers, non-finite numbers, empty
    /// ids/param names, trailing/blank lines). Unknown `kind`s and unlinked
    /// ids still decode (open vocabulary + tolerant linking); the trace
    /// section is validated structurally but not stored (it is deterministic
    /// and re-derived on encode). Never panics.
    pub fn from_text(text: &str) -> Option<FeatureGraph> {
        let lines: Vec<&str> = text.lines().collect();
        let mut idx = 0usize;

        let tag = format!("format {FORMAT}");
        if take(&lines, &mut idx)? != tag.as_str() {
            return None;
        }
        if take_field(&lines, &mut idx, "version ").and_then(|v| v.parse::<u64>().ok())? != VERSION
        {
            return None;
        }
        let n = take_field(&lines, &mut idx, "nodes ")?
            .parse::<usize>()
            .ok()?;

        let mut nodes = Vec::with_capacity(n);
        let mut seen = std::collections::HashSet::new();
        for i in 0..n {
            if take(&lines, &mut idx)? != format!("node {i}") {
                return None;
            }
            let id = take_field(&lines, &mut idx, "id ")?;
            if id.is_empty() || !seen.insert(id.clone()) {
                return None;
            }
            let name = take_field(&lines, &mut idx, "name ")?;
            let kind = take_field(&lines, &mut idx, "kind ")?;
            let parent = {
                let raw = take_field(&lines, &mut idx, "parent ")?;
                if raw == "-" {
                    None
                } else {
                    Some(raw)
                }
            };
            let r = take_field(&lines, &mut idx, "refs ")?
                .parse::<usize>()
                .ok()?;
            let mut refs = Vec::with_capacity(r);
            for _ in 0..r {
                refs.push(take(&lines, &mut idx)?.to_string());
            }
            if refs.len() != r {
                return None;
            }
            let p = take_field(&lines, &mut idx, "params ")?
                .parse::<usize>()
                .ok()?;
            let mut params = Vec::with_capacity(p);
            for _ in 0..p {
                let line = take(&lines, &mut idx)?;
                let (k, v) = line.split_once('=')?;
                if k.is_empty() {
                    return None;
                }
                let value = if let Some(rest) = v.strip_prefix("str:") {
                    ParamValue::Str(rest.to_string())
                } else {
                    let x: f64 = v.parse().ok()?;
                    if !x.is_finite() {
                        return None;
                    }
                    ParamValue::Num(x)
                };
                let pos = params
                    .binary_search_by(|(k0, _): &(String, ParamValue)| k0.as_str().cmp(k))
                    .unwrap_or_else(|e| e);
                params.insert(pos, (k.to_string(), value));
            }
            if params.len() != p {
                return None;
            }
            nodes.push(FeatureNode {
                id,
                name,
                kind,
                parent,
                refs,
                params,
            });
        }

        let m = take_field(&lines, &mut idx, "trace ")?
            .parse::<usize>()
            .ok()?;
        for j in 0..m {
            if take(&lines, &mut idx)? != format!("step {j}") {
                return None;
            }
            take_field(&lines, &mut idx, "id ")?;
            take_field(&lines, &mut idx, "kind ")?;
            let box_line = take_field(&lines, &mut idx, "box ")?;
            let coords: Vec<&str> = box_line.split(';').collect();
            if coords.len() != 6 {
                return None;
            }
            for c in coords {
                let x: f64 = c.parse().ok()?;
                if !x.is_finite() {
                    return None;
                }
            }
        }
        if idx != lines.len() {
            return None;
        }
        Some(FeatureGraph { nodes })
    }
}

/// The reference stand-in evaluator: folds each feature into a bounding box.
///
/// Mini vocabulary (documented in `docs/p6-m3-oep-geo-feature-graph.md`, not
/// CATIA semantics):
/// - `Extrude` — union of the resolved inputs with a box of size
///   `length × width × height` built from the origin.
/// - `Offset` — shift the last input by `(dx, dy, dz)`.
/// - `Union` — bounding-box envelope of all inputs.
/// - `Trim` — remove `dz` material from the top (`maxz -= dz`) of the input.
/// - `Mirror` — reflect the last input about plane `axis` (`x`/`y`/`z`) at
///   coordinate `at`.
///
/// Any other kind yields [`ReplayError::UnknownKind`].
#[derive(Debug, Clone, Copy)]
pub struct MiniBoxEvaluator {
    start: BBox,
}

impl MiniBoxEvaluator {
    /// A fresh evaluator starting from empty stock `[0,0,0,0,0,0]`.
    pub fn new() -> Self {
        Self { start: [0.0; 6] }
    }

    /// An evaluator whose roots union against a pre-existing start bbox.
    pub fn with_start(start: BBox) -> Self {
        Self { start }
    }
}

impl Default for MiniBoxEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator for MiniBoxEvaluator {
    type Value = BBox;

    fn start_value(&mut self) -> BBox {
        self.start
    }

    fn evaluate_node(&mut self, node: &FeatureNode, inputs: &[BBox]) -> Result<BBox, ReplayError> {
        match node.kind.as_str() {
            "Extrude" => {
                let length = need(node, "length")?;
                let width = need(node, "width")?;
                let height = need(node, "height")?;
                let bbox = [0.0, 0.0, 0.0, length, width, height];
                let mut all: Vec<BBox> = inputs.to_vec();
                all.push(bbox);
                Ok(bbox_envelope(&all))
            }
            "Offset" => {
                let b = *inputs.last().ok_or_else(|| {
                    ReplayError::Evaluator(format!("Offset '{}' needs an input", node.id))
                })?;
                let dx = node.param_num("dx").unwrap_or(0.0);
                let dy = node.param_num("dy").unwrap_or(0.0);
                let dz = node.param_num("dz").unwrap_or(0.0);
                Ok([
                    b[0] + dx,
                    b[1] + dy,
                    b[2] + dz,
                    b[3] + dx,
                    b[4] + dy,
                    b[5] + dz,
                ])
            }
            "Union" => {
                if inputs.is_empty() {
                    return Err(ReplayError::Evaluator(format!(
                        "Union '{}' needs at least one input",
                        node.id
                    )));
                }
                Ok(bbox_envelope(inputs))
            }
            "Trim" => {
                let dz = need(node, "dz")?;
                let b = *inputs.last().ok_or_else(|| {
                    ReplayError::Evaluator(format!("Trim '{}' needs an input", node.id))
                })?;
                Ok([b[0], b[1], b[2], b[3], b[4], b[5] - dz])
            }
            "Mirror" => {
                let axis = node.param_str("axis").ok_or_else(|| {
                    ReplayError::Evaluator(format!("Mirror '{}' needs an axis param", node.id))
                })?;
                let at = need(node, "at")?;
                let b = *inputs.last().ok_or_else(|| {
                    ReplayError::Evaluator(format!("Mirror '{}' needs an input", node.id))
                })?;
                let (lo, hi) = match axis {
                    "x" => (0, 3),
                    "y" => (1, 4),
                    "z" => (2, 5),
                    _ => {
                        return Err(ReplayError::Evaluator(format!(
                            "Mirror '{}' has unknown axis {:?}",
                            node.id, axis
                        )))
                    }
                };
                let mut out = b;
                out[lo] = 2.0 * at - b[hi];
                out[hi] = 2.0 * at - b[lo];
                Ok(out)
            }
            other => Err(ReplayError::UnknownKind {
                feature: node.id.clone(),
                kind: other.to_string(),
            }),
        }
    }
}

/// Read a required numeric parameter, mapping absence to a `Evaluator` error.
fn need(node: &FeatureNode, key: &str) -> Result<f64, ReplayError> {
    node.param_num(key).ok_or_else(|| {
        ReplayError::Evaluator(format!("feature '{}' is missing param '{key}'", node.id))
    })
}

/// Per-axis bounding-box envelope of two or more boxes (caller guarantees
/// non-empty input).
fn bbox_envelope(boxes: &[BBox]) -> BBox {
    let mut out = boxes[0];
    for b in &boxes[1..] {
        out[0] = out[0].min(b[0]);
        out[1] = out[1].min(b[1]);
        out[2] = out[2].min(b[2]);
        out[3] = out[3].max(b[3]);
        out[4] = out[4].max(b[4]);
        out[5] = out[5].max(b[5]);
    }
    out
}

fn take<'a>(lines: &[&'a str], idx: &mut usize) -> Option<&'a str> {
    let l = lines.get(*idx)?;
    *idx += 1;
    Some(l)
}

fn take_field(lines: &[&str], idx: &mut usize, prefix: &str) -> Option<String> {
    let l = take(lines, idx)?;
    Some(l.strip_prefix(prefix)?.to_string())
}
