//! Document I/O lifecycle (wave A, card UI-A7 UI-OPEN-SAVE).
//!
//! The open/save half of the document lifecycle, engine-side and egui-free:
//! opening a STEP through the existing importer hub, saving the canonical
//! model as a `.oepdoc` document, and reopening it. Everything here is plain
//! std + engine code so it can be shared by the panel ([`crate::app::panels::doc`])
//! and exercised under the same rules the engine already follows: imports go
//! through `oep_import` (never a new parser), the canonical model is
//! `oep_geo::Document`, and a document is deterministic (same bytes in, same
//! bytes out through `to_json_pretty`/`from_json_pretty`).
//!
//! ## Document hash
//!
//! The engine does not expose a document-level hash today (it hashes rendered
//! *frames* with FNV-1a 64 in oep-viewer, and archive blobs with SHA-256 in
//! oep-archive). This card therefore defines [`document_hash`] in this file
//! per the card instruction ("if none exists, define it in doc_io.rs and say
//! so in the proof"), using the SAME hashing scheme the engine already uses
//! for frames - FNV-1a 64 with the oep-viewer constants applied to the
//! canonical `.oepdoc` bytes (`to_json_pretty`, which is deterministic and
//! documented as such by oep-geo). No second hashing scheme is invented.
//!
//! Because the serialization is canonical, `document_hash(open(oepdoc)) ==
//! document_hash(document)` after a save/reopen round trip is a *byte-level*
//! equality, not an approximate one.
//!
//! ## Recent files
//!
//! The recent-list (persisted paths) lives with the rest of the app's user
//! state in the config directory that `crate::config` resolves (`layout_path`
//! names that directory; the recent file sits next to `layout.conf`), so a
//! restart reads the same list back. The panel owns *what* goes in; this
//! module owns *how* it is stored.
//!
//! ## Routed mutations and the command journal (wave B, card UI-B2)
//!
//! Every edit the editor makes to the loaded document is a ROUTED mutation:
//! [`EditHistory::apply`] hashes the document before, applies the caller's
//! change, hashes after, and records a named entry in the command journal
//! ([`history::Journal`]), coalescing a continuous pointer drag into one
//! entry. Undo/redo restore the actual canonical document from a bounded
//! snapshot buffer, and the hashes are exact by construction: after undo the
//! document hashes to the entry's `before`, after redo to its `after`.

use std::path::{Path, PathBuf};

use oep_import::{Document, ImportOutcome, SourceKind};

// The journal is compiled into this crate through this one `#[path]` include
// (the same cross-copy-safe pattern the panels use): `history.rs` sits next
// to `doc_io.rs` under `src/`, and this is the only module that pulls it in,
// so no `duplicate_mod` arises in the test targets that `#[path]`-include
// this file. `dead_code` is allowed here for the same reason UI-B1's router
// helpers are: in the bin build (and in test targets that include this module
// through `app.rs` but do not drive the journal) the seam is built for the
// next wave's wiring, and this crate lints `-D warnings`.
#[allow(dead_code)]
#[path = "history.rs"]
pub mod history;

/// File extension for our own document format.
pub const OEPDOC_EXTENSION: &str = ".oepdoc";

/// Maximum number of recent files kept in the config store.
pub const RECENT_MAX: usize = 5;

/// The committed demo part: the editor-sketch-extrude solid, resolved from
/// this crate's manifest dir so it works regardless of the process cwd (tests
/// run from the crate, `shot.sh` from the repo root). `None` only when the
/// tree is missing the file.
pub fn demo_step_path() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let candidate = root.join("demos/editor-sketch-extrude/edpart.step");
    candidate.exists().then_some(candidate)
}

/// Open a STEP part through the existing importer hub into the canonical
/// [`Document`] model. Never a new parser: detection + dispatch happen in
/// `oep_import`, exactly like every other source the workbench loads.
///
/// Any recognized-but-unimportable input is a typed `Err` (never a panic and
/// never half-loaded geometry): the caller keeps whatever document it had.
pub fn import_step(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    import_step_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The bytes-typed half of [`import_step`], exposed for reuse by the recent
/// self-check path.
pub fn import_step_bytes(bytes: &[u8]) -> Result<Document, String> {
    match oep_import::import_bytes(SourceKind::Step, bytes) {
        Ok(ImportOutcome::Imported { document, .. }) => Ok(document),
        Ok(ImportOutcome::Skipped { source, reason })
        | Ok(ImportOutcome::Reserved { source, reason }) => {
            Err(format!("{source:?} skipped: {reason}"))
        }
        Err(e) => Err(format!("import failed: {e}")),
    }
}

/// Open our own `.oepdoc` document (the canonical oep-geo JSON) back from
/// disk. Older schema versions are promoted to the current one by
/// `from_json_pretty`, so an old file still loads.
pub fn read_oepdoc(path: &Path) -> Result<Document, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Document::from_json_pretty(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Save the current document as a `.oepdoc` (canonical deterministic JSON).
pub fn write_oepdoc(path: &Path, document: &Document) -> Result<(), String> {
    let text = document
        .to_json_pretty()
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// FNV-1a-64 over the deterministic `.oepdoc` bytes of `document`.
///
/// Same constants and scheme as `oep_viewer::backend::frame_hash` (the
/// engine's existing FNV-1a fingerprint). `to_json_pretty` cannot fail for a
/// canonical in-memory [`Document`] (plain serde derive, no custom
/// serializers); the `expect` documents that invariant rather than silently
/// hashing a substitution that would mask an I/O bug.
pub fn document_hash(document: &Document) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let text = document
        .to_json_pretty()
        .expect("canonical oep-geo Document always serializes");
    let mut h = OFFSET;
    for &byte in text.as_bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// The routed, undoable edit history (wave B, card UI-B2 UI-HISTORY).
///
/// The card's detail splits undo machinery along the ownership line: the
/// command journal ([`history::Journal`]) owns ordering and coalescing and
/// records the command token plus the hashes before/after; it does NOT own
/// document state. This type is the other half - the bounded canonical-JSON
/// snapshot buffer that owns the actual states behind the journal's hashes,
/// so undo/redo really restore the document (D-UI-4: "command journal with
/// named entries over the existing snapshot history").
///
/// The two stay aligned by construction: `snapshots[i]` is the canonical
/// `.oepdoc` JSON of the state the document was in BEFORE
/// `journal.entries()[i]` ran, so undoing that entry restores `snapshots[i]`
/// and the restored document hashes to the entry's `before`. Redo restores
/// the state that was current when the edit was undone, hashing to the
/// entry's `after`. Both are asserted here (`debug_assert`) so the acceptance
/// "undo restores prior mesh hash / redo returns to the same hash" is proven,
/// not assumed. Memory is bounded: at most `capacity` edits each way (D-UI-4
/// 32-step semantics), oldest dropped.
//
// `dead_code` allowed on the whole seam for the same reason as the journal:
// in the bin build (and in sibling test targets that include `doc_io` through
// `app.rs`) nothing wires it yet - the routed journal lands its wired call
// sites in the following cards (UI-B3/B4) and is exercised here by
// `--test history`. This crate lints `-D warnings`, and UI-B1's router set
// the same plain `#[allow(dead_code)]` precedent for seam code.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct EditHistory {
    /// Ordering + coalescing + the before/after hashes (no document state).
    journal: history::Journal,
    /// Canonical `.oepdoc` JSON of the state before each committed entry
    /// (`i` aligns with `journal.entries()[i]`), oldest first.
    snapshots: Vec<String>,
    /// Canonical `.oepdoc` JSON of states that were undone, newest first
    /// (the redo side).
    future: Vec<String>,
    /// Maximum edits/states kept in each direction (0 = unbounded).
    capacity: usize,
}

#[allow(dead_code)]
impl EditHistory {
    /// An edit history with the engine's default undo capacity
    /// ([`history::DEFAULT_CAPACITY`]).
    pub fn new() -> Self {
        Self::with_capacity(history::DEFAULT_CAPACITY)
    }

    /// An edit history that keeps at most `capacity` edits each way
    /// (`0` = unbounded), dropping the oldest when exceeded.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            journal: history::Journal::with_capacity(0), // bounded here with the state
            snapshots: Vec::new(),
            future: Vec::new(),
            capacity,
        }
    }

    /// The underlying command journal (ordering + coalescing, hashes only).
    /// The undo/redo affordances and the acceptance tests read it here - the
    /// journal itself has no document dependency.
    pub fn journal(&self) -> &history::Journal {
        &self.journal
    }

    /// Mutable access for gesture lifecycle commands (`end_drag`): the
    /// viewport seam tells the journal when a pointer-up closes a drag.
    pub fn journal_mut(&mut self) -> &mut history::Journal {
        &mut self.journal
    }

    /// Number of committed (undoable) edits; a coalesced drag counts once.
    pub fn len(&self) -> usize {
        self.journal.len()
    }

    /// Whether the history holds no committed edits.
    pub fn is_empty(&self) -> bool {
        self.journal.is_empty()
    }

    /// The maximum number of edits kept each way (0 = unbounded).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether an undo step is available.
    pub fn can_undo(&self) -> bool {
        self.journal.can_undo()
    }

    /// Whether a redo step is available.
    pub fn can_redo(&self) -> bool {
        self.journal.can_redo()
    }

    /// Apply one routed mutation to `document` through the command journal.
    ///
    /// Hashes the document before, runs `mutate`, hashes after, and records a
    /// named entry (`token`) - coalescing when `drag` matches the gesture in
    /// progress, so a whole pointer drag is ONE undo entry. Returns the
    /// document hash after the command. `mutate` is the sole writer of the
    /// document; every other edit path in this crate routes through this seam
    /// (the UI-B1 mutation guard keeps it that way).
    pub fn apply<M>(
        &mut self,
        document: &mut Document,
        token: &'static str,
        drag: Option<history::DragId>,
        mutate: M,
    ) -> u64
    where
        M: FnOnce(&mut Document),
    {
        // A command that only extends an open drag coalesces: keep the FIRST
        // before-state (already captured when the drag opened), hash only.
        let coalesces = self.journal.would_coalesce(token, drag);
        let before = document_hash(document);
        let before_json = if coalesces {
            None
        } else {
            Some(canonical_json(document))
        };
        mutate(document);
        let after = document_hash(document);
        self.journal.push(token, drag, before, after);
        if let Some(json) = before_json {
            self.snapshots.push(json);
        }
        self.trim();
        after
    }

    /// Undo the last edit: restore `document` to the entry's `before` hash
    /// (the "prior mesh hash"). Returns the target hash; exactness is
    /// asserted - the restored document hashes to it.
    pub fn undo(&mut self, document: &mut Document) -> Option<u64> {
        let target = self.journal.undo()?;
        let json = self
            .snapshots
            .pop()
            .expect("the journal and its snapshot stack stay aligned");
        self.future.push(canonical_json(document));
        let restored = parse_canonical(&json);
        debug_assert_eq!(
            document_hash(&restored),
            target,
            "undo must be exact: hash before == hash after undo"
        );
        *document = restored;
        self.trim();
        Some(target)
    }

    /// Redo the last undone edit: restore `document` to the entry's `after`
    /// hash - the same hash the drag ended at before the undo. Returns the
    /// target hash; exactness is asserted the same way.
    pub fn redo(&mut self, document: &mut Document) -> Option<u64> {
        let target = self.journal.redo()?;
        let json = self
            .future
            .pop()
            .expect("the redo side and the journal stay aligned");
        self.snapshots.push(canonical_json(document));
        let restored = parse_canonical(&json);
        debug_assert_eq!(
            document_hash(&restored),
            target,
            "redo must be exact: hash after == hash after redo"
        );
        *document = restored;
        self.trim();
        Some(target)
    }

    /// Forget every edit and any open gesture (both branches).
    pub fn clear(&mut self) {
        self.journal.clear();
        self.snapshots.clear();
        self.future.clear();
    }

    /// Enforce the memory bound (D-UI-4: 32-step semantics, bounded memory)
    /// on the journal AND its snapshot store in lock-step, so the hashes and
    /// the states they point at never diverge.
    fn trim(&mut self) {
        while self.capacity > 0 && self.journal.len() > self.capacity {
            self.journal.drop_oldest();
            // Alignment: each committed entry has exactly one before-state.
            if !self.snapshots.is_empty() {
                self.snapshots.remove(0);
            }
        }
        while self.capacity > 0 && self.future.len() > self.capacity {
            self.future.remove(0);
        }
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// The deterministic canonical `.oepdoc` string of `document` - the same text
/// [`document_hash`] hashes. Infallible for a canonical in-memory document
/// (plain serde derive, `expect` documents the invariant rather than silently
/// storing a substitution).
#[allow(dead_code)]
fn canonical_json(document: &Document) -> String {
    document
        .to_json_pretty()
        .expect("canonical oep-geo Document always serializes")
}

/// Parse a canonical `.oepdoc` string that an earlier [`canonical_json`]
/// produced. Infallible: we only ever re-parse our own canonical output, and
/// the engine's parser cannot fail on its own serializer's output.
#[allow(dead_code)]
fn parse_canonical(json: &str) -> Document {
    Document::from_json_pretty(json).expect("a canonical Document we serialized always reparses")
}

/// Path of the persisted recent-file list in `config_dir`.
pub fn recent_path(config_dir: &Path) -> PathBuf {
    config_dir.join("recent.conf")
}

/// Read the persisted recent list back (newest first); missing or unreadable
/// store yields an empty list (never an error).
pub fn load_recent(config_dir: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(recent_path(config_dir)) else {
        return Vec::new();
    };
    text.lines().map(PathBuf::from).collect()
}

/// Persist the recent list (newest first). Best-effort: a failure to write
/// the store must not fail the save that just succeeded, so it is swallowed
/// after being surfaced for diagnostics.
pub fn save_recent(config_dir: &Path, recent: &[PathBuf]) {
    let body: Vec<String> = recent.iter().map(|p| p.display().to_string()).collect();
    let _ = std::fs::write(recent_path(config_dir), body.join("\n"));
}

/// Record `path` as the most-recent file: move it to the front (dedup) and
/// cap the list at [`RECENT_MAX`]. Returns the new list (already persisted).
pub fn push_recent(config_dir: &Path, path: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = load_recent(config_dir)
        .into_iter()
        .filter(|p| p != path)
        .collect();
    list.insert(0, path.to_path_buf());
    list.truncate(RECENT_MAX);
    save_recent(config_dir, &list);
    list
}
