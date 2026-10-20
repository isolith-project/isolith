//! P4 -- QIF (Quality Information Framework) import/export basic cut.
//!
//! P4-M4-T01 delivers oep-qif: read/write of QIF 3.0 documents carrying
//! **MBD quality data** — datums, datum reference frames, GD&T characteristics
//! (the closed 6-kind basic set: position, surface profile, perpendicularity,
//! flatness, diameter size, angle), characteristic nominals, standards, primary
//! units, and part identity — per the QIF standard documentation (DMSC
//! ANSI QIF 3.0 / ISO 23952:2020) and its public XSD schemas.
//!
//! Scope of the "basic" cut (full contract in
//! `docs/qif-basic-cut.md`):
//!
//! - **IN** (modeled + re-emitted): document envelope (versionQIF, idMax,
//!   QPId), Header, StandardsDefinitions, FileUnits/PrimaryUnits (linear),
//!   DatumDefinitions, DatumReferenceFrames, Product part identity (PartSet +
//!   ModelNumber + RootPart), Characteristics (definitions + nominals).
//! - **OUT** (tolerantly skipped, counted in the parse report, never a crash,
//!   never an invented value): GeometrySet/TopologySet, ViewSet/Visualization/
//!   PMIDisplay, Features, Plans/Results/Statistics/Rules, NoteSet, vendor
//!   Attributes, and any other section outside the basic cut.
//!
//! Round-trip contract: `parse(bytes)` -> [`QifDoc`]; `write(&QifDoc)` ->
//! canonical normalized QIF XML that re-parses to an equal [`QifDoc`]
//! (structural equality). The emitted document is a normalized basic-cut
//! document: it is deliberately NOT the geometry/plan/result document the
//! source may have been — see the scope doc for the round-trip contract.
//!
//! Syntax validation: the exported bytes must validate against the official
//! QIF 3.0 XSD (QIFDocument.xsd tree). That assertion lives in the Python
//! acceptance test (`tests/test_oep_qif_roundtrip_acceptance.py`) with the
//! official schema bundle (OEP_QIF_XSD_ROOT), plus structural self-checks in
//! [`self_check`]. Tolerant-failure: any byte input yields a typed
//! [`Error`] or a [`ParseReport`], never a panic.
#![deny(missing_docs)]

pub mod error;
pub mod model;
pub mod read;
pub mod write;

pub use error::{Error, Result, SkipNote};
pub use model::{
    CharDef, CharKind, CharNominal, DatumDef, DatumFrame, FrameDatum, Header, Part, QifDoc,
    Standard, Tolerance, ToleranceValue, Unit, ZoneShape,
};
pub use read::{parse, ParseReport};
pub use write::write;

/// Workstream identity anchored by the monorepo layout test.
pub const WORKSTREAM: &str = "P4-QIF";

/// Structural self-check of a parsed/exported document: all reference ids
/// resolve to their targets (datum definition ids, DRF ids, characteristic
/// definition ids), and `id_max` covers every modeled id. Returns a list of
/// problems (empty == self-consistent). Never panics.
pub fn self_check(doc: &QifDoc) -> Vec<String> {
    let mut problems = Vec::new();

    let datum_ids: Vec<u32> = doc.datums.iter().map(|d| d.id).collect();
    let frame_ids: Vec<u32> = doc.frames.iter().map(|f| f.id).collect();
    let def_ids: Vec<u32> = doc.char_defs.iter().map(|d| d.id).collect();

    let mut max_id = 0u32;
    for d in &doc.datums {
        max_id = max_id.max(d.id);
    }
    for f in &doc.frames {
        max_id = max_id.max(f.id);
    }
    for p in &doc.parts {
        max_id = max_id.max(p.id);
    }
    for c in &doc.char_defs {
        max_id = max_id.max(c.id);
    }
    for n in &doc.char_nominals {
        max_id = max_id.max(n.id);
    }
    if doc.id_max < max_id {
        problems.push(format!("idMax {} < max modeled id {}", doc.id_max, max_id));
    }

    for fd in &doc.frames {
        for d in &fd.datums {
            match d.definition_id.parse::<u32>() {
                Ok(id) if datum_ids.contains(&id) => {}
                Ok(id) => problems.push(format!(
                    "frame {} datum ref {}: id {} undefined",
                    fd.id, d.definition_id, id
                )),
                Err(_) => problems.push(format!(
                    "frame {} datum ref non-numeric '{}'",
                    fd.id, d.definition_id
                )),
            }
        }
    }

    for c in &doc.char_defs {
        if let Some(df) = &c.datum_frame_id {
            match df.parse::<u32>() {
                Ok(id) if frame_ids.contains(&id) => {}
                Ok(id) => problems.push(format!(
                    "characteristic {} DRF ref {}: id {} undefined",
                    c.id, df, id
                )),
                Err(_) => problems.push(format!(
                    "characteristic {} DRF ref non-numeric '{}'",
                    c.id, df
                )),
            }
        }
    }

    for n in &doc.char_nominals {
        match n.definition_id.parse::<u32>() {
            Ok(id) if def_ids.contains(&id) => {}
            Ok(id) => problems.push(format!(
                "nominal {} def ref {}: id {} undefined",
                n.id, n.definition_id, id
            )),
            Err(_) => problems.push(format!(
                "nominal {} def ref non-numeric '{}'",
                n.id, n.definition_id
            )),
        }
    }

    problems
}
