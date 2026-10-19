//! The declarative sketch model: points, line segments, and constraints.
//!
//! This is the chosen shell's OWN sketch model (P7-M4-T01) - it carries no
//! FreeCAD types and no OSS-solver types. Dimensions reference variable-set
//! entries by name through [`DimExpr::Variable`], which is the
//! FreeCAD-v1-variable-set semantics: the sketch is parametric, a variable
//! edit re-solves the sketch (see [`crate::solve`]).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A sketch dimension target: a literal value or a variable-set entry name.
///
/// `Variable("width")` means "the resolved value of variable-set entry
/// `width` at solve time" - the parametric-variable-set behavior. `Literal`
/// is the fixed-dimension fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DimExpr {
    /// A fixed numeric value.
    Literal(f64),
    /// Reference to a variable-set entry by name (resolved at solve time).
    Variable(String),
}

impl DimExpr {
    /// The literal, if this is [`DimExpr::Literal`].
    pub fn literal(&self) -> Option<f64> {
        match self {
            DimExpr::Literal(v) => Some(*v),
            DimExpr::Variable(_) => None,
        }
    }

    /// The referenced variable name, if this is [`DimExpr::Variable`].
    pub fn variable(&self) -> Option<&str> {
        match self {
            DimExpr::Literal(_) => None,
            DimExpr::Variable(name) => Some(name),
        }
    }
}

/// A geometric or dimensional constraint in the sketch.
///
/// `kind` is a closed vocabulary of the constraints this milestone supports
/// (fixed anchor, horizontal/vertical, coincident, and dimensional
/// length/distance tied to a [`DimExpr`]); every one is solved by the adopted
/// OSS solver behind `crate::solve`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    /// Pin a point at a fixed (x, y) location (the sketch anchor).
    Fixed {
        /// Point id.
        point: String,
        /// Anchored x.
        x: f64,
        /// Anchored y.
        y: f64,
    },
    /// Force a line horizontal (parallel to the sketch x axis).
    Horizontal {
        /// Line id.
        line: String,
    },
    /// Force a line vertical (parallel to the sketch y axis).
    Vertical {
        /// Line id.
        line: String,
    },
    /// Two points coincide.
    Coincident {
        /// First point id.
        point_a: String,
        /// Second point id.
        point_b: String,
    },
    /// A line has a driven length (literal or variable-set-referenced).
    Length {
        /// Line id.
        line: String,
        /// Target length expression.
        expr: DimExpr,
    },
    /// The distance between two points equals a driven value.
    Distance {
        /// First point id.
        point_a: String,
        /// Second point id.
        point_b: String,
        /// Target distance expression.
        expr: DimExpr,
    },
}

impl Constraint {
    /// The dimensional expression carried by a dimensional constraint, if any.
    pub fn dimension_expr(&self) -> Option<&DimExpr> {
        match self {
            Constraint::Length { expr, .. } | Constraint::Distance { expr, .. } => Some(expr),
            _ => None,
        }
    }
}

/// A line segment in the sketch: two point references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineSeg {
    /// Line id.
    pub id: String,
    /// Start point id.
    pub start: String,
    /// End point id.
    pub end: String,
}

/// A point in the sketch: an id and a 2D location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Point id.
    pub id: String,
    /// X location (sketch plane).
    pub x: f64,
    /// Y location (sketch plane).
    pub y: f64,
}

/// A declarative 2D sketch.
///
/// Stores the geometry as id-keyed maps (deterministic serialization) plus an
/// ordered constraint list. Unknown entity references are only resolved at
/// solve time (tolerant-failure doctrine) so a partially-defined sketch still
/// loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    /// Sketch id.
    pub id: String,
    /// Points by id.
    pub points: BTreeMap<String, Point>,
    /// Line segments by id.
    pub lines: BTreeMap<String, LineSeg>,
    /// Ordered constraints.
    pub constraints: Vec<Constraint>,
}

impl Sketch {
    /// A new empty sketch with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            points: BTreeMap::new(),
            lines: BTreeMap::new(),
            constraints: Vec::new(),
        }
    }

    /// Add a point at (x, y); returns its id.
    pub fn add_point(&mut self, id: &str, x: f64, y: f64) -> String {
        let id = id.to_string();
        self.points.insert(
            id.clone(),
            Point {
                id: id.clone(),
                x,
                y,
            },
        );
        id
    }

    /// Add a line segment between two point ids; returns its id.
    pub fn add_line(&mut self, id: &str, start: &str, end: &str) -> String {
        let id = id.to_string();
        self.lines.insert(
            id.clone(),
            LineSeg {
                id: id.clone(),
                start: start.to_string(),
                end: end.to_string(),
            },
        );
        id
    }

    /// Pin a point at a fixed location.
    pub fn constrain_fixed(&mut self, point: &str, x: f64, y: f64) {
        self.constraints.push(Constraint::Fixed {
            point: point.to_string(),
            x,
            y,
        });
    }

    /// Force a line horizontal.
    pub fn constrain_horizontal(&mut self, line: &str) {
        self.constraints.push(Constraint::Horizontal {
            line: line.to_string(),
        });
    }

    /// Force a line vertical.
    pub fn constrain_vertical(&mut self, line: &str) {
        self.constraints.push(Constraint::Vertical {
            line: line.to_string(),
        });
    }

    /// Coincide two points.
    pub fn constrain_coincident(&mut self, point_a: &str, point_b: &str) {
        self.constraints.push(Constraint::Coincident {
            point_a: point_a.to_string(),
            point_b: point_b.to_string(),
        });
    }

    /// Give a line a driven length (literal or variable-set-referenced).
    pub fn constrain_length(&mut self, line: &str, expr: DimExpr) {
        self.constraints.push(Constraint::Length {
            line: line.to_string(),
            expr,
        });
    }

    /// Give the distance between two points a driven value.
    pub fn constrain_distance(&mut self, point_a: &str, point_b: &str, expr: DimExpr) {
        self.constraints.push(Constraint::Distance {
            point_a: point_a.to_string(),
            point_b: point_b.to_string(),
            expr,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_expr_accessors() {
        assert_eq!(DimExpr::Literal(3.0).literal(), Some(3.0));
        assert_eq!(DimExpr::Literal(3.0).variable(), None);
        assert_eq!(DimExpr::Variable("w".into()).variable(), Some("w"));
        assert_eq!(DimExpr::Variable("w".into()).literal(), None);
    }

    #[test]
    fn sketch_builds_and_serializes_deterministically() {
        let mut s = Sketch::new("s1");
        let p0 = s.add_point("p0", 0.0, 0.0);
        let p1 = s.add_point("p1", 3.0, 1.0);
        let l0 = s.add_line("l0", &p0, &p1);
        s.constrain_fixed(&p0, 0.0, 0.0);
        s.constrain_length(&l0, DimExpr::Variable("width".into()));
        let a = serde_json::to_string(&s).unwrap();
        let b = serde_json::to_string(&s).unwrap();
        assert_eq!(a, b);
        let back: Sketch = serde_json::from_str(&a).unwrap();
        assert_eq!(back, s);
    }
}
