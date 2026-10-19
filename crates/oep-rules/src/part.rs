//! [`PartContext`]: a read-only view of a real `oep_geo` part for the
//! rules/checks sandbox.
//!
//! The sandbox never mutates the canonical model; it reads the part's identity
//! (id, name, part number), the envelope of its first solid body, and the
//! parameters of its history features, and exposes them to formulas through the
//! documented `part.*` member paths. This is the "real part" half of the
//! P7-M4-T02 acceptance: formulas and checks run against committed oep-geo
//! material.

use std::collections::BTreeMap;

use oep_geo::body::BoundingBox;
use oep_geo::Document;
use oep_geo::Feature;

use crate::error::RulesError;
use crate::expr::{Index as Idx, PathSeg, Value};

/// A read-only view of one part of an [`oep_geo::Document`].
#[derive(Clone, Debug, PartialEq)]
pub struct PartContext {
    /// Part id.
    pub id: String,
    /// Part name.
    pub name: String,
    /// Optional part number.
    pub part_number: Option<String>,
    /// Minimum corner of the first solid body's bounding box.
    pub bbox_min: [f64; 3],
    /// Maximum corner of the first solid body's bounding box.
    pub bbox_max: [f64; 3],
    /// Feature id -> (parameter key -> value), from the part's history.
    pub features: BTreeMap<String, BTreeMap<String, Value>>,
}

/// Width/length/depth of the part envelope, named after the body bounding box.
pub const ENVELOPE_WIDTH: &str = "width";
/// Envelope extent along the document Y axis.
pub const ENVELOPE_LENGTH: &str = "length";
/// Envelope extent along the document Z axis (typically the sheet/plate thickness).
pub const ENVELOPE_DEPTH: &str = "depth";

impl PartContext {
    /// Extract a part from `doc`.
    ///
    /// With `part_id` given, that part is selected and an error is returned if
    /// it is absent. Without it, the first part in the document is used; an
    /// empty document is a typed error.
    pub fn from_document(doc: &Document, part_id: Option<&str>) -> Result<Self, RulesError> {
        let part = match part_id {
            Some(id) => doc
                .parts
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| RulesError::NoPart(format!("no part with id {id:?}")))?,
            None => doc
                .parts
                .first()
                .ok_or_else(|| RulesError::NoPart("document has no parts".to_string()))?,
        };

        // Envelope: first body id on the part that resolves to a body record.
        let mut bbox = BoundingBox::new([0.0; 3], [0.0; 3]);
        for body_id in &part.body_ids {
            if let Some(body) = doc.bodies.iter().find(|b| &b.id == body_id) {
                if let Some(actual) = &body.bbox {
                    bbox = *actual;
                }
                break;
            }
        }

        // Features: parameters transcribed to the sandbox value model.
        let mut features: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for feature_id in &part.feature_ids {
            if let Some(feature) = doc.features.iter().find(|f| &f.id == feature_id) {
                features.insert(feature.id.clone(), transcribe_feature(feature));
            }
        }

        Ok(PartContext {
            id: part.id.clone(),
            name: part.name.clone(),
            part_number: part.part_number.clone(),
            bbox_min: bbox.min,
            bbox_max: bbox.max,
            features,
        })
    }

    /// Resolve a `part.*` member path to a value, or a typed error.
    ///
    /// Supported shapes (documented in `P7-M4/rules-checks-sandbox.md`):
    /// - `part.name` -> string
    /// - `part.part_number` -> string (error when the part has none)
    /// - `part.bbox.width | length | depth` -> number
    /// - `part.bbox.min[i] | part.bbox.max[i]` -> number (i = 0..2)
    /// - `part.features["<id>"].parameters["<key>"]` -> number/bool/string
    pub fn resolve_member(&self, segs: &[PathSeg]) -> Result<Value, RulesError> {
        if segs.first() != Some(&PathSeg::Field("part".to_string())) {
            return Err(RulesError::InvalidMemberPath(format!("{segs:?}")));
        }

        let head = segs
            .get(1)
            .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
        match field_in(head, segs)? {
            "name" => {
                expect_len(segs, 2)?;
                Ok(Value::Str(self.name.clone()))
            }
            "part_number" => {
                expect_len(segs, 2)?;
                match &self.part_number {
                    Some(pn) => Ok(Value::Str(pn.clone())),
                    None => Err(RulesError::MissingPartNumber),
                }
            }
            "bbox" => {
                let second = segs
                    .get(2)
                    .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                match field_in(second, segs)? {
                    ENVELOPE_WIDTH => {
                        expect_len(segs, 3)?;
                        Ok(Value::Number(self.bbox_max[0] - self.bbox_min[0]))
                    }
                    ENVELOPE_LENGTH => {
                        expect_len(segs, 3)?;
                        Ok(Value::Number(self.bbox_max[1] - self.bbox_min[1]))
                    }
                    ENVELOPE_DEPTH => {
                        expect_len(segs, 3)?;
                        Ok(Value::Number(self.bbox_max[2] - self.bbox_min[2]))
                    }
                    "min" => {
                        let third = segs
                            .get(3)
                            .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                        let i = idx_int_in(third, segs)?;
                        expect_len(segs, 4)?;
                        Ok(Value::Number(self.bbox_min[i]))
                    }
                    "max" => {
                        let third = segs
                            .get(3)
                            .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                        let i = idx_int_in(third, segs)?;
                        expect_len(segs, 4)?;
                        Ok(Value::Number(self.bbox_max[i]))
                    }
                    other => Err(RulesError::InvalidMemberPath(format!(
                        "unknown bbox member {other:?}: {segs:?}"
                    ))),
                }
            }
            "features" => {
                // part.features["<id>"].parameters["<key>"]
                let id_seg = segs
                    .get(2)
                    .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                let feature_id = idx_str_in(id_seg, segs)?;
                let mid_seg = segs
                    .get(3)
                    .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                let middle = field_in(mid_seg, segs)?;
                if middle != "parameters" {
                    return Err(RulesError::InvalidMemberPath(format!(
                        "expected .parameters after feature id: {segs:?}"
                    )));
                }
                let key_seg = segs
                    .get(4)
                    .ok_or_else(|| RulesError::InvalidMemberPath(format!("{segs:?}")))?;
                let key = idx_str_in(key_seg, segs)?;
                expect_len(segs, 5)?;
                let feature = self
                    .features
                    .get(feature_id)
                    .ok_or_else(|| RulesError::UnknownFeature(feature_id.to_string()))?;
                feature
                    .get(key)
                    .cloned()
                    .ok_or_else(|| RulesError::UnknownFeatureParameter {
                        feature: feature_id.to_string(),
                        key: key.to_string(),
                    })
            }
            other => Err(RulesError::InvalidMemberPath(format!(
                "unknown part member {other:?}: {segs:?}"
            ))),
        }
    }
}

/// Expect the path to have exactly `want` segments.
fn expect_len(segs: &[PathSeg], want: usize) -> Result<(), RulesError> {
    if segs.len() == want {
        Ok(())
    } else {
        Err(RulesError::InvalidMemberPath(format!(
            "{segs:?} (expected {want} segments)"
        )))
    }
}

/// A dotted member name, or a typed error.
fn field_in<'a>(seg: &'a PathSeg, segs: &[PathSeg]) -> Result<&'a str, RulesError> {
    match seg {
        PathSeg::Field(f) => Ok(f.as_str()),
        _ => Err(RulesError::InvalidMemberPath(format!(
            "expected a member name but found an index: {segs:?}"
        ))),
    }
}

/// A string index, or a typed error.
fn idx_str_in<'a>(seg: &'a PathSeg, segs: &[PathSeg]) -> Result<&'a str, RulesError> {
    match seg {
        PathSeg::Index(Idx::Str(v)) => Ok(v.as_str()),
        _ => Err(RulesError::BadIndex(format!(
            "string index required: {segs:?}"
        ))),
    }
}

/// An integer 0..2 index, or a typed error.
fn idx_int_in(seg: &PathSeg, segs: &[PathSeg]) -> Result<usize, RulesError> {
    match seg {
        PathSeg::Index(Idx::Int(v)) if *v >= 0 && *v < 3 => Ok(*v as usize),
        _ => Err(RulesError::BadIndex(format!(
            "integer 0..2 index required: {segs:?}"
        ))),
    }
}

/// Transcribe a feature's parameter map into the sandbox value model.
///
/// Numbers, booleans and strings are kept; structured JSON values
/// (arrays/objects/null) have no representation in the knowledge-light value
/// model and are omitted (documented not parity).
fn transcribe_feature(feature: &Feature) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (key, value) in &feature.parameters {
        if let Some(v) = from_json_value(value) {
            out.insert(key.clone(), v);
        }
    }
    out
}

/// Convert a `serde_json::Value` into the sandbox [`Value`] where representable.
fn from_json_value(value: &serde_json::Value) -> Option<Value> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(Value::Number),
        serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
        serde_json::Value::String(s) => Some(Value::Str(s.clone())),
        _ => None,
    }
}
