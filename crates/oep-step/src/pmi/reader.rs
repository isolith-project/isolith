//! P4-M2-T02: AP242 PMI round-trip reader.
//!
//! This is the "over and back" reader for the **subset this crate's writer
//! emits** ([`crate::pmi::writer::write_ap242_pmi`]). It parses the AP242
//! semantic-PMI entities the writer produces (geometric tolerances, datums,
//! datum reference frames, notes) back into an [`oep_pmi::PmiDocument`] so a
//! round-trip test can re-import the exported PMI and compare within
//! tolerance.
//!
//! ## Honest scope
//!
//! - It is a *subset* reader: it understands exactly the entities
//!   [`crate::pmi::writer`] emits. The full general AP242 PMI importer is a
//!   separate P4 card (P4-M2-T01); this reader deliberately does not claim to
//!   cover arbitrary AP242 files.
//! - Unknown entities are skipped by type (never fatal) — this is the
//!   tolerant-failure doctrine applied to our own round-trip lane.
//! - Values compared against the source model happen in the round-trip test at
//!   the tolerance the test applies; this reader performs no rounding.

use std::collections::HashMap;

use oep_pmi::{
    Characteristic, Datum, DatumFrame, DatumFrameRef, GeometricTolerance, MaterialCondition, Note,
    PmiDocument,
};

/// Errors from [`read_ap242_pmi`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The file is not framed as ISO 10303-21 (missing HEADER/DATA/ENDSEC).
    NotPart21(String),
    /// A numeric value inside an entity could not be parsed.
    BadNumber(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotPart21(s) => write!(f, "not a PART 21 file: {s}"),
            ReadError::BadNumber(s) => write!(f, "bad number: {s}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// One PART 21 instance: simple (one group) or complex (multiple groups, each
/// `NAME(params)`).
#[derive(Debug, Clone)]
struct Instance {
    groups: Vec<(String, String)>,
}

/// A minimal PART 21 DATA-section parser for the entity subset we emit.
/// Handles typed params `#n`, enums `.T.`/`.TOKEN.`, strings, reals, `$`,
/// lists `(...)`, and complex instances `#n=( g1 g2 ... );`.
fn parse_data(data: &str) -> Result<HashMap<u64, Instance>, ReadError> {
    let mut insts = HashMap::new();
    let chars: Vec<char> = data.chars().collect();
    let n = chars.len();
    let mut i = 0usize;

    while i < n {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n || chars[i] != '#' {
            // Stop at a section boundary we don't understand.
            break;
        }
        // read id
        let start = i + 1;
        let mut j = start;
        while j < n && chars[j].is_ascii_digit() {
            j += 1;
        }
        let id: u64 = chars[start..j]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| ReadError::NotPart21("bad entity id".to_string()))?;
        // expect '='
        let mut k = j;
        while k < n && chars[k].is_whitespace() {
            k += 1;
        }
        if k >= n || chars[k] != '=' {
            return Err(ReadError::NotPart21("expected '='".to_string()));
        }
        let mut l = k + 1;
        while l < n && chars[l].is_whitespace() {
            l += 1;
        }
        if l >= n {
            break;
        }
        if chars[l] == '(' {
            // complex instance — scan to the matching close paren + ';'
            let mut depth = 0usize;
            let mut j2 = l;
            while j2 < n {
                match chars[j2] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j2 += 1;
            }
            if j2 >= n {
                return Err(ReadError::NotPart21("unbalanced complex".to_string()));
            }
            let body: String = chars[l + 1..j2].iter().collect();
            let mut t = j2 + 1;
            while t < n && chars[t] != ';' {
                t += 1;
            }
            let groups = split_groups(&body)?;
            insts.insert(id, Instance { groups });
            i = t + 1;
        } else {
            // simple instance: scan to ';' at paren depth 0
            let mut depth = 0usize;
            let mut j2 = l;
            while j2 < n {
                match chars[j2] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    ';' if depth == 0 => break,
                    _ => {}
                }
                j2 += 1;
            }
            if j2 >= n {
                return Err(ReadError::NotPart21("unterminated instance".to_string()));
            }
            let body: String = chars[l..j2].iter().collect();
            let (name, params) = split_head(&body)?;
            insts.insert(
                id,
                Instance {
                    groups: vec![(name, params)],
                },
            );
            i = j2 + 1;
        }
    }
    Ok(insts)
}

/// Split `NAME(params)` -> (name, params).
fn split_head(body: &str) -> Result<(String, String), ReadError> {
    let open = body
        .find('(')
        .ok_or_else(|| ReadError::NotPart21("no params".to_string()))?;
    let name = body[..open].trim().to_string();
    let params = body[open + 1..].trim_end_matches(')').trim().to_string();
    Ok((name, params))
}

/// Split a complex body (already stripped of outer parens) into groups.
fn split_groups(body: &str) -> Result<Vec<(String, String)>, ReadError> {
    let mut groups = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    while i < n {
        // find next name char (not whitespace)
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        let gstart = i;
        // scan until ')' at depth 0 — each group is `NAME(...)`
        let mut depth = 0usize;
        while i < n {
            match chars[i] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let piece: String = chars[gstart..i].iter().collect();
        let piece = piece.trim();
        if !piece.is_empty() {
            let (name, params) = split_head(piece)?;
            groups.push((name, params));
        }
    }
    Ok(groups)
}

/// Unquote a PART 21 string token (may be `$`). Decodes `''` -> `'` and the
/// `\X2\<hex4>\X0\` non-ASCII escape from ISO 10303-21 8.8.
fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    if s == "$" {
        return None;
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        let inner = &s[1..s.len() - 1];
        return Some(decode_part21_string(inner));
    }
    None
}

/// Decode PART 21 string escapes: `''` -> `'`, `\X2\HEX\X0\` -> UTF-8 char.
fn decode_part21_string(s: &str) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '\'' && chars[i + 1] == '\'' {
            out.push('\'');
            i += 2;
        } else if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == 'X' {
            // \X2\XXXX\X0\ (skipping the initial "\X": handled via slice)
            let rest: String = chars[i..].iter().collect();
            if let Some(hex) = rest.strip_prefix("\\X2\\") {
                if let Some(end) = hex.find("\\X0\\") {
                    if let Ok(cp) = u32::from_str_radix(&hex[..end], 16) {
                        if let Some(c) = char::from_u32(cp) {
                            out.push(c);
                            let consumed = "\\X2\\".len() + end + "\\X0\\".len();
                            i += consumed.min(chars.len() - i);
                            continue;
                        }
                    }
                }
            }
            // unhandled escape: carry the backslash literally
            out.push('\\');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Split params on top-level commas.
fn top_level_split(params: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    for c in params.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Return all `#n` references found inside a params string (any position).
fn all_refs(params: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let chars: Vec<char> = params.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '#' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                if let Ok(v) = chars[i + 1..j].iter().collect::<String>().parse() {
                    out.push(v);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Resolve a model value reference by walking from `#id` through an instance
/// that contains `LENGTH_MEASURE(...)`.
fn resolved_length(insts: &HashMap<u64, Instance>, id: u64) -> Option<f64> {
    let inst = insts.get(&id)?;
    for (_name, params) in &inst.groups {
        if let Some(v) = length_measure_in(params) {
            return Some(v);
        }
        // value may be one more hop (complex measure -> unit)
        for r in all_refs(params) {
            if let Some(v) = length_measure_in(&insts.get(&r)?.groups.first()?.1) {
                return Some(v);
            }
        }
    }
    None
}

/// Find `LENGTH_MEASURE(<n>)` anywhere in a params string.
fn length_measure_in(params: &str) -> Option<f64> {
    let idx = params.find("LENGTH_MEASURE(")?;
    let rest = &params[idx + "LENGTH_MEASURE(".len()..];
    let end = rest.find(')')?;
    let num: String = rest[..end].trim().to_string();
    num.parse().ok()
}

/// Determine the length-unit string from the GLOBAL_UNIT_ASSIGNED_CONTEXT:
/// SI_UNIT(.MILLI.,.METRE.) -> "mm", NON_SI_UNIT(.INCH.) -> "in",
/// (.METRE.) -> "m". Defaults to "mm".
fn infer_units(insts: &HashMap<u64, Instance>, default: &str) -> String {
    for inst in insts.values() {
        for (name, params) in &inst.groups {
            if name == "SI_UNIT" || (name == "NAMED_UNIT" && params.contains(".METRE.")) {
                if params.contains(".MILLI.") {
                    return "mm".to_string();
                }
                if params.contains(".METRE.") {
                    return "m".to_string();
                }
            }
            if params.contains("NON_SI_UNIT(.INCH.)") {
                return "in".to_string();
            }
        }
    }
    let _ = default;
    "mm".to_string()
}

/// Parse an AP242 STEP file written by our writer back into a PmiDocument.
pub fn read_ap242_pmi(text: &str, default_units: &str) -> Result<PmiDocument, ReadError> {
    let data_start = text
        .find("\nDATA;")
        .map(|i| i + "\nDATA;".len())
        .or_else(|| text.find("DATA;").map(|i| i + "DATA;".len()))
        .ok_or_else(|| ReadError::NotPart21("no DATA section".to_string()))?;
    let tail = &text[data_start..];
    let rel = tail
        .find("ENDSEC")
        .ok_or_else(|| ReadError::NotPart21("no ENDSEC".to_string()))?;
    let data_end = data_start + rel;
    let data_body = &text[data_start..data_end];
    let insts = parse_data(data_body)?;

    let units = infer_units(&insts, default_units);
    let mut doc = PmiDocument::new(units);

    // The writer assigns instance ids in document order, so a single ascending
    // id sweep restores emission order for each typed collection.
    let mut ids: Vec<u64> = insts.keys().copied().collect();
    ids.sort_unstable();

    // ---- Datums ----------------------------------------------------------
    // DATUM('<id>|<feature>',$,'pds',.F.,'<label>')
    // Build a map from instance id -> datum.id; capture datums.
    let mut entity_datum: HashMap<u64, String> = HashMap::new();
    for id in &ids {
        let inst = &insts[id];
        for (name, params) in &inst.groups {
            if name != "DATUM" {
                continue;
            }
            let toks = top_level_split(params);
            if toks.len() < 5 {
                continue;
            }
            let name_field = unquote(&toks[0]).unwrap_or_default();
            let label = unquote(&toks[4]).unwrap_or_default();
            let (datum_id, feature) = name_field.split_once('|').map_or_else(
                || (name_field.clone(), None),
                |(a, b)| (a.to_string(), Some(b.to_string())),
            );
            entity_datum.insert(*id, datum_id.clone());
            let mut d = Datum::new(datum_id, label);
            if let Some(f) = feature {
                d.feature_ids.push(f);
            }
            doc.push_datum(d);
        }
    }

    // ---- Datum frames ----------------------------------------------------
    // Build datum_id -> label map for restoration from compartments.
    let _datum_label: HashMap<String, String> = doc
        .datums
        .iter()
        .map(|d| (d.id.clone(), d.label.clone()))
        .collect();

    // compartment entity: DATUM_REFERENCE_COMPARTMENT('',$,pds,.F.,#datum,mod)
    let mut entity_compartment: HashMap<u64, (u64, Option<MaterialCondition>)> = HashMap::new();
    for (id, inst) in &insts {
        for (name, params) in &inst.groups {
            if name != "DATUM_REFERENCE_COMPARTMENT" {
                continue;
            }
            let toks = top_level_split(params);
            if toks.len() < 5 {
                continue;
            }
            let datum_ref = all_refs(&toks[4]).into_iter().next();
            let mc = match toks.get(5) {
                Some(m) if m.contains(".MAXIMUM_MATERIAL_REQUIREMENT.") => {
                    Some(MaterialCondition::Mmc)
                }
                Some(m) if m.contains(".LEAST_MATERIAL_REQUIREMENT.") => {
                    Some(MaterialCondition::Lmc)
                }
                Some(m) if m.contains(".REGARDLESS_OF_FEATURE_SIZE.") => {
                    Some(MaterialCondition::Rfs)
                }
                _ => None,
            };
            if let Some(d) = datum_ref {
                entity_compartment.insert(*id, (d, mc));
            }
        }
    }

    // DATUM_SYSTEM('<frame_id>','',pds,.F.,(#c1,#c2,#c3))
    for id in &ids {
        let inst = &insts[id];
        for (name, params) in &inst.groups {
            if name != "DATUM_SYSTEM" {
                continue;
            }
            let toks = top_level_split(params);
            if toks.len() < 5 {
                continue;
            }
            let frame_id = unquote(&toks[0]).unwrap_or_default();
            let comp_ids = all_refs(&toks[4]);
            let mut refs = Vec::new();
            // precedence derives from list order (1 = primary = first).
            for (i, cid) in comp_ids.iter().enumerate() {
                if let Some((datum_ent, mc)) = entity_compartment.get(cid) {
                    if let Some(datum_id) = entity_datum.get(datum_ent) {
                        let mut r = DatumFrameRef::new(datum_id.clone(), (i + 1) as u8);
                        if let Some(m) = mc {
                            r = r.with_modifier(*m);
                        }
                        refs.push(r);
                    }
                }
            }
            let mut f = DatumFrame::new(frame_id.clone(), frame_id);
            f.refs = refs;
            doc.push_frame(f);
        }
    }

    // ---- Geometric tolerances (complex entities) ------------------------
    for id in &ids {
        let inst = &insts[id];
        // complex GT instances have multiple groups; skip simple instances
        if inst.groups.len() < 2 {
            continue;
        }
        // find the concrete subtype leaf
        let subtype = inst
            .groups
            .iter()
            .rev()
            .find_map(|(n, _)| subtype_to_characteristic(n));
        let Some(characteristic) = subtype else {
            continue;
        };

        // base group: GEOMETRIC_TOLERANCE('<gt_id>','',#val,#sa)
        let base = inst
            .groups
            .iter()
            .find(|(n, _)| n == "GEOMETRIC_TOLERANCE")
            .map(|(_, p)| p.clone());
        let Some(base_params) = base else { continue };
        let toks = top_level_split(&base_params);
        if toks.len() < 4 {
            continue;
        }
        let gt_id = unquote(&toks[0]).unwrap_or_default();
        // skip composite second tolerance leaves (handled via relationship)
        if gt_id.ends_with("~2") {
            continue;
        }
        let val = all_refs(&toks[2])
            .into_iter()
            .find_map(|r| resolved_length(&insts, r))
            .unwrap_or(0.0);
        let feature = all_refs(&toks[3])
            .into_iter()
            .find_map(|r| {
                insts
                    .get(&r)
                    .and_then(|i| i.groups.first().cloned())
                    .and_then(|(n2, p2)| {
                        if n2 == "SHAPE_ASPECT" {
                            // SHAPE_ASPECT('<feature>','',#pds,.F.) — take the
                            // first top-level field.
                            top_level_split(&p2)
                                .into_iter()
                                .next()
                                .and_then(|s| unquote(&s))
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or_default();

        // datum frame
        let mut frame_id: Option<String> = None;
        if let Some((_, p)) = inst
            .groups
            .iter()
            .find(|(n, _)| n == "GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE")
        {
            // ( #dsys )
            for r in all_refs(p) {
                if let Some(di) = insts.get(&r) {
                    // the ref is a DATUM_SYSTEM; its name is the frame id
                    if let Some((_, p2)) = di.groups.iter().find(|(n, _)| n == "DATUM_SYSTEM") {
                        let toks2 = top_level_split(p2);
                        frame_id = toks2.first().and_then(|s| unquote(s));
                    }
                }
            }
        }

        // material condition
        let mut material_condition: Option<MaterialCondition> = None;
        if let Some((_, p)) = inst
            .groups
            .iter()
            .find(|(n, _)| n == "GEOMETRIC_TOLERANCE_WITH_MODIFIERS")
        {
            if p.contains(".MAXIMUM_MATERIAL_REQUIREMENT.") {
                material_condition = Some(MaterialCondition::Mmc);
            } else if p.contains(".LEAST_MATERIAL_REQUIREMENT.") {
                material_condition = Some(MaterialCondition::Lmc);
            } else if p.contains(".REGARDLESS_OF_FEATURE_SIZE.") {
                material_condition = Some(MaterialCondition::Rfs);
            }
        }

        let mut tol = GeometricTolerance::new(gt_id.clone(), characteristic, val);
        if !feature.is_empty() {
            tol.feature_ids.push(feature);
        }
        if let Some(f) = frame_id {
            tol.frame_id = Some(f);
        }
        if let Some(mc) = material_condition {
            tol.material_condition = Some(mc);
        }

        // diameter zone: a TOLERANCE_ZONE whose target list references `id`
        if tolerance_has_diameter(&insts, *id) {
            tol.diameter_zone = true;
        }

        doc.push_tolerance(tol);
    }

    // ---- composite second values ----------------------------------------
    // GEOMETRIC_TOLERANCE_RELATIONSHIP('composite','',#relating,#related)
    for inst in insts.values() {
        for (name, params) in &inst.groups {
            if name != "GEOMETRIC_TOLERANCE_RELATIONSHIP" {
                continue;
            }
            let toks = top_level_split(params);
            if toks.len() < 4 || unquote(&toks[0]).as_deref() != Some("composite") {
                continue;
            }
            let related_ids = all_refs(toks[2].as_str());
            // get first ref (relating is toks[2]) — find its ~2 leaf
            let relating = related_ids.into_iter().next();
            let _ = relating;
            let related = all_refs(toks[3].as_str()).into_iter().next();
            let Some(rel_id) = related else { continue };
            // the related ~2 tolerance: base id == "x~2" we stored as "x"
            let Some(rel) = insts.get(&rel_id) else {
                continue;
            };
            let base = rel
                .groups
                .iter()
                .find(|(n, _)| n == "GEOMETRIC_TOLERANCE")
                .map(|(_, p)| p.clone());
            let Some(bp) = base else { continue };
            let toks2 = top_level_split(&bp);
            if toks2.len() < 3 {
                continue;
            }
            let base_id = unquote(&toks2[0])
                .unwrap_or_default()
                .trim_end_matches("~2")
                .to_string();
            let sec_val = all_refs(&toks2[2])
                .into_iter()
                .find_map(|r| resolved_length(&insts, r))
                .unwrap_or(0.0);
            if let Some(t) = doc.tolerances.iter_mut().find(|t| t.id == base_id) {
                t.second_value = Some(sec_val);
            }
        }
    }

    // ---- Notes ----------------------------------------------------------
    // DESCRIPTIVE_REPRESENTATION_ITEM('<id>|<feature>','<text>')
    // Preserve emission (document) order: gather and sort by instance id.
    let mut notes: Vec<(u64, String, String, Vec<String>)> = Vec::new();
    for (id, inst) in &insts {
        for (name, params) in &inst.groups {
            if name != "DESCRIPTIVE_REPRESENTATION_ITEM" {
                continue;
            }
            let toks = top_level_split(params);
            if toks.len() < 2 {
                continue;
            }
            let name_field = unquote(&toks[0]).unwrap_or_default();
            let text = unquote(&toks[1]).unwrap_or_default();
            let (nid, feature) = name_field.split_once('|').map_or_else(
                || (name_field.clone(), None),
                |(a, b)| (a.to_string(), Some(b.to_string())),
            );
            notes.push((*id, nid, text, feature.map(|f| vec![f]).unwrap_or_default()));
        }
    }
    notes.sort_by_key(|(id, _, _, _)| *id);
    for (_id, nid, text, feat) in notes {
        let mut n = Note::new(nid, text);
        n.feature_ids = feat;
        doc.push_note(n);
    }

    Ok(doc)
}

/// Reverse of [`crate::pmi::writer::subtype_to_characteristic`] — the
/// AP242 tolerance subtype leaf name -> internal characteristic.
fn subtype_to_characteristic(name: &str) -> Option<Characteristic> {
    use Characteristic::*;
    match name {
        "STRAIGHTNESS_TOLERANCE" => Some(Straightness),
        "FLATNESS_TOLERANCE" => Some(Flatness),
        "ROUNDNESS_TOLERANCE" => Some(Circularity),
        "CYLINDRICITY_TOLERANCE" => Some(Cylindricity),
        "LINE_PROFILE_TOLERANCE" => Some(ProfileOfLine),
        "SURFACE_PROFILE_TOLERANCE" => Some(ProfileOfSurface),
        "ANGULARITY_TOLERANCE" => Some(Angularity),
        "PERPENDICULARITY_TOLERANCE" => Some(Perpendicularity),
        "PARALLELISM_TOLERANCE" => Some(Parallelism),
        "POSITION_TOLERANCE" => Some(Position),
        "CONCENTRICITY_TOLERANCE" => Some(Concentricity),
        "SYMMETRY_TOLERANCE" => Some(Symmetry),
        "CIRCULAR_RUNOUT_TOLERANCE" => Some(CircularRunout),
        "TOTAL_RUNOUT_TOLERANCE" => Some(TotalRunout),
        _ => None,
    }
}

fn tolerance_has_diameter(insts: &HashMap<u64, Instance>, tol_id: u64) -> bool {
    for inst in insts.values() {
        for (name, params) in &inst.groups {
            if name == "TOLERANCE_ZONE" && all_refs(params).contains(&tol_id) {
                return true;
            }
        }
    }
    false
}
