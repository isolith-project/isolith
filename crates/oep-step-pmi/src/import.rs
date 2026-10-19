//! The private STEP PMI walk, the ONLY module that touches the OSS STEP
//! library (`step-io`). Public surface (lib.rs) never names step-io types.
//!
//! Strategy, verified against the three real NIST MBE AP242 fixtures before
//! this code was written (see `.hermes/plans/2026-10-16_101500-p4m2t01-pmi-import.md`):
//! - `step_io::read` parses any mainstream AP (universal schema);
//! - `Scene::tolerances()`/`datums()`/`dimensions()` surface characteristic,
//!   magnitude, and datum refs IN PRECEDENCE ORDER;
//! - a private raw-model walk (left in this module only) reads
//!   `GEOMETRIC_TOLERANCE_WITH_MODIFIERS` facets on the tolerance's ComplexUnit
//!   parts (MMC/LMC) and the `TOLERANCE_ZONE` form ("cylindrical or circular",
//!   "spherical") that designates that same ComplexUnit, giving the
//!   `diameter_zone` flag.
//!
//! Everything the walk cannot map is recorded in the report as a dropped item,
//! never invented.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use oep_pmi::{
    Characteristic, Datum, DatumFrame, DatumFrameRef, GeometricTolerance, MaterialCondition,
    PmiDocument,
};
use step_io::generated::model as m;
use step_io::read;

use crate::error::Error;

/// A neutral record of one PMI item the walker had to drop, with a reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedItem {
    /// What was dropped (a STEP entity family / name).
    pub what: String,
    /// Why (a closed-set miss, a missing magnitude, an unsupported link, ...).
    pub reason: String,
}

/// A neutral report of what an import found, mapped, and honestly skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmiImportReport {
    /// True when the file carries at least one PMI signal (tolerance, datum,
    /// or dimension). False means "parsed fine, no PMI"; never fabricated.
    pub pmi_present: bool,
    /// Number of datum features mapped.
    pub datum_count: usize,
    /// Number of datum reference frames mapped.
    pub frame_count: usize,
    /// Number of geometric tolerances mapped.
    pub tolerance_count: usize,
    /// Dimensional characteristics present but not mapped by the oep-pmi v1
    /// schema (its Note class is free text, NOT a dimension class; converting
    /// dimensions into notes would synthesize text, so they are counted and
    /// reported, never invented).
    pub dimension_count: usize,
    /// Document unit string surfaced by the reader ("unknown" when absent).
    pub units: String,
    /// Every PMI item the walker saw but could not map, with reasons.
    pub dropped: Vec<DroppedItem>,
    /// Raw navigation warnings from the wrapped library (verbatim, honest).
    pub warnings: Vec<String>,
}

impl PmiImportReport {
    fn empty(units: String) -> Self {
        Self {
            pmi_present: false,
            datum_count: 0,
            frame_count: 0,
            tolerance_count: 0,
            dimension_count: 0,
            units,
            dropped: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// The result of a semantic PMI import: the canonical oep-pmi document plus an
/// honest report of what was mapped / skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct PmiImport {
    /// The canonical internal PMI document (`oep-pmi` format, version 1).
    pub document: PmiDocument,
    /// What was mapped, and what was honestly skipped.
    pub report: PmiImportReport,
}

impl PmiImport {
    /// Convenience: did this file carry no PMI at all?
    pub fn no_pmi(&self) -> bool {
        !self.report.pmi_present
    }
}

/// Import semantic PMI from STEP (ISO 10303-21) bytes into the oep-pmi model.
///
/// A valid STEP file with no PMI returns a valid empty document with
/// `report.pmi_present == false` (honest absence), NOT an error. Junk /
/// non-STEP / unreadable input returns a typed [`Error`] (never a panic).
pub fn import(bytes: &[u8]) -> Result<PmiImport, Error> {
    walk(bytes, false)
}

/// Like [`import`], but a PMI-less (yet valid) STEP file is a typed
/// [`Error::NoPmi`] instead of an empty-document result.
pub fn import_strict(bytes: &[u8]) -> Result<PmiImport, Error> {
    walk(bytes, true)
}

/// Import from a path (read + import). IO errors are typed [`Error::Io`].
pub fn import_path(path: &std::path::Path) -> Result<PmiImport, Error> {
    let bytes = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    import(&bytes)
}

fn walk(bytes: &[u8], strict: bool) -> Result<PmiImport, Error> {
    // STEP part-21 files start with the ISO-10303-21 banner. Check before
    // handing to the OSS parse stack so junk fails fast and honestly.
    let head: String = bytes.iter().take(64).map(|&b| b as char).collect();
    if !head.trim_start().starts_with("ISO-10303-21") {
        return Err(Error::NotStep(
            "missing ISO-10303-21 header line".to_string(),
        ));
    }

    let (model, _report) = read(bytes).map_err(|e| Error::Parse(format!("{e:?}")))?;
    let scene = model.scene();

    let units_name = scene
        .units()
        .length
        .map(|u| u.name)
        .unwrap_or_else(|| "unknown".to_string());
    let warnings = scene.warnings();

    let mut imp = Mapper {
        doc: PmiDocument::new(&units_name),
        report: PmiImportReport::empty(units_name),
        datum_by_label: HashMap::new(),
        frame_key_to_id: HashMap::new(),
    };
    imp.report.warnings = warnings;

    imp.map_datums(&scene);
    imp.map_tolerances(&scene, &model);
    imp.count_dimensions(&scene);

    // Honest presence: any real PMI signal counts, absence is never fabricated.
    imp.report.pmi_present = imp.report.tolerance_count > 0
        || imp.report.datum_count > 0
        || imp.report.dimension_count > 0;

    if strict && !imp.report.pmi_present {
        return Err(Error::NoPmi(
            "STEP file parsed but contains no geometric tolerances, datums, or dimensions".into(),
        ));
    }

    Ok(PmiImport {
        document: imp.doc,
        report: imp.report,
    })
}

/// Internal mapper state. Lives entirely inside the private walk.
struct Mapper {
    doc: PmiDocument,
    report: PmiImportReport,
    /// datum label ("A") -> datum id ("datum_A"); keeps labels unique (schema rule).
    datum_by_label: HashMap<String, String>,
    /// ordered datum-letter tuple key -> frame id; dedups identical frames.
    frame_key_to_id: HashMap<String, String>,
}

impl Mapper {
    fn map_datums(&mut self, scene: &step_io::scene::Scene<'_>) {
        let mut seen_labels: HashSet<String> = HashSet::new();
        for d in scene.datums() {
            let label = d.letter().to_string();
            if label.is_empty() {
                self.report.dropped.push(DroppedItem {
                    what: "datum".into(),
                    reason: "empty identification label".into(),
                });
                continue;
            }
            if !seen_labels.insert(label.clone()) {
                self.report.dropped.push(DroppedItem {
                    what: format!("datum {label}"),
                    reason: "duplicate datum label (schema: labels unique per document)".into(),
                });
                continue;
            }
            let id = format!("datum_{label}");
            self.datum_by_label.insert(label.clone(), id.clone());
            self.doc.push_datum(Datum::new(id, label));
        }
        self.report.datum_count = self.doc.datums.len();
    }

    fn map_tolerances(&mut self, scene: &step_io::scene::Scene<'_>, model: &m::StepModel) {
        for t in scene.tolerances() {
            self.map_one_tolerance(scene, model, &t);
        }
        self.report.tolerance_count = self.doc.tolerances.len();
        self.report.frame_count = self.doc.frames.len();
    }

    fn map_one_tolerance(
        &mut self,
        _scene: &step_io::scene::Scene<'_>,
        model: &m::StepModel,
        t: &step_io::scene::pmi::Tolerance<'_>,
    ) {
        let kind = match char_of(t.kind()) {
            Some(c) => c,
            None => {
                self.report.dropped.push(DroppedItem {
                    what: format!("tolerance {:?}", t.name()),
                    reason: "characteristic not in the closed 14-value Y14.5 set".into(),
                });
                return;
            }
        };
        let Some(value) = t.magnitude() else {
            self.report.dropped.push(DroppedItem {
                what: format!("tolerance {:?} ({:?})", t.name(), t.kind()),
                reason: "no magnitude resolved by the reader".into(),
            });
            return;
        };
        if !value.is_finite() || value < 0.0 {
            self.report.dropped.push(DroppedItem {
                what: format!("tolerance {:?}", t.name()),
                reason: format!("non-finite or negative magnitude {value}"),
            });
            return;
        }

        let id = format!("gt_{}", self.doc.tolerances.len() + 1);

        // Datum frame from the tolerance's ordered datum refs (primary->...).
        let ord: Vec<String> = t.datums().iter().map(|d| d.letter().to_string()).collect();
        let mut frame_id: Option<String> = None;
        if !ord.is_empty() {
            frame_id = self.frame_for(&ord);
        }

        // Material condition from the raw ComplexUnit modifier facets.
        let mc = material_condition_of(t.key(), model);

        // Diameter zone from the TOLERANCE_ZONE that designates this tolerance.
        let diameter = diameter_zone_of(t.key(), model);

        let name = t.name().to_string();
        let feature_ids: Vec<String> = t
            .feature()
            .map(|f| vec![format!("step-feature-{}", f.key().to_key_string())])
            .unwrap_or_default();

        let mut gt = GeometricTolerance::new(id, kind, value)
            .with_diameter_zone(diameter)
            .with_note(if name.is_empty() {
                format!("{:?}", t.kind())
            } else {
                name.clone()
            });
        if let Some(mc) = mc {
            gt = gt.with_material_condition(mc);
        }
        if let Some(fid) = &frame_id {
            gt = gt.with_frame(fid.clone());
        }
        for f in feature_ids {
            gt = gt.with_feature(f);
        }
        self.doc.push_tolerance(gt);
    }

    /// Find-or-create a datum frame for an ordered datum letter tuple.
    fn frame_for(&mut self, letters: &[String]) -> Option<String> {
        if letters.is_empty() {
            return None;
        }
        let key = letters.join("-");
        if let Some(fid) = self.frame_key_to_id.get(&key) {
            return Some(fid.clone());
        }
        // Only frames whose datum labels all resolved may be created.
        let mut refs = Vec::new();
        let mut ok = true;
        for (i, label) in letters.iter().enumerate() {
            let Some(did) = self.datum_by_label.get(label) else {
                self.report.dropped.push(DroppedItem {
                    what: format!("datum frame {key}"),
                    reason: format!("frame references unresolved datum {label}"),
                });
                ok = false;
                break;
            };
            refs.push(DatumFrameRef::new(did.clone(), (i + 1) as u8));
        }
        if !ok {
            return None;
        }
        let fid = format!("drf_{}", letters.join("").to_lowercase());
        self.frame_key_to_id.insert(key.clone(), fid.clone());
        self.doc.push_frame(DatumFrame::new(fid.clone(), key));
        Some(fid)
    }

    fn count_dimensions(&mut self, scene: &step_io::scene::Scene<'_>) {
        self.report.dimension_count = scene.dimensions().count();
    }
}

/// Map step-io's tolerance characteristic onto the closed oep-pmi set.
/// Returns None for step-io characteristics that are not in the committed
/// 14-value ASME Y14.5 / ISO 1101 set (they are recorded as drops, never
/// coerced into a wrong slot).
fn char_of(k: step_io::scene::pmi::ToleranceKind) -> Option<Characteristic> {
    use step_io::scene::pmi::ToleranceKind as K;
    match k {
        K::Position => Some(Characteristic::Position),
        K::Flatness => Some(Characteristic::Flatness),
        K::Straightness => Some(Characteristic::Straightness),
        K::Roundness => Some(Characteristic::Circularity),
        K::Cylindricity => Some(Characteristic::Cylindricity),
        K::LineProfile => Some(Characteristic::ProfileOfLine),
        K::SurfaceProfile => Some(Characteristic::ProfileOfSurface),
        K::Angularity => Some(Characteristic::Angularity),
        K::Parallelism => Some(Characteristic::Parallelism),
        K::Perpendicularity => Some(Characteristic::Perpendicularity),
        K::Concentricity => Some(Characteristic::Concentricity),
        K::Symmetry => Some(Characteristic::Symmetry),
        K::CircularRunout => Some(Characteristic::CircularRunout),
        K::TotalRunout => Some(Characteristic::TotalRunout),
        // Coaxiality is ISO-only and not in the committed ASME set.
        K::Coaxiality | K::Other => None,
    }
}

/// Material-condition modifier from the tolerance's ComplexUnit facets.
fn material_condition_of(key: m::EntityKey, model: &m::StepModel) -> Option<MaterialCondition> {
    let cuid = match key {
        m::EntityKey::ComplexUnit(id) => id,
        _ => return None,
    };
    let cu = model.complex_unit_arena.items.get(cuid.0)?;
    for part in &cu.parts {
        if let m::UnitPart::GeometricToleranceWithModifiers { modifiers, .. } = part {
            for mo in modifiers {
                match mo {
                    m::GeometricToleranceModifier::MaximumMaterialRequirement => {
                        return Some(MaterialCondition::Mmc);
                    }
                    m::GeometricToleranceModifier::LeastMaterialRequirement => {
                        return Some(MaterialCondition::Lmc);
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// True when a TOLERANCE_ZONE whose form is a diameter zone (cylindrical or
/// circular / spherical) designates this tolerance's ComplexUnit.
fn diameter_zone_of(key: m::EntityKey, model: &m::StepModel) -> bool {
    let cuid = match key {
        m::EntityKey::ComplexUnit(id) => id,
        _ => return false,
    };
    for z in &model.tolerance_zone_arena.items {
        let targets_this = z
            .defining_tolerance
            .iter()
            .any(|t| matches!(t, m::ToleranceZoneTargetRef::Complex(c) if *c == cuid));
        if !targets_this {
            continue;
        }
        match z.form {
            m::ToleranceZoneFormRef::ToleranceZoneForm(fid) => {
                if let Some(f) = model.tolerance_zone_form_arena.items.get(fid.0) {
                    let name = f.name.to_lowercase();
                    if name.contains("cylindrical")
                        || name.contains("circular")
                        || name.contains("spherical")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// EntityKey has no Display; ds mirror the key through Debug into a short id.
trait KeyString {
    fn to_key_string(&self) -> String;
}
impl KeyString for m::EntityKey {
    fn to_key_string(&self) -> String {
        let mut s = String::new();
        let _ = write!(s, "{self:?}");
        s
    }
}
