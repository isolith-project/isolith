//! Internal model for the QIF basic cut (MBD quality data).
//!
//! Mirrors the IN set from `docs/qif-basic-cut.md`: document
//! envelope, header, standards, primary linear unit, datums, datum reference
//! frames, part identity, and characteristics (definitions + nominals).
//!
//! Determinism: all collections are ordered `Vec`s in document order; the
//! model never uses a HashMap, so `write(parse(x))` is byte-stable and
//! `PartialEq` on the model is the round-trip comparison contract.

/// A QIF document (basic cut).
#[derive(Debug, Clone, PartialEq)]
pub struct QifDoc {
    /// versionQIF root attribute (e.g. "3.0.0"). Kept verbatim as text.
    pub version_qif: String,
    /// idMax root attribute — the value carried in the source; `self_check`
    /// verifies it covers every modeled id.
    pub id_max: u32,
    /// QPId root child (required by the XSD).
    pub qp_id: String,
    /// Optional document Header (Application/Author).
    pub header: Option<Header>,
    /// StandardsDefinitions (e.g. ASME Y14.5 2009).
    pub standards: Vec<Standard>,
    /// Primary linear unit from FileUnits/PrimaryUnits (e.g. mm).
    pub primary_linear_unit: Option<Unit>,
    /// DatumDefinitions (labels A/B/C...).
    pub datums: Vec<DatumDef>,
    /// DatumReferenceFrames (SimpleDatum + precedence).
    pub frames: Vec<DatumFrame>,
    /// Product part identity (PartSet / Part).
    pub parts: Vec<Part>,
    /// Product RootPart id (optional in files; we carry what we read).
    pub root_part: Option<u32>,
    /// Characteristics / FormalStandardId (a reference id, kept verbatim).
    pub formal_standard_id: Option<String>,
    /// CharacteristicDefinitions (the 6-kind basic set).
    pub char_defs: Vec<CharDef>,
    /// CharacteristicNominals.
    pub char_nominals: Vec<CharNominal>,
}

impl QifDoc {
    /// New empty document with the fixed QIF 3.0 version.
    pub fn new(qp_id: impl Into<String>) -> Self {
        QifDoc {
            version_qif: "3.0.0".to_string(),
            id_max: 0,
            qp_id: qp_id.into(),
            header: None,
            standards: Vec::new(),
            primary_linear_unit: None,
            datums: Vec::new(),
            frames: Vec::new(),
            parts: Vec::new(),
            root_part: None,
            formal_standard_id: None,
            char_defs: Vec::new(),
            char_nominals: Vec::new(),
        }
    }
}

/// Document Header (Application + Author identity).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Header {
    /// Application/Name text.
    pub application: Option<String>,
    /// Author/Name text.
    pub author: Option<String>,
    /// Author/Organization text.
    pub author_org: Option<String>,
}

/// A standards definition (StandardsDefinitions/Standard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standard {
    /// The Standard `id` reference.
    pub id: u32,
    /// StandardsOrganizationEnum text (e.g. "ASME").
    pub org: String,
    /// Designator text (e.g. "Y14.5 2009").
    pub designator: String,
}

/// One unit (SI name / unit name / conversion factor).
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// SIUnitName text (e.g. "meter").
    pub si_name: String,
    /// UnitName text (e.g. "mm").
    pub name: String,
    /// UnitConversion/Factor (optional; absent when factor is 1).
    pub factor: Option<f64>,
    /// LinearUnit angular flag (angular units are kept but not validated here).
    pub angular: bool,
}

/// A datum definition (DatumDefinitions/DatumDefinition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatumDef {
    /// DatumDefinition id.
    pub id: u32,
    /// DatumLabel text (e.g. "A").
    pub label: String,
}

/// A datum reference frame (DatumReferenceFrames/DatumReferenceFrame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatumFrame {
    /// DatumReferenceFrame id.
    pub id: u32,
    /// Datums in precedence order.
    pub datums: Vec<FrameDatum>,
}

/// One datum-with-precedence inside a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDatum {
    /// SimpleDatum/DatumDefinitionId (verbatim id text).
    pub definition_id: String,
    /// SimpleDatum/MaterialModifier (e.g. "NONE").
    pub material_modifier: String,
    /// SimpleDatum/ReferencedComponent (e.g. "NOMINAL").
    pub referenced_component: String,
    /// Precedence/PrecedenceEnum (e.g. "PRIMARY").
    pub precedence: String,
}

/// Product part identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// Part id.
    pub id: u32,
    /// ModelNumber text (e.g. "PART-001"), if present.
    pub model_number: Option<String>,
}

/// The closed 6-kind basic-cut characteristic set (ASME Y14.5 / ISO 1101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CharKind {
    /// Position (of a feature) — tolerant/positional tolerance.
    Position,
    /// Surface profile.
    SurfaceProfile,
    /// Perpendicularity.
    Perpendicularity,
    /// Flatness.
    Flatness,
    /// Diameter / nominal size (linear size).
    Diameter,
    /// Angle / angular size.
    Angle,
}

impl CharKind {
    /// QIF element name for this kind's Definition.
    pub fn def_element(self) -> &'static str {
        match self {
            CharKind::Position => "PositionCharacteristicDefinition",
            CharKind::SurfaceProfile => "SurfaceProfileCharacteristicDefinition",
            CharKind::Perpendicularity => "PerpendicularityCharacteristicDefinition",
            CharKind::Flatness => "FlatnessCharacteristicDefinition",
            CharKind::Diameter => "DiameterCharacteristicDefinition",
            CharKind::Angle => "AngleCharacteristicDefinition",
        }
    }

    /// QIF element name for this kind's Nominal.
    pub fn nominal_element(self) -> &'static str {
        match self {
            CharKind::Position => "PositionCharacteristicNominal",
            CharKind::SurfaceProfile => "SurfaceProfileCharacteristicNominal",
            CharKind::Perpendicularity => "PerpendicularityCharacteristicNominal",
            CharKind::Flatness => "FlatnessCharacteristicNominal",
            CharKind::Diameter => "DiameterCharacteristicNominal",
            CharKind::Angle => "AngleCharacteristicNominal",
        }
    }
}

/// Zone shape for positional/planar tolerances (ZoneShape element child).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoneShape {
    /// `<NonDiametricalZone/>`.
    NonDiametrical,
    /// `<PlanarZone/>`.
    Planar,
}

/// Tolerance bounds (Tolerance > MaxValue/MinValue/DefinedAsLimit) as raw
/// text so round-trip is byte-exact (decimalPlaces/unit attrs preserved).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tolerance {
    /// MaxValue text.
    pub max: String,
    /// MaxValue decimalPlaces attr (raw, optional).
    pub max_decimals: Option<String>,
    /// MaxValue linearUnit attr (raw, optional).
    pub max_unit: Option<String>,
    /// MinValue text.
    pub min: String,
    /// MinValue decimalPlaces attr (raw, optional).
    pub min_decimals: Option<String>,
    /// MinValue linearUnit attr (raw, optional).
    pub min_unit: Option<String>,
    /// DefinedAsLimit text ("true"/"false").
    pub defined_as_limit: String,
}

/// One characteristic definition (basic cut).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharDef {
    /// CharacteristicDefinition id.
    pub id: u32,
    /// GD&T kind (drives the element name).
    pub kind: CharKind,
    /// StatisticalCharacteristic text ("true"/"false"), optional.
    pub statistical: Option<String>,
    /// ToleranceValue element: (value_text, linearUnit attr) — present for
    /// position/surface-profile/perpendicularity/flatness.
    pub tolerance_value: Option<ToleranceValue>,
    /// Tolerance element — present for diameter/angle (our basic cut).
    pub tolerance: Option<Tolerance>,
    /// DatumReferenceFrameId reference (raw text), optional.
    pub datum_frame_id: Option<String>,
    /// MaterialCondition text (e.g. "NONE"), optional.
    pub material_condition: Option<String>,
    /// ZoneShape for position/perpendicularity, optional.
    pub zone_shape: Option<ZoneShape>,
    /// Extent/ExtentEnum text for surface profile, optional.
    pub extent: Option<String>,
}

/// A characteristic nominal (basic cut): ties a definition to a designator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharNominal {
    /// Nominal id.
    pub id: u32,
    /// GD&T kind.
    pub kind: CharKind,
    /// CharacteristicDefinitionId reference (raw text).
    pub definition_id: String,
    /// Name text (optional).
    pub name: Option<String>,
    /// CharacteristicDesignator/Designator text.
    pub designator: Option<String>,
    /// TargetValue text for size/angle nominals (optional).
    pub target_value: Option<String>,
}

/// ToleranceValue element: simple text value + optional linearUnit attr.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToleranceValue {
    /// The tolerance value text.
    pub value: String,
    /// linearUnit attribute text (raw, optional).
    pub unit: Option<String>,
}
