//! P6 -- Geometry core and model representation (workstream P6).
//!
//! P6-M1-T01 delivers the `oep-geo` model schema — the six OEP geometry
//! classes **part, body, feature, assembly, instance, drawing** — plus serde
//! serialization in our own documented format (JSON, `format = "oep-geo"`,
//! `version = 2`; spec in `docs/geometry-model-schema.md` and the Class 2
//! drawing extension in `docs/geometry-drawing-schema.md`).
//!
//! The schema is a flat store: a [`Document`] owns id-keyed collections for
//! every class and every link between classes is an id reference resolved by
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
//! - [`drawing`] — the Class 2 drafting model ([`Sheet`]/[`View`]/[`Dimension`]/[`TitleBlock`])
//! - [`transform`]: the 4x4 row-major [`Transform`].
//! - [`validate`]: structural invariant rules as [`ValidationError`]s.
//! - [`math`]: vendored std-only f64 math core (P6-M2-T01).
//! - [`nurbs`]: NURBS/B-spline curve and surface math (P6-M2-T02).
//! - [`predicates`]: geometric predicates, tolerance and containment (P6-M2-T03).
//! - [`units`]/[`tolerance`]: units and accuracy discipline (P6-M1-T03).
//! - [`history`]/[`topology_state`]: serializable undo/redo topology stack (P6-M3-T02).
//! - [`feature_graph`]: feature-graph (history) model with replay (P6-M3-T03).
//! - [`kernel`]: the geometry-kernel seam (P6-M3-T04 / ADR v1.2.0) -- exact
//!   B-Rep booleans, fillet/chamfer, primitives, extrude and revolve, typed
//!   errors, implemented by the oep-engine-occ OCCT binding behind the seam.
//! - [`export`]: STL/glTF/OBJ/SVG/PDF/STEP exporters on the model (P6-M4-T01).
#![deny(missing_docs)]

pub mod assembly;
pub mod body;
pub mod document;
pub mod drawing;
pub mod export;
pub mod feature;
pub mod feature_graph;
pub mod history;
pub mod kernel;
pub mod kernel_eval;
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
/// Class 2 drafting model (schema v2): sheets, views, dimensions, title block.
pub use drawing::{
    Dimension, DimensionKind, Drawing, ProjectionType, Sheet, SheetSize, TitleBlock, View, ViewKind,
};
/// [`Feature`]: a modelling operation in a part's history.
pub use feature::Feature;
/// P6-M3-T03 feature-graph model with replay.
pub use feature_graph::{
    BBox, Evaluator, FeatureGraph, FeatureNode, GraphError, MiniBoxEvaluator, ParamValue,
    ReplayError, ReplayResult, ReplayStep,
};
/// [`History`]/[`HistoryState`]: serializable topology undo/redo stack (P6-M3-T02).
pub use history::{History, HistoryState};
/// Kernel seam: the geometry-kernel seam (P6-M3-T04 / ADR v1.2.0), exposed as
/// oep-geo-typed types and free functions only (no kernel identifier leaks,
/// ADR section 5). Without the `kernel` cargo feature every call is the typed
/// [`kernel::KernelError::EngineUnavailable`] / [`kernel::BooleanError::EngineUnavailable`].
pub use kernel::{
    make_box, make_cylinder, make_sphere, solid_bbox, solid_boolean, solid_chamfer, solid_edges,
    solid_extrude, solid_fillet, solid_mirror, solid_revolve, solid_translate, sys_drop, sys_new,
    BooleanError, BooleanOp, EdgeRef, KernelError, MirrorAxis, Point2D, Solid, SolidMeasures,
    SysHandle, Tolerance,
};
/// KERNEL-REPLAY: the real kernel behind the feature-graph [`Evaluator`] seam
/// (P6-M3-T03 + ADR v1.3.0). A feature graph replays through [`KernelEvaluator`]
/// into real B-Rep solids; [`KernelValue`] is its value type.
pub use kernel_eval::{KernelEvaluator, KernelValue};
/// [`Part`]: engineering part owning bodies and a feature history.
pub use part::Part;
/// [`Transform`]: 4x4 row-major instance placement matrix.
pub use transform::Transform;
/// [`ValidationError`]: a single documented structural violation.
pub use validate::ValidationError;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P6-GEO";
