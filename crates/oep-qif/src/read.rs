//! Tolerant QIF XML reader (basic cut).
//!
//! Parses QIF 3.0 documents into the [`QifDoc`] IN-set model. Anything outside
//! the basic cut (geometry, topology, visualization, plans, results, notes,
//! vendor attributes, ...) is skipped and recorded in [`ParseReport::skips`]
//! with a reason — never a crash, never an invented value. The reader first
//! loads a small internal DOM (bounded by input size), then extracts the
//! modeled sections from it; the two-phase split keeps the OUT-culling logic
//! explicit and panic-free.

use crate::error::{Error, Result, SkipNote};
use crate::model::{
    CharDef, CharKind, CharNominal, DatumDef, DatumFrame, FrameDatum, Header, Part, QifDoc,
    Standard, Tolerance, ToleranceValue, Unit, ZoneShape,
};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parse QIF document bytes into the basic-cut model plus a tolerant-parse
/// report (skips). Returns a typed error on genuinely malformed XML or a
/// non-QIF root; never panics.
pub fn parse(bytes: &[u8]) -> Result<(QifDoc, ParseReport)> {
    let dom = Dom::parse(bytes)?;
    let (doc, skips) = extract(&dom)?;
    let report = ParseReport {
        root_local: dom.root.name.clone(),
        skipped_sections: skips.len(),
        skips,
    };
    Ok((doc, report))
}

/// A plus/round-trip parse report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseReport {
    /// The document root local name actually read.
    pub root_local: String,
    /// Number of out-of-cut sections skipped.
    pub skipped_sections: usize,
    /// The skip notes (element -> reason), in first-seen order.
    pub skips: Vec<SkipNote>,
}

// ---------------------------------------------------------------------------
// minimal internal DOM
// ---------------------------------------------------------------------------

/// A parsed element: local name, attributes, concatenated text, children.
#[derive(Debug, Clone)]
struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    kids: Vec<Elem>,
}

impl Elem {
    fn attr(&self, key: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn child(&self, name: &str) -> Option<&Elem> {
        self.kids.iter().find(|k| k.name == name)
    }

    fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Elem> + 'a {
        self.kids.iter().filter(move |k| k.name == name)
    }

    fn child_text(&self, name: &str) -> Option<String> {
        self.child(name).map(|c| c.text.trim().to_string())
    }
}

/// The DOM root.
struct Dom {
    root: Elem,
}

/// A DOM element frame while streaming: (name, attrs, text, kids).
type Frame = (String, Vec<(String, String)>, String, Vec<Elem>);

impl Dom {
    fn parse(bytes: &[u8]) -> Result<Dom> {
        let mut r = Reader::from_reader(bytes);
        r.config_mut().trim_text(true);
        let mut buf = Vec::new();

        // stack frames: (name, attrs, text, kids)
        let mut stack: Vec<Frame> = Vec::new();
        let mut text_buf = String::new();
        let mut root: Option<Elem> = None;

        // Append the accumulated text into a frame and reset the buffer.
        fn flush_text(frame: &mut Frame, text_buf: &mut String) {
            let t = text_buf.trim().to_string();
            if !t.is_empty() {
                if !frame.2.is_empty() {
                    frame.2.push(' ');
                }
                frame.2.push_str(&t);
            }
            text_buf.clear();
        }

        loop {
            buf.clear();
            match r.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    // trailing text (if any) belongs to the current parent
                    if let Some(top) = stack.last_mut() {
                        flush_text(top, &mut text_buf);
                    }
                    let attrs = read_attrs(&e)?;
                    let name = lname(e.name().as_ref());
                    stack.push((name, attrs, String::new(), Vec::new()));
                }
                Ok(Event::Empty(e)) => {
                    if let Some(top) = stack.last_mut() {
                        flush_text(top, &mut text_buf);
                    }
                    let attrs = read_attrs(&e)?;
                    let name = lname(e.name().as_ref());
                    let el = Elem {
                        name,
                        attrs,
                        text: String::new(),
                        kids: Vec::new(),
                    };
                    if let Some(top) = stack.last_mut() {
                        top.3.push(el);
                    } else {
                        root = Some(el);
                    }
                }
                Ok(Event::End(_)) => {
                    // text inside the being-closed element lands on it
                    if let Some(top) = stack.last_mut() {
                        flush_text(top, &mut text_buf);
                    }
                    if let Some((name, attrs, text, kids)) = stack.pop() {
                        let el = Elem {
                            name,
                            attrs,
                            text,
                            kids,
                        };
                        if let Some(top) = stack.last_mut() {
                            top.3.push(el);
                        } else {
                            root = Some(el);
                        }
                    }
                }
                Ok(Event::Text(t)) => {
                    let raw =
                        std::str::from_utf8(t.as_ref()).map_err(|e| Error::Xml(e.to_string()))?;
                    let s =
                        quick_xml::escape::unescape(raw).map_err(|e| Error::Xml(e.to_string()))?;
                    text_buf.push_str(&s);
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => return Err(Error::from(e)),
            }
        }
        if !stack.is_empty() {
            // unbalanced input (no EOF End) -> malformed
            return Err(Error::Xml("unbalanced XML: EOF inside element".to_string()));
        }
        let root = root.ok_or_else(|| Error::NotQif("empty document".to_string()))?;
        if root.name != "QIFDocument" {
            return Err(Error::NotQif(format!("root element is <{}>", root.name)));
        }
        Ok(Dom { root })
    }
}

/// Lowercase QFNames: QIF uses no prefixes on its elements; strip any prefix
/// defensively (e.g. a `<qif:QIFDocument>` would be malformed for the qif3
/// namespace but we still read the local name).
fn lname(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

fn read_attrs(start: &quick_xml::events::BytesStart) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for a in start.attributes() {
        let a = a.map_err(|e| Error::Xml(e.to_string()))?;
        let key = lname(a.key.as_ref());
        let val = a.unescape_value().map_err(|e| Error::Xml(e.to_string()))?;
        out.push((key, val.into_owned()));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// DOM -> model extraction (tolerant IN-set)
// ---------------------------------------------------------------------------

/// Direct children of QIFDocument that are fully OUT of the basic cut.
const DOC_OUT: &[&str] = &[
    "Attributes",
    "VersionHistory",
    "Version",
    "ValidationCounts",
    "ProductDataQuality",
    "ExternalQIFReferences",
    "SoftwareDefinitions",
    "AlgorithmDefinitions",
    "PreInspectionTraceability",
    "DatumTargetDefinitions",
    "Transforms",
    "CoordinateSystems",
    "MeasurementResources",
    "ThreadSpecifications",
    "Features",
    "FeatureZones",
    "Plan",
    "Results",
    "Statistics",
    "ManufacturingProcessTraceabilities",
    "Rules",
    "UserDataXML",
    "Signature",
];

/// Product children that are OUT of the basic cut.
const PRODUCT_OUT: &[&str] = &[
    "Attributes",
    "CoordinateSystems",
    "GeometrySet",
    "TopologySet",
    "NoteSet",
    "PartNoteSet",
    "ViewSet",
    "CameraSet",
    "SavedViewSet",
    "AnnotationViewSet",
    "VisualizationSet",
    "Fonts",
    "PMIDisplaySet",
    "FeatureSets",
];

/// Feature definition/nominal families (all kinds) are OUT of the basic cut.
const FEATURE_OUT_PREFIX: &[&str] = &[
    "CylinderFeature",
    "GroupFeature",
    "PlaneFeature",
    "OppositeParallelPlanesFeature",
    "OppositeAngledPlanesFeature",
    "LineFeature",
    "OtherNonShapeFeature",
    "BossFeature",
    "PocketFeature",
    "HoleFeature",
    "SlotFeature",
    "FeatureDefinition",
    "FeatureNominal",
];

fn is_out(name: &str) -> bool {
    DOC_OUT.contains(&name)
        || PRODUCT_OUT.contains(&name)
        || FEATURE_OUT_PREFIX.iter().any(|p| name.starts_with(p))
        || matches!(
            name,
            "NetSet" | "EntityInternalIds" | "ExternalIds" | "Attributes"
        )
}

fn push_skip(skips: &mut Vec<SkipNote>, el: &str, reason: &str) {
    skips.push(SkipNote {
        element: el.to_string(),
        reason: reason.to_string(),
    });
}

fn extract(dom: &Dom) -> Result<(QifDoc, Vec<SkipNote>)> {
    let root = &dom.root;
    let mut skips: Vec<SkipNote> = Vec::new();

    let version_qif = root
        .attr("versionQIF")
        .ok_or_else(|| Error::Missing("versionQIF attribute".to_string()))?;
    if version_qif != "3.0.0" {
        return Err(Error::UnsupportedVersion(version_qif));
    }
    let id_max = root
        .attr("idMax")
        .ok_or_else(|| Error::Missing("idMax attribute".to_string()))?
        .parse::<u32>()
        .map_err(|_| Error::Value("idMax is not an unsigned int".to_string()))?;

    let mut doc = QifDoc::new("");

    // QPId (required text child)
    let qp_id = root
        .child_text("QPId")
        .ok_or_else(|| Error::Missing("QPId".to_string()))?;
    doc.qp_id = qp_id;
    doc.id_max = id_max;
    doc.version_qif = version_qif;

    // namespace sanity: if an xmlns att is present it must be the qif3 ns.
    if let Some(ns) = root.attr("xmlns") {
        if ns != "http://qifstandards.org/xsd/qif3" {
            // not fatal for extraction but worth surfacing; keep tolerant.
            push_skip(&mut skips, "xmlns", &format!("non-qif3 namespace '{ns}'"));
        }
    }

    for kid in &root.kids {
        match kid.name.as_str() {
            "Header" => {
                doc.header = parse_header(kid);
            }
            "StandardsDefinitions" => {
                for s in kid.children("Standard") {
                    doc.standards.push(parse_standard(s, &mut skips));
                }
            }
            "FileUnits" => {
                doc.primary_linear_unit = parse_file_units(kid);
            }
            "DatumDefinitions" => {
                for d in kid.children("DatumDefinition") {
                    doc.datums.push(parse_datum(d, &mut skips));
                }
            }
            "DatumReferenceFrames" => {
                for f in kid.children("DatumReferenceFrame") {
                    doc.frames.push(parse_frame(f, &mut skips));
                }
            }
            "Product" => {
                parse_product(kid, &mut doc, &mut skips);
            }
            "Characteristics" => {
                parse_characteristics(kid, &mut doc, &mut skips);
            }
            "QPId" => {} // handled above
            _ => {
                if is_out(&kid.name) {
                    push_skip(&mut skips, &kid.name, "outside basic cut");
                } else {
                    push_skip(&mut skips, &kid.name, "unknown document section");
                }
            }
        }
    }

    // Sort determinism is NOT applied: collections stay in document order =
    // the writer's emission order -> byte-stable round trips.
    Ok((doc, skips))
}

fn parse_header(h: &Elem) -> Option<Header> {
    let mut header = Header::default();
    if let Some(app) = h.child("Application") {
        header.application = app.child_text("Name");
    }
    if let Some(auth) = h.child("Author") {
        header.author = auth.child_text("Name");
        header.author_org = auth.child_text("Organization");
    }
    if header.application.is_none() && header.author.is_none() && header.author_org.is_none() {
        None
    } else {
        Some(header)
    }
}

fn parse_standard(s: &Elem, skips: &mut Vec<SkipNote>) -> Standard {
    let id = s
        .attr("id")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let org = s
        .child("Organization")
        .and_then(|o| o.child_text("StandardsOrganizationEnum"))
        .unwrap_or_default();
    let designator = s.child_text("Designator").unwrap_or_default();
    if s.child("Attributes").is_some() {
        skips.push(SkipNote {
            element: "Standard/Attributes".to_string(),
            reason: "outside basic cut".to_string(),
        });
    }
    Standard {
        id,
        org,
        designator,
    }
}

fn parse_file_units(fu: &Elem) -> Option<Unit> {
    let primary = fu.child("PrimaryUnits")?;
    if let Some(lu) = primary.child("LinearUnit") {
        return Some(Unit {
            si_name: lu.child_text("SIUnitName").unwrap_or_default(),
            name: lu.child_text("UnitName").unwrap_or_default(),
            factor: lu
                .child("UnitConversion")
                .and_then(|c| c.child_text("Factor"))
                .and_then(|s| s.trim().parse::<f64>().ok()),
            angular: false,
        });
    }
    if let Some(au) = primary
        .child("PMIAngularUnit")
        .or_else(|| primary.child("AngularUnit"))
    {
        return Some(Unit {
            si_name: au.child_text("SIUnitName").unwrap_or_default(),
            name: au.child_text("UnitName").unwrap_or_default(),
            factor: au
                .child("UnitConversion")
                .and_then(|c| c.child_text("Factor"))
                .and_then(|s| s.trim().parse::<f64>().ok()),
            angular: true,
        });
    }
    None
}

fn parse_datum(d: &Elem, skips: &mut Vec<SkipNote>) -> DatumDef {
    let id = d
        .attr("id")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    if let Some(_a) = d.child("Attributes") {
        skips.push(SkipNote {
            element: "DatumDefinition/Attributes".to_string(),
            reason: "outside basic cut".to_string(),
        });
    }
    if let Some(_f) = d.child("FeatureNominalIds") {
        skips.push(SkipNote {
            element: "DatumDefinition/FeatureNominalIds".to_string(),
            reason: "links features (features out of basic cut)".to_string(),
        });
    }
    let label = d.child_text("DatumLabel").unwrap_or_default();
    DatumDef { id, label }
}

fn parse_frame(f: &Elem, skips: &mut Vec<SkipNote>) -> DatumFrame {
    let id = f
        .attr("id")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    if let Some(_a) = f.child("Attributes") {
        skips.push(SkipNote {
            element: "DatumReferenceFrame/Attributes".to_string(),
            reason: "outside basic cut".to_string(),
        });
    }
    let mut datums = Vec::new();
    if let Some(datums_el) = f.child("Datums") {
        for d in datums_el.children("Datum") {
            let definition_id = d
                .child("SimpleDatum")
                .and_then(|s| s.child_text("DatumDefinitionId"))
                .unwrap_or_default();
            let material_modifier = d
                .child("SimpleDatum")
                .and_then(|s| s.child_text("MaterialModifier"))
                .unwrap_or_default();
            let referenced_component = d
                .child("SimpleDatum")
                .and_then(|s| s.child_text("ReferencedComponent"))
                .unwrap_or_default();
            let precedence = d
                .child("Precedence")
                .and_then(|p| p.child_text("PrecedenceEnum"))
                .unwrap_or_default();
            datums.push(FrameDatum {
                definition_id,
                material_modifier,
                referenced_component,
                precedence,
            });
        }
    }
    DatumFrame { id, datums }
}

fn parse_product(p: &Elem, doc: &mut QifDoc, skips: &mut Vec<SkipNote>) {
    if let Some(h) = p.child("Header") {
        doc.header = parse_header(h);
    }
    if let Some(ps) = p.child("PartSet") {
        for part in ps.children("Part") {
            let id = part
                .attr("id")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let model_number = part.child_text("ModelNumber");
            doc.parts.push(Part { id, model_number });
        }
    }
    if let Some(rp) = p.child("RootPart") {
        doc.root_part = rp.child_text("Id").and_then(|s| s.parse::<u32>().ok());
    }
    // FoldersPart / other structural containers: skip.
    if let Some(_fp) = p.child("FoldersPart") {
        skips.push(SkipNote {
            element: "Product/FoldersPart".to_string(),
            reason: "folder structure out of basic cut".to_string(),
        });
    }
    // OUT sections inside Product are recorded separately.
    for kid in &p.kids {
        if is_out(&kid.name) || matches!(kid.name.as_str(), "FoldersPart") {
            skips.push(SkipNote {
                element: format!("Product/{}", kid.name),
                reason: "outside basic cut".to_string(),
            });
        }
    }
}

fn parse_characteristics(c: &Elem, doc: &mut QifDoc, skips: &mut Vec<SkipNote>) {
    doc.formal_standard_id = c.child_text("FormalStandardId");
    if let Some(defs) = c.child("CharacteristicDefinitions") {
        for d in defs.kids.iter() {
            if let Some(def) = parse_char_def(d, skips) {
                doc.char_defs.push(def);
            }
        }
    }
    if let Some(noms) = c.child("CharacteristicNominals") {
        for n in noms.kids.iter() {
            if let Some(nom) = parse_char_nominal(n, skips) {
                doc.char_nominals.push(nom);
            }
        }
    }
}

fn kind_from_def_element(name: &str) -> Option<CharKind> {
    match name {
        "PositionCharacteristicDefinition" => Some(CharKind::Position),
        "SurfaceProfileCharacteristicDefinition" => Some(CharKind::SurfaceProfile),
        "PerpendicularityCharacteristicDefinition" => Some(CharKind::Perpendicularity),
        "FlatnessCharacteristicDefinition" => Some(CharKind::Flatness),
        "DiameterCharacteristicDefinition" => Some(CharKind::Diameter),
        "AngleCharacteristicDefinition" => Some(CharKind::Angle),
        _ => None,
    }
}

fn parse_char_def(d: &Elem, skips: &mut Vec<SkipNote>) -> Option<CharDef> {
    let kind = kind_from_def_element(&d.name)?;
    let id = d
        .attr("id")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let statistical = d.child_text("StatisticalCharacteristic");
    let tolerance_value = d.child("ToleranceValue").map(|tv| ToleranceValue {
        value: tv.text.trim().to_string(),
        unit: tv.attr("linearUnit"),
    });
    let tolerance = d.child("Tolerance").map(parse_tolerance);
    let datum_frame_id = d.child_text("DatumReferenceFrameId");
    let material_condition = d.child_text("MaterialCondition");
    let zone_shape = d.child("ZoneShape").and_then(|z| {
        z.kids.first().and_then(|inner| match inner.name.as_str() {
            "NonDiametricalZone" => Some(ZoneShape::NonDiametrical),
            "PlanarZone" => Some(ZoneShape::Planar),
            _ => None,
        })
    });
    let extent = d.child("Extent").and_then(|e| e.child_text("ExtentEnum"));

    // other known-but-unmodeled children get a skip note once each
    for k in &d.kids {
        match k.name.as_str() {
            "StatisticalCharacteristic"
            | "ToleranceValue"
            | "Tolerance"
            | "DatumReferenceFrameId"
            | "MaterialCondition"
            | "ZoneShape"
            | "Extent" => {}
            other => skips.push(SkipNote {
                element: format!("CharacteristicDefinition/{other}"),
                reason: "outside basic cut".to_string(),
            }),
        }
    }

    Some(CharDef {
        id,
        kind,
        statistical,
        tolerance_value,
        tolerance,
        datum_frame_id,
        material_condition,
        zone_shape,
        extent,
    })
}

fn parse_tolerance(t: &Elem) -> Tolerance {
    let maxv = t.child("MaxValue");
    let minv = t.child("MinValue");
    Tolerance {
        max: maxv.map(|m| m.text.trim().to_string()).unwrap_or_default(),
        max_decimals: maxv.and_then(|m| m.attr("decimalPlaces")),
        max_unit: maxv.and_then(|m| m.attr("linearUnit")),
        min: minv.map(|m| m.text.trim().to_string()).unwrap_or_default(),
        min_decimals: minv.and_then(|m| m.attr("decimalPlaces")),
        min_unit: minv.and_then(|m| m.attr("linearUnit")),
        defined_as_limit: t.child_text("DefinedAsLimit").unwrap_or_default(),
    }
}

fn parse_char_nominal(n: &Elem, skips: &mut Vec<SkipNote>) -> Option<CharNominal> {
    let kind = match n.name.as_str() {
        "PositionCharacteristicNominal" => CharKind::Position,
        "SurfaceProfileCharacteristicNominal" => CharKind::SurfaceProfile,
        "PerpendicularityCharacteristicNominal" => CharKind::Perpendicularity,
        "FlatnessCharacteristicNominal" => CharKind::Flatness,
        "DiameterCharacteristicNominal" => CharKind::Diameter,
        "AngleCharacteristicNominal" => CharKind::Angle,
        _ => return None,
    };
    let id = n
        .attr("id")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let definition_id = n
        .child_text("CharacteristicDefinitionId")
        .unwrap_or_default();
    let name = n.child_text("Name");
    let designator = n
        .child("CharacteristicDesignator")
        .and_then(|cd| cd.child_text("Designator"));
    let target_value = n.child_text("TargetValue");
    for k in &n.kids {
        match k.name.as_str() {
            "CharacteristicDefinitionId" | "Name" | "CharacteristicDesignator" | "TargetValue" => {}
            other => skips.push(SkipNote {
                element: format!("CharacteristicNominal/{other}"),
                reason: "outside basic cut".to_string(),
            }),
        }
    }
    Some(CharNominal {
        id,
        kind,
        definition_id,
        name,
        designator,
        target_value,
    })
}
