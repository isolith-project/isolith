//! P4-M2-T02: AP242 PMI writer — internal `oep-pmi` model -> ISO 10303-21
//! (STEP) AP242 file with **semantic PMI** (geometric tolerances, datums,
//! datum reference frames, notes).
//!
//! ## What we emit
//!
//! The writer maps a [`oep_pmi::PmiDocument`] onto the AP242 MIM
//! (`AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF`) entity families that
//! the NIST MBE PMI test models and the CAx-IF recommended practices use for
//! *semantic* (non-presentation) PMI:
//!
//! - one minimal product/shape cluster (product, product_definition,
//!   product_definition_shape), shared by all PMI;
//! - one `GLOBAL_UNIT_ASSIGNED_CONTEXT` carrying the document length unit;
//! - each [`oep_pmi::Datum`]    -> `DATUM` (id in `name`, letter in
//!   `identification`);
//! - each [`oep_pmi::DatumFrame`] -> `DATUM_SYSTEM` with one
//!   `DATUM_REFERENCE_COMPARTMENT` per precedence slot (1=primary, in list
//!   order), each with an optional `SIMPLE_DATUM_REFERENCE_MODIFIER`;
//! - each [`oep_pmi::GeometricTolerance`] -> the *complex entity instance*
//!   built from `GEOMETRIC_TOLERANCE` + `GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE`
//!   (when a frame is attached) + `GEOMETRIC_TOLERANCE_WITH_MODIFIERS` (when a
//!   material condition is attached), plus the concrete subtype leaf
//!   (`POSITION_TOLERANCE()`, `FLATNESS_TOLERANCE()`, ...). The tolerance
//!   value is a qualified `LENGTH_MEASURE_WITH_UNIT`, the toleranced feature a
//!   `SHAPE_ASPECT`;
//! - the diameter-zone flag  -> `TOLERANCE_ZONE` + `TOLERANCE_ZONE_FORM('cylindrical or circular')`
//!   whose defining target is the tolerance (AP242 `tolerance_zone_target`
//!   select includes `geometric_tolerance`);
//! - the second (composite) value -> a second tolerance linked by
//!   `GEOMETRIC_TOLERANCE_RELATIONSHIP('composite',...)` (the AP242 composite
//!   FCF pattern);
//! - each [`oep_pmi::Note`] -> `DESCRIPTIVE_REPRESENTATION_ITEM` inside a
//!   notes `REPRESENTATION` attached to the shape via
//!   `SHAPE_DEFINITION_REPRESENTATION`.
//!
//! ## Honest contract
//!
//! - Semantic PMI only: no presentation/anchor geometry, no tessellation.
//! - The model's cross-document feature/body *id strings* are carried where
//!   AP242 has a natural slot (toleranced `SHAPE_ASPECT` name, `DATUM` name,
//!   descriptor name) so the round-trip reader can restore them.
//! - Note 3-D anchor `position` is presentation information, not semantic
//!   PMI: it is dropped on export (honest N/A in the fidelity matrix).
//! - Output is deterministic for a given document (document order), so the
//!   committed golden can be byte-locked.
//!
//! ## Errors
//!
//! Typed [`WriteError`] only; never panics. A document that fails
//! [`oep_pmi::PmiDocument::validate`] is refused before anything is emitted.

use std::fmt::Write as _;
use std::io::Write as IoWrite;

use oep_pmi::{Characteristic, GeometricTolerance, MaterialCondition, PmiDocument};

/// The AP242 edition-1 MIM long-form schema identifier we declare in
/// `FILE_SCHEMA` (matches the NIST AP242 e1 semantic PMI files).
pub const AP242_SCHEMA: &str =
    "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 1 1 4 }";

/// Subtype entity name (the concrete `-TOLERANCE()` leaf) for a characteristic.
///
/// This is the AP242 MIM name set used by the NIST AP242 semantic PMI files
/// (ISO 10303-1050 `geometric_tolerance` hierarchy).
pub fn characteristic_subtype(c: Characteristic) -> &'static str {
    match c {
        Characteristic::Straightness => "STRAIGHTNESS_TOLERANCE",
        Characteristic::Flatness => "FLATNESS_TOLERANCE",
        Characteristic::Circularity => "ROUNDNESS_TOLERANCE",
        Characteristic::Cylindricity => "CYLINDRICITY_TOLERANCE",
        Characteristic::ProfileOfLine => "LINE_PROFILE_TOLERANCE",
        Characteristic::ProfileOfSurface => "SURFACE_PROFILE_TOLERANCE",
        Characteristic::Angularity => "ANGULARITY_TOLERANCE",
        Characteristic::Perpendicularity => "PERPENDICULARITY_TOLERANCE",
        Characteristic::Parallelism => "PARALLELISM_TOLERANCE",
        Characteristic::Position => "POSITION_TOLERANCE",
        Characteristic::Concentricity => "CONCENTRICITY_TOLERANCE",
        Characteristic::Symmetry => "SYMMETRY_TOLERANCE",
        Characteristic::CircularRunout => "CIRCULAR_RUNOUT_TOLERANCE",
        Characteristic::TotalRunout => "TOTAL_RUNOUT_TOLERANCE",
    }
}

/// The `SIMPLE_DATUM_REFERENCE_MODIFIER` / `GEOMETRIC_TOLERANCE_WITH_MODIFIERS`
/// enum token for a material condition.
pub fn material_condition_token(mc: MaterialCondition) -> &'static str {
    match mc {
        MaterialCondition::Mmc => ".MAXIMUM_MATERIAL_REQUIREMENT.",
        MaterialCondition::Lmc => ".LEAST_MATERIAL_REQUIREMENT.",
        MaterialCondition::Rfs => ".REGARDLESS_OF_FEATURE_SIZE.",
    }
}

/// The AP242 length-unit complex instance for a document unit string.
pub fn length_unit_instance(doc_units: &str) -> &'static str {
    match doc_units {
        "mm" | "millimeter" | "millimetre" => {
            "( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )"
        }
        "in" | "inch" => "( LENGTH_UNIT() NAMED_UNIT(*) NON_SI_UNIT(.INCH.) )",
        "m" | "metre" | "meter" => "( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.) )",
        // Fallback: assume millimetre (documented; callers should pass a known
        // unit string).
        _ => "( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )",
    }
}

/// Errors from [`write_ap242_pmi`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The document failed structural validation — refuse to emit.
    InvalidDocument(Vec<String>),
    /// I/O target failed.
    Io(String),
}

impl From<std::io::Error> for WriteError {
    fn from(e: std::io::Error) -> Self {
        WriteError::Io(e.to_string())
    }
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::InvalidDocument(v) => write!(f, "document invalid: {:?}", v),
            WriteError::Io(s) => write!(f, "io: {s}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Format a finite `f64` as a PART 21 REAL token (forcing a decimal point so
/// `5` -> `5.`), matching how NIST files and the oep-geo STEP exporter spell
/// reals.
fn fmt_real(v: f64) -> String {
    if !v.is_finite() {
        // Should not happen (validate rejects non-finite values); never panic.
        return "0.".to_string();
    }
    if v == 0.0 {
        return "0.".to_string();
    }
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.")
    }
}

/// Escape a label for a PART 21 string (single-quote -> doubled; non-ASCII
/// chars -> `\X2\<hex>\X0\` PART 21 escape per ISO 10303-21 8.8).
fn esc_label(s: &str) -> String {
    esc_part21(s)
}

/// Escape free text for a PART 21 string (same rules, plus newlines -> space).
fn esc_text(s: &str) -> String {
    esc_part21(&s.replace('\n', " "))
}

/// Shared PART 21 string escaping: ASCII is passed through (`'` doubled, `*`
/// left as-is), non-ASCII code points are encoded as `\X2\<hex4>\X0\`.
fn esc_part21(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii() {
            if c == '\'' {
                out.push_str("''");
            } else {
                out.push(c);
            }
        } else {
            let cp = c as u32;
            let _ = write!(out, "\\X2\\{cp:04X}\\X0\\");
        }
    }
    out
}

/// Deterministic entity-id allocator + output buffer.
#[derive(Debug, Clone)]
struct Emit {
    next_id: u64,
    out: String,
}

impl Emit {
    fn new() -> Self {
        Emit {
            next_id: 1,
            out: String::new(),
        }
    }
    fn id(&mut self) -> u64 {
        let i = self.next_id;
        self.next_id += 1;
        i
    }
    fn line(&mut self, body: &str) -> u64 {
        let id = self.id();
        let _ = writeln!(self.out, "#{id}={body};");
        id
    }
    /// Emit a complex entity instance `#id=(\nparts\n)...;`.
    fn complex(&mut self, parts: &[String]) -> u64 {
        let id = self.id();
        let joined = parts.join("\n");
        let _ = writeln!(self.out, "#{id}=({joined});");
        id
    }
}

/// Options for [`write_ap242_pmi`].
#[derive(Debug, Clone)]
pub struct Ap242Opts<'a> {
    /// `PRODUCT` name (also used for the file's product + FILE_NAME).
    pub product_name: &'a str,
    /// File name recorded in `FILE_NAME`.
    pub file_name: &'a str,
    /// `oep-step` version string recorded in `FILE_NAME`.
    pub version: &'a str,
}

impl<'a> Default for Ap242Opts<'a> {
    fn default() -> Self {
        Ap242Opts {
            product_name: "oep-pmi model",
            file_name: "oep-pmi-ap242.stp",
            version: "0.1.0",
        }
    }
}

/// Serialize a whole `oep-pmi` document to an AP242 semantic-PMI PART 21 file.
pub fn write_ap242_pmi<W: IoWrite>(
    doc: &PmiDocument,
    opts: &Ap242Opts<'_>,
    w: &mut W,
) -> Result<(), WriteError> {
    // Validate before touching output (honest gate: refuse to emit a PMI
    // document that fails our own invariants).
    let issues = doc.validate();
    if !issues.is_empty() {
        let msgs: Vec<String> = issues
            .iter()
            .map(|e| format!("[{}] {}", e.code, e.message))
            .collect();
        return Err(WriteError::InvalidDocument(msgs));
    }

    let mut em = Emit::new();

    // ---- Header -----------------------------------------------------------
    em.out.push_str("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(
        em.out,
        "FILE_DESCRIPTION(('oep-pmi {} AP242 semantic PMI export', 'P4-M2-T02'),'2;1');",
        opts.product_name
    );
    let _ = writeln!(
        em.out,
        "FILE_NAME('{}','',('Keelworks OEP'),(''),'oep-step {}','Keelworks OEP','');",
        opts.file_name, opts.version
    );
    let _ = writeln!(em.out, "FILE_SCHEMA (('{}'));", AP242_SCHEMA);
    em.out.push_str("ENDSEC;\n\nDATA;\n");

    // ---- Product / shape cluster ------------------------------------------
    let axctx = em.line("APPLICATION_CONTEXT('mechanical design')");
    let apd = em.line(&format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','managed_model_based_3d_engineering',2018,#{axctx})"
    ));
    let prod_ctx = em.line(&format!("PRODUCT_CONTEXT('',#{apd},'mechanical')"));
    let product = em.line(&format!(
        "PRODUCT('{}','{}','oep-pmi PMI model',(#{prod_ctx}))",
        opts.product_name, opts.product_name
    ));
    let pdf = em.line(&format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
    let pdc = em.line(&format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{apd},'design')"
    ));
    let pd = em.line(&format!("PRODUCT_DEFINITION('design','',#{pdf},#{pdc})"));
    let pds = em.line(&format!(
        "PRODUCT_DEFINITION_SHAPE('','oep-pmi semantic PMI',#{pd})"
    ));

    // ---- Units ---------------------------------------------------------------
    let len_unit = em.line(length_unit_instance(&doc.units));
    let ang_unit = em.line("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )");
    let solid_unit = em.line("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )");
    let geom_ctx = em.line(&format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{len_unit},#{ang_unit},#{solid_unit})) REPRESENTATION_CONTEXT('','') )"
    ));

    // ---- Datums ----------------------------------------------------------------
    let mut datum_ids: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for d in &doc.datums {
        let feat = d.feature_ids.first().map(|s| s.as_str()).unwrap_or("");
        let name = if feat.is_empty() {
            d.id.clone()
        } else {
            format!("{}|{}", d.id, feat)
        };
        let id = em.line(&format!(
            "DATUM('{}',$,#{pds},.F.,'{}')",
            esc_label(&name),
            esc_label(&d.label)
        ));
        datum_ids.insert(d.id.as_str(), id);
    }

    // ---- Datum frames -----------------------------------------------------------
    let mut frame_ids: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for f in &doc.frames {
        let mut refs = f.refs.clone();
        refs.sort_by_key(|r| r.precedence);
        let mut comp_ids = Vec::with_capacity(refs.len());
        for r in &refs {
            let datum_id = datum_ids.get(r.datum_id.as_str()).copied().ok_or_else(|| {
                WriteError::InvalidDocument(vec![format!(
                    "frame {} references unknown datum {}",
                    f.id, r.datum_id
                )])
            })?;
            let modifier = match r.modifier {
                Some(mc) => format!(
                    ",(SIMPLE_DATUM_REFERENCE_MODIFIER({}))",
                    material_condition_token(mc)
                ),
                None => ",$".to_string(),
            };
            let c = em.line(&format!(
                "DATUM_REFERENCE_COMPARTMENT('',$,#{pds},.F.,#{datum_id}{modifier})"
            ));
            comp_ids.push(c);
        }
        let comps = comp_ids
            .iter()
            .map(|c| format!("#{c}"))
            .collect::<Vec<_>>()
            .join(",");
        let dsys = em.line(&format!(
            "DATUM_SYSTEM('{}',$,#{pds},.F.,({comps}))",
            esc_label(&f.id)
        ));
        frame_ids.insert(f.id.as_str(), dsys);
    }

    // ---- Geometric tolerances -----------------------------------------------------
    for t in &doc.tolerances {
        write_tolerance(&mut em, pds, len_unit, t, &frame_ids)?;
    }

    // ---- Notes (descriptors in a representation) ------------------------
    let note_ids: Vec<u64> = doc
        .notes
        .iter()
        .map(|n| {
            let feat = n.feature_ids.first().map(|s| s.as_str()).unwrap_or("");
            let name = if feat.is_empty() {
                n.id.clone()
            } else {
                format!("{}|{}", n.id, feat)
            };
            em.line(&format!(
                "DESCRIPTIVE_REPRESENTATION_ITEM('{}','{}')",
                esc_label(&name),
                esc_text(&n.text)
            ))
        })
        .collect();
    if !note_ids.is_empty() {
        let items = note_ids
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let note_rep = em.line(&format!(
            "REPRESENTATION('oep-pmi notes',({items}),#{geom_ctx})"
        ));
        em.line(&format!(
            "SHAPE_DEFINITION_REPRESENTATION(#{pds},#{note_rep})"
        ));
    }

    em.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    w.write_all(em.out.as_bytes())?;
    Ok(())
}

/// Write a single geometric tolerance as a complex entity instance.
fn write_tolerance(
    em: &mut Emit,
    pds: u64,
    len_unit: u64,
    t: &GeometricTolerance,
    frame_ids: &std::collections::HashMap<&str, u64>,
) -> Result<(), WriteError> {
    // Primary tolerance value -> qualified length measure.
    let val = em.line(&format!(
        "( LENGTH_MEASURE_WITH_UNIT() MEASURE_REPRESENTATION_ITEM() MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{len_unit}) REPRESENTATION_ITEM('') )",
        fmt_real(t.value)
    ));

    // Toleranced feature -> SHAPE_ASPECT (feature id carried in name).
    let feat = t.feature_ids.first().map(|s| s.as_str()).unwrap_or("");
    let sa_name = if feat.is_empty() { t.id.as_str() } else { feat };
    let sa = em.line(&format!(
        "SHAPE_ASPECT('{}','',#{pds},.F.)",
        esc_label(sa_name)
    ));

    // Complex instance: GEOMETRIC_TOLERANCE base + optional refs/modifiers +
    // concrete subtype leaf.
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "GEOMETRIC_TOLERANCE('{}','',#{val},#{sa})",
        esc_label(&t.id)
    ));
    if let Some(fid) = &t.frame_id {
        let frame = frame_ids.get(fid.as_str()).copied().ok_or_else(|| {
            WriteError::InvalidDocument(vec![format!(
                "tolerance {} references unknown frame {}",
                t.id, fid
            )])
        })?;
        parts.push(format!(
            "GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE((#{frame}))"
        ));
    }
    if let Some(mc) = t.material_condition {
        parts.push(format!(
            "GEOMETRIC_TOLERANCE_WITH_MODIFIERS(({}))",
            material_condition_token(mc)
        ));
    }
    let subtype = characteristic_subtype(t.characteristic);
    parts.push(format!("{subtype}()"));
    let gt = em.complex(&parts);

    // Composite second value -> second tolerance + composite relationship.
    if let Some(second) = t.second_value {
        let val2 = em.line(&format!(
            "( LENGTH_MEASURE_WITH_UNIT() MEASURE_REPRESENTATION_ITEM() MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{len_unit}) REPRESENTATION_ITEM('') )",
            fmt_real(second)
        ));
        let sa2 = em.line(&format!(
            "SHAPE_ASPECT('{}','',#{pds},.F.)",
            esc_label(&format!("{}~2", sa_name))
        ));
        let mut parts2: Vec<String> = Vec::new();
        parts2.push(format!(
            "GEOMETRIC_TOLERANCE('{}~2','',#{val2},#{sa2})",
            esc_label(&t.id)
        ));
        if let Some(fid) = &t.frame_id {
            let frame = frame_ids.get(fid.as_str()).copied().ok_or_else(|| {
                WriteError::InvalidDocument(vec![format!(
                    "tolerance {} references unknown frame {}",
                    t.id, fid
                )])
            })?;
            parts2.push(format!(
                "GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE((#{frame}))"
            ));
        }
        let subtype2 = characteristic_subtype(t.characteristic);
        parts2.push(format!("{subtype2}()"));
        let gt2 = em.complex(&parts2);
        em.line(&format!(
            "GEOMETRIC_TOLERANCE_RELATIONSHIP('composite','',#{gt},#{gt2})"
        ));
    }

    // Diameter zone -> TOLERANCE_ZONE whose target is the tolerance.
    if t.diameter_zone {
        let form = em.line("TOLERANCE_ZONE_FORM('cylindrical or circular')");
        em.line(&format!("TOLERANCE_ZONE('',$,#{pds},.F.,(#{gt}),#{form})"));
    }

    Ok(())
}
