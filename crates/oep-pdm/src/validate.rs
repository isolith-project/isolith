//! Structural validation for [`VaultDoc`]s.
//!
//! Validation is a separate walk (nothing runs at deserialization time):
//! [`validate_doc`] checks invariant rules and returns every violation as a
//! [`ValidationError`], never panicking on malformed input (tolerant-failure
//! doctrine, P2-M1-T04). Rules and codes:
//!
//! - `bad-format` / `unsupported-version`: header is not `oep-vault` version 1.
//! - `duplicate-id`: an id is reused anywhere in the document's id space
//!   (one vault = one id space, across all five collections).
//! - `duplicate-ref`: a reference list contains the same id twice.
//! - `invalid-context-kind` / `invalid-item-kind` / `invalid-revision-state` /
//!   `invalid-relationship-kind`: a closed-vocabulary field is out of bounds.
//! - `invalid-blob-address`: a file's `blob_sha256` is not exactly 64
//!   lowercase hex characters (the content-address contract).
//! - `unresolved-context`: an item's `context_id` is not in `contexts`.
//! - `unresolved-item`: a revision's `item_id` is not in `items`.
//! - `unresolved-file`: a revision references a file id not in `files`.
//! - `context-cycle`: the context parent tree contains a cycle.
//! - `bad-relationship`: a relationship endpoint references a nonexistent
//!   object of the declared class.
//! - `relationship-kind-mismatch`: an edge kind is used with disallowed
//!   endpoints (`composed_of`/`depends_on`/`references` need item endpoints;
//!   `supersedes` needs two revisions of the same item).
//!
//! Blob **presence and integrity** are not validated here (bytes live outside
//! the document); see [`crate::blobstore::BlobStore::verify_refs`].

use crate::model::{
    VaultDoc, CONTEXT_KINDS, FORMAT, ITEM_KINDS, RELATION_KINDS, REL_KIND_COMPOSED_OF,
    REL_KIND_DEPENDS_ON, REL_KIND_REFERENCES, REL_KIND_SUPERSEDES, REVISION_STATES, VERSION,
};
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

/// A 64-char lowercase-hex sha-256 string is the content-address contract.
fn is_lower_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Run every rule; returns all findings (empty vector = structurally valid).
pub fn validate_doc(doc: &VaultDoc) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Header.
    if doc.format != FORMAT {
        errors.push(ValidationError::new(
            "bad-format",
            format!("document format {:?} is not {FORMAT:?}", doc.format),
        ));
    }
    if doc.version != VERSION {
        errors.push(ValidationError::new(
            "unsupported-version",
            format!(
                "document version {} is not supported (expected {VERSION})",
                doc.version
            ),
        ));
    }

    // One id space across all five collections.
    let mut seen: HashMap<String, &'static str> = HashMap::new();
    for c in &doc.contexts {
        record_id(&mut seen, "contexts", &c.id, &mut errors);
    }
    for i in &doc.items {
        record_id(&mut seen, "items", &i.id, &mut errors);
    }
    for r in &doc.revisions {
        record_id(&mut seen, "revisions", &r.id, &mut errors);
    }
    for f in &doc.files {
        record_id(&mut seen, "files", &f.id, &mut errors);
    }
    for r in &doc.relationships {
        record_id(&mut seen, "relationships", &r.id, &mut errors);
    }

    // Reference tables.
    let context_ids: HashSet<&str> = doc.contexts.iter().map(|c| c.id.as_str()).collect();
    let item_ids: HashSet<&str> = doc.items.iter().map(|i| i.id.as_str()).collect();
    let revision_ids: HashSet<&str> = doc.revisions.iter().map(|r| r.id.as_str()).collect();
    let file_ids: HashSet<&str> = doc.files.iter().map(|f| f.id.as_str()).collect();

    // Closed vocabularies.
    for c in &doc.contexts {
        if !CONTEXT_KINDS.contains(&c.kind.as_str()) {
            errors.push(ValidationError::new(
                "invalid-context-kind",
                format!(
                    "context {} kind {:?} is not in {CONTEXT_KINDS:?}",
                    c.id, c.kind
                ),
            ));
        }
    }
    for i in &doc.items {
        if !ITEM_KINDS.contains(&i.kind.as_str()) {
            errors.push(ValidationError::new(
                "invalid-item-kind",
                format!("item {} kind {:?} is not in {ITEM_KINDS:?}", i.id, i.kind),
            ));
        }
    }
    for r in &doc.revisions {
        if !REVISION_STATES.contains(&r.state.as_str()) {
            errors.push(ValidationError::new(
                "invalid-revision-state",
                format!(
                    "revision {} state {:?} is not in {REVISION_STATES:?}",
                    r.id, r.state
                ),
            ));
        }
    }
    for r in &doc.relationships {
        if !RELATION_KINDS.contains(&r.kind.as_str()) {
            errors.push(ValidationError::new(
                "invalid-relationship-kind",
                format!(
                    "relationship {} kind {:?} is not in {RELATION_KINDS:?}",
                    r.id, r.kind
                ),
            ));
        }
    }

    // Content-address contract on files.
    for f in &doc.files {
        if !is_lower_hex64(&f.blob_sha256) {
            errors.push(ValidationError::new(
                "invalid-blob-address",
                format!(
                    "file {} blob_sha256 must be 64 lowercase hex characters, got {:?}",
                    f.id, f.blob_sha256
                ),
            ));
        }
    }

    // Items -> contexts.
    for i in &doc.items {
        if let Some(cid) = &i.context_id {
            if !context_ids.contains(cid.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-context",
                    format!("item {} references missing context {cid:?}", i.id),
                ));
            }
        }
    }

    // Revisions -> items and files.
    for r in &doc.revisions {
        if !item_ids.contains(r.item_id.as_str()) {
            errors.push(ValidationError::new(
                "unresolved-item",
                format!("revision {} references missing item {:?}", r.id, r.item_id),
            ));
        }
        duplicate_refs(
            "duplicate-ref",
            &format!("revision {}", r.id),
            &r.file_ids,
            &mut errors,
        );
        for fid in &r.file_ids {
            if !file_ids.contains(fid.as_str()) {
                errors.push(ValidationError::new(
                    "unresolved-file",
                    format!("revision {} references missing file {fid:?}", r.id),
                ));
            }
        }
    }

    // Context parent tree: no cycle, and roots are allowed only for
    // project/product kinds (a folder must have a parent).
    for ctx in &doc.contexts {
        if ctx.parent_id.is_none() && ctx.kind == crate::model::CONTEXT_KIND_FOLDER {
            errors.push(ValidationError::new(
                "root-folder",
                format!(
                    "context {} is a folder with no parent (folders must nest)",
                    ctx.id
                ),
            ));
        }
    }
    for start in &doc.contexts {
        let mut cur = start.parent_id.as_deref();
        let mut hops = HashSet::new();
        let mut guard = 0;
        while let Some(pid) = cur {
            if pid == start.id {
                errors.push(ValidationError::new(
                    "context-cycle",
                    format!("context {} is its own (transitive) parent", start.id),
                ));
                break;
            }
            hops.insert(pid.to_string());
            guard += 1;
            if guard > doc.contexts.len() {
                errors.push(ValidationError::new(
                    "context-cycle",
                    format!("context {} parent chain is cyclic", start.id),
                ));
                break;
            }
            cur = doc
                .contexts
                .iter()
                .find(|c| c.id == pid)
                .and_then(|c| c.parent_id.as_deref());
        }
        let _ = hops;
    }

    // Relationships: endpoint resolution + per-kind class contract.
    for r in &doc.relationships {
        let from_ok = match r.from.class.as_str() {
            "item" => item_ids.contains(r.from.id.as_str()),
            "revision" => revision_ids.contains(r.from.id.as_str()),
            _ => false,
        };
        let to_ok = match r.to.class.as_str() {
            "item" => item_ids.contains(r.to.id.as_str()),
            "revision" => revision_ids.contains(r.to.id.as_str()),
            _ => false,
        };
        if !from_ok {
            errors.push(ValidationError::new(
                "bad-relationship",
                format!(
                    "relationship {} from endpoint {}/{} does not resolve",
                    r.id, r.from.class, r.from.id
                ),
            ));
        }
        if !to_ok {
            errors.push(ValidationError::new(
                "bad-relationship",
                format!(
                    "relationship {} to endpoint {}/{} does not resolve",
                    r.id, r.to.class, r.to.id
                ),
            ));
        }
        let item_edge = matches!(
            r.kind.as_str(),
            REL_KIND_COMPOSED_OF | REL_KIND_DEPENDS_ON | REL_KIND_REFERENCES
        );
        if item_edge && (r.from.class != "item" || r.to.class != "item") {
            errors.push(ValidationError::new(
                "relationship-kind-mismatch",
                format!(
                    "relationship {} kind {:?} requires item endpoints, got {}/{} -> {}/{}",
                    r.id, r.kind, r.from.class, r.from.id, r.to.class, r.to.id
                ),
            ));
        }
        if r.kind == REL_KIND_SUPERSEDES {
            if r.from.class != "revision" || r.to.class != "revision" {
                errors.push(ValidationError::new(
                    "relationship-kind-mismatch",
                    format!(
                        "relationship {} kind supersedes requires revision endpoints",
                        r.id
                    ),
                ));
            } else {
                // Both endpoints must be revisions of the SAME item.
                let item_of = |rid: &str| -> Option<&str> {
                    doc.revisions
                        .iter()
                        .find(|rv| rv.id == rid)
                        .map(|rv| rv.item_id.as_str())
                };
                match (item_of(&r.from.id), item_of(&r.to.id)) {
                    (Some(a), Some(b)) if a == b => {}
                    _ => errors.push(ValidationError::new(
                        "relationship-kind-mismatch",
                        format!(
                            "relationship {} supersedes must join revisions of the same item",
                            r.id
                        ),
                    )),
                }
            }
        }
    }

    errors
}

/// Keep the where-used primitives on [`crate::model::VaultDoc`] reachable from
/// this module (they ship here because they are pure graph walks over
/// validated relationships).
impl VaultDoc {
    /// All `composed_of` component item ids whose source is `assembly_item`,
    /// in document order. Reverse traversal (component -> assembly) is the
    /// primitive the P8-M2-T03 where-used query builds on.
    pub fn components_of(&self, assembly_item: &str) -> Vec<String> {
        self.relationships
            .iter()
            .filter(|r| {
                r.kind == REL_KIND_COMPOSED_OF
                    && r.from.class == "item"
                    && r.from.id == assembly_item
                    && r.to.class == "item"
            })
            .map(|r| r.to.id.clone())
            .collect()
    }

    /// Seed of the where-used query: ids of items that compose `item` (i.e.
    /// reverse `composed_of` edges). P8-M2-T03 builds the full transitive
    /// impact query on this primitive.
    pub fn where_used(&self, item: &str) -> Vec<String> {
        self.relationships
            .iter()
            .filter(|r| {
                r.kind == REL_KIND_COMPOSED_OF
                    && r.to.class == "item"
                    && r.to.id == item
                    && r.from.class == "item"
            })
            .map(|r| r.from.id.clone())
            .collect()
    }
}
