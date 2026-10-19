//! P6 -- Geometry core and model representation (workstream P6).
//!
//! P6-M1-T01 delivers the `oep-geo` model schema — the five OEP geometry
//! classes **part, body, feature, assembly, instance** — plus serde
//! serialization in our own documented format (JSON, `format = "oep-geo"`,
//! `version = 1`; spec in `docs/p6-m1-oep-geo-model-schema.md`).
//!
//! The schema is a flat store: a [`Document`] owns five id-keyed collections
//! and every link between classes is an id reference resolved by
//! [`Document::validate`] (never during deserialization, so partially-linked
//! files still load — tolerant-failure doctrine). Geometry kernel slots /
//! PMI / units model / feature replay land in later cards (P6-M1-T02/T03,
//! P6-M2, P6-M3).
//!
//! Modules:
//! - [`document`]: the [`Document`] container + our-own-format (de)serializer.
//! - [`part`]: [`Part`], the engineering-artifact class.
//! - [`body`]: [`Body`], [`BodyKind`], [`BoundingBox`].
//! - [`feature`]: [`Feature`] with a deterministic parameter map.
//! - [`assembly`]: [`Assembly`], [`Instance`], [`InstanceSource`].
//! - [`transform`]: the 4x4 row-major [`Transform`].
//! - [`validate`]: structural invariant rules as [`ValidationError`]s.
//! - [`math`]: vendored std-only f64 math core (P6-M2-T01).
//! - [`nurbs`]: NURBS/B-spline curve and surface math (P6-M2-T02).
//! - [`predicates`]: geometric predicates, tolerance and containment (P6-M2-T03).
//! - [`units`]/[`tolerance`]: units and accuracy discipline (P6-M1-T03).
//! - [`history`]/[`topology_state`]: serializable undo/redo topology stack (P6-M3-T02).
//! - [`feature_graph`]: feature-graph (history) model with replay (P6-M3-T03).
//! - [`export`]: STL/glTF/OBJ/SVG/PDF/STEP exporters on the model (P6-M4-T01).
#![deny(missing_docs)]

pub mod assembly;
pub mod body;
pub mod document;
pub mod export;
pub mod feature;
pub mod feature_graph;
pub mod history;
pub mod math;
pub mod nurbs;
pub mod part;
pub mod predicates;
pub mod tolerance;
pub mod topology_state;
pub mod transform;
pub mod units;
pub mod validate;

/// [`Assembly`]: tree frame over [`Instance`] placements.
pub use assembly::{Assembly, Instance, InstanceSource};
/// [`Body`]/[`BodyKind`]/[`BoundingBox`]: independent material lumps.
pub use body::{Body, BodyKind, BoundingBox};
/// [`Document`]: the unit of our own serializable model format.
pub use document::Document;
/// [`Feature`]: a modelling operation in a part's history.
pub use feature::Feature;
/// P6-M3-T03 feature-graph model with replay.
pub use feature_graph::{
    BBox, Evaluator, FeatureGraph, FeatureNode, GraphError, MiniBoxEvaluator, ParamValue,
    ReplayError, ReplayResult, ReplayStep,
};
/// [`History`]/[`HistoryState`]: serializable topology undo/redo stack (P6-M3-T02).
pub use history::{History, HistoryState};
/// [`Part`]: engineering part owning bodies and a feature history.
pub use part::Part;
/// [`Transform`]: 4x4 row-major instance placement matrix.
pub use transform::Transform;
/// [`ValidationError`]: a single documented structural violation.
pub use validate::ValidationError;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P6-GEO";
