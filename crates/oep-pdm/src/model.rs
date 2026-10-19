//! The vault object model: **item / revision / file / relationship / context**.
//!
//! A [`VaultDoc`] is a flat store for the five model classes and is the object
//! a vault-metadata file in our own format (`format = "oep-vault"`,
//! `version = 1`) deserializes into. All links between classes are id
//! references resolved at [`VaultDoc::validate`] time, never during
//! deserialization, so a partially-linked vault is still loadable
//! (tolerant-failure doctrine). Blob bytes are NOT part of the document:
//! [`File::blob_sha256`] is the content address, resolved and verified by
//! [`crate::blobstore::BlobStore`].
//!
//! The schema anticipates the V5-era folder-tree migration (P10-M1-T03):
//! items and revisions carry an optional [`Origin`], directories re-materialize
//! as [`Context`] nodes, and the [`VaultDoc::imports`] ledger makes re-runs
//! idempotent.

use serde::{Deserialize, Serialize};

/// The canonical `format` header value identifying the vault model format.
pub const FORMAT: &str = "oep-vault";
/// The current schema version of the format.
pub const VERSION: u32 = 1;

/// Item kind: `part` -- a single component artifact.
pub const ITEM_KIND_PART: &str = "part";
/// Item kind: `assembly` -- an ordered collection of components.
pub const ITEM_KIND_ASSEMBLY: &str = "assembly";
/// Item kind: `document` -- a non-geometry artifact (notes, specs).
pub const ITEM_KIND_DOCUMENT: &str = "document";
/// The closed vocabulary of [`Item::kind`]; mirrors the P7-M2-T02 contract's
/// `ALLOWED_KINDS` so the model and the REST API agree by name.
pub const ITEM_KINDS: [&str; 3] = [ITEM_KIND_PART, ITEM_KIND_ASSEMBLY, ITEM_KIND_DOCUMENT];

/// Revision state: `working` -- editable draft.
pub const STATE_WORKING: &str = "working";
/// Revision state: `under_review` -- submitted but not released.
pub const STATE_UNDER_REVIEW: &str = "under_review";
/// Revision state: `released` -- approved and consumable downstream.
pub const STATE_RELEASED: &str = "released";
/// Revision state: `obsolete` -- superseded/withdrawn.
pub const STATE_OBSOLETE: &str = "obsolete";
/// The closed vocabulary of [`Revision::state`]; matches the P7-M2-T02
/// contract's `ALLOWED_STATES`. Transition rules are P8-M2-T01's work; the
/// model only bounds the stored values.
pub const REVISION_STATES: [&str; 4] = [
    STATE_WORKING,
    STATE_UNDER_REVIEW,
    STATE_RELEASED,
    STATE_OBSOLETE,
];

/// Context kind: `project` -- a top-level umbrella (no parent).
pub const CONTEXT_KIND_PROJECT: &str = "project";
/// Context kind: `product` -- a product line / program frame.
pub const CONTEXT_KIND_PRODUCT: &str = "product";
/// Context kind: `folder` -- a directory node (the V5-era folder-tree shape).
pub const CONTEXT_KIND_FOLDER: &str = "folder";
/// The closed vocabulary of [`Context::kind`].
pub const CONTEXT_KINDS: [&str; 3] = [
    CONTEXT_KIND_PROJECT,
    CONTEXT_KIND_PRODUCT,
    CONTEXT_KIND_FOLDER,
];

/// Relationship kind: `composed_of` -- assembly item -> component item (BOM).
pub const REL_KIND_COMPOSED_OF: &str = "composed_of";
/// Relationship kind: `depends_on` -- item -> item general dependency.
pub const REL_KIND_DEPENDS_ON: &str = "depends_on";
/// Relationship kind: `references` -- item -> item (e.g. a document citing a part).
pub const REL_KIND_REFERENCES: &str = "references";
/// Relationship kind: `supersedes` -- revision -> earlier revision of the same item.
pub const REL_KIND_SUPERSEDES: &str = "supersedes";
/// Relationship kind: `derived_from` -- item/revision -> source item/revision.
pub const REL_KIND_DERIVED_FROM: &str = "derived_from";
/// The closed vocabulary of [`Relationship::kind`].
pub const RELATION_KINDS: [&str; 5] = [
    REL_KIND_COMPOSED_OF,
    REL_KIND_DEPENDS_ON,
    REL_KIND_REFERENCES,
    REL_KIND_SUPERSEDES,
    REL_KIND_DERIVED_FROM,
];

/// Origin source tag: a V5-era folder-tree import (P10-M1-T03).
pub const ORIGIN_SOURCE_V5_FOLDER_TREE: &str = "v5-folder-tree";
/// Revision-origin kind: a normal save through the PDM save-revision verb.
pub const ORIGIN_KIND_PDM_SAVE: &str = "pdm-save";
/// Revision-origin kind: a revision created by a V5-era folder-tree import.
pub const ORIGIN_KIND_V5_FOLDER_TREE: &str = "v5-folder-tree";

/// Provenance of an [`Item`] -- the migration-anticipation hook (P10-M1-T03).
///
/// When a V5-era folder tree is imported, every produced item records where it
/// came from (`legacy_path`, the source byte hash, and when it was collected)
/// so the migration never loses ground truth and can be re-run idempotently.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Origin {
    /// Source kind, e.g. [`ORIGIN_SOURCE_V5_FOLDER_TREE`]. Open-ended: other
    /// sources (ODB, spreadsheet, ...) may be added without a format bump.
    pub source: String,
    /// Absolute legacy path the item was collected from.
    pub legacy_path: String,
    /// RFC 3339 UTC timestamp of collection/import.
    pub collected_at: String,
    /// sha-256 of the original legacy bytes, when known (verification anchor).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Importer tool identifier (e.g. `oep-vault-migrate/0.1`).
    pub importer: String,
}

impl Origin {
    /// An `Origin` describing a V5-era folder-tree capture.
    pub fn v5_folder_tree(
        legacy_path: impl Into<String>,
        collected_at: impl Into<String>,
        sha256: Option<String>,
        importer: impl Into<String>,
    ) -> Self {
        Origin {
            source: ORIGIN_SOURCE_V5_FOLDER_TREE.to_string(),
            legacy_path: legacy_path.into(),
            collected_at: collected_at.into(),
            sha256,
            importer: importer.into(),
        }
    }
}

/// Provenance of a single [`Revision`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RevisionOrigin {
    /// How this revision came to be: [`ORIGIN_KIND_PDM_SAVE`] or
    /// [`ORIGIN_KIND_V5_FOLDER_TREE`].
    pub kind: String,
    /// Legacy source path, present only when `kind` is `v5-folder-tree`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_path: Option<String>,
}

impl RevisionOrigin {
    /// A revision born from an ordinary save-revision verb.
    pub fn pdm_save() -> Self {
        RevisionOrigin {
            kind: ORIGIN_KIND_PDM_SAVE.to_string(),
            legacy_path: None,
        }
    }

    /// A revision born from a V5-era folder-tree import of `legacy_path`.
    pub fn v5_folder_tree(legacy_path: impl Into<String>) -> Self {
        RevisionOrigin {
            kind: ORIGIN_KIND_V5_FOLDER_TREE.to_string(),
            legacy_path: Some(legacy_path.into()),
        }
    }
}

/// A named organizational tree node in the vault.
///
/// A V5-era folder tree re-materializes as a chain of these: the tree root
/// becomes a `product` (or `project`) context and every directory becomes a
/// `folder` context whose `parent_id` is its parent directory's context.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Context {
    /// Unique document-wide id.
    pub id: String,
    /// Human-readable context name (a directory name for `folder` nodes).
    pub name: String,
    /// One of [`CONTEXT_KINDS`].
    pub kind: String,
    /// Parent context id, or `None` for a root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Optional free-text note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Context {
    /// A context with the given id, name and kind and no parent yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Context {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            parent_id: None,
            note: None,
        }
    }

    /// Builder: set the parent context id.
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Builder: set an optional note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A persistent engineering identity inside the vault.
///
/// An item is the thing the browser browses: it has a name, a kind, an
/// optional home [`Context`], and (once revisions exist) a latest revision.
/// The revision chain carries the actual files; an item with no revisions has
/// no content.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Item {
    /// Unique document-wide id.
    pub id: String,
    /// Human item name; unique within a store (the browse key).
    pub name: String,
    /// One of [`ITEM_KINDS`].
    pub kind: String,
    /// Home context id, if the item belongs to a context subtree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    /// Optional part number (PN) for BOM/PDM interop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    /// Optional free-text description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Provenance, present when this item was imported (e.g. V5 folder tree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}

impl Item {
    /// An item with the given id, name and kind.
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Item {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            context_id: None,
            part_number: None,
            description: None,
            created_at: String::new(),
            origin: None,
        }
    }

    /// Builder: place the item in a context.
    pub fn with_context(mut self, context_id: impl Into<String>) -> Self {
        self.context_id = Some(context_id.into());
        self
    }

    /// Builder: set the optional part number.
    pub fn with_part_number(mut self, part_number: impl Into<String>) -> Self {
        self.part_number = Some(part_number.into());
        self
    }

    /// Builder: set RFC 3339 creation time.
    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = created_at.into();
        self
    }

    /// Builder: set provenance.
    pub fn with_origin(mut self, origin: Origin) -> Self {
        self.origin = Some(origin);
        self
    }
}

/// A versioned snapshot of an [`Item`] at a point in time.
///
/// Revisions are the thing that holds files: each revision references the
/// [`File`]s that together made up the item at that moment. Revision numbers
/// are strings (`"1"`, `"2"`, ...) assigned by the store, newest-last in
/// ascending order (list views newest-first) -- matching the P7-M2-T02
/// contract.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Revision {
    /// Unique document-wide id.
    pub id: String,
    /// Owning item id (the item this revision belongs to).
    pub item_id: String,
    /// Revision number as a string (`"1"`, `"2"`, ...).
    pub number: String,
    /// Lifecycle state: one of [`REVISION_STATES`].
    pub state: String,
    /// Optional change comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Acting principal that created the revision.
    pub created_by: String,
    /// Ids of the files that make up this revision.
    #[serde(default)]
    pub file_ids: Vec<String>,
    /// Provenance of this specific revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<RevisionOrigin>,
}

impl Revision {
    /// A revision of `item_id` numbered `number` in `state`, by `created_by`.
    pub fn new(
        id: impl Into<String>,
        item_id: impl Into<String>,
        number: impl Into<String>,
        state: impl Into<String>,
        created_by: impl Into<String>,
    ) -> Self {
        Revision {
            id: id.into(),
            item_id: item_id.into(),
            number: number.into(),
            state: state.into(),
            comment: None,
            created_at: String::new(),
            created_by: created_by.into(),
            file_ids: Vec::new(),
            origin: None,
        }
    }

    /// Builder: attach a file id.
    pub fn with_file(mut self, file_id: impl Into<String>) -> Self {
        self.file_ids.push(file_id.into());
        self
    }

    /// Builder: set RFC 3339 creation time.
    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = created_at.into();
        self
    }

    /// Builder: set provenance.
    pub fn with_origin(mut self, origin: RevisionOrigin) -> Self {
        self.origin = Some(origin);
        self
    }
}

/// A named payload attached to a revision.
///
/// The payload **bytes** are never part of a [`VaultDoc`]: [`File::blob_sha256`]
/// is the lowercase-hex sha-256 content address, resolved to bytes by
/// [`crate::blobstore::BlobStore`], which re-hashes before returning so a
/// corrupted or swapped blob is detected, never silently trusted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct File {
    /// Unique document-wide id.
    pub id: String,
    /// Original file name as stored.
    pub filename: String,
    /// Format tag used for client-side routing (`catpart`, `catproduct`,
    /// `step`, `txt`, ...).
    pub format: String,
    /// Stored media type (usually `application/octet-stream`).
    pub content_type: String,
    /// Decoded blob size in bytes.
    pub size: u64,
    /// Content address: 64 lowercase hex characters of the sha-256 of bytes.
    pub blob_sha256: String,
}

impl File {
    /// A content-addressed file record. `blob_sha256` must be exactly
    /// 64 lowercase hex characters (validated by [`crate::validate`]).
    pub fn new(
        id: impl Into<String>,
        filename: impl Into<String>,
        format: impl Into<String>,
        blob_sha256: impl Into<String>,
    ) -> Self {
        File {
            id: id.into(),
            filename: filename.into(),
            format: format.into(),
            content_type: "application/octet-stream".to_string(),
            size: 0,
            blob_sha256: blob_sha256.into(),
        }
    }

    /// Builder: set the stored media type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }

    /// Builder: set the decoded size in bytes.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }
}

/// One endpoint of a [`Relationship`]: a reference into the item or revision
/// collection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Ref {
    /// Target class: `item` or `revision`.
    pub class: String,
    /// Target id within that class.
    pub id: String,
}

impl Ref {
    /// An endpoint reference into the item collection.
    pub fn item(id: impl Into<String>) -> Self {
        Ref {
            class: "item".to_string(),
            id: id.into(),
        }
    }

    /// An endpoint reference into the revision collection.
    pub fn revision(id: impl Into<String>) -> Self {
        Ref {
            class: "revision".to_string(),
            id: id.into(),
        }
    }
}

/// A typed, directed edge between two vault objects (the where-used graph).
///
/// Each [`RELATION_KINDS`] kind constrains the endpoint classes (see
/// [`crate::validate`]): composition/dependency/reference edges connect items,
/// `supersedes` connects revisions of the **same** item, `derived_from`
/// connects item or revision to the source it was copied from. The P8-M2-T03
/// card builds the where-used / impact query layer on top of this store.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Relationship {
    /// Unique document-wide id.
    pub id: String,
    /// One of [`RELATION_KINDS`].
    pub kind: String,
    /// Source endpoint.
    pub from: Ref,
    /// Target endpoint.
    pub to: Ref,
    /// Optional free-text note on the edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Relationship {
    /// A relationship of `kind` from one endpoint to another.
    pub fn new(kind: impl Into<String>, from: Ref, to: Ref) -> Self {
        Relationship {
            id: String::new(),
            kind: kind.into(),
            from,
            to,
            note: None,
        }
    }

    /// Builder: set the relationship id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Builder: set an optional note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A ledger row recording a completed legacy import (migration anticipation).
///
/// P10-M1-T03 writes exactly one row per folder-tree migration run. The
/// `keys` list carries the idempotency key (sha-256 of the normalized legacy
/// path) of every file that run imported, so a re-run skips already-imported
/// files instead of duplicating them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ImportLedger {
    /// Import kind, e.g. [`ORIGIN_SOURCE_V5_FOLDER_TREE`].
    pub kind: String,
    /// Absolute path of the tree root that was imported.
    pub root_legacy_path: String,
    /// RFC 3339 UTC timestamp of the import run.
    pub imported_at: String,
    /// Importer tool identifier.
    pub importer: String,
    /// Number of contexts created by this run.
    pub contexts: u64,
    /// Number of items created by this run.
    pub items: u64,
    /// Number of file records created by this run.
    pub files: u64,
    /// Number of distinct content-addressed blobs stored by this run.
    pub blobs: u64,
    /// Idempotency keys of the files this run imported.
    #[serde(default)]
    pub keys: Vec<String>,
}

/// A complete vault-metadata document in our own serde format.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VaultDoc {
    /// Format tag -- always [`FORMAT`] (`"oep-vault"`). Required.
    pub format: String,
    /// Schema version -- currently [`VERSION`] (`1`). Required.
    pub version: u32,
    /// All context nodes in the vault.
    #[serde(default)]
    pub contexts: Vec<Context>,
    /// All items in the vault.
    #[serde(default)]
    pub items: Vec<Item>,
    /// All revisions in the vault.
    #[serde(default)]
    pub revisions: Vec<Revision>,
    /// All file records in the vault.
    #[serde(default)]
    pub files: Vec<File>,
    /// All relationships in the vault.
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    /// Migration ledger: one row per completed legacy import.
    #[serde(default)]
    pub imports: Vec<ImportLedger>,
}

impl VaultDoc {
    /// A new, empty vault at the current schema version.
    pub fn new() -> Self {
        VaultDoc {
            format: FORMAT.to_string(),
            version: VERSION,
            contexts: Vec::new(),
            items: Vec::new(),
            revisions: Vec::new(),
            files: Vec::new(),
            relationships: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// The format tag (`"oep-vault"`).
    pub fn format(&self) -> &str {
        &self.format
    }

    /// The schema version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Serialize to our canonical pretty JSON (2-space indent, trailing LF).
    ///
    /// Output is deterministic: key order follows struct declaration order,
    /// every list keeps insertion order, and there is no `HashMap` anywhere in
    /// the serialized types -- the same logical document always produces the
    /// same bytes (golden-byte-lock meaningful).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Deserialize a vault document from our JSON format (accepts pretty or
    /// compact). Does NOT validate; call [`VaultDoc::validate`] separately.
    pub fn from_json_pretty(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Validate structural invariants (see [`crate::validate::ValidationError`]
    /// rules). Returns every violation; an empty vector is a valid vault.
    pub fn validate(&self) -> Vec<crate::validate::ValidationError> {
        crate::validate::validate_doc(self)
    }

    /// Append a context.
    pub fn push_context(&mut self, context: Context) {
        self.contexts.push(context);
    }

    /// Append an item.
    pub fn push_item(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Append a revision.
    pub fn push_revision(&mut self, revision: Revision) {
        self.revisions.push(revision);
    }

    /// Append a file record.
    pub fn push_file(&mut self, file: File) {
        self.files.push(file);
    }

    /// Append a relationship.
    pub fn push_relationship(&mut self, relationship: Relationship) {
        self.relationships.push(relationship);
    }

    /// Append a migration-ledger row.
    pub fn push_import(&mut self, import: ImportLedger) {
        self.imports.push(import);
    }
}

impl Default for VaultDoc {
    fn default() -> Self {
        Self::new()
    }
}
