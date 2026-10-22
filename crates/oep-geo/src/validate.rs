//! Structural validation for [`Document`]s.
//!
//! Validation is a separate walk (nothing runs at deserialization time):
//! [`validate_document`] checks invariant rules and returns every violation as
//! a [`ValidationError`], never panicking on malformed input (tolerant-failure
//! doctrine). Rules and codes:
//!
//! - `bad-format` / `unsupported-version`: header is not `oep-geo` at the
//!   current schema version.
//! - `duplicate-id`: an id is reused anywhere in the document's id space
//!   (one document = one id space, across all six collections and every
//!   drawing/sheet/view/dimension id).
//! - `duplicate-ref`: a reference list contains the same id twice.
//! - `unresolved-body` / `unresolved-feature`: a part/body/feature references
//!   an id that is not in the document's body/feature table.
//! - `unresolved-source`: an instance sources a part/assembly id that does not
//!   exist.
//! - `invalid-bbox`: a body bounding box is not finite and min<=max per axis.
//! - `non-finite-transform`: an instance transform contains a non-finite
//!   component.
//! - `assembly-cycle`: the instance graph (assemblies instancing assemblies)
//!   contains a cycle — an assembly must not (transitively) instance itself.
//! - `bad-sheet-size`: a sheet paper size has a zero dimension.
//! - `bad-scale`: a view scale is not finite and positive.
//! - `unresolved-part`: a view names a `part_id` that does not exist.
//! - `unresolved-view`: a dimension names a `view_id` that is not on the same
//!   sheet.

use crate::assembly::InstanceSource;
use crate::document::Document;
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

/// Record `id` in the one-document id space; report a `duplicate-id` on reuse.
fn record_id(
    seen: &mut HashMap<String, &'static str>,
    collection: &'static str,
    id: &str,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(&first) = seen.get(id) {
        errors.push(ValidationError::new(
            "duplicate-id",
            format!("id {id:?} appears in both {first} and {collection}"),
        ));
    } else {
        seen.insert(id.to_string(), collection);
    }
}

/// Run every rule; returns all findings (empty vector = structurally valid).
pub(crate) fn validate_document(doc: &Document) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Header.
    if doc.format != crate::document::FORMAT {
        errors.push(ValidationError::new(
            "bad-format",
            format!(
                "document format {:?} is not {}",
                doc.format,
                crate::document::FORMAT
            ),
        ));
    }
    if doc.version != crate::document::VERSION {
        errors.push(ValidationError::new(
            "unsupported-version",
            format!(
                "document version {} is not supported (expected {})",
                doc.version,
                crate::document::VERSION
            ),
        ));
    }

    // One id space across all five collections.
    let mut seen: HashMap<String, &'static str> = HashMap::new();
    for p in &doc.parts {
        record_id(&mut seen, "parts", &p.id, &mut errors);
    }
    for b in &doc.bodies {
        record_id(&mut seen, "bodies", &b.id, &mut errors);
    }
    for f in &doc.features {
        record_id(&mut seen, "features", &f.id, &mut errors);
    }
    for a in &doc.assemblies {
        record_id(&mut seen, "assemblies", &a.id, &mut errors);
        for i in &a.instances {
            record_id(&mut seen, "instances", &i.id, &mut errors);
        }
    }
    for d in &doc.drawings {
        record_id(&mut seen, "drawings", &d.id, &mut errors);
        for s in &d.sheets {
            record_id(&mut seen, "sheets", &s.id, &mut errors);
            for v in &s.views {
                record_id(&mut seen, "views", &v.id, &mut errors);
            }
            for dim in &s.dimensions {
                record_id(&mut seen, "dimensions", &dim.id, &mut errors);
            }
        }
    }

    // Reference tables.
    let body_ids: HashSet<&str> = doc.bodies.iter().map(|b| b.id.as_str()).collect();
    let feature_ids: HashSet<&str> = doc.features.iter().map(|f| f.id.as_str()).collect();
    let part_ids: HashSet<&str> = doc.parts.iter().map(|p| p.id.as_str()).collect();
    let assembly_ids: HashSet<&str> = doc.assemblies.iter().map(|a| a.id.as_str()).collect();

    // Parts.
    for p in &doc.parts {
        duplicate_refs(
            "duplicate-ref",
            &format!("part {}", p.id),
            &p.body_ids,
            &mut errors,
        );
        duplicate_refs(
            "duplicate-ref",
            &format!("part {}", p.id),
            &p.feature_ids,
            &mut errors,
        );
        for id in &p.body_ids {
            if !body_ids.contains(id.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-body",
                    format!("part {} references missing body {id:?}", p.id),
                ));
            }
        }
        for id in &p.feature_ids {
            if !feature_ids.contains(id.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-feature",
                    format!("part {} references missing feature {id:?}", p.id),
                ));
            }
        }
    }

    // Bodies.
    for b in &doc.bodies {
        duplicate_refs(
            "duplicate-ref",
            &format!("body {}", b.id),
            &b.feature_ids,
            &mut errors,
        );
        if let Some(bbox) = &b.bbox {
            if !bbox.is_valid() {
                errors.push(ValidationError::new(
                    "invalid-bbox",
                    format!("body {} has an invalid bounding box", b.id),
                ));
            }
        }
        for id in &b.feature_ids {
            if !feature_ids.contains(id.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-feature",
                    format!("body {} references missing feature {id:?}", b.id),
                ));
            }
        }
    }

    // Features.
    for f in &doc.features {
        duplicate_refs(
            "duplicate-ref",
            &format!("feature {}", f.id),
            &f.body_ids,
            &mut errors,
        );
        for id in &f.body_ids {
            if !body_ids.contains(id.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-body",
                    format!("feature {} references missing body {id:?}", f.id),
                ));
            }
        }
    }

    // Instances.
    for a in &doc.assemblies {
        for i in &a.instances {
            let t = &i.transform;
            if !t.components().iter().all(|v| v.is_finite()) {
                errors.push(ValidationError::new(
                    "non-finite-transform",
                    format!("instance {} has a non-finite transform", i.id),
                ));
            }
            let missing = match &i.source {
                InstanceSource::Part { id } => !part_ids.contains(id.as_str()),
                InstanceSource::Assembly { id } => !assembly_ids.contains(id.as_str()),
            };
            if missing {
                errors.push(ValidationError::new(
                    "unresolved-source",
                    format!(
                        "instance {} sources missing {} {:?}",
                        i.id,
                        if i.source.is_assembly() {
                            "assembly"
                        } else {
                            "part"
                        },
                        i.source.referenced_id()
                    ),
                ));
            }
        }
    }

    // Drawings.
    for d in &doc.drawings {
        for s in &d.sheets {
            if !s.size.is_valid() {
                errors.push(ValidationError::new(
                    "bad-sheet-size",
                    format!(
                        "sheet {} has a non-positive paper size {:?}x{:?} mm",
                        s.id, s.size.width_mm, s.size.height_mm
                    ),
                ));
            }
            let view_ids: HashSet<&str> = s.views.iter().map(|v| v.id.as_str()).collect();
            for v in &s.views {
                if !v.scale.is_finite() || v.scale <= 0.0 {
                    errors.push(ValidationError::new(
                        "bad-scale",
                        format!("view {} has an invalid scale {:?}", v.id, v.scale),
                    ));
                }
                if let Some(pid) = &v.part_id {
                    if !part_ids.contains(pid.as_str()) {
                        errors.push(ValidationError::new(
                            "unresolved-part",
                            format!("view {} references missing part {pid:?}", v.id),
                        ));
                    }
                }
            }
            for dim in &s.dimensions {
                if !view_ids.contains(dim.view_id.as_str()) {
                    errors.push(ValidationError::new(
                        "unresolved-view",
                        format!(
                            "dimension {} references view {:?} not on sheet {}",
                            dim.id, dim.view_id, s.id
                        ),
                    ));
                }
            }
        }
    }

    // Assembly graph: reject cycles (an assembly must not instance itself
    // transitively). Done for every assembly; DFS with per-node state.
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for a in &doc.assemblies {
        let subs: Vec<String> = a
            .instances
            .iter()
            .filter(|i| i.source.is_assembly())
            .map(|i| match &i.source {
                InstanceSource::Assembly { id } => id.clone(),
                _ => unreachable!("filter keeps only assemblies"),
            })
            .collect();
        adjacency.insert(a.id.clone(), subs);
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Visit {
        Empty,
        Visiting,
        Done,
    }
    let mut state: HashMap<String, Visit> = HashMap::new();

    fn walk(
        node: &str,
        adjacency: &HashMap<String, Vec<String>>,
        state: &mut HashMap<String, Visit>,
        errors: &mut Vec<ValidationError>,
    ) {
        match state.get(node).copied().unwrap_or(Visit::Empty) {
            Visit::Done => return,
            Visit::Visiting => {
                errors.push(ValidationError::new(
                    "assembly-cycle",
                    format!("assembly {node:?} is part of an instance cycle"),
                ));
                return;
            }
            Visit::Empty => {}
        }
        state.insert(node.to_string(), Visit::Visiting);
        if let Some(children) = adjacency.get(node) {
            for child in children {
                walk(child, adjacency, state, errors);
            }
        }
        state.insert(node.to_string(), Visit::Done);
    }

    for root in &doc.assemblies {
        walk(&root.id, &adjacency, &mut state, &mut errors);
    }

    errors
}
