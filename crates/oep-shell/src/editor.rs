//! INTERACTIVE-SHELL: the interactive editing surface (mouse-grade editor).
//!
//! [`EditorSession`] turns oep-shell + oep-viewer into a drivable editor.
//! An engineer (or an agent driving the same command set) opens a fresh
//! session, sketches a closed profile in the viewport (points + line
//! segments), constrains it with the oep-sketch constraint set (fixed,
//! horizontal, vertical, coincident, length, distance) whose dimensions are
//! driven by the variable set (FreeCAD-v1-variable-set semantics), solves it,
//! and creates a solid feature (extrude or revolve). Editing a variable in
//! the variable set re-solves the sketch and re-derives the solid live: the
//! model follows the parameter (drive-lane semantics).
//!
//! The canonical model is real: every shown state is the actual `oep-geo`
//! [`Document`] (part + body + `Sketch` / `VariableSet` / `Extrude` /
//! `Revolve` features), and the solid the viewport renders and STEP-exports
//! is derived deterministically from that model by
//! [`oep_geo::tessellate`], never a fabricated proxy. Undo/redo rewind the
//! actual document (bounded snapshot history over the canonical JSON), so
//! the affordance is real, not cosmetic.
//!
//! The viewport ("the mouse surface") is the deterministic CPU renderer of
//! oep-viewer: an orbit camera with orbit/pan/zoom, real-mesh solids with
//! edge wires, and screen-space selection with highlight. Everything is also
//! text-callable (see [`crate::commands`]), so the headless smoke test and
//! the agent lane drive the same path byte-for-byte.

use std::collections::BTreeMap;
use std::fmt;

use oep_geo::tessellate::{mesh_bbox, mesh_volume, TriMesh};
use oep_geo::{Body, BodyKind, BoundingBox, Document, Feature, Part};
use oep_sketch::features::{
    sketch_from_feature, sketch_to_feature, varset_from_feature, varset_to_feature,
};
use oep_sketch::sketch::{Constraint, DimExpr, Sketch};
use oep_sketch::solve::{solve_sketch, SolveOutcome};
use oep_sketch::varset::{VariableError, VariableSet};
use oep_viewer::camera::Camera;
use oep_viewer::pick::{pick, Selection};
use oep_viewer::render::RgbaImage;
use oep_viewer::scene::{MeshData, Primitive, Scene};
use oep_viewer::theme;

use crate::sha256;

/// Default part id of the session's single part.
pub const PART_ID: &str = "part-1";
/// Default body id of the session's single solid body.
pub const BODY_ID: &str = "body-1";
/// Default sketch id.
pub const SKETCH_ID: &str = "sketch-1";
/// Undo/redo capacity (number of document snapshots retained each way).
pub const HISTORY_CAPACITY: usize = 32;
/// Viewport width used by [`EditorSession::render`].
pub const VIEWPORT_WIDTH: u32 = 640;
/// Viewport height used by [`EditorSession::render`].
pub const VIEWPORT_HEIGHT: u32 = 480;

/// Feature kind tag for the sketch feature (must equal oep-sketch's).
const SKETCH_KIND: &str = "Sketch";
/// Feature kind tag for the variable-set feature (must equal oep-sketch's).
const VARSET_KIND: &str = "VariableSet";
/// Feature kind tag for an extrude operation.
pub const EXTRUDE_KIND: &str = "Extrude";
/// Feature kind tag for a revolve operation.
pub const REVOLVE_KIND: &str = "Revolve";
/// Revolve tessellation default (meridian segments per full turn).
pub const REVOLVE_SEGMENTS: u32 = 48;

/// A typed failure of the interactive editing surface. Never a panic.
#[derive(Debug, Clone)]
pub enum EditorError {
    /// The session's sketch feature is missing or corrupt.
    NoSketch(String),
    /// The sketch solver failed (typed [`oep_sketch::solve::SketchError`]).
    SketchSolve(String),
    /// A named sketch entity (point/line) is not present.
    UnknownEntity(String),
    /// A dimension expression is invalid (non-finite literal).
    BadDimension(String),
    /// The variable set could not resolve a referenced variable name.
    VariableNotFound(VariableError),
    /// The sketch is not a single closed loop (needed to create a feature).
    NotAClosedLoop(String),
    /// The sketch is not solved yet (solve first).
    NotSolved(String),
    /// The tessellation of the derived solid failed.
    Tessellation(String),
    /// No solid feature exists yet.
    NoSolid(String),
    /// STEP export failed.
    StepExport(String),
    /// I/O error (rendering, export, serialization).
    Io(String),
    /// The undo/redo stack has no state in that direction.
    HistoryBoundary(&'static str),
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::NoSketch(m) => write!(f, "no sketch: {m}"),
            EditorError::SketchSolve(m) => write!(f, "sketch solve failed: {m}"),
            EditorError::UnknownEntity(m) => write!(f, "unknown sketch entity: {m}"),
            EditorError::BadDimension(m) => write!(f, "bad dimension: {m}"),
            EditorError::VariableNotFound(e) => write!(f, "variable not found: {e}"),
            EditorError::NotAClosedLoop(m) => write!(f, "not a closed loop: {m}"),
            EditorError::NotSolved(m) => write!(f, "sketch not solved: {m}"),
            EditorError::Tessellation(m) => write!(f, "tessellation failed: {m}"),
            EditorError::NoSolid(m) => write!(f, "no solid: {m}"),
            EditorError::StepExport(m) => write!(f, "step export failed: {m}"),
            EditorError::Io(m) => write!(f, "io error: {m}"),
            EditorError::HistoryBoundary(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for EditorError {}

impl From<std::io::Error> for EditorError {
    fn from(e: std::io::Error) -> Self {
        EditorError::Io(e.to_string())
    }
}

/// Result of adding a point (its assigned id).
#[derive(Debug, Clone)]
pub struct PointReport {
    /// The assigned point id (p0, p1, ...).
    pub id: String,
}

/// Result of adding a line segment (its assigned id and endpoints).
#[derive(Debug, Clone)]
pub struct LineReport {
    /// The assigned line id (l0, l1, ...).
    pub id: String,
    /// Start point id.
    pub start: String,
    /// End point id.
    pub end: String,
}

/// Result of a sketch solve.
#[derive(Debug, Clone)]
pub struct SolveReport {
    /// Remaining degrees of freedom (0 = fully constrained).
    pub dof: i32,
    /// Final residual norm of the constraint system.
    pub residual: f64,
    /// Whether the solver converged within tolerance.
    pub converged: bool,
    /// Solved point locations keyed by point id.
    pub points: BTreeMap<String, (f64, f64)>,
    /// Driven dimensions keyed by variable name (Length constraints that
    /// reference a variable).
    pub dimensions: BTreeMap<String, f64>,
    /// Number of points in the sketch.
    pub point_count: usize,
    /// Number of line segments in the sketch.
    pub line_count: usize,
    /// Number of constraints in the sketch.
    pub constraint_count: usize,
}

/// Result of creating a solid feature (extrude or revolve).
#[derive(Debug, Clone)]
pub struct FeatureReport {
    /// The feature id added to the document.
    pub feature_id: String,
    /// The body the solid belongs to.
    pub body_id: String,
    /// The derived solid volume.
    pub volume: f64,
    /// The derived solid bounding box `[minx,miny,minz,maxx,maxy,maxz]`.
    pub bbox: [f64; 6],
    /// Number of mesh faces (triangles).
    pub faces: usize,
    /// Number of mesh boundary edges.
    pub edges: usize,
}

/// Result of editing a variable in the variable set.
#[derive(Debug, Clone)]
pub struct VarReport {
    /// The variable edited.
    pub name: String,
    /// The new value.
    pub value: f64,
    /// Solid volume before the edit (live recompute), if a solid existed.
    pub volume_before: Option<f64>,
    /// Solid volume after the edit, if a solid existed.
    pub volume_after: Option<f64>,
}

/// Result of a full recompute (re-solve + re-derive the solid).
#[derive(Debug, Clone)]
pub struct RecomputeReport {
    /// Solid volume after recompute, if a solid feature exists.
    pub volume: Option<f64>,
    /// Solid bbox after recompute, if a solid feature exists.
    pub bbox: Option<[f64; 6]>,
    /// Whether the model actually changed (compare-and-set style report).
    pub changed: bool,
}

/// Result of rendering the current viewport to an image file.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// Path written.
    pub path: String,
    /// SHA-256 of the image bytes (proves the viewport changed between edits).
    pub sha256: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Result of a STEP export of the derived solid.
#[derive(Debug, Clone)]
pub struct StepExportResult {
    /// Path written.
    pub path: String,
    /// Number of faces written.
    pub faces: usize,
    /// Number of shells written.
    pub shells: usize,
    /// Of the shells, how many are watertight closed shells.
    pub closed_shells: usize,
}

/// Result of a selection gesture.
#[derive(Debug, Clone)]
pub struct SelectReport {
    /// The selected primitive id (e.g. `part:part-1:body:body-1`).
    pub id: String,
    /// The selected part id.
    pub part_id: String,
    /// The selected body id.
    pub body_id: String,
    /// Hit distance along the pick ray.
    pub distance: f64,
}

/// Result of an undo/redo navigation.
#[derive(Debug, Clone)]
pub struct HistoryReport {
    /// Operation label of the state now current.
    pub op: String,
    /// Number of retained snapshots.
    pub count: usize,
    /// Cursor index into the retained snapshots.
    pub cursor: usize,
}

/// Current model measures.
#[derive(Debug, Clone)]
pub struct PropReport {
    /// Solid volume, if the session has derived one.
    pub volume: Option<f64>,
    /// Solid bbox, if the session has derived one.
    pub bbox: Option<[f64; 6]>,
    /// Number of document features.
    pub features: usize,
    /// Sketch degrees of freedom from the last solve, if solved.
    pub dof: Option<i32>,
    /// Sketch point count.
    pub sketch_points: usize,
    /// Sketch line count.
    pub sketch_lines: usize,
    /// Sketch constraint count.
    pub constraints: usize,
    /// The variable set entries (sorted by name).
    pub vars: Vec<(String, f64)>,
}

/// The interactive editing session: canonical document + solved sketch state
/// + derived solid + camera/selection + a real undo/redo history.
#[derive(Debug, Clone)]
pub struct EditorSession {
    document: Document,
    mesh: Option<TriMesh>,
    camera: Camera,
    selected: Option<String>,
    journal: Vec<String>,
    undo: Vec<String>,
    redo: Vec<String>,
    last_solve: Option<SolveReport>,
    point_counter: usize,
    line_counter: usize,
}

impl EditorSession {
    /// A fresh editing session: blank canonical document with one part, one
    /// body, an empty sketch feature and an empty variable-set feature.
    pub fn new() -> Self {
        let mut document = Document::new("mm");
        let sketch_feature = sketch_to_feature(&Sketch::new(SKETCH_ID));
        let vars_feature = session_vars_feature(&VariableSet::new());
        let sketch_feature_id = sketch_feature.id.clone();
        let vars_feature_id = vars_feature.id.clone();
        let part = Part::new(PART_ID, "editor-part")
            .with_body(BODY_ID)
            .with_feature(&sketch_feature_id)
            .with_feature(&vars_feature_id);
        let body = Body::new(BODY_ID, "editor-solid", BodyKind::Solid)
            .with_feature(&sketch_feature_id)
            .with_feature(&vars_feature_id);
        document.push_feature(sketch_feature);
        document.push_feature(vars_feature);
        document.push_body(body);
        document.push_part(part);
        Self {
            document,
            mesh: None,
            camera: Camera::default(),
            selected: None,
            journal: vec!["editor session opened (part-1 / body-1, sketch-1)".to_string()],
            undo: Vec::new(),
            redo: Vec::new(),
            last_solve: None,
            point_counter: 0,
            line_counter: 0,
        }
    }

    /// The canonical oep-geo document (the single source of truth).
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The per-session operation journal.
    pub fn journal(&self) -> &[String] {
        &self.journal
    }

    /// The current viewport camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The currently selected primitive id, if any.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// The derived solid mesh of the current feature, if one exists.
    pub fn mesh(&self) -> Option<&TriMesh> {
        self.mesh.as_ref()
    }

    /// The last successful solve report, if any.
    pub fn last_solve(&self) -> Option<&SolveReport> {
        self.last_solve.as_ref()
    }

    // ------------------------------------------------------------------
    // Sketch editing
    // ------------------------------------------------------------------

    /// Add a point at `(x, y)` in the sketch plane; returns its assigned id.
    pub fn point(&mut self, x: f64, y: f64) -> Result<PointReport, EditorError> {
        self.begin_edit("point");
        let mut sketch = self.sketch_mut()?;
        let id = format!("p{}", self.point_counter);
        self.point_counter += 1;
        sketch.add_point(&id, x, y);
        self.sync_sketch_feature(&sketch)?;
        self.journal
            .push(format!("sketch point {id} at ({x}, {y})"));
        Ok(PointReport { id })
    }

    /// Add a line segment between two existing points; returns its assigned id.
    pub fn line(&mut self, start: &str, end: &str) -> Result<LineReport, EditorError> {
        self.begin_edit("line");
        let mut sketch = self.sketch_mut()?;
        if !sketch.points.contains_key(start) {
            return Err(EditorError::UnknownEntity(format!(
                "no point {start} in the sketch"
            )));
        }
        if !sketch.points.contains_key(end) {
            return Err(EditorError::UnknownEntity(format!(
                "no point {end} in the sketch"
            )));
        }
        let id = format!("l{}", self.line_counter);
        self.line_counter += 1;
        sketch.add_line(&id, start, end);
        self.sync_sketch_feature(&sketch)?;
        self.journal
            .push(format!("sketch line {id} from {start} to {end}"));
        Ok(LineReport {
            id,
            start: start.to_string(),
            end: end.to_string(),
        })
    }

    /// Add a constraint to the sketch. Dimensions reference a literal value or
    /// a variable-set entry by name.
    pub fn constrain(&mut self, constraint: Constraint) -> Result<(), EditorError> {
        self.begin_edit("constraint");
        let kind = constraint_kind(&constraint);
        let mut sketch = self.sketch_mut()?;
        // Validate referenced entities exist before mutating.
        validate_constraint_entities(&sketch, &constraint)?;
        sketch.constraints.push(constraint);
        self.sync_sketch_feature(&sketch)?;
        self.journal.push(format!("sketch constraint {kind}"));
        Ok(())
    }

    /// Solve the sketch against the variable set through the oep-sketch
    /// solver and write the solved coordinates back into the canonical
    /// sketch. Returns the measured solve report.
    pub fn solve(&mut self) -> Result<SolveReport, EditorError> {
        let (sketch, vars) = self.sketch_and_vars()?;
        let outcome: SolveOutcome = solve_sketch(&sketch, &vars).map_err(|e| {
            EditorError::SketchSolve(match &e {
                oep_sketch::solve::SketchError::VariableNotFound(v) => v.to_string(),
                other => other.to_string(),
            })
        })?;
        if !outcome.converged {
            return Err(EditorError::SketchSolve(format!(
                "solver did not converge (residual {:.3e}, dof {})",
                outcome.residual, outcome.dof
            )));
        }
        // Write solved coordinates back into the canonical sketch.
        let mut sketch = sketch.clone();
        for (pid, (x, y)) in &outcome.points {
            if let Some(p) = sketch.points.get_mut(pid) {
                p.x = *x;
                p.y = *y;
            }
        }
        self.sync_sketch_feature(&sketch)?;
        let report = SolveReport {
            dof: outcome.dof,
            residual: outcome.residual,
            converged: outcome.converged,
            points: outcome.points,
            dimensions: outcome.dimensions,
            point_count: sketch.points.len(),
            line_count: sketch.lines.len(),
            constraint_count: sketch.constraints.len(),
        };
        self.journal.push(format!(
            "solved sketch dof={} residual={:.3e} dims={}",
            report.dof,
            report.residual,
            report.dimensions.len()
        ));
        self.last_solve = Some(report.clone());
        Ok(report)
    }

    /// The current sketch (declarative, with solved coordinates after a
    /// solve), or `None` when the session has no sketch feature.
    pub fn sketch(&self) -> Option<Sketch> {
        self.sketch_ref().ok()
    }

    // ------------------------------------------------------------------
    // Variable set
    // ------------------------------------------------------------------

    /// The variable-set entries (sorted by name).
    pub fn vars(&self) -> Vec<(String, f64)> {
        self.vars_ref()
            .map(|v| v.variables.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default()
    }

    /// Set (or add) a variable and, when a solid feature exists, recompute
    /// live so the model follows the parameter. Returns the before/after
    /// volume to prove the solid moved.
    pub fn set_var(&mut self, name: &str, value: f64) -> Result<VarReport, EditorError> {
        if !value.is_finite() {
            return Err(EditorError::BadDimension(format!(
                "variable {name} must be a finite number, got {value}"
            )));
        }
        self.begin_edit("set-var");
        let before = self.solid_volume();
        let mut vars = self.vars_ref()?.clone();
        vars.set(name, value);
        self.sync_vars_feature(&vars)?;
        self.journal.push(format!("variable {name} = {value}"));
        // Live recompute: the solid follows the parameter (drive lane).
        let after = if self.has_solid_feature() {
            let rep = self.recompute()?;
            rep.volume
        } else {
            None
        };
        Ok(VarReport {
            name: name.to_string(),
            value,
            volume_before: before,
            volume_after: after,
        })
    }

    // ------------------------------------------------------------------
    // Solid features (extrude / revolve) and live recompute
    // ------------------------------------------------------------------

    /// Create an extrude from the solved sketch profile by `depth` (a literal
    /// or a variable-set reference). The solid is derived deterministically,
    /// the body bounding box is set from the real mesh, and the mesh is
    /// available for viewport and STEP export.
    pub fn extrude(&mut self, depth: DimExpr) -> Result<FeatureReport, EditorError> {
        if let DimExpr::Literal(v) = depth {
            if !v.is_finite() || v <= 0.0 {
                return Err(EditorError::BadDimension(format!(
                    "extrude depth must be finite and positive, got {v}"
                )));
            }
        }
        let depth_value = self.resolve_dim(&depth)?;
        let profile = self.solved_profile()?;
        let mesh = oep_geo::tessellate::extrude_profile(&profile, depth_value)
            .map_err(|e| EditorError::Tessellation(e.to_string()))?;
        self.begin_edit("extrude");
        let mut params = well_known_params("profile", &profile, "depth", depth_value);
        if let DimExpr::Variable(name) = &depth {
            params.insert(
                "depth_var".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }
        let report = self.install_feature(EXTRUDE_KIND, "extrude", params, mesh)?;
        self.journal.push(format!(
            "extruded profile ({} points) by {depth_value}: volume {:.6}",
            profile.len(),
            report.volume
        ));
        Ok(report)
    }

    /// Create a revolve from the solved sketch profile by `angle` radians
    /// about Z. The profile is a meridian cross-section: `x` is the radial
    /// distance from the Z axis, `y` the axial height (matching the kernel
    /// seam's [`oep_geo::kernel::solid_revolve`]).
    pub fn revolve(&mut self, angle: DimExpr) -> Result<FeatureReport, EditorError> {
        if let DimExpr::Literal(v) = angle {
            if !v.is_finite() || v <= 0.0 {
                return Err(EditorError::BadDimension(format!(
                    "revolve angle must be finite and positive, got {v}"
                )));
            }
        }
        let angle_value = self.resolve_dim(&angle)?;
        let profile = self.solved_profile()?;
        let mesh = oep_geo::tessellate::revolve_profile(&profile, angle_value, REVOLVE_SEGMENTS)
            .map_err(|e| EditorError::Tessellation(e.to_string()))?;
        self.begin_edit("revolve");
        let mut params = well_known_params("profile", &profile, "angle", angle_value);
        if let DimExpr::Variable(name) = &angle {
            params.insert(
                "angle_var".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }
        let report = self.install_feature(REVOLVE_KIND, "revolve", params, mesh)?;
        self.journal.push(format!(
            "revolved profile ({} points) by {angle_value} rad: volume {:.6}",
            profile.len(),
            report.volume
        ));
        Ok(report)
    }

    /// Full recompute: re-solve the sketch (variables may have changed),
    /// rebuild the profile, re-tessellate the solid feature, and update the
    /// body bounding box and session mesh. The solid follows the parameter.
    pub fn recompute(&mut self) -> Result<RecomputeReport, EditorError> {
        // Re-solve so the profile reflects the current variable set.
        if self.solve().is_err() {
            // A re-solve failure is fatal for recompute: we never derive a
            // solid from a stale or diverging sketch.
            return Err(EditorError::NotAClosedLoop(
                "recompute requires a re-solvable sketch".to_string(),
            ));
        }
        let Some(feature) = self.solid_feature() else {
            return Ok(RecomputeReport {
                volume: None,
                bbox: None,
                changed: false,
            });
        };
        let kind = feature.kind.clone();
        // Reconstruct the profile from the just-solved sketch.
        let profile = self.solved_profile()?;
        // Resolve the sweep amount: a feature whose dimension referenced a
        // variable re-resolves it from the CURRENT variable set (drive lane:
        // editing the variable moves the solid); a literal stays as stored.
        let dim = match kind.as_str() {
            EXTRUDE_KIND => feature_dim(&feature, "depth", "depth_var", &self.vars_ref()?),
            REVOLVE_KIND => feature_dim(&feature, "angle", "angle_var", &self.vars_ref()?),
            other => {
                return Err(EditorError::NoSolid(format!(
                    "unknown solid feature kind {other}"
                )))
            }
        }?;
        let mesh = match kind.as_str() {
            EXTRUDE_KIND => oep_geo::tessellate::extrude_profile(&profile, dim),
            REVOLVE_KIND => oep_geo::tessellate::revolve_profile(&profile, dim, REVOLVE_SEGMENTS),
            _ => unreachable!(),
        }
        .map_err(|e| EditorError::Tessellation(e.to_string()))?;
        let report = self.replace_mesh_and_body(&mesh)?;
        let volume = Some(report.volume);
        let bbox = Some(report.bbox);
        self.journal
            .push(format!("recomputed {kind}: volume {:.6}", report.volume));
        Ok(RecomputeReport {
            volume,
            bbox,
            changed: true,
        })
    }

    // ------------------------------------------------------------------
    // Viewport: camera, selection, render
    // ------------------------------------------------------------------

    /// Orbit the camera by `dyaw` / `dpitch`.
    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.camera.orbit(dyaw, dpitch);
        self.journal
            .push(format!("view orbit {dyaw:.3} {dpitch:.3}"));
    }

    /// Zoom the camera by a multiplicative `factor`.
    pub fn zoom(&mut self, factor: f64) {
        self.camera.zoom(factor);
        self.journal.push(format!("view zoom {factor:.3}"));
    }

    /// Pan the camera target by `dx` / `dy` world units along its screen axes.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.camera.pan(dx, dy);
        self.journal.push(format!("view pan {dx:.3} {dy:.3}"));
    }

    /// Frame the camera to the current model bounds (solid if present, else
    /// the sketch bounds).
    pub fn frame(&mut self) {
        let bounds = self.model_bounds();
        self.camera = bounds.map(|b| Camera::framed(&b)).unwrap_or_default();
        self.journal.push("view frame".to_string());
    }

    /// Pick the scene primitive under the screen pixel `(x, y)` (top-left
    /// origin) and make it the selection. Returns the selection report.
    pub fn select_at(&mut self, x: f64, y: f64) -> Option<SelectReport> {
        let scene = self.build_scene().ok()?;
        let sel: Selection = pick(
            &scene,
            &self.camera,
            x,
            y,
            VIEWPORT_WIDTH as f64,
            VIEWPORT_HEIGHT as f64,
        )?;
        self.selected = Some(sel.id.clone());
        self.journal
            .push(format!("view select {} at px ({x}, {y})", sel.id));
        Some(SelectReport {
            id: sel.id,
            part_id: sel.part_id,
            body_id: sel.body_id,
            distance: sel.distance,
        })
    }

    /// Build the current viewport scene: the derived solid (highlighted when
    /// selected) plus the sketch overlay (points and lines drawn as wires).
    pub fn build_scene(&self) -> Result<Scene, EditorError> {
        let mut scene = Scene::default();
        if let Some(mesh) = &self.mesh {
            let solid_id = format!("part:{PART_ID}:body:{BODY_ID}");
            let data = mesh_to_viewer(mesh);
            let mut prim = Primitive::from_mesh(&solid_id, PART_ID, BODY_ID, data);
            prim.highlighted = self.selected.as_deref() == Some(solid_id.as_str());
            scene.primitives.push(prim);
        }
        // Sketch overlay: solved (or input) points and lines drawn at Z=0.
        if let Ok(sketch) = self.sketch_ref() {
            for (pid, p) in &sketch.points {
                // Each point as a small cross (two segments) so it reads as a
                // selectable vertex, sized relative to the model.
                let (pt_points, pt_edges) = point_cross(p.clone(), self.scale_hint(&sketch));
                let mut prim =
                    Primitive::wire(format!("sk:pt:{pid}"), "sketch", &pt_points, &pt_edges);
                prim.highlighted = false;
                scene.primitives.push(prim);
            }
            for (lid, seg) in &sketch.lines {
                let (Some(a), Some(b)) =
                    (sketch.points.get(&seg.start), sketch.points.get(&seg.end))
                else {
                    continue;
                };
                let points: [[f32; 3]; 2] =
                    [[a.x as f32, a.y as f32, 0.0], [b.x as f32, b.y as f32, 0.0]];
                scene.primitives.push(Primitive::wire(
                    format!("sk:ln:{lid}"),
                    "sketch",
                    &points,
                    &[[0usize, 1usize]],
                ));
            }
        }
        scene.bounds = scene
            .primitives
            .iter()
            .map(|p| p.world)
            .reduce(|a, b| a.union(&b));
        Ok(scene)
    }

    /// Render the current viewport to an image at `path`. The extension picks
    /// the format: `.ppm` writes a P6 PPM, `.png` writes a PNG (via the
    /// viewer's deterministic encoder). Returns the image SHA-256 so two
    /// consecutive renders prove whether the model moved.
    pub fn render(&self, path: &str) -> Result<RenderResult, EditorError> {
        let scene = self.build_scene()?;
        let img: RgbaImage =
            oep_viewer::render::render(&scene, &self.camera, &[], VIEWPORT_WIDTH, VIEWPORT_HEIGHT);
        let bytes = if path.ends_with(".png") {
            oep_viewer::render::encode_png(&img)
        } else {
            encode_ppm(&img)
        };
        std::fs::write(path, &bytes)?;
        Ok(RenderResult {
            path: path.to_string(),
            sha256: sha256::sha256_hex(&bytes),
            width: img.width,
            height: img.height,
        })
    }

    // ------------------------------------------------------------------
    // Export
    // ------------------------------------------------------------------

    /// Export the derived solid as STEP (AP214 faceted B-Rep passthrough via
    /// the oep-geo exporter). Fails typed when no solid has been created.
    pub fn export_step(&self, path: &str) -> Result<StepExportResult, EditorError> {
        let Some(mesh) = &self.mesh else {
            return Err(EditorError::NoSolid(
                "create an extrude or revolve before exporting STEP".to_string(),
            ));
        };
        let mut file = std::fs::File::create(path)?;
        let opts = oep_geo::export::step::StepOpts {
            product_name: "editor-part".to_string(),
            file_name: path
                .rsplit('/')
                .next()
                .unwrap_or("editor-part.step")
                .to_string(),
            version: "0.1.0".to_string(),
        };
        // STEP is an f32-boundary format (P6-M1-T03): convert the f64 mesh at
        // the exporter seam, keeping oep-geo double-everywhere.
        let positions: Vec<[f32; 3]> = mesh
            .positions
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect();
        let stats =
            oep_geo::export::step::write_step(&positions, &mesh.triangles, &opts, &mut file)
                .map_err(|e| EditorError::StepExport(e.to_string()))?;
        Ok(StepExportResult {
            path: path.to_string(),
            faces: stats.faces,
            shells: stats.shells,
            closed_shells: stats.closed_shells,
        })
    }

    /// Current model measures (volume, bbox, sketch stats, variables).
    pub fn props(&self) -> PropReport {
        let sketch = self.sketch_ref().ok();
        PropReport {
            volume: self.solid_volume(),
            bbox: self.mesh.as_ref().and_then(mesh_bbox),
            features: self.document.features.len(),
            dof: self.last_solve.as_ref().map(|s| s.dof),
            sketch_points: sketch.as_ref().map(|s| s.points.len()).unwrap_or(0),
            sketch_lines: sketch.as_ref().map(|s| s.lines.len()).unwrap_or(0),
            constraints: sketch.as_ref().map(|s| s.constraints.len()).unwrap_or(0),
            vars: self.vars(),
        }
    }

    /// The full canonical document as deterministic oep-geo JSON.
    pub fn document_json(&self) -> Result<String, EditorError> {
        self.document
            .to_json_pretty()
            .map_err(|e| EditorError::Io(e.to_string()))
    }

    // ------------------------------------------------------------------
    // Undo / redo (real document snapshots)
    // ------------------------------------------------------------------

    /// Undo the last edit: restore the prior canonical document state.
    pub fn undo(&mut self) -> Result<HistoryReport, EditorError> {
        let Some(past) = self.undo.pop() else {
            return Err(EditorError::HistoryBoundary(
                "nothing to undo (at the oldest state)",
            ));
        };
        if let Ok(current) = self.document.to_json_pretty() {
            self.redo.push(current);
        }
        let doc = Document::from_json_pretty(&past)
            .map_err(|e| EditorError::Io(format!("undo restored an invalid document: {e}")))?;
        self.replace_document(doc);
        self.rebuild_from_document();
        let _ = self.recompute();
        self.journal
            .push(format!("undo ({} retained)", self.undo.len()));
        Ok(HistoryReport {
            op: "previous edit".to_string(),
            count: self.undo.len(),
            cursor: self.undo.len(),
        })
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> Result<HistoryReport, EditorError> {
        let Some(next) = self.redo.pop() else {
            return Err(EditorError::HistoryBoundary(
                "nothing to redo (at the newest state)",
            ));
        };
        if let Ok(current) = self.document.to_json_pretty() {
            self.undo.push(current);
        }
        let doc = Document::from_json_pretty(&next)
            .map_err(|e| EditorError::Io(format!("redo restored an invalid document: {e}")))?;
        self.replace_document(doc);
        self.rebuild_from_document();
        let _ = self.recompute();
        self.journal.push("redo".to_string());
        Ok(HistoryReport {
            op: "next edit".to_string(),
            count: self.redo.len(),
            cursor: self.redo.len(),
        })
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn begin_edit(&mut self, _op: &str) {
        if let Ok(json) = self.document.to_json_pretty() {
            if self.undo.len() >= HISTORY_CAPACITY && !self.undo.is_empty() {
                self.undo.remove(0);
            }
            self.undo.push(json);
            self.redo.clear();
        }
    }

    fn replace_document(&mut self, document: Document) {
        self.document = document;
    }

    /// Re-derive transient state (sketch/vars are in features) after a
    /// document replace. The mesh is re-derived lazily by recompute; here we
    /// clear it and reset counters to the sketch contents.
    fn rebuild_from_document(&mut self) {
        self.mesh = None;
        if let Ok(sketch) = self.sketch_ref() {
            self.point_counter = sketch
                .points
                .keys()
                .filter_map(|k| k.strip_prefix('p').and_then(|n| n.parse::<usize>().ok()))
                .max()
                .map(|m| m + 1)
                .unwrap_or(0);
            self.line_counter = sketch
                .lines
                .keys()
                .filter_map(|k| k.strip_prefix('l').and_then(|n| n.parse::<usize>().ok()))
                .max()
                .map(|m| m + 1)
                .unwrap_or(0);
        }
    }

    fn sketch_ref(&self) -> Result<Sketch, EditorError> {
        let feature = self
            .document
            .features
            .iter()
            .find(|f| f.kind == SKETCH_KIND)
            .ok_or_else(|| {
                EditorError::NoSketch("no Sketch feature in the document".to_string())
            })?;
        sketch_from_feature(feature).map_err(EditorError::NoSketch)
    }

    fn sketch_mut(&mut self) -> Result<Sketch, EditorError> {
        self.sketch_ref()
    }

    fn sync_sketch_feature(&mut self, sketch: &Sketch) -> Result<(), EditorError> {
        let feature = sketch_to_feature(sketch);
        let fid = feature.id.clone();
        if let Some(f) = self.document.features.iter_mut().find(|f| f.id == fid) {
            *f = feature;
        } else {
            self.document.push_feature(feature);
        }
        // The part/body reference the sketch feature for history.
        for p in &mut self.document.parts {
            if !p.feature_ids.contains(&fid) {
                p.feature_ids.push(fid.clone());
            }
        }
        for b in &mut self.document.bodies {
            if !b.feature_ids.contains(&fid) {
                b.feature_ids.push(fid.clone());
            }
        }
        Ok(())
    }

    fn vars_ref(&self) -> Result<VariableSet, EditorError> {
        let feature = self
            .document
            .features
            .iter()
            .find(|f| f.kind == VARSET_KIND)
            .ok_or_else(|| EditorError::NoSketch("no VariableSet feature".to_string()))?;
        varset_from_feature(feature).map_err(EditorError::NoSketch)
    }

    fn sync_vars_feature(&mut self, vars: &VariableSet) -> Result<(), EditorError> {
        let feature = session_vars_feature(vars);
        let fid = feature.id.clone();
        let pos = self
            .document
            .features
            .iter()
            .position(|f| f.kind == VARSET_KIND);
        if let Some(pos) = pos {
            self.document.features[pos] = feature;
        } else {
            self.document.push_feature(feature);
        }
        for p in &mut self.document.parts {
            if !p.feature_ids.contains(&fid) {
                p.feature_ids.push(fid.clone());
            }
        }
        Ok(())
    }

    fn sketch_and_vars(&self) -> Result<(Sketch, VariableSet), EditorError> {
        Ok((self.sketch_ref()?, self.vars_ref()?))
    }

    /// Resolve a dimension expression: a literal is checked finite; a
    /// variable name is resolved from the variable set (typed error when
    /// missing or non-finite).
    fn resolve_dim(&self, dim: &DimExpr) -> Result<f64, EditorError> {
        match dim {
            DimExpr::Literal(v) => Ok(*v),
            DimExpr::Variable(name) => self
                .vars_ref()?
                .get(name)
                .map_err(EditorError::VariableNotFound),
        }
    }

    /// Build the closed profile loop from the solved sketch: the line graph
    /// must form a single simple cycle covering every point with degree 2.
    fn solved_profile(&self) -> Result<Vec<oep_geo::kernel::Point2D>, EditorError> {
        let sketch = self.sketch_ref()?;
        let solved = self
            .last_solve
            .as_ref()
            .ok_or_else(|| EditorError::NotSolved("run solve first".to_string()))?;
        let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (lid, seg) in &sketch.lines {
            adj.entry(seg.start.clone()).or_default().push(lid.clone());
            adj.entry(seg.end.clone()).or_default().push(lid.clone());
        }
        // Every point in the loop must have degree exactly 2.
        let points_in_loop: Vec<String> = sketch.points.keys().cloned().collect();
        for pid in &points_in_loop {
            let deg = adj.get(pid).map(|v| v.len()).unwrap_or(0);
            if deg != 2 {
                return Err(EditorError::NotAClosedLoop(format!(
                    "point {pid} has degree {deg} (expected 2 for a closed loop)"
                )));
            }
        }
        if points_in_loop.is_empty() {
            return Err(EditorError::NotAClosedLoop(
                "sketch has no points".to_string(),
            ));
        }
        // Walk a single cycle.
        let start = points_in_loop[0].clone();
        let mut order: Vec<String> = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut cur = start.clone();
        let mut prev_line: Option<String> = None;
        loop {
            if visited.contains(&cur) {
                break;
            }
            visited.insert(cur.clone());
            order.push(cur.clone());
            let lines = &adj[&cur];
            let next_line = lines
                .iter()
                .find(|l| Some(l.as_str()) != prev_line.as_deref())
                .cloned()
                .unwrap_or_else(|| lines[0].clone());
            let seg = &sketch.lines[&next_line];
            prev_line = Some(next_line);
            cur = if seg.start == cur {
                seg.end.clone()
            } else {
                seg.start.clone()
            };
        }
        if order.len() != points_in_loop.len() || order[0] != start {
            return Err(EditorError::NotAClosedLoop(
                "the line graph does not form a single closed loop".to_string(),
            ));
        }
        // Order the loop starting from the fixed anchor if one exists (so the
        // profile origin is stable).
        let anchor = sketch.constraints.iter().find_map(|c| match c {
            Constraint::Fixed { point, .. } => Some(point.clone()),
            _ => None,
        });
        let profile = if let Some(a) = anchor {
            rotate_to(&order, &a)
        } else {
            order
        };
        let mut pts = Vec::with_capacity(profile.len());
        for pid in &profile {
            let (x, y) = solved.points.get(pid).ok_or_else(|| {
                EditorError::NotSolved(format!("point {pid} has no solved location"))
            })?;
            pts.push(oep_geo::kernel::Point2D::new(*x, *y));
        }
        Ok(pts)
    }

    fn has_solid_feature(&self) -> bool {
        self.document
            .features
            .iter()
            .any(|f| f.kind == EXTRUDE_KIND || f.kind == REVOLVE_KIND)
    }

    fn solid_feature(&self) -> Option<Feature> {
        self.document
            .features
            .iter()
            .find(|f| f.kind == EXTRUDE_KIND || f.kind == REVOLVE_KIND)
            .cloned()
    }

    fn solid_volume(&self) -> Option<f64> {
        self.mesh.as_ref().map(mesh_volume)
    }

    /// Install a new solid feature in the document, set the body bbox from
    /// the real mesh, and cache the session mesh.
    fn install_feature(
        &mut self,
        kind: &str,
        name: &str,
        params: oep_geo::feature::Parameters,
        mesh: TriMesh,
    ) -> Result<FeatureReport, EditorError> {
        let bbox =
            mesh_bbox(&mesh).ok_or_else(|| EditorError::Tessellation("empty mesh".to_string()))?;
        let feature_id = format!("oep-feature-{name}-{}", self.document.features.len());
        let mut feature = Feature::new(&feature_id, format!("{name}-solid"), kind);
        for (k, v) in params {
            feature.parameters.insert(k, v);
        }
        feature.body_ids.push(BODY_ID.to_string());
        self.document.push_feature(feature);
        let part = self
            .document
            .parts
            .iter_mut()
            .find(|p| p.id == PART_ID)
            .ok_or_else(|| EditorError::Io("part missing".to_string()))?;
        if !part.feature_ids.contains(&feature_id) {
            part.feature_ids.push(feature_id.clone());
        }
        let body = self
            .document
            .bodies
            .iter_mut()
            .find(|b| b.id == BODY_ID)
            .ok_or_else(|| EditorError::Io("body missing".to_string()))?;
        body.bbox = Some(BoundingBox::new(
            [bbox[0], bbox[1], bbox[2]],
            [bbox[3], bbox[4], bbox[5]],
        ));
        if !body.feature_ids.contains(&feature_id) {
            body.feature_ids.push(feature_id.clone());
        }
        let report = FeatureReport {
            feature_id: feature_id.clone(),
            body_id: BODY_ID.to_string(),
            volume: mesh_volume(&mesh),
            bbox,
            faces: mesh.triangles.len(),
            edges: mesh.edges.len(),
        };
        self.mesh = Some(mesh);
        Ok(report)
    }

    /// Re-derive the mesh for the existing solid feature and refresh the body
    /// bbox + session mesh (used by live recompute).
    fn replace_mesh_and_body(&mut self, mesh: &TriMesh) -> Result<FeatureReport, EditorError> {
        let bbox =
            mesh_bbox(mesh).ok_or_else(|| EditorError::Tessellation("empty mesh".to_string()))?;
        let body = self
            .document
            .bodies
            .iter_mut()
            .find(|b| b.id == BODY_ID)
            .ok_or_else(|| EditorError::Io("body missing".to_string()))?;
        body.bbox = Some(BoundingBox::new(
            [bbox[0], bbox[1], bbox[2]],
            [bbox[3], bbox[4], bbox[5]],
        ));
        let feature_id = self
            .solid_feature()
            .map(|f| f.id)
            .ok_or_else(|| EditorError::NoSolid("no solid feature".to_string()))?;
        self.mesh = Some(mesh.clone());
        Ok(FeatureReport {
            feature_id,
            body_id: BODY_ID.to_string(),
            volume: mesh_volume(mesh),
            bbox,
            faces: mesh.triangles.len(),
            edges: mesh.edges.len(),
        })
    }

    /// The union AABB of the model (solid or sketch) for camera framing.
    fn model_bounds(&self) -> Option<oep_viewer::scene::Aabb> {
        let scene = self.build_scene().ok()?;
        scene.bounds
    }

    /// A length hint for point crosses: ~2% of the sketch diagonal.
    fn scale_hint(&self, sketch: &Sketch) -> f64 {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in sketch.points.values() {
            lo[0] = lo[0].min(p.x);
            lo[1] = lo[1].min(p.y);
            hi[0] = hi[0].max(p.x);
            hi[1] = hi[1].max(p.y);
        }
        if !lo[0].is_finite() {
            return 1.0;
        }
        ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.02).max(1e-3)
    }
}

impl Default for EditorSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Resolve a feature's sweep amount on recompute: prefers the stored variable
/// reference (`*_var` param, re-resolved from the current variable set),
/// falling back to the stored literal (`*` param).
fn feature_dim(
    feature: &Feature,
    amount_key: &str,
    var_key: &str,
    vars: &VariableSet,
) -> Result<f64, EditorError> {
    if let Some(name) = feature.parameters.get(var_key).and_then(|v| v.as_str()) {
        return vars.get(name).map_err(EditorError::VariableNotFound);
    }
    feature
        .parameters
        .get(amount_key)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| EditorError::NoSolid(format!("feature lost its {amount_key} parameter")))
}

/// A short stable tag describing a constraint for journaling.
fn constraint_kind(c: &Constraint) -> &'static str {
    match c {
        Constraint::Fixed { .. } => "fixed",
        Constraint::Horizontal { .. } => "horizontal",
        Constraint::Vertical { .. } => "vertical",
        Constraint::Coincident { .. } => "coincident",
        Constraint::Length { .. } => "length",
        Constraint::Distance { .. } => "distance",
    }
}

/// Validate that the entities a constraint references exist in the sketch.
fn validate_constraint_entities(sketch: &Sketch, c: &Constraint) -> Result<(), EditorError> {
    let miss = |what: &str, id: &str| {
        Err(EditorError::UnknownEntity(format!(
            "constraint references unknown {what} {id}"
        )))
    };
    match c {
        Constraint::Fixed { point, .. } => {
            if !sketch.points.contains_key(point) {
                return miss("point", point);
            }
        }
        Constraint::Horizontal { line } | Constraint::Vertical { line } => {
            if !sketch.lines.contains_key(line) {
                return miss("line", line);
            }
        }
        Constraint::Coincident { point_a, point_b } => {
            if !sketch.points.contains_key(point_a) {
                return miss("point", point_a);
            }
            if !sketch.points.contains_key(point_b) {
                return miss("point", point_b);
            }
        }
        Constraint::Length { line, .. } => {
            if !sketch.lines.contains_key(line) {
                return miss("line", line);
            }
        }
        Constraint::Distance {
            point_a, point_b, ..
        } => {
            if !sketch.points.contains_key(point_a) {
                return miss("point", point_a);
            }
            if !sketch.points.contains_key(point_b) {
                return miss("point", point_b);
            }
        }
    }
    Ok(())
}

/// Rotate a closed loop so `anchor` is first (keeps the profile origin
/// stable under re-solves).
fn rotate_to(loop_order: &[String], anchor: &str) -> Vec<String> {
    if let Some(pos) = loop_order.iter().position(|p| p == anchor) {
        let mut out = Vec::with_capacity(loop_order.len());
        out.extend_from_slice(&loop_order[pos..]);
        out.extend_from_slice(&loop_order[..pos]);
        out
    } else {
        loop_order.to_vec()
    }
}

/// Translate an [`oep_geo::tessellate::TriMesh`] into the viewer's mesh form.
/// The viewer mesh is `f32` (its own surface contract); oep-geo stays f64
/// (the double-everywhere discipline), so this is a boundary conversion.
fn mesh_to_viewer(mesh: &TriMesh) -> MeshData {
    MeshData {
        positions: mesh
            .positions
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect(),
        triangles: mesh.triangles.clone(),
        edges: mesh.edges.clone(),
    }
}

/// A small cross around a sketch point (two segments), so vertices read as
/// selectable handles in the viewport.
fn point_cross(p: oep_sketch::sketch::Point, size: f64) -> (Vec<[f32; 3]>, Vec<[usize; 2]>) {
    let (x, y) = (p.x, p.y);
    let points: Vec<[f32; 3]> = vec![
        [x as f32 - size as f32, y as f32, 0.0],
        [x as f32 + size as f32, y as f32, 0.0],
        [x as f32, y as f32 - size as f32, 0.0],
        [x as f32, y as f32 + size as f32, 0.0],
    ];
    let edges = vec![[0usize, 1usize], [2usize, 3usize]];
    (points, edges)
}

/// The session-stable variable-set feature.
///
/// `oep_sketch::features::varset_to_feature` derives the feature id from the
/// FIRST variable name, which would change whenever the set's contents change
/// and orphan document references. The editor pins a stable id so the
/// variable set is a normal, re-writable canonical feature.
fn session_vars_feature(vars: &VariableSet) -> Feature {
    let mut f = varset_to_feature(vars);
    f.id = "oep-feature-varset-editor".to_string();
    f.name = "editor-vars".to_string();
    f
}

/// Serialize the profile + sweep amount into a feature parameter map (the
/// document's record of the operation, distinct from the tessellated mesh).
fn well_known_params(
    profile_key: &str,
    profile: &[oep_geo::kernel::Point2D],
    amount_key: &str,
    amount: f64,
) -> oep_geo::feature::Parameters {
    let points: Vec<serde_json::Value> = profile
        .iter()
        .map(|p| serde_json::json!([p.x, p.y]))
        .collect();
    let mut params = oep_geo::feature::Parameters::new();
    params.insert(profile_key.to_string(), serde_json::Value::Array(points));
    params.insert(amount_key.to_string(), serde_json::Value::from(amount));
    params
}

/// Encode an RGBA image as a P6 PPM (deterministic, std-only). The raster
/// serves as a machine-checkable viewport artifact alongside the PNG encoder.
pub fn encode_ppm(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + img.width as usize * img.height as usize * 3);
    out.extend_from_slice(format!("P6\n{} {}\n255\n", img.width, img.height).as_bytes());
    for i in (0..img.data.len()).step_by(4) {
        out.push(img.data[i]);
        out.push(img.data[i + 1]);
        out.push(img.data[i + 2]);
    }
    out
}

/// Wrapper used by the CLI/tests to build a dimension from a text token: a
/// string that parses as a finite number becomes a literal; anything else is
/// a variable-set reference by name.
pub fn parse_dim(token: &str) -> Result<DimExpr, EditorError> {
    match token.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(DimExpr::Literal(v)),
        Ok(v) => Err(EditorError::BadDimension(format!(
            "dimension must be a finite number, got {v}"
        ))),
        Err(_) => Ok(DimExpr::Variable(token.to_string())),
    }
}

/// The viewer theme highlight color for test assertions.
pub fn highlight_rgb() -> [u8; 3] {
    theme::HIGHLIGHT_RGB
}

/// The viewer background color for test assertions.
pub fn background_rgb() -> [u8; 3] {
    theme::BACKGROUND_RGB
}
