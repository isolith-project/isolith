//! Structural validation for [`PmiDocument`]s.
//!
//! Validation is a separate walk (nothing runs at deserialization time):
//! [`validate_document`] checks invariant rules and returns every violation as
//! a [`ValidationError`], never panicking on malformed input (tolerant-failure
//! doctrine). Rules and codes:
//!
//! - `bad-format` / `unsupported-version`: header is not `oep-pmi` version 1.
//! - `duplicate-id`: an id is reused anywhere in the document's id space
//!   (one document = one id space, across all four collections).
//! - `duplicate-ref`: a reference list contains the same id twice.
//! - `unresolved-datum`: a frame ref names a datum id not in `datums`.
//! - `unresolved-frame`: a tolerance names a frame id not in `frames`.
//! - `invalid-precedence`: a frame ref precedence is outside 1..=3.
//! - `duplicate-precedence`: a frame has two refs with the same precedence.
//! - `non-finite-value`: a tolerance value (or second value) is not finite or
//!   is negative.
//! - `invalid-position`: a note position is not exactly three finite numbers.
//! - `empty-label` / `empty-text`: a datum label or note text is blank.

use crate::document::PmiDocument;
use std::collections::{HashMap, HashSet};

/// A single validation finding with a stable machine-readable `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Stable error code (see module docs for the full rule list).
    pub code: &'static str,
    /// Human-readable explanation, including the offending id(s) where known.
    pub message: String,
}

impl ValidationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Check that an ordered id list contains no duplicates.
fn duplicate_refs(
    code: &'static str,
    subject: &str,
    ids: &[String],
    errors: &mut Vec<ValidationError>,
) {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            errors.push(ValidationError::new(
                code,
                format!("{subject} references {id:?} more than once"),
            ));
        }
    }
}

/// Validate a whole PMI document against every documented rule.
pub fn validate_document(doc: &PmiDocument) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if doc.format != crate::document::FORMAT {
        errors.push(ValidationError::new(
            "bad-format",
            format!(
                "expected format {:?}, got {:?}",
                crate::document::FORMAT,
                doc.format
            ),
        ));
    }
    if doc.version != crate::document::VERSION {
        errors.push(ValidationError::new(
            "unsupported-version",
            format!("unsupported version {}", doc.version),
        ));
    }

    // One document = one id space across all four collections.
    let mut ids: HashSet<&str> = HashSet::new();
    fn collect_id<'a>(
        ids: &mut HashSet<&'a str>,
        eid: &'a String,
        errors: &mut Vec<ValidationError>,
    ) {
        if !ids.insert(eid.as_str()) {
            errors.push(ValidationError::new(
                "duplicate-id",
                format!("id {eid:?} appears more than once in the document"),
            ));
        }
    }
    for d in &doc.datums {
        collect_id(&mut ids, &d.id, &mut errors);
        duplicate_refs(
            "duplicate-ref",
            &format!("datum {}", d.id),
            &d.feature_ids,
            &mut errors,
        );
        duplicate_refs(
            "duplicate-ref",
            &format!("datum {}", d.id),
            &d.body_ids,
            &mut errors,
        );
    }
    for f in &doc.frames {
        collect_id(&mut ids, &f.id, &mut errors);
    }
    for t in &doc.tolerances {
        collect_id(&mut ids, &t.id, &mut errors);
        duplicate_refs(
            "duplicate-ref",
            &format!("tolerance {}", t.id),
            &t.feature_ids,
            &mut errors,
        );
        duplicate_refs(
            "duplicate-ref",
            &format!("tolerance {}", t.id),
            &t.body_ids,
            &mut errors,
        );
    }
    for n in &doc.notes {
        collect_id(&mut ids, &n.id, &mut errors);
        duplicate_refs(
            "duplicate-ref",
            &format!("note {}", n.id),
            &n.feature_ids,
            &mut errors,
        );
        duplicate_refs(
            "duplicate-ref",
            &format!("note {}", n.id),
            &n.body_ids,
            &mut errors,
        );
    }

    // Datum labels must be unique and non-empty.
    let mut labels: HashMap<&str, &str> = HashMap::new(); // label -> datum id
    for d in &doc.datums {
        if d.label.trim().is_empty() {
            errors.push(ValidationError::new(
                "empty-label",
                format!("datum {} has an empty label", d.id),
            ));
            continue;
        }
        if let Some(prev) = labels.insert(&d.label, &d.id) {
            errors.push(ValidationError::new(
                "duplicate-label",
                format!(
                    "datum label {:?} used by both {} and {}",
                    d.label, prev, d.id
                ),
            ));
        }
    }

    // Frame -> datum resolution + precedence discipline.
    let datum_ids: HashSet<&str> = doc.datums.iter().map(|d| d.id.as_str()).collect();
    for f in &doc.frames {
        let mut seen_prec = HashSet::new();
        for r in &f.refs {
            if !datum_ids.contains(r.datum_id.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-datum",
                    format!("frame {} references unknown datum {}", f.id, r.datum_id),
                ));
            }
            if r.precedence < 1 || r.precedence > 3 {
                errors.push(ValidationError::new(
                    "invalid-precedence",
                    format!(
                        "frame {} ref {} has precedence {}",
                        f.id, r.datum_id, r.precedence
                    ),
                ));
            }
            if !seen_prec.insert(r.precedence) {
                errors.push(ValidationError::new(
                    "duplicate-precedence",
                    format!(
                        "frame {} has two refs with precedence {}",
                        f.id, r.precedence
                    ),
                ));
            }
        }
    }

    // Tolerance -> frame resolution + value sanity.
    let frame_ids: HashSet<&str> = doc.frames.iter().map(|f| f.id.as_str()).collect();
    for t in &doc.tolerances {
        if let Some(ref fid) = t.frame_id {
            if !frame_ids.contains(fid.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-frame",
                    format!("tolerance {} references unknown frame {}", t.id, fid),
                ));
            }
        }
        if !t.value.is_finite() || t.value < 0.0 {
            errors.push(ValidationError::new(
                "non-finite-value",
                format!(
                    "tolerance {} has non-finite or negative value {}",
                    t.id, t.value
                ),
            ));
        }
        if let Some(second) = t.second_value {
            if !second.is_finite() || second < 0.0 {
                errors.push(ValidationError::new(
                    "non-finite-value",
                    format!(
                        "tolerance {} has non-finite or negative second value {second}",
                        t.id
                    ),
                ));
            }
        }
    }

    // Note position sanity.
    for n in &doc.notes {
        if let Some(pos) = n.position {
            let finite = pos.iter().all(|x| x.is_finite());
            if pos.len() != 3 || !finite {
                errors.push(ValidationError::new(
                    "invalid-position",
                    format!("note {} has a non-finite or malformed position", n.id),
                ));
            }
        }
    }

    errors
}
