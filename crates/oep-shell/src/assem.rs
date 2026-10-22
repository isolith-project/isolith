//! EDITOR-ASSEM-EDIT: the interactive assembly editing surface.
//!
//! This module turns the assembly solver into a part of the editor surface:
//! an engineer (or an agent driving the same command set) loads a committed
//! fixture, creates/edits/deletes mates from the oep-assem vocabulary (fixed,
//! coincident, on-plane, plane-distance, parallel), and every mate edit
//! re-solves live through the vendor-neutral seam (oep-assem workbench over
//! the oep-kinematics damped Gauss-Newton lane). The constrained instances
//! follow immediately; a diverging solve returns a typed error and never
//! fabricates a pose (the last converged placement stays, never a pretend
//! one).
//!
//! The surface also plays mechanism motion through the kinematics lane
//! (P7-M3-T02): `animate` drives the committed crank-slider fixture from its
//! joint set and records a deterministic per-frame trace plus rendered frames,
//! so a simple mechanism's motion is observable and repeatable.
//!
//! Rendering ("the viewport") is the deterministic CPU raster of the
//! kinematics lane (`oep_kinematics::raster`): each body renders as its
//! bounding-box proxy placed by the solved instance transform, written as a
//! PPM image. The surface is std-only and GPU-less, so the whole interactive
//! acceptance runs on the CI container.
//!
//! Honesty contract (checked by tests): a diverging solve returns a typed
//! [`MateOutcome::AppliedButDiverged`] carrying the exact
//! [`oep_assem::WorkbenchError`]; the document's poses are never mutated on
//! failure, and rendered frames are content-hashed (self-contained SHA-256)
//! so a follow is provable from two consecutive renders.

use std::fmt;

use oep_assem::mates::{AnchorRef, ConstraintSet, Mate};
use oep_assem::workbench::{self, SolveReport, WorkbenchError};
use oep_assem::InstanceInfo;
use oep_geo::Document;
use oep_kinematics::raster::{Camera, Mesh, SceneItem};

use crate::sha256;

/// The committed 3-part model the surface opens by default.
pub const FIXTURE_MODEL: &str =
    include_str!("../../oep-assem/tests/golden/three-part-fixture.oepgeo.json");

/// The committed mate set for the 3-part model.
pub const FIXTURE_CONSTRAINTS: &str =
    include_str!("../../oep-assem/tests/golden/three-part-fixture.constraints.json");

/// Assembly id inside the fixture.
pub const FIXTURE_ASSEMBLY: &str = "oep-assembly-fixture3";

/// Debounce/tolerance for a mate edit live-solve write-back (document units).
pub const SOLVE_TOLERANCE: f64 = 1e-6;

/// Error type of the assembly surface (shell-local; distinct from the
/// workbench's solve-time errors which are typed and passed through).
#[derive(Debug, Clone, PartialEq)]
pub enum AssemError {
    /// The named assembly is not present in the document.
    AssemblyNotFound(String),
    /// A mate index is out of range for the current constraint set.
    MateIndexOutOfBounds {
        /// Requested index.
        index: usize,
        /// Length of the current mate set.
        count: usize,
    },
    /// The mate arguments do not parse into a Mate.
    BadMateArguments(String),
    /// I/O error (rendering or recording).
    Io(String),
    /// The surfaces cannot be extracted (document has no geometry to draw).
    NoGeometry(String),
}

impl fmt::Display for AssemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssemError::AssemblyNotFound(id) => write!(f, "assembly not found: {id}"),
            AssemError::MateIndexOutOfBounds { index, count } => {
                write!(f, "mate index {index} out of range (count {count})")
            }
            AssemError::BadMateArguments(msg) => write!(f, "bad mate arguments: {msg}"),
            AssemError::Io(msg) => write!(f, "io error: {msg}"),
            AssemError::NoGeometry(msg) => write!(f, "no geometry: {msg}"),
        }
    }
}

impl std::error::Error for AssemError {}

impl From<std::io::Error> for AssemError {
    fn from(e: std::io::Error) -> Self {
        AssemError::Io(e.to_string())
    }
}

/// A short, stable tag naming a workbench error kind (the typed error the
/// surface reports on a diverging solve, so agents and tests can match it).
pub fn workbench_error_kind(e: &WorkbenchError) -> &'static str {
    match e {
        WorkbenchError::AssemblyNotFound(_) => "assembly-not-found",
        WorkbenchError::InstanceNotFound(_) => "instance-not-found",
        WorkbenchError::AnchorNotFound { .. } => "anchor-not-found",
        WorkbenchError::NoAnchors(_) => "no-anchors",
        WorkbenchError::SubAssemblyNotSupported(_) => "sub-assembly-not-supported",
        WorkbenchError::SolverFailed { .. } => "solver-failed",
        WorkbenchError::ContradictoryFixed(_) => "contradictory-fixed",
        WorkbenchError::NotZAxisAnchor { .. } => "not-z-axis-anchor",
        WorkbenchError::RigidOffsetNotSupported { .. } => "rigid-offset-not-supported",
        WorkbenchError::PlaneOnMovingNotSupported { .. } => "plane-on-moving-not-supported",
        WorkbenchError::DocumentInvalid(_) => "document-invalid",
        WorkbenchError::Serialization(_) => "serialization",
    }
}

/// Outcome of a live-solve that followed a mate edit.
#[derive(Debug, Clone)]
pub enum MateOutcome {
    /// The mate was applied and the assembly re-solved; instances followed.
    AppliedAndSolved {
        /// Index of the edited mate in the constraint set.
        mate_index: usize,
        /// The re-solve report (success).
        report: SolveReport,
    },
    /// The mate is applied but the live solve diverged: a typed error, and
    /// the document's poses are unchanged (the last converged placement
    /// stays; nothing is fabricated).
    AppliedButDiverged {
        /// Index of the edited mate.
        mate_index: usize,
        /// The typed solve error.
        error: WorkbenchError,
    },
    /// The mate was malformed for this assembly and refused (not applied).
    Rejected {
        /// Index it would have taken.
        mate_index: usize,
        /// The typed reason.
        error: MateEditError,
    },
}

/// A mate edit was refused before/during solve for a non-divergence reason.
#[derive(Debug, Clone)]
pub enum MateEditError {
    /// Index out of bounds (edit/delete).
    IndexOutOfBounds(usize, usize),
    /// Arguments did not parse.
    BadArguments(String),
    /// The workbench rejected the mate structurally.
    Workbench(WorkbenchError),
    /// Not a workbench error.
    Other(AssemError),
}

impl fmt::Display for MateEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MateEditError::IndexOutOfBounds(index, count) => {
                write!(f, "mate index {index} out of range (count {count})")
            }
            MateEditError::BadArguments(msg) => write!(f, "bad mate arguments: {msg}"),
            MateEditError::Workbench(e) => {
                write!(f, "{}: {e}", workbench_error_kind(e))
            }
            MateEditError::Other(e) => write!(f, "{e}"),
        }
    }
}

/// Outcome of an explicit solve command.
#[derive(Debug, Clone)]
pub enum SolveOutcome {
    /// The solve converged and placements were written back.
    Solved(SolveReport),
    /// The solve diverged: typed error, poses untouched, never fabricated.
    Diverged(WorkbenchError),
}

/// Result of rendering the current assembly pose to PPM.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// Path written.
    pub path: String,
    /// SHA-256 of the PPM bytes (proof the viewport changed between solves).
    pub sha256: String,
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels.
    pub height: usize,
}

/// Result of a mechanism-motion recording over the kinematics lane.
#[derive(Debug, Clone)]
pub struct AnimateResult {
    /// Number of recorded frames (one crank turn).
    pub frames: usize,
    /// Worst closed-loop joint residual over the turn.
    pub worst_residual: f64,
    /// Worst translation error vs the analytic crank-slider solution (mm).
    pub worst_translation_error: f64,
    /// Worst orientation error vs the analytic solution (rad).
    pub worst_orientation_error: f64,
    /// Free DOF of the mechanism when the crank is not driven (1 for a
    /// crank-slider).
    pub free_dof_undriven: usize,
    /// SHA-256 of the recorded trace (determinism proof).
    pub trace_sha256: String,
    /// Output directory.
    pub out_dir: String,
}

/// The interactive assembly editing session: current document + mate set +
/// a journal of every edit and solve.
#[derive(Debug, Clone)]
pub struct AssemblySession {
    document: Document,
    constraint_set: ConstraintSet,
    journal: Vec<String>,
    last_solve: Option<SolveReport>,
}

impl AssemblySession {
    /// Open the committed 3-part fixture (document + mate set).
    pub fn open_fixture() -> Result<Self, AssemError> {
        Self::open(FIXTURE_MODEL, FIXTURE_CONSTRAINTS)
    }

    /// Open an arbitrary canonical oep-geo document + oep-assem constraint
    /// set from their serialized forms.
    pub fn open(model_json: &str, constraints_json: &str) -> Result<Self, AssemError> {
        let document = Document::from_json_pretty(model_json)
            .map_err(|e| AssemError::Io(format!("bad model json: {e}")))?;
        let errors = document.validate();
        if !errors.is_empty() {
            return Err(AssemError::Io(format!(
                "model invalid: {} validation error(s)",
                errors.len()
            )));
        }
        let constraint_set = ConstraintSet::from_json_pretty(constraints_json)
            .map_err(|e| AssemError::BadMateArguments(format!("bad constraint json: {e}")))?;
        if !document
            .assemblies
            .iter()
            .any(|a| a.id == constraint_set.assembly)
        {
            return Err(AssemError::AssemblyNotFound(
                constraint_set.assembly.clone(),
            ));
        }
        let journal = vec![format!(
            "assembled {} (parts={} instances={} mates={})",
            constraint_set.assembly,
            document.parts.len(),
            document
                .assemblies
                .iter()
                .find(|a| a.id == constraint_set.assembly)
                .map(|a| a.instances.len())
                .unwrap_or(0),
            constraint_set.mates.len()
        )];
        Ok(Self {
            document,
            constraint_set,
            journal,
            last_solve: None,
        })
    }

    /// The canonical document with the current (solved) placements.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The assembly id being edited.
    pub fn assembly_id(&self) -> &str {
        &self.constraint_set.assembly
    }

    /// The current mate set (ordered).
    pub fn mates(&self) -> &[Mate] {
        &self.constraint_set.mates
    }

    /// The per-session operation journal.
    pub fn journal(&self) -> &[String] {
        &self.journal
    }

    /// The last successful solve report, if any.
    pub fn last_solve(&self) -> Option<&SolveReport> {
        self.last_solve.as_ref()
    }

    /// List the assembly instances (id, part, current transform).
    pub fn instances(&self) -> Vec<InstanceInfo> {
        workbench::list_instances(&self.document, self.assembly_id()).unwrap_or_default()
    }

    /// Which instance ids the live solve will move: every assembly instance
    /// except those pinned by a `Fixed` mate.
    pub fn moving_ids(&self) -> Vec<String> {
        let fixed: Vec<&str> = self
            .constraint_set
            .mates
            .iter()
            .filter_map(|m| match m {
                Mate::Fixed { instance } => Some(instance.as_str()),
                _ => None,
            })
            .collect();
        let mut out = Vec::new();
        if let Some(a) = self
            .document
            .assemblies
            .iter()
            .find(|a| a.id == self.assembly_id())
        {
            for inst in &a.instances {
                if !fixed.contains(&inst.id.as_str()) {
                    out.push(inst.id.clone());
                }
            }
        }
        out
    }

    /// Solve the current mate set through the vendor-neutral seam and write
    /// the placements back into the document. On divergence the document is
    /// left untouched (never a fabricated pose) and the typed error is
    /// returned.
    pub fn solve(&mut self) -> SolveOutcome {
        let moving = self.moving_ids();
        let req = workbench::SolveRequest::new(
            self.assembly_id().to_string(),
            self.constraint_set.mates.clone(),
            moving,
        );
        match workbench::solve(&mut self.document, &req) {
            Ok(report) => {
                self.last_solve = Some(report.clone());
                self.journal.push(format!(
                    "solved dof={} moved={} max_dist={:.3e} max_angle={:.3e}",
                    report.dof,
                    report.moved.len(),
                    report.max_distance_error,
                    report.max_angle_error
                ));
                SolveOutcome::Solved(report)
            }
            Err(e) => {
                self.journal
                    .push(format!("solve diverged: {}: {e}", workbench_error_kind(&e)));
                SolveOutcome::Diverged(e)
            }
        }
    }

    /// Create a mate and re-solve live. The mate is kept in the set; when the
    /// live solve diverges the typed error is returned and the document poses
    /// stay at the last converged placement.
    pub fn add_mate(&mut self, mate: Mate) -> MateOutcome {
        let idx = self.constraint_set.mates.len();
        self.constraint_set.mates.push(mate);
        match self.solve() {
            SolveOutcome::Solved(report) => {
                self.journal.push(format!("added mate {idx}: kept, solved"));
                MateOutcome::AppliedAndSolved {
                    mate_index: idx,
                    report,
                }
            }
            SolveOutcome::Diverged(error) => {
                if matches!(error, WorkbenchError::SolverFailed { .. }) {
                    self.journal.push(format!(
                        "added mate {idx}: kept, solve diverged ({})",
                        workbench_error_kind(&error)
                    ));
                    MateOutcome::AppliedButDiverged {
                        mate_index: idx,
                        error,
                    }
                } else {
                    self.constraint_set.mates.pop();
                    self.journal.push(format!(
                        "added mate {idx}: rejected ({})",
                        workbench_error_kind(&error)
                    ));
                    MateOutcome::Rejected {
                        mate_index: idx,
                        error: MateEditError::Workbench(error),
                    }
                }
            }
        }
    }

    /// Replace the mate at `index` and re-solve live (same divergence rule as
    /// [`Self::add_mate`]).
    pub fn edit_mate(&mut self, index: usize, mate: Mate) -> MateOutcome {
        if index >= self.constraint_set.mates.len() {
            return MateOutcome::Rejected {
                mate_index: index,
                error: MateEditError::IndexOutOfBounds(index, self.constraint_set.mates.len()),
            };
        }
        self.constraint_set.mates[index] = mate;
        match self.solve() {
            SolveOutcome::Solved(report) => {
                self.journal
                    .push(format!("edited mate {index}: kept, solved"));
                MateOutcome::AppliedAndSolved {
                    mate_index: index,
                    report,
                }
            }
            SolveOutcome::Diverged(error) => {
                if matches!(error, WorkbenchError::SolverFailed { .. }) {
                    self.journal.push(format!(
                        "edited mate {index}: kept, solve diverged ({})",
                        workbench_error_kind(&error)
                    ));
                    MateOutcome::AppliedButDiverged {
                        mate_index: index,
                        error,
                    }
                } else {
                    self.journal.push(format!(
                        "edited mate {index}: rejected ({})",
                        workbench_error_kind(&error)
                    ));
                    MateOutcome::Rejected {
                        mate_index: index,
                        error: MateEditError::Workbench(error),
                    }
                }
            }
        }
    }

    /// Delete the mate at `index` and re-solve live.
    pub fn remove_mate(&mut self, index: usize) -> MateOutcome {
        if index >= self.constraint_set.mates.len() {
            return MateOutcome::Rejected {
                mate_index: index,
                error: MateEditError::IndexOutOfBounds(index, self.constraint_set.mates.len()),
            };
        }
        self.constraint_set.mates.remove(index);
        match self.solve() {
            SolveOutcome::Solved(report) => {
                self.journal.push(format!("removed mate {index}: solved"));
                MateOutcome::AppliedAndSolved {
                    mate_index: index,
                    report,
                }
            }
            SolveOutcome::Diverged(error) => {
                self.journal.push(format!(
                    "removed mate {index}: solve diverged ({})",
                    workbench_error_kind(&error)
                ));
                MateOutcome::AppliedButDiverged {
                    mate_index: index,
                    error,
                }
            }
        }
    }

    /// Render the current assembly pose to a PPM image (the viewport).
    pub fn render_ppm(&self, path: &str) -> Result<RenderResult, AssemError> {
        let scene = self.scene()?;
        self.render_scene_ppm(&scene, path)
    }

    fn render_scene_ppm(
        &self,
        scene: &[SceneItem],
        path: &str,
    ) -> Result<RenderResult, AssemError> {
        let cam = Camera {
            eye: [150.0, 160.0, 180.0],
            target: [30.0, 25.0, 5.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 40.0,
            width: 640,
            height: 480,
        };
        cam.render_ppm(scene, path)?;
        let bytes = std::fs::read(path)?;
        Ok(RenderResult {
            path: path.to_string(),
            sha256: sha256::sha256_hex(&bytes),
            width: cam.width,
            height: cam.height,
        })
    }

    /// Build the bbox-proxy scene for the current assembly pose.
    pub fn scene(&self) -> Result<Vec<SceneItem>, AssemError> {
        let assembly = self
            .document
            .assemblies
            .iter()
            .find(|a| a.id == self.assembly_id())
            .ok_or_else(|| AssemError::AssemblyNotFound(self.assembly_id().to_string()))?;
        let mut items = Vec::new();
        for (order, inst) in assembly.instances.iter().enumerate() {
            let part_id = match &inst.source {
                oep_geo::InstanceSource::Part { id } => id.as_str(),
                oep_geo::InstanceSource::Assembly { id } => {
                    return Err(AssemError::NoGeometry(format!(
                        "sub-assembly instance {} (of {id}) is not rendered in v1",
                        inst.id
                    )))
                }
            };
            let Some(part) = self.document.parts.iter().find(|p| p.id == part_id) else {
                continue;
            };
            // Union of the part's body bounding boxes.
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            let mut any = false;
            for body in &self.document.bodies {
                if !part.body_ids.iter().any(|id| id == &body.id) {
                    continue;
                }
                if let Some(bbox) = &body.bbox {
                    for i in 0..3 {
                        lo[i] = lo[i].min(bbox.min[i]);
                        hi[i] = hi[i].max(bbox.max[i]);
                    }
                    any = true;
                }
            }
            if !any {
                continue;
            }
            let t = &inst.transform;
            let c = t.components();
            // oep-geo transform is column-major; extract the basis columns for
            // the raster (which is the same column-vector convention).
            let rot9 = [c[0], c[1], c[2], c[4], c[5], c[6], c[8], c[9], c[10]];
            let pos = [c[12], c[13], c[14]];
            let mesh = Mesh::box_mesh(lo, hi).transformed(pos, &rot9);
            items.push(SceneItem {
                mesh,
                color: palette_for(order, &inst.id),
            });
        }
        if items.is_empty() {
            return Err(AssemError::NoGeometry("no drawable bodies".to_string()));
        }
        Ok(items)
    }

    /// Play mechanism motion through the kinematics lane (P7-M3-T02 damped
    /// Gauss-Newton) and record a deterministic trace + rendered frames.
    pub fn animate(&self, frames: usize, out_dir: &str) -> Result<AnimateResult, AssemError> {
        let result = oep_kinematics::demo::run_demo(frames);
        std::fs::create_dir_all(out_dir)?;
        let trace_path = format!("{out_dir}/trace.json");
        let frames_dir = format!("{out_dir}/frames");
        oep_kinematics::demo::write_trace(&result, &result.frames, &trace_path)?;
        oep_kinematics::demo::render_frames(&result.mechanism, &result.frames, &frames_dir)?;
        let trace_bytes = std::fs::read(&trace_path)?;
        let trace_sha256 = sha256::sha256_hex(&trace_bytes);
        self.write_animate_manifest(&result, out_dir, &trace_path, &frames_dir);
        Ok(AnimateResult {
            frames: result.frames.len(),
            worst_residual: result.worst_residual,
            worst_translation_error: result.worst_translation_error,
            worst_orientation_error: result.worst_orientation_error,
            free_dof_undriven: result.free_dof_undriven,
            trace_sha256,
            out_dir: out_dir.to_string(),
        })
    }

    fn write_animate_manifest(
        &self,
        result: &oep_kinematics::demo::DemoResult,
        out_dir: &str,
        trace_path: &str,
        frames_dir: &str,
    ) {
        let g = result.gates();
        let mut b = String::new();
        b.push_str("{\n");
        b.push_str("  \"task\": \"editor-assem-motion\",\n");
        b.push_str(&format!("  \"frames\": {},\n", result.frames.len()));
        b.push_str(&format!(
            "  \"worst_residual_max\": {:.3e},\n",
            g.worst_residual
        ));
        b.push_str(&format!(
            "  \"worst_translation_error_mm\": {:.3e},\n",
            g.worst_translation_error
        ));
        b.push_str(&format!(
            "  \"worst_orientation_error_rad\": {:.3e},\n",
            g.worst_orientation_error
        ));
        b.push_str(&format!(
            "  \"free_dof_undriven\": {},\n",
            g.free_dof_undriven
        ));
        b.push_str(&format!("  \"trace\": \"{trace_path}\",\n"));
        b.push_str(&format!("  \"frame_dir\": \"{frames_dir}\"\n"));
        b.push_str("}\n");
        let _ = std::fs::write(std::path::Path::new(out_dir).join("manifest.json"), b);
    }
}

/// Human-readable description of one mate (used by the surface listing).
pub fn describe_mate(index: usize, mate: &Mate) -> String {
    match mate {
        Mate::Fixed { instance } => format!("{index} fixed {instance}"),
        Mate::Coincident { a, b } => format!(
            "{index} coincident {}:{} == {}:{}",
            a.instance, a.anchor, b.instance, b.anchor
        ),
        Mate::OnPlane { point, plane } => format!(
            "{index} on-plane {}:{} on {}:{}",
            point.instance, point.anchor, plane.instance, plane.anchor
        ),
        Mate::PlaneDistance {
            point,
            plane,
            distance,
        } => format!(
            "{index} plane-distance {}:{} to {}:{} = {distance}",
            point.instance, point.anchor, plane.instance, plane.anchor
        ),
        Mate::Parallel { a, b } => format!(
            "{index} parallel {}:{} || {}:{}",
            a.instance, a.anchor, b.instance, b.anchor
        ),
    }
}

/// Parse a mate from the CLI/agent vocabulary:
///
/// - `fixed:INSTANCE`
/// - `coincident:INST_A:ANCH_A:INST_B:ANCH_B`
/// - `on-plane:INST:ANCH:P_INST:P_ANCH`
/// - `plane-distance:INST:ANCH:P_INST:P_ANCH:DIST`
/// - `parallel:INST_A:ANCH_A:INST_B:ANCH_B`
pub fn parse_mate(kind: &str, args: &[&str]) -> Result<Mate, AssemError> {
    let aref = |i: usize| -> Result<AnchorRef, AssemError> {
        args.get(i)
            .map(|s| AnchorRef {
                instance: s.to_string(),
                anchor: args.get(i + 1).unwrap_or(&"").to_string(),
            })
            .ok_or_else(|| AssemError::BadMateArguments(kind.to_string()))
    };
    match kind {
        "fixed" => {
            if args.len() != 1 {
                return Err(AssemError::BadMateArguments(
                    "fixed expects INSTANCE".to_string(),
                ));
            }
            Ok(Mate::Fixed {
                instance: args[0].to_string(),
            })
        }
        "coincident" => {
            if args.len() != 4 {
                return Err(AssemError::BadMateArguments(
                    "coincident expects INSTA ANCHA INSTB ANCHB".to_string(),
                ));
            }
            Ok(Mate::Coincident {
                a: aref(0)?,
                b: aref(2)?,
            })
        }
        "on-plane" => {
            if args.len() != 4 {
                return Err(AssemError::BadMateArguments(
                    "on-plane expects INST ANCH P_INST P_ANCH".to_string(),
                ));
            }
            Ok(Mate::OnPlane {
                point: aref(0)?,
                plane: aref(2)?,
            })
        }
        "plane-distance" => {
            if args.len() != 5 {
                return Err(AssemError::BadMateArguments(
                    "plane-distance expects INST ANCH P_INST P_ANCH DIST".to_string(),
                ));
            }
            let distance: f64 = args[4].parse().map_err(|_| {
                AssemError::BadMateArguments("distance is not a number".to_string())
            })?;
            Ok(Mate::PlaneDistance {
                point: aref(0)?,
                plane: aref(2)?,
                distance,
            })
        }
        "parallel" => {
            if args.len() != 4 {
                return Err(AssemError::BadMateArguments(
                    "parallel expects INSTA ANCHA INSTB ANCHB".to_string(),
                ));
            }
            Ok(Mate::Parallel {
                a: aref(0)?,
                b: aref(2)?,
            })
        }
        other => Err(AssemError::BadMateArguments(format!(
            "unknown mate kind '{other}'"
        ))),
    }
}

/// Deterministic per-instance palette (indexed first, then id hash).
fn palette_for(order: usize, id: &str) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [
        [52, 120, 210], // blue
        [52, 170, 84],  // green
        [214, 82, 48],  // red
        [216, 158, 40], // amber
        [140, 76, 200], // violet
        [20, 140, 150], // teal
        [180, 90, 60],  // rust
        [90, 110, 130], // slate
    ];
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    PALETTE[((order as u32).wrapping_add(h)) as usize % PALETTE.len()]
}
