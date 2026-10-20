//! The adopted-OSS-solver seam: solve a [`Sketch`] with a [`VariableSet`].
//!
//! The P7-M1 shell decision and this card's solver ADR (`workstreams/modeler/sketch-and-rules/sketch-solver-adr.md`)
//! mandate "adopt an OSS solver, do not rebuild". This module is the ONLY
//! place the adopted solver appears: the public function [`solve_sketch`] and
//! its return type [`SolveOutcome`] are our own types, and nothing solver-
//! specific leaks into the crate surface (mirror tested in the acceptance).
//!
//! Semantics: a dimension constraint that references a variable
//! ([`crate::sketch::DimExpr::Variable`]) is resolved against the
//! [`VariableSet`] at solve time; editing the variable and re-solving moves
//! the solved geometry. Each call constructs a fresh numeric model from the
//! declarative sketch (a fresh [`Sketch2D`]), solves it, and reads back the
//! solved point locations and driven lengths.

use crate::sketch::{Constraint, DimExpr, Sketch};
use crate::varset::{VariableError, VariableSet};
use std::collections::BTreeMap;

/// A typed error from solving a sketch.
#[derive(Debug, Clone)]
pub enum SketchError {
    /// A variable-set reference could not be resolved.
    VariableNotFound(VariableError),
    /// A constraint references an entity that is not in the sketch.
    UnknownEntity(String),
    /// The adopted OSS solver reported a divergence / numeric failure.
    SolveNotConverged {
        /// Why the solver stopped (e.g. max iterations, singular matrix).
        reason: String,
        /// Residual norm at the stopping point.
        residual: f64,
    },
    /// A dimensional target is negative or invalid for its constraint kind.
    InvalidDimension(f64),
    /// The variable set used for solving, for diagnostics.
    #[allow(dead_code)]
    Message(String),
}

impl std::fmt::Display for SketchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SketchError::VariableNotFound(e) => write!(f, "variable not found: {e}"),
            SketchError::UnknownEntity(e) => write!(f, "unknown sketch entity: {e}"),
            SketchError::SolveNotConverged { reason, residual } => {
                write!(
                    f,
                    "solver did not converge ({reason}, residual {residual:.3e})"
                )
            }
            SketchError::InvalidDimension(v) => write!(f, "invalid dimension value {v}"),
            SketchError::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for SketchError {}

/// The outcome of solving a sketch: solved geometry + measured dimensions.
#[derive(Debug, Clone)]
pub struct SolveOutcome {
    /// Solved point locations keyed by point id.
    pub points: BTreeMap<String, (f64, f64)>,
    /// Solved length of every dimensioned line, keyed by line id.
    pub lengths: BTreeMap<String, f64>,
    /// Solved dimension values keyed by the VARIABLE name they reference
    /// (FreeCAD-v1 semantics: the dimension is the variable-set entry, so a
    /// driven model dimension is addressed by its variable name, not by a
    /// line id).
    pub dimensions: BTreeMap<String, f64>,
    /// Final residual norm of the constraint system.
    pub residual: f64,
    /// Degrees of freedom remaining after solving (0 = fully constrained).
    pub dof: i32,
    /// Whether the solver converged within tolerance.
    pub converged: bool,
}

impl SolveOutcome {
    /// The solved location of a point.
    pub fn point(&self, id: &str) -> Option<(f64, f64)> {
        self.points.get(id).copied()
    }

    /// The solved length of a line.
    pub fn length_of(&self, line: &str) -> Option<f64> {
        self.lengths.get(line).copied()
    }

    /// The driven value of a dimension (a length that referenced a variable).
    pub fn driven(&self, name: &str) -> Option<f64> {
        self.dimensions.get(name).copied()
    }
}

/// Solve a sketch against a variable set with the adopted OSS constraint
/// solver, returning measured solved geometry. Never panics: any failure is a
/// typed [`SketchError`].
pub fn solve_sketch(sketch: &Sketch, vars: &VariableSet) -> Result<SolveOutcome, SketchError> {
    use vcad_kernel_constraints::{EntityRef, Sketch2D};

    let mut sk = Sketch2D::new();
    // Point id -> vcad entity id.
    let mut vp: BTreeMap<String, _> = BTreeMap::new();
    // Line id -> (start vid, end vid).
    let mut vl: BTreeMap<String, (_, _)> = BTreeMap::new();

    for (pid, pt) in &sketch.points {
        let e = sk.add_point(pt.x, pt.y);
        vp.insert(pid.clone(), e);
    }
    for (lid, seg) in &sketch.lines {
        let s = vp
            .get(&seg.start)
            .ok_or_else(|| SketchError::UnknownEntity(seg.start.clone()))?;
        let e = vp
            .get(&seg.end)
            .ok_or_else(|| SketchError::UnknownEntity(seg.end.clone()))?;
        let le = sk.add_line(*s, *e);
        vl.insert(lid.clone(), (*s, le));
    }

    for c in &sketch.constraints {
        match c {
            Constraint::Fixed { point, x, y } => {
                let p = vp
                    .get(point)
                    .ok_or_else(|| SketchError::UnknownEntity(point.clone()))?;
                sk.constrain_fixed(EntityRef::Point(*p), *x, *y);
            }
            Constraint::Horizontal { line } => {
                let (_, le) = vl
                    .get(line)
                    .ok_or_else(|| SketchError::UnknownEntity(line.clone()))?;
                sk.constrain_horizontal(*le);
            }
            Constraint::Vertical { line } => {
                let (_, le) = vl
                    .get(line)
                    .ok_or_else(|| SketchError::UnknownEntity(line.clone()))?;
                sk.constrain_vertical(*le);
            }
            Constraint::Coincident { point_a, point_b } => {
                let a = vp
                    .get(point_a)
                    .ok_or_else(|| SketchError::UnknownEntity(point_a.clone()))?;
                let b = vp
                    .get(point_b)
                    .ok_or_else(|| SketchError::UnknownEntity(point_b.clone()))?;
                sk.constrain_coincident(EntityRef::Point(*a), EntityRef::Point(*b));
            }
            Constraint::Length { line, expr } => {
                let value = resolve(expr, vars)?;
                if value <= 0.0 {
                    return Err(SketchError::InvalidDimension(value));
                }
                let (_, le) = vl
                    .get(line)
                    .ok_or_else(|| SketchError::UnknownEntity(line.clone()))?;
                sk.constrain_length(*le, value);
            }
            Constraint::Distance {
                point_a,
                point_b,
                expr,
            } => {
                let value = resolve(expr, vars)?;
                if value <= 0.0 {
                    return Err(SketchError::InvalidDimension(value));
                }
                let a = vp
                    .get(point_a)
                    .ok_or_else(|| SketchError::UnknownEntity(point_a.clone()))?;
                let b = vp
                    .get(point_b)
                    .ok_or_else(|| SketchError::UnknownEntity(point_b.clone()))?;
                sk.constrain_distance(EntityRef::Point(*a), EntityRef::Point(*b), value);
            }
        }
    }

    // Enforce convergence: a non-converged solve means the sketch is
    // over/under-constrained or numerically divergent - we must not fabricate
    // geometry, so a typed error is the honest outcome.
    let result = sk.solve_default();
    if !result.converged {
        return Err(SketchError::SolveNotConverged {
            reason: format!("{:?}", result.status),
            residual: result.residual_norm,
        });
    }

    let mut points = BTreeMap::new();
    for (pid, id) in &vp {
        let (x, y) = sk
            .get_point(*id)
            .ok_or_else(|| SketchError::Message(format!("solver dropped point {pid}")))?;
        points.insert(pid.clone(), (x, y));
    }

    let mut lengths = BTreeMap::new();
    let mut dimensions = BTreeMap::new();
    for c in &sketch.constraints {
        if let Constraint::Length { line, expr } = c {
            if let Some(((x1, y1), (x2, y2))) = sk.get_line_endpoints(vl[line].1) {
                let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                lengths.insert(line.clone(), len);
                // FreeCAD-v1 variable-set semantics: the dimension a length
                // references is addressed by its variable name.
                if let DimExpr::Variable(name) = expr {
                    dimensions.insert(name.clone(), len);
                }
            }
        }
    }

    Ok(SolveOutcome {
        points,
        lengths,
        dimensions,
        residual: result.residual_norm,
        dof: sk.degrees_of_freedom(),
        converged: result.converged,
    })
}

/// Resolve a dimension expression against the variable set.
fn resolve(expr: &DimExpr, vars: &VariableSet) -> Result<f64, SketchError> {
    match expr {
        DimExpr::Literal(v) => Ok(*v),
        DimExpr::Variable(name) => vars.get(name).map_err(SketchError::VariableNotFound),
    }
}
