//! [`Drawing`]: the drafting workbench model — a sixth model class.
//!
//! EDITOR-2D-DRAFTING pulls drafting into the editor spine (decision #19).
//! A drawing is a first-class, id-linked record in the canonical [`Document`],
//! exactly like the original five classes, so a drawing's dimension
//! annotations can reference the SAME parameter set that drives the 3D model
//! (the `oep-sketch` `VariableSet` features, FreeCAD-v1-variable-set
//! behavior) and survive PDM round-trips in the same file.
//!
//! This is a **Class 2 model extension** (scope-register cost class 2,
//! "model-extend"): it bumps the `oep-geo` schema version `1 -> 2` with a
//! tolerant migration (a v1 payload loads with `drawings` defaulted empty and
//! is promoted to v2 on parse — see [`crate::document::Document::migrate`]).
//! The full spec lives in `docs/geometry-drawing-schema.md`.
//!
//! What lives here is the *record* model: sheets, standard views, title
//! block, and dimension annotations bound to a variable-set entry BY NAME.
//! Computing projected edges and resolving dimension values from the model is
//! the separate `oep-drafting` workbench (that crate replays the
//! `oep-geo` feature graph into a solid mesh and projects it).
//!
//! Model classes here:
//!
//! - [`Drawing`] — a document's drawing set (one or more [`Sheet`]s).
//! - [`Sheet`] — a paper sheet: size, projection convention, title block,
//!   an ordered set of standard [`View`]s and [`Dimension`] annotations.
//! - [`View`] — *which* standard view (front/top/right) of *which* part.
//! - [`Dimension`] — a horizontal/vertical annotation bound to a variable-set
//!   entry by name ([`Dimension::variable`]); its drawn value is resolved at
//!   render time, so an edit to the variable updates the drawing dimension.
//! - [`TitleBlock`], [`ProjectionType`], [`SheetSize`], [`DimensionKind`],
//!   [`ViewKind`] — the closed vocabularies and frame fields.

use serde::{Deserialize, Serialize};

/// How the standard views are arranged on the sheet (a per-document setting).
///
/// - **Third-angle** (ASME Y14.3 / ISO first-standard usage in the Americas):
///   the top view sits ABOVE the front view and the right view sits to the
///   RIGHT of the front view.
/// - **First-angle** (ISO/EU convention): the projected views are folded away
///   from the viewer, so the top view sits BELOW the front view and the right
///   view sits to the LEFT of the front view.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionType {
    /// First-angle (ISO/EU) arrangement.
    FirstAngle,
    /// Third-angle (ASME) arrangement. The default for a new sheet.
    #[default]
    ThirdAngle,
}

/// Paper sheet size in millimetres (landscape orientation).
///
/// A plain value object: `name()` renders the conventional label. Sizes are
/// unsigned whole millimetres (paper is discrete); the default is `A4_LANDSCAPE`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetSize {
    /// Sheet width in millimetres (> 0).
    pub width_mm: u32,
    /// Sheet height in millimetres (> 0).
    pub height_mm: u32,
}

impl SheetSize {
    /// ISO A4 landscape: 297 × 210 mm. The default sheet size.
    pub const A4_LANDSCAPE: Self = Self {
        width_mm: 297,
        height_mm: 210,
    };
    /// ISO A3 landscape: 420 × 297 mm.
    pub const A3_LANDSCAPE: Self = Self {
        width_mm: 420,
        height_mm: 297,
    };

    /// The conventional label for this size.
    pub fn name(&self) -> &'static str {
        if *self == Self::A4_LANDSCAPE {
            "A4 landscape"
        } else if *self == Self::A3_LANDSCAPE {
            "A3 landscape"
        } else {
            "custom"
        }
    }

    /// True when the size is a valid, drawable sheet (positive in both axes).
    pub fn is_valid(&self) -> bool {
        self.width_mm > 0 && self.height_mm > 0
    }
}

impl Default for SheetSize {
    fn default() -> Self {
        Self::A4_LANDSCAPE
    }
}

/// Title block fields for a sheet (drawn in the sheet's border frame).
///
/// Optional fields are omitted from serialization when absent (house serde
/// convention, mirrors [`crate::body::Body::bbox`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TitleBlock {
    /// Drawing title (e.g. "Bracket").
    pub title: String,
    /// Part number, when the drawing has one (BOM/PDM interop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    /// Material/stock callout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Nominal scale callout (e.g. "1:1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
    /// Who drew it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn_by: Option<String>,
    /// Drawing date (free text or ISO date).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Revision identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Sheet number callout (e.g. "1 of 1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// Company/organization line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
}

impl TitleBlock {
    /// A title block whose only required field is the drawing title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            part_number: None,
            material: None,
            scale: None,
            drawn_by: None,
            date: None,
            revision: None,
            sheet: None,
            company: None,
        }
    }

    /// Builder: set the part number.
    pub fn with_part_number(mut self, value: impl Into<String>) -> Self {
        self.part_number = Some(value.into());
        self
    }

    /// Builder: set the material callout.
    pub fn with_material(mut self, value: impl Into<String>) -> Self {
        self.material = Some(value.into());
        self
    }

    /// Builder: set the scale callout.
    pub fn with_scale(mut self, value: impl Into<String>) -> Self {
        self.scale = Some(value.into());
        self
    }

    /// Builder: set who drew it.
    pub fn with_drawn_by(mut self, value: impl Into<String>) -> Self {
        self.drawn_by = Some(value.into());
        self
    }

    /// Builder: set the date.
    pub fn with_date(mut self, value: impl Into<String>) -> Self {
        self.date = Some(value.into());
        self
    }

    /// Builder: set the revision.
    pub fn with_revision(mut self, value: impl Into<String>) -> Self {
        self.revision = Some(value.into());
        self
    }

    /// Builder: set the sheet callout.
    pub fn with_sheet(mut self, value: impl Into<String>) -> Self {
        self.sheet = Some(value.into());
        self
    }

    /// Builder: set the company line.
    pub fn with_company(mut self, value: impl Into<String>) -> Self {
        self.company = Some(value.into());
        self
    }
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self::new("untitled")
    }
}

/// The standard views this drafting milestone computes from the 3D model.
///
/// Each variant maps to a fixed orthographic projection direction and an
/// in-plane coordinate frame (documented in `docs/geometry-drawing-schema.md`
/// and implemented by the `oep-drafting` view projector).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// Front view — looks along `-Y`, shows the model's `X`×`Z` extent.
    Front,
    /// Top view — looks along `-Z`, shows the model's `X`×`Y` extent.
    Top,
    /// Right view — looks along `-X`, shows the model's `Y`×`Z` extent.
    Right,
}

/// One standard view of the 3D model on a sheet.
///
/// The view is a *projection instruction*: WHICH view of WHICH part. The
/// projected edge geometry itself is always computed from the model at render
/// time by the `oep-drafting` workbench (never stored, never fabricated).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct View {
    /// Unique id (document-wide id space).
    pub id: String,
    /// Human-readable view label (e.g. "Front").
    pub name: String,
    /// Which standard projection this view uses.
    pub kind: ViewKind,
    /// Id of the part whose geometry this view shows (document id space).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    /// Drawing scale applied to this view (model units → paper units).
    #[serde(default = "default_scale")]
    pub scale: f64,
}

const fn default_scale() -> f64 {
    1.0
}

impl View {
    /// A standard view of the given kind at 1:1.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: ViewKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            part_id: None,
            scale: 1.0,
        }
    }

    /// Builder: bind the view to a part id.
    pub fn with_part(mut self, part_id: impl Into<String>) -> Self {
        self.part_id = Some(part_id.into());
        self
    }

    /// Builder: set the drawing scale.
    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
}

/// The axis a horizontal or vertical dimension measures in view space.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    /// Measures the view's projected extent along its horizontal axis.
    Horizontal,
    /// Measures the view's projected extent along its vertical axis.
    Vertical,
}

/// A linear dimension annotation on a view.
///
/// The value is NOT stored: it is resolved at render time. When
/// [`Dimension::variable`] is `Some(name)`, the drawn value is the resolved
/// entry of the model's `oep-sketch` `VariableSet` feature — the SAME
/// parameter that drives the 3D model (FreeCAD-v1-variable-set behavior) — and
/// the renderer cross-checks it against the measured projected extent of the
/// view. When `None`, the drawn value is the measured projected extent itself.
/// Editing the variable re-drives the model, re-solves, and the redrawn
/// dimension follows (the card acceptance).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Dimension {
    /// Unique id (document-wide id space).
    pub id: String,
    /// Human-readable dimension label (e.g. "bracket width").
    pub name: String,
    /// Id of the view this dimension annotates (within the same sheet).
    pub view_id: String,
    /// Horizontal or vertical.
    pub kind: DimensionKind,
    /// Variable-set entry name that drives this dimension (FreeCAD-v1), or
    /// `None` for a measured-only dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
}

impl Dimension {
    /// A dimension annotation on `view_id` with no variable binding yet.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        view_id: impl Into<String>,
        kind: DimensionKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            view_id: view_id.into(),
            kind,
            variable: None,
        }
    }

    /// Builder: bind the dimension to a variable-set entry by name.
    pub fn with_variable(mut self, name: impl Into<String>) -> Self {
        self.variable = Some(name.into());
        self
    }
}

/// A paper sheet: size, projection convention, title block, views, dims.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Sheet {
    /// Unique id (document-wide id space).
    pub id: String,
    /// Human-readable sheet name (e.g. "Sheet 1").
    pub name: String,
    /// Paper size (ISO A4 landscape default).
    #[serde(default = "default_sheet_size")]
    pub size: SheetSize,
    /// First- or third-angle arrangement for this sheet's views.
    #[serde(default)]
    pub projection: ProjectionType,
    /// Title block contents drawn in the sheet frame.
    #[serde(default)]
    pub title_block: TitleBlock,
    /// The standard views on this sheet, in placement order.
    #[serde(default)]
    pub views: Vec<View>,
    /// Dimension annotations on this sheet's views.
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
}

const fn default_sheet_size() -> SheetSize {
    SheetSize::A4_LANDSCAPE
}

impl Sheet {
    /// A sheet at the default A4 landscape size, third-angle projection.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            size: default_sheet_size(),
            projection: ProjectionType::default(),
            title_block: TitleBlock::default(),
            views: Vec::new(),
            dimensions: Vec::new(),
        }
    }

    /// Builder: set the paper size.
    pub fn with_size(mut self, size: SheetSize) -> Self {
        self.size = size;
        self
    }

    /// Builder: set the first/third-angle convention.
    pub fn with_projection(mut self, projection: ProjectionType) -> Self {
        self.projection = projection;
        self
    }

    /// Builder: set the title block.
    pub fn with_title_block(mut self, title_block: TitleBlock) -> Self {
        self.title_block = title_block;
        self
    }

    /// Builder: add a view.
    pub fn with_view(mut self, view: View) -> Self {
        self.views.push(view);
        self
    }

    /// Builder: add a dimension annotation.
    pub fn with_dimension(mut self, dimension: Dimension) -> Self {
        self.dimensions.push(dimension);
        self
    }
}

/// A drawing: one or more sheets, stored in the canonical document.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Drawing {
    /// Unique id (document-wide id space).
    pub id: String,
    /// Human-readable drawing name.
    pub name: String,
    /// The sheets that make up this drawing.
    #[serde(default)]
    pub sheets: Vec<Sheet>,
}

impl Drawing {
    /// A drawing with no sheets yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            sheets: Vec::new(),
        }
    }

    /// Builder: add a sheet.
    pub fn with_sheet(mut self, sheet: Sheet) -> Self {
        self.sheets.push(sheet);
        self
    }
}
