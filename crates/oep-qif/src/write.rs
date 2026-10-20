//! Deterministic QIF XML writer (basic cut).
//!
//! Emits a normalized basic-cut QIF 3.0 document from the [`QifDoc`] model
//! using the exact element order and attribute grammar verified against the
//! official QIF 3.0 XSD (see `docs/qif-basic-cut.md`): root
//! QIFDocument (versionQIF=3.0.0, idMax, xmlns qif3), QPId, Header,
//! StandardsDefinitions, FileUnits/PrimaryUnits, DatumDefinitions,
//! DatumReferenceFrames, Product (PartSet + RootPart), Characteristics.
//! Std-only, never panics, output is fully determined by the model.

use crate::model::{
    CharDef, CharNominal, DatumFrame, Header, QifDoc, Standard, Tolerance, ToleranceValue, Unit,
    ZoneShape,
};
use crate::Result;

/// The QIF 3.0 target namespace.
pub const QIF_NS: &str = "http://qifstandards.org/xsd/qif3";

/// Serialize a document to canonical QIF 3.0 XML.
pub fn write(doc: &QifDoc) -> Result<String> {
    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    el_open(
        &mut out,
        0,
        "QIFDocument",
        &[
            ("xmlns", QIF_NS.to_string()),
            ("versionQIF", doc.version_qif.clone()),
            ("idMax", doc.id_max.to_string()),
        ],
    );
    txt(&mut out, 1, "QPId", &doc.qp_id);

    if let Some(h) = &doc.header {
        write_header(&mut out, h);
    }
    if !doc.standards.is_empty() {
        write_standards(&mut out, &doc.standards);
    }
    if let Some(u) = &doc.primary_linear_unit {
        write_file_units(&mut out, u);
    }
    if !doc.datums.is_empty() {
        write_datums(&mut out, &doc.datums);
    }
    if !doc.frames.is_empty() {
        write_frames(&mut out, &doc.frames);
    }
    if !doc.parts.is_empty() {
        write_product(&mut out, doc);
    }
    if !doc.char_defs.is_empty() || doc.formal_standard_id.is_some() {
        write_characteristics(&mut out, doc);
    }

    el_close(&mut out, 0, "QIFDocument");
    Ok(out)
}

fn write_header(out: &mut String, h: &Header) {
    el_open(out, 1, "Header", &[]);
    if let Some(app) = &h.application {
        el_wrap_text(out, 2, "Application", &[], "Name", app);
    }
    if h.author.is_some() || h.author_org.is_some() {
        el_open(out, 2, "Author", &[]);
        if let Some(a) = &h.author {
            txt(out, 3, "Name", a);
        }
        if let Some(o) = &h.author_org {
            txt(out, 3, "Organization", o);
        }
        el_close(out, 2, "Author");
    }
    el_close(out, 1, "Header");
}

fn write_standards(out: &mut String, standards: &[Standard]) {
    el_open_n(out, 1, "StandardsDefinitions", standards.len(), &[]);
    for s in standards {
        el_open(out, 2, "Standard", &[("id", s.id.to_string())]);
        el_open(out, 3, "Organization", &[]);
        txt(out, 4, "StandardsOrganizationEnum", &s.org);
        el_close(out, 3, "Organization");
        txt(out, 3, "Designator", &s.designator);
        el_close(out, 2, "Standard");
    }
    el_close(out, 1, "StandardsDefinitions");
}

fn write_file_units(out: &mut String, u: &Unit) {
    el_open(out, 1, "FileUnits", &[]);
    el_open(out, 2, "PrimaryUnits", &[]);
    let (tag, close) = if u.angular {
        ("PMIAngularUnit", "PMIAngularUnit")
    } else {
        ("LinearUnit", "LinearUnit")
    };
    el_open(out, 3, tag, &[]);
    txt(out, 4, "SIUnitName", &u.si_name);
    txt(out, 4, "UnitName", &u.name);
    if let Some(f) = u.factor {
        el_open(out, 4, "UnitConversion", &[]);
        txt(out, 5, "Factor", &fmt_f64(f));
        el_close(out, 4, "UnitConversion");
    }
    el_close(out, 3, close);
    el_close(out, 2, "PrimaryUnits");
    el_close(out, 1, "FileUnits");
}

fn write_datums(out: &mut String, datums: &[crate::model::DatumDef]) {
    el_open_n(out, 1, "DatumDefinitions", datums.len(), &[]);
    for d in datums {
        el_open(out, 2, "DatumDefinition", &[("id", d.id.to_string())]);
        txt(out, 3, "DatumLabel", &d.label);
        el_close(out, 2, "DatumDefinition");
    }
    el_close(out, 1, "DatumDefinitions");
}

fn write_frames(out: &mut String, frames: &[DatumFrame]) {
    el_open_n(out, 1, "DatumReferenceFrames", frames.len(), &[]);
    for f in frames {
        el_open(out, 2, "DatumReferenceFrame", &[("id", f.id.to_string())]);
        el_open_n(out, 3, "Datums", f.datums.len(), &[]);
        for d in &f.datums {
            el_open(out, 4, "Datum", &[]);
            el_open(out, 5, "SimpleDatum", &[]);
            txt(out, 6, "DatumDefinitionId", &d.definition_id);
            txt(out, 6, "MaterialModifier", &d.material_modifier);
            txt(out, 6, "ReferencedComponent", &d.referenced_component);
            el_close(out, 5, "SimpleDatum");
            el_open(out, 5, "Precedence", &[]);
            txt(out, 6, "PrecedenceEnum", &d.precedence);
            el_close(out, 5, "Precedence");
            el_close(out, 4, "Datum");
        }
        el_close(out, 3, "Datums");
        el_close(out, 2, "DatumReferenceFrame");
    }
    el_close(out, 1, "DatumReferenceFrames");
}

fn write_product(out: &mut String, doc: &QifDoc) {
    el_open(out, 1, "Product", &[]);
    if let Some(h) = &doc.header {
        // Product Header mirrors the document Header (Application).
        if let Some(app) = &h.application {
            el_wrap_text(out, 2, "Application", &[], "Name", app);
        }
    }
    el_open_n(out, 2, "PartSet", doc.parts.len(), &[]);
    for p in &doc.parts {
        el_open(out, 3, "Part", &[("id", p.id.to_string())]);
        if let Some(mn) = &p.model_number {
            txt(out, 4, "ModelNumber", mn);
        }
        el_close(out, 3, "Part");
    }
    el_close(out, 2, "PartSet");
    if let Some(rp) = doc.root_part {
        el_open(out, 2, "RootPart", &[]);
        txt(out, 3, "Id", &rp.to_string());
        el_close(out, 2, "RootPart");
    }
    el_close(out, 1, "Product");
}

fn write_characteristics(out: &mut String, doc: &QifDoc) {
    el_open(out, 1, "Characteristics", &[]);
    if let Some(fsid) = &doc.formal_standard_id {
        txt(out, 2, "FormalStandardId", fsid);
    }
    if !doc.char_defs.is_empty() {
        el_open_n(
            out,
            2,
            "CharacteristicDefinitions",
            doc.char_defs.len(),
            &[],
        );
        for d in &doc.char_defs {
            write_char_def(out, d);
        }
        el_close(out, 2, "CharacteristicDefinitions");
    }
    if !doc.char_nominals.is_empty() {
        el_open_n(
            out,
            2,
            "CharacteristicNominals",
            doc.char_nominals.len(),
            &[],
        );
        for n in &doc.char_nominals {
            write_char_nominal(out, n);
        }
        el_close(out, 2, "CharacteristicNominals");
    }
    el_close(out, 1, "Characteristics");
}

fn write_char_def(out: &mut String, d: &CharDef) {
    let el = d.kind.def_element();
    el_open(out, 3, el, &[("id", d.id.to_string())]);
    if let Some(st) = &d.statistical {
        txt(out, 4, "StatisticalCharacteristic", st);
    }
    if let Some(tv) = &d.tolerance_value {
        write_tolerance_value(out, tv);
    }
    if let Some(t) = &d.tolerance {
        write_tolerance(out, t);
    }
    if let Some(df) = &d.datum_frame_id {
        txt(out, 4, "DatumReferenceFrameId", df);
    }
    if let Some(mc) = &d.material_condition {
        txt(out, 4, "MaterialCondition", mc);
    }
    if let Some(zs) = d.zone_shape {
        write_zone_shape(out, zs);
    }
    if let Some(ext) = &d.extent {
        el_wrap_text(out, 4, "Extent", &[], "ExtentEnum", ext);
    }
    el_close(out, 3, el);
}

fn write_tolerance_value(out: &mut String, tv: &ToleranceValue) {
    let attrs = match &tv.unit {
        Some(u) => vec![("linearUnit", u.clone())],
        None => Vec::new(),
    };
    el_open(out, 4, "ToleranceValue", &attrs);
    out.push_str("        ");
    out.push_str(&esc_text(&tv.value));
    out.push('\n');
    el_close(out, 4, "ToleranceValue");
}

fn write_tolerance(out: &mut String, t: &Tolerance) {
    el_open(out, 4, "Tolerance", &[]);
    let max_attrs = opt_attrs(&t.max_decimals, &t.max_unit);
    let min_attrs = opt_attrs(&t.min_decimals, &t.min_unit);
    el_open(out, 5, "MaxValue", &max_attrs);
    out.push_str("          ");
    out.push_str(&esc_text(&t.max));
    out.push('\n');
    el_close(out, 5, "MaxValue");
    el_open(out, 5, "MinValue", &min_attrs);
    out.push_str("          ");
    out.push_str(&esc_text(&t.min));
    out.push('\n');
    el_close(out, 5, "MinValue");
    txt(out, 5, "DefinedAsLimit", &t.defined_as_limit);
    el_close(out, 4, "Tolerance");
}

fn write_zone_shape(out: &mut String, zs: ZoneShape) {
    el_open(out, 4, "ZoneShape", &[]);
    match zs {
        ZoneShape::NonDiametrical => el_self_close(out, 5, "NonDiametricalZone"),
        ZoneShape::Planar => el_self_close(out, 5, "PlanarZone"),
    }
    el_close(out, 4, "ZoneShape");
}

fn write_char_nominal(out: &mut String, n: &CharNominal) {
    let el = n.kind.nominal_element();
    el_open(out, 3, el, &[("id", n.id.to_string())]);
    txt(out, 4, "CharacteristicDefinitionId", &n.definition_id);
    if let Some(name) = &n.name {
        txt(out, 4, "Name", name);
    }
    if let Some(ds) = &n.designator {
        el_open(out, 4, "CharacteristicDesignator", &[]);
        txt(out, 5, "Designator", ds);
        el_close(out, 4, "CharacteristicDesignator");
    }
    if let Some(tv) = &n.target_value {
        txt(out, 4, "TargetValue", tv);
    }
    el_close(out, 3, el);
}

// ---------------------------------------------------------------------------
// low-level emit helpers
// ---------------------------------------------------------------------------

fn opt_attrs(decimals: &Option<String>, unit: &Option<String>) -> Vec<(&'static str, String)> {
    let mut v = Vec::new();
    if let Some(d) = decimals {
        v.push(("decimalPlaces", d.clone()));
    }
    if let Some(u) = unit {
        v.push(("linearUnit", u.clone()));
    }
    v
}

fn ind(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// `<name attrs>\n`
fn el_open(out: &mut String, depth: usize, name: &str, attrs: &[(&str, String)]) {
    ind(out, depth);
    out.push('<');
    out.push_str(name);
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&esc_attr(v));
        out.push('"');
    }
    out.push_str(">\n");
}

/// `<name n=N attrs>\n`
fn el_open_n(out: &mut String, depth: usize, name: &str, n: usize, _attrs: &[(&str, String)]) {
    ind(out, depth);
    out.push('<');
    out.push_str(name);
    out.push_str(" n=\"");
    out.push_str(&n.to_string());
    out.push_str("\">\n");
}

/// `</name>\n`
fn el_close(out: &mut String, depth: usize, name: &str) {
    ind(out, depth);
    out.push_str("</");
    out.push_str(name);
    out.push_str(">\n");
}

/// `<name/>\n`
fn el_self_close(out: &mut String, depth: usize, name: &str) {
    ind(out, depth);
    out.push('<');
    out.push_str(name);
    out.push_str("/>\n");
}

/// `<name>text</name>\n`
fn txt(out: &mut String, depth: usize, name: &str, text: &str) {
    ind(out, depth);
    out.push('<');
    out.push_str(name);
    out.push('>');
    out.push_str(&esc_text(text));
    out.push_str("</");
    out.push_str(name);
    out.push_str(">\n");
}

/// `<outer attrs>\n<inner>text</inner>\n</outer>\n`
fn el_wrap_text(
    out: &mut String,
    depth: usize,
    outer: &str,
    _attrs: &[(&str, String)],
    inner: &str,
    text: &str,
) {
    el_open(out, depth, outer, &[]);
    txt(out, depth + 1, inner, text);
    el_close(out, depth, outer);
}

fn esc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

fn esc_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn fmt_f64(v: f64) -> String {
    format!("{v}")
}
