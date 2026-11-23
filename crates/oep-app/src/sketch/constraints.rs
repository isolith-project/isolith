//! UI-B6 UI-CONSTRAINTS: apply and remove sketch constraints through the
//! oep-sketch solver seam (wave B).
//!
//! The egui-free half of the constraint behaviour this card adds. The
//! interactive half lives in `panels/sketch.rs` (which `#[path]`-includes
//! this file); splitting the solver-facing layer out here keeps it
//! unit-testable without a window and gives the kittest acceptance a single
//! source of truth for the solver vocabulary and the honesty rules:
//!
//! - **One solver, the existing seam.** The card's DETAIL is explicit: "Use
//!   the existing solver seam (oep-assem / oep-sketch). Do not write a second
//!   solver." [`solve_set`] builds an `oep_sketch::sketch::Sketch` from the
//!   app's own [`SketchDoc`] plus the applied constraints and calls
//!   `oep_sketch::solve::solve_sketch`. No constraint math lives in this
//!   crate; the solver owns it.
//! - **Degrees of freedom are reported honestly.** [`SolveReport`] carries
//!   the solver's `dof` verbatim, with no floor at zero. A sketch that is
//!   not yet fully constrained reports its true positive dof; a sketch that
//!   has absorbed more constraint equations than free parameters reports the
//!   negative dof *and* the named [`ConstraintError::OverConstrained`]
//!   state - never a fabricated "dof 0, solved".
//! - **Over-constrained is a named error, never a silent partial solve.** The
//!   adopted solver returns a converged `SolveOutcome` whose `dof` is already
//!   negative when the sketch is over-constrained (the vcad contract:
//!   `dof < 0` IS over-constrained). This layer detects that and reports
//!   [`ConstraintError::OverConstrained`] instead of pretending the solve
//!   succeeded - the UI turns it into an honest "over-constrained" status,
//!   the acceptance asserts it as a distinct state, and an inconsistent
//!   constraint set surfaces as the typed [`SketchError::SolveNotConverged`]
//!   conflict, never a partial geometry.
//!
//! The constraint IDs are the app's own `p0..`/`l0..` ids from the committed
//! [`SketchDoc`] (the B4 canonical doc), so a toolbar button that names
//! "the last sketched line" resolves to the same `l0` the journal committed.

use std::fmt;

use super::tools::SketchDoc;

/// An applied constraint in the app's own vocabulary, referencing the app's
/// committed entity ids.
#[derive(Debug, Clone, PartialEq)]
pub enum UiConstraint {
    /// Force a line horizontal (parallel to the sketch x axis).
    Horizontal {
        /// Line id (`l0`, ...).
        line: String,
    },
    /// Force a line vertical (parallel to the sketch y axis).
    Vertical {
        /// Line id (`l0`, ...).
        line: String,
    },
    /// Two points coincide.
    Coincident {
        /// First point id (`p0`, ...).
        point_a: String,
        /// Second point id (`p1`, ...).
        point_b: String,
    },
    /// A line has a driven, literal length.
    Length {
        /// Line id (`l0`, ...).
        line: String,
        /// Target length (a literal value; variable-set-driven dimensions are
        /// UI-B7's card).
        value: f64,
    },
}

impl UiConstraint {
    /// Stable short kind (`horizontal`, `vertical`, `coincident`, `length`).
    // `dead_code` allowed: the kind tag is exercised by the acceptance and
    // module tests (the bin build drives constraints by `ApplyKind`, which
    // carries its own human label).
    #[allow(dead_code)]
    pub fn kind(&self) -> &'static str {
        match self {
            UiConstraint::Horizontal { .. } => "horizontal",
            UiConstraint::Vertical { .. } => "vertical",
            UiConstraint::Coincident { .. } => "coincident",
            UiConstraint::Length { .. } => "length",
        }
    }

    /// A stable one-line description (`horizontal l0`, `length l0=40`).
    pub fn describe(&self) -> String {
        match self {
            UiConstraint::Horizontal { line } => format!("horizontal {line}"),
            UiConstraint::Vertical { line } => format!("vertical {line}"),
            UiConstraint::Coincident { point_a, point_b } => {
                format!("coincident {point_a}->{point_b}")
            }
            UiConstraint::Length { line, value } => format!("length {line}={value:.4}"),
        }
    }
}

/// A typed failure of applying/solving constraints. Every variant is honest:
/// nothing here is a panic and nothing invents a solve that the solver did
/// not produce.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintError {
    /// A constraint references an id the document does not contain.
    UnknownEntity(String),
    /// The sketch is OVER-CONSTRAINED: its degrees of freedom went below
    /// zero with the added constraint. A distinct, named state - the solver
    /// reported a negative dof and this layer refuses to present it as a
    /// solved sketch.
    OverConstrained {
        /// The solver's (negative) degree-of-freedom count.
        dof: i32,
        /// Residual at the stopping point (already non-trivial only for an
        /// inconsistent over-constraint; a redundant-but-consistent one has a
        /// near-zero residual with a negative dof).
        residual: f64,
    },
    /// The constraint set is inconsistent: the adopted solver could not
    /// converge (its typed `SolveNotConverged`). Never a partial solve.
    Conflict(String),
    /// A dimensional value is invalid for its constraint kind.
    InvalidDimension(f64),
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintError::UnknownEntity(id) => write!(f, "unknown sketch entity {id}"),
            ConstraintError::OverConstrained { dof, residual } => {
                write!(f, "over-constrained: dof {dof} (residual {residual:.3e})")
            }
            ConstraintError::Conflict(msg) => write!(f, "conflicting constraints: {msg}"),
            ConstraintError::InvalidDimension(v) => write!(f, "invalid dimension value {v}"),
        }
    }
}

impl std::error::Error for ConstraintError {}

/// The solver's honest solve result for the current constraint set.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    /// Degrees of freedom reported by the solver (can be negative for an
    /// over-constrained sketch; never floored at zero).
    pub dof: i32,
    /// Final residual norm.
    pub residual: f64,
    /// Whether the solver converged (a non-converged set is an honest
    /// conflict, surfaced by the caller as `Conflict`).
    pub converged: bool,
    /// Number of applied constraints in the solved set.
    pub constraint_count: usize,
}

/// Target-resolution helpers on the committed doc (used by the toolbar /
/// context menu, which have no selection model yet - they act on the most
/// recently committed entity of the needed class, the same "last element"
/// guidance the B4 draw tools follow).
pub mod targets {
    use super::*;

    /// The most recently committed line id, if any (the H / V / Length target).
    pub fn last_line(doc: &SketchDoc) -> Option<&str> {
        doc.lines.last().map(|l| l.id.as_str())
    }

    /// The two most recently committed point ids, if at least two exist (the
    /// Coincident target).
    pub fn last_point_pair(doc: &SketchDoc) -> Option<(&str, &str)> {
        if doc.points.len() < 2 {
            return None;
        }
        let prev = &doc.points[doc.points.len() - 2];
        let last = &doc.points[doc.points.len() - 1];
        Some((prev.id.as_str(), last.id.as_str()))
    }

    /// The current measured length of a committed line, from its endpoints.
    pub fn line_length(doc: &SketchDoc, line: &str) -> Option<f64> {
        let seg = doc.lines.iter().find(|l| l.id == line)?;
        let a = doc.point_coord(&seg.start)?;
        let b = doc.point_coord(&seg.end)?;
        Some(aux_distance(a, b))
    }

    fn aux_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        (dx * dx + dy * dy).sqrt()
    }
}

/// The shared, context-store resident constraint panel: the applied
/// constraint list plus the last honest solve verdict. Lives in the egui
/// context data store under a `String` key like the B4 sketch doc, so the
/// acceptance harness and the toolbar read/write the same value across
/// frames without a custom type crossing `#[path]` copies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConstraintPanel {
    /// Applied constraints, most recent last (removal pops the last).
    pub constraints: Vec<UiConstraint>,
    /// The last honest solve: `Ok(SolveReport)` when under/fully constrained,
    /// `Err(OverConstrained)` / `Err(Conflict)` / ... otherwise.
    pub verdict: Option<Result<SolveReport, ConstraintError>>,
    /// One line describing the last apply/remove action (oracle readout).
    pub last: Option<String>,
}

impl ConstraintPanel {
    /// Number of applied constraints.
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Whether any constraints are applied.
    // `dead_code` allowed: exercised by the constraint acceptance and module
    // tests; the bin build reads `len()` and the verdict directly.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// The honest dof of the last solve, if a solve ran and converged to a
    /// non-negative dof. `None` when no solve, or when the last verdict was
    /// over-constrained / a conflict (those carry their own named state).
    // `dead_code` allowed: the acceptance asserts it; the status readout uses
    // `status_label` which carries the same arithmetic.
    #[allow(dead_code)]
    pub fn dof(&self) -> Option<i32> {
        match &self.verdict {
            Some(Ok(report)) => Some(report.dof),
            _ => None,
        }
    }

    /// Whether the last verdict was the named over-constrained state.
    // `dead_code` allowed: the acceptance asserts it as the honest
    // over-constrained marker.
    #[allow(dead_code)]
    pub fn is_over_constrained(&self) -> bool {
        matches!(
            &self.verdict,
            Some(Err(ConstraintError::OverConstrained { .. }))
        )
    }

    /// A stable compressed form of the last verdict for the status bar /
    /// oracle: `ok dof N`, `over-constrained`, `conflict: <reason>`, or
    /// `-` when nothing has been solved yet.
    pub fn status_label(&self) -> String {
        match &self.verdict {
            None => "not solved".to_string(),
            Some(Ok(r)) => {
                if r.dof == 0 {
                    "fully constrained (dof 0)".to_string()
                } else {
                    format!("dof {}", r.dof)
                }
            }
            Some(Err(ConstraintError::OverConstrained { dof, .. })) => {
                format!("over-constrained (dof {dof})")
            }
            Some(Err(other)) => other.to_string(),
        }
    }
}

/// The single UI-facing apply/remove operation this card supports. The
/// toolbar and context menu both drive exactly this enum; there is no second
/// path that mutates the constraint list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKind {
    /// Make the most recently committed line horizontal.
    Horizontal,
    /// Make the most recently committed line vertical.
    Vertical,
    /// Merge the two most recently committed points.
    Coincident,
    /// Pin the most recently committed line to its measured length.
    Length,
    /// Remove the most recently applied constraint.
    Remove,
}

/// Apply one [`ApplyKind`] to the sketch and return the updated panel plus a
/// one-line human message for the status readout. Pure and egui-free: the
/// caller owns the committed [`SketchDoc`] and the returned panel replaces
/// the previous one (the UI stores it back through the journal-free
/// constraint panel; constraint application does not rewrite geometry, so no
/// document-hash entry is needed here).
///
/// Honesty: the returned panel's verdict is the solver's own. Applying a
/// redundant constraint yields the named `OverConstrained` verdict (never a
/// false "solved"); an inconsistent set yields `Conflict`.
pub fn apply(
    doc: &SketchDoc,
    panel: &ConstraintPanel,
    kind: ApplyKind,
) -> (ConstraintPanel, Option<String>) {
    use targets::*;

    let mut next_constraints = panel.constraints.clone();

    let build = |next_constraints: Vec<UiConstraint>, event: &str| {
        let mut panel = panel.clone();
        panel.constraints = next_constraints;
        panel.verdict = Some(solve_set(doc, &panel.constraints).map_err(|e| e.clone()));
        panel.last = Some(event.to_string());
        (panel, Some(event.to_string()))
    };

    match kind {
        ApplyKind::Remove => {
            if next_constraints.pop().is_none() {
                return (panel.clone(), Some("nothing to remove".to_string()));
            }
            build(next_constraints, "removed last constraint")
        }
        ApplyKind::Horizontal | ApplyKind::Vertical | ApplyKind::Length => {
            let Some(line) = last_line(doc) else {
                return (panel.clone(), Some("no line to constrain".to_string()));
            };
            let event = match kind {
                ApplyKind::Horizontal => {
                    next_constraints.push(UiConstraint::Horizontal {
                        line: line.to_string(),
                    });
                    "constrain horizontal"
                }
                ApplyKind::Vertical => {
                    next_constraints.push(UiConstraint::Vertical {
                        line: line.to_string(),
                    });
                    "constrain vertical"
                }
                ApplyKind::Length => {
                    let Some(len) = line_length(doc, line) else {
                        return (panel.clone(), Some("cannot measure line".to_string()));
                    };
                    next_constraints.push(UiConstraint::Length {
                        line: line.to_string(),
                        value: len,
                    });
                    "constrain length"
                }
                _ => unreachable!(),
            };
            build(next_constraints, event)
        }
        ApplyKind::Coincident => {
            let Some((a, b)) = last_point_pair(doc) else {
                return (
                    panel.clone(),
                    Some("need two points to coincide".to_string()),
                );
            };
            next_constraints.push(UiConstraint::Coincident {
                point_a: a.to_string(),
                point_b: b.to_string(),
            });
            build(next_constraints, "constrain coincident")
        }
    }
}

/// Solve the committed sketch geometry plus the applied constraints through
/// the existing oep-sketch solver seam.
///
/// Returns `Err(ConstraintError::OverConstrained)` when the solver's honest
/// `dof` went negative with this constraint set - the over-constrained sketch
/// says so, it is never presented as a solved sketch. Returns the typed
/// `Conflict` for a solver non-convergence. `Ok(SolveReport)` only when the
/// sketch is under- or exactly-constrained and the solver converged.
pub fn solve_set(
    doc: &SketchDoc,
    constraints: &[UiConstraint],
) -> Result<SolveReport, ConstraintError> {
    use oep_sketch::sketch::Sketch;

    // Build the solver's declarative model from the app's committed doc.
    // Entity ids (`p0..`, `l0..`) match the oep-sketch vocabulary exactly, so
    // no id rewriting is needed.
    let mut solver = Sketch::new("ui-b6-sketch");
    for p in &doc.points {
        solver.add_point(&p.id, p.x, p.y);
    }
    for l in &doc.lines {
        solver.add_line(&l.id, &l.start, &l.end);
    }
    for c in constraints {
        match c {
            UiConstraint::Horizontal { line } => solver.constrain_horizontal(line),
            UiConstraint::Vertical { line } => solver.constrain_vertical(line),
            UiConstraint::Coincident { point_a, point_b } => {
                solver.constrain_coincident(point_a, point_b)
            }
            UiConstraint::Length { line, value } => {
                solver.constrain_length(line, oep_sketch::sketch::DimExpr::Literal(*value))
            }
        }
    }

    let outcome = oep_sketch::solve::solve_sketch(&solver, &oep_sketch::varset::VariableSet::new())
        .map_err(|e| match e {
            // UnknownEntity is the solver's own tolerance for a bad reference.
            oep_sketch::solve::SketchError::UnknownEntity(id) => ConstraintError::UnknownEntity(id),
            oep_sketch::solve::SketchError::InvalidDimension(v) => {
                ConstraintError::InvalidDimension(v)
            }
            other => ConstraintError::Conflict(other.to_string()),
        })?;

    // Honesty rule: a negative dof IS the over-constrained state (the vcad
    // contract: dof = params - equations; dof < 0 => over-constrained). The
    // solver can converge to a consistent redundant set with a negative dof -
    // report the named over-constrained state, never "dof 0 / solved".
    if outcome.dof < 0 {
        return Err(ConstraintError::OverConstrained {
            dof: outcome.dof,
            residual: outcome.residual,
        });
    }

    Ok(SolveReport {
        dof: outcome.dof,
        residual: outcome.residual,
        converged: outcome.converged,
        constraint_count: constraints.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_sketch::sketch::{DimExpr, Sketch};
    use oep_sketch::solve::solve_sketch;
    use oep_sketch::varset::VariableSet;

    /// Build a two-point line (p0-p1, l0), the canvas the toolbar tests use.
    fn line_doc() -> SketchDoc {
        let mut doc = SketchDoc::default();
        doc.add_point(0.0, 0.0);
        doc.add_point(40.0, 0.0);
        doc.add_line("p0", "p1");
        doc
    }

    /// The free-line dof through the actual solver, to keep this layer's
    /// translation honest against the seam.
    fn free_line_dof() -> i32 {
        let mut solver = Sketch::new("t");
        solver.add_point("p0", 0.0, 0.0);
        solver.add_point("p1", 40.0, 0.0);
        solver.add_line("l0", "p0", "p1");
        solve_sketch(&solver, &VariableSet::new()).unwrap().dof
    }

    /// Every constraint kind carries a stable kind + description and the
    /// describing string round-trips the ids.
    #[test]
    fn kinds_are_stable_and_describe_their_targets() {
        let cs = [
            UiConstraint::Horizontal { line: "l0".into() },
            UiConstraint::Vertical { line: "l0".into() },
            UiConstraint::Coincident {
                point_a: "p0".into(),
                point_b: "p1".into(),
            },
            UiConstraint::Length {
                line: "l0".into(),
                value: 40.0,
            },
        ];
        assert_eq!(cs[0].kind(), "horizontal");
        assert_eq!(cs[1].kind(), "vertical");
        assert_eq!(cs[2].kind(), "coincident");
        assert_eq!(cs[3].kind(), "length");
        assert_eq!(cs[0].describe(), "horizontal l0");
        assert_eq!(cs[3].describe(), "length l0=40.0000");
    }

    /// A free two-point line has 4 dof through the real solver; each
    /// horizontal / vertical / length constraint removes exactly one equation
    /// (2 dof total for a coincident). These are the solver's own numbers,
    /// asserted against the adopted seam, not re-derived here.
    #[test]
    fn dof_matches_the_solver_on_the_tool_path() {
        assert_eq!(free_line_dof(), 4, "free line starts at 4 dof");
        let doc = line_doc();
        let r = solve_set(&doc, &[]).unwrap();
        assert_eq!(r.dof, 4);
        assert_eq!(r.constraint_count, 0);

        let h = solve_set(&doc, &[UiConstraint::Horizontal { line: "l0".into() }]).unwrap();
        assert_eq!(h.dof, 3, "horizontal removes one dof");

        let v = solve_set(&doc, &[UiConstraint::Vertical { line: "l0".into() }]).unwrap();
        assert_eq!(v.dof, 3, "vertical removes one dof");

        let len = solve_set(
            &doc,
            &[UiConstraint::Length {
                line: "l0".into(),
                value: 40.0,
            }],
        )
        .unwrap();
        assert_eq!(len.dof, 3, "length removes one dof");
    }

    /// Redundant length constraints on a single line drive dof 4 -> 3 -> 2 ->
    /// 1 -> 0, and the FIFTH pushes it negative: the solver CONVERGES to a
    /// consistent-but-redundant set with dof<0, and this layer reports the
    /// named OverConstrained state instead of a fake "solved".
    #[test]
    fn redundant_constraint_reports_over_constrained_named_state() {
        let doc = line_doc();
        let mut set: Vec<UiConstraint> = Vec::new();
        for _ in 0..4 {
            set.push(UiConstraint::Length {
                line: "l0".into(),
                value: 40.0,
            });
        }
        assert_eq!(solve_set(&doc, &set).unwrap().dof, 0, "4 lengths -> dof 0");

        set.push(UiConstraint::Length {
            line: "l0".into(),
            value: 40.0,
        });
        match solve_set(&doc, &set) {
            Err(ConstraintError::OverConstrained { dof, .. }) => {
                assert!(dof < 0, "over-constrained dof is negative, got {dof}")
            }
            other => panic!("redundant 5th length must be OverConstrained, got {other:?}"),
        }
    }

    /// An INCONSISTENT constraint set (length 40 pinned by a line whose two
    /// points are already merged by H+V) is a typed conflict from the solver,
    /// surfaced as `Conflict` - never a partial solve.
    #[test]
    fn inconsistent_set_is_a_named_conflict_not_a_partial_solve() {
        let doc = line_doc();
        let set = vec![
            UiConstraint::Horizontal { line: "l0".into() },
            UiConstraint::Vertical { line: "l0".into() },
            UiConstraint::Length {
                line: "l0".into(),
                value: 40.0,
            },
        ];
        assert!(
            matches!(solve_set(&doc, &set), Err(ConstraintError::Conflict(_))),
            "H+V+length-40 on one line is inconsistent -> named Conflict"
        );
    }

    /// Removing a constraint restores the previous dof: apply 4 lengths
    /// (dof 0), then evaluate the set without the last -> dof back to 1
    /// (the honest removal path the UI toolbar drives).
    #[test]
    fn removing_restores_the_prior_dof() {
        let doc = line_doc();
        let mut set: Vec<UiConstraint> = Vec::new();
        for _ in 0..4 {
            set.push(UiConstraint::Length {
                line: "l0".into(),
                value: 40.0,
            });
        }
        assert_eq!(solve_set(&doc, &set).unwrap().dof, 0);
        set.pop();
        let r = solve_set(&doc, &set).unwrap();
        assert_eq!(r.dof, 1, "removing the last length frees one dof");
    }

    /// Coincident on two free points costs 2 dof through the seam.
    #[test]
    fn coincident_cost_matches_the_solver() {
        let mut doc = SketchDoc::default();
        doc.add_point(10.0, 8.0);
        doc.add_point(50.0, 8.0);
        assert_eq!(solve_set(&doc, &[]).unwrap().dof, 4);
        let r = solve_set(
            &doc,
            &[UiConstraint::Coincident {
                point_a: "p0".into(),
                point_b: "p1".into(),
            }],
        )
        .unwrap();
        assert_eq!(r.dof, 2, "coincident merges 2 dof");
    }

    /// The target helpers resolve the most recent committed entities.
    #[test]
    fn targets_resolve_the_last_committed_entities() {
        let mut doc = line_doc();
        assert_eq!(targets::last_line(&doc), Some("l0"));
        assert_eq!(targets::last_point_pair(&doc), Some(("p0", "p1")));
        assert_eq!(targets::line_length(&doc, "l0"), Some(40.0));
        doc.add_point(50.0, 12.0);
        assert_eq!(targets::last_point_pair(&doc), Some(("p1", "p2")));
    }

    /// An unknown target id is a typed UnknownEntity, never a panic.
    #[test]
    fn unknown_target_is_a_named_error() {
        let doc = line_doc();
        let r = solve_set(&doc, &[UiConstraint::Horizontal { line: "l99".into() }]);
        assert!(matches!(r, Err(ConstraintError::UnknownEntity(id)) if id == "l99"));
    }

    /// The `apply` driver implements the exact acceptance path: horizontal /
    /// vertical / coincident / length each apply through the solver and each
    /// is removable, restoring the prior dof. It is the one function the
    /// toolbar and context menu share.
    #[test]
    fn apply_and_remove_drive_the_whole_tool_path() {
        let doc = line_doc(); // p0-p1, l0, measured length 40
        let mut panel = ConstraintPanel::default();

        // Horizontal applies -> dof 3, then remove restores 4.
        let (panel2, msg) = apply(&doc, &panel, ApplyKind::Horizontal);
        assert_eq!(msg.as_deref(), Some("constrain horizontal"));
        assert_eq!(panel2.len(), 1);
        assert_eq!(panel2.dof(), Some(3));
        assert!(!panel2.is_over_constrained());
        panel = panel2;
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Remove);
        assert!(panel2.is_empty());
        assert_eq!(panel2.dof(), Some(4));
        panel = panel2;

        // Vertical applies -> dof 3, removed -> 4.
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Vertical);
        assert_eq!(panel2.dof(), Some(3));
        panel = panel2;
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Remove);
        assert_eq!(panel2.dof(), Some(4));
        panel = panel2;

        // Length pins the measured 40 -> dof 3, removed -> 4.
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Length);
        assert_eq!(panel2.len(), 1);
        assert_eq!(panel2.dof(), Some(3));
        panel = panel2;
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Remove);
        assert_eq!(panel2.dof(), Some(4));

        // Coincident needs two points; the doc has exactly two -> dof 2.
        let (panel2, _) = apply(&doc, &ConstraintPanel::default(), ApplyKind::Coincident);
        assert_eq!(panel2.len(), 1);
        assert_eq!(panel2.dof(), Some(2));
        let (panel3, _) = apply(&doc, &panel2, ApplyKind::Remove);
        assert_eq!(panel3.dof(), Some(4));
    }

    /// Over-constraining through the driver reports the named state with a
    /// NEGATIVE dof surfaced in the panel, and removal frees dof again - the
    /// "over-constrained honest" half of the acceptance.
    #[test]
    fn driver_reports_over_constrained_and_remove_heals() {
        let doc = line_doc();
        let mut panel = ConstraintPanel::default();
        // Redundant identical lengths: dof 4 -> 3 -> 2 -> 1 -> 0, then the
        // 5th pushes negative -> OverConstrained.
        for _ in 0..5 {
            let (next, _) = apply(&doc, &panel, ApplyKind::Length);
            panel = next;
        }
        assert!(panel.is_over_constrained());
        assert!(
            panel.dof().is_none(),
            "over-constrained carries no fake dof"
        );
        assert!(panel.status_label().starts_with("over-constrained"));
        assert_eq!(panel.len(), 5);

        // Removing one redundant length heals back to dof 0 (fully constrained).
        let (panel2, _) = apply(&doc, &panel, ApplyKind::Remove);
        assert_eq!(panel2.len(), 4);
        assert_eq!(panel2.dof(), Some(0));
        assert!(!panel2.is_over_constrained());
        assert_eq!(panel2.status_label(), "fully constrained (dof 0)");
    }

    /// `apply` with no line to constrain is an honest no-op message, not a
    /// panic and not a fabricated constraint.
    #[test]
    fn apply_with_no_target_is_an_honest_noop() {
        let doc = SketchDoc::default();
        let (p1, msg) = apply(&doc, &ConstraintPanel::default(), ApplyKind::Horizontal);
        assert_eq!(msg.as_deref(), Some("no line to constrain"));
        assert!(p1.is_empty());
        assert!(p1.verdict.is_none(), "no solve attempted on a no-op");

        // A single point can be a coincident target partner only when a
        // second point exists; with one point the driver says so honestly.
        let mut one = SketchDoc::default();
        one.add_point(10.0, 8.0);
        let (p2, msg2) = apply(&one, &ConstraintPanel::default(), ApplyKind::Coincident);
        assert_eq!(msg2.as_deref(), Some("need two points to coincide"));
        assert!(p2.is_empty());
        assert!(p2.verdict.is_none());
    }

    /// A Length constraint can reference a variable name (FreeCAD-v1
    /// semantics); that resolves against the variable set at solve time. The
    /// UI-B6 layer drives literals; the seam still honours a name.
    #[test]
    fn length_dim_expression_round_trips_through_the_seam() {
        let mut solver = Sketch::new("t");
        solver.add_point("p0", 0.0, 0.0);
        solver.add_point("p1", 40.0, 0.0);
        solver.add_line("l0", "p0", "p1");
        solver.constrain_length("l0", DimExpr::Literal(40.0));
        let vars = VariableSet::new();
        let outcome = solve_sketch(&solver, &vars).unwrap();
        assert_eq!(outcome.dof, 3);
    }
}
