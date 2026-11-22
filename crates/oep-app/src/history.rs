//! Command journal (wave B, card UI-B2 UI-HISTORY).
//!
//! The undo/redo record for the editor: an ordered journal of named commands,
//! each carrying the document hashes before and after, that coalesces a
//! continuous pointer drag into a single undo entry.
//!
//! D-UI-4 splits the undo machinery along the ownership line: the journal
//! owns ORDERING and COALESCING, never document state. An [`Entry`] is the
//! command token plus two hashes - the canonical document bytes those hashes
//! point at live in the routed mutation seam (`doc_io::EditHistory`), which
//! restores them verbatim on undo/redo. Because undo/redo are exact *hash*
//! contracts, the journal never needs to see a document.
//!
//! Coalescing rule (the card): consecutive commands with the same [`token`]
//! and the same [`DragId`] collapse into one entry until the gesture ends
//! ([`Journal::end_drag`]). The merged entry keeps the FIRST `before` and the
//! LAST `after`, so one pointer drag is exactly one undo step and undo lands
//! on the hash the drag started from.
//!
//! Memory bound: [`Journal::with_capacity`] keeps the newest `capacity`
//! entries and drops the oldest - 32-step semantics, parity with
//! `oep_shell::editor::HISTORY_CAPACITY` - and [`Journal::drop_oldest`] lets
//! an owner of the *state* enforce the same bound on its snapshot store in
//! lock-step (0 = unbounded).

/// Default undo capacity (parity with `oep_shell::editor::HISTORY_CAPACITY`).
pub const DEFAULT_CAPACITY: usize = 32;

/// Identifier of one continuous pointer gesture.
///
/// Consecutive commands pushed with the same `(token, drag id)` while the
/// gesture is open coalesce into one entry; a new or ended gesture opens a
/// fresh entry. The id is opaque to the journal - the viewport seam assigns
/// one per pointer-down-to-pointer-up gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DragId(pub u64);

/// One undoable command: the command token plus the document hashes before
/// and after the command ran.
///
/// A coalesced drag records only its first `before` and its last `after`,
/// so the whole gesture undoes in one step and redoes to the same hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The command token (by contract one `oep_shell::commands` validates -
    /// the router's obligation, per UI-B1; the journal only records).
    pub token: &'static str,
    /// Document hash before the command ran ([`crate::doc_io::document_hash`]).
    pub before: u64,
    /// Document hash after the command ran.
    pub after: u64,
}

/// The ordered, coalescing command journal (engine-side, egui-free).
///
/// Ordering and coalescing live here; document state does not (D-UI-4).
#[derive(Debug, Clone)]
pub struct Journal {
    /// Committed entries, oldest first: `entries[i].before` is the hash of
    /// the document state the `i`-th edit started from.
    entries: Vec<Entry>,
    /// Entries popped by [`Journal::undo`], newest first (the redo branch).
    redo: Vec<Entry>,
    /// The `(drag id, token)` of the entry currently open for coalescing -
    /// set by the last `push` when that push carried a drag id, cleared when
    /// the gesture ends, a discrete command intervenes, or undo/redo runs.
    open: Option<(DragId, &'static str)>,
    /// Maximum committed entries kept (0 = unbounded).
    capacity: usize,
}

impl Journal {
    /// A journal with the engine's default undo capacity ([`DEFAULT_CAPACITY`]).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// A journal that keeps at most `capacity` committed entries, dropping
    /// the oldest when exceeded (`0` keeps everything).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            redo: Vec::new(),
            open: None,
            capacity,
        }
    }

    /// Record one command.
    ///
    /// `drag` is `Some(DragId)` while a continuous pointer gesture is in
    /// progress, `None` for a discrete command. A command whose `(token,
    /// drag id)` matches the entry still open for coalescing extends that
    /// entry (its `after` is updated, no new entry is created), so `N` drag
    /// commands become one undo step; any other command commits a fresh
    /// entry.
    pub fn push(&mut self, token: &'static str, drag: Option<DragId>, before: u64, after: u64) {
        if self.would_coalesce(token, drag) {
            // Same token, same drag id, the gesture still open: extend the
            // open entry - the net effect runs from the first before to this
            // last after. No redo-truncation (this is not a fresh edit).
            if let Some(last) = self.entries.last_mut() {
                last.after = after;
            }
            return;
        }
        match drag {
            Some(id) => self.open = Some((id, token)),
            None => self.open = None,
        }
        self.push_entry(Entry {
            token,
            before,
            after,
        });
    }

    /// True when `push(token, drag, ..)` would extend the entry still open
    /// for coalescing instead of growing the journal (same token, same drag
    /// id, the gesture not yet ended).
    ///
    /// A caller can skip state capture for commands that only extend a drag:
    /// `doc_io::EditHistory::apply` uses this to avoid snapshotting between
    /// drag frames.
    pub fn would_coalesce(&self, token: &'static str, drag: Option<DragId>) -> bool {
        match drag {
            Some(id) => self.open == Some((id, token)) && self.entries.last().is_some(),
            None => false,
        }
    }

    /// Close the coalescing window for gesture `id`: a later command with the
    /// same token and drag id starts a fresh entry (the rule's "until the
    /// gesture ends"). No-op when no such window is open.
    pub fn end_drag(&mut self, id: DragId) {
        if let Some((open_id, _)) = self.open {
            if open_id == id {
                self.open = None;
            }
        }
    }

    /// The committed entries, oldest first (a coalesced drag counts once).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Number of committed entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the journal holds no committed entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The maximum number of committed entries kept (0 = unbounded).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether an undo step is available.
    pub fn can_undo(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Whether a redo step is available.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo the last committed edit: returns `Some(before)` - the document
    /// hash the caller must restore to - and moves the entry onto the redo
    /// branch.
    ///
    /// Exactness: the restored document must hash to the returned value
    /// (`doc_io::EditHistory` asserts this, so `hash before == hash after
    /// undo` holds by construction). An undo boundary also ends any gesture
    /// still open for coalescing.
    pub fn undo(&mut self) -> Option<u64> {
        self.open = None;
        let entry = self.entries.pop()?;
        let before = entry.before;
        self.redo.push(entry);
        Some(before)
    }

    /// Redo the last undone edit: returns `Some(after)` - the document hash
    /// the caller must restore to.
    ///
    /// Exactness: the restored document must hash to the returned value, so
    /// `hash after == hash after redo` holds by construction.
    pub fn redo(&mut self) -> Option<u64> {
        let entry = self.redo.pop()?;
        let after = entry.after;
        self.entries.push(entry);
        Some(after)
    }

    /// Drop the oldest committed entry and return it. The entry never lands
    /// on the one still open for coalescing: the open window is the newest
    /// entry, and this removes index 0.
    ///
    /// An owner of the document *state* calls this in lock-step when it
    /// enforces the same capacity bound on its snapshot store, so the journal
    /// and the states it points at stay aligned (D-UI-4 memory bound).
    pub fn drop_oldest(&mut self) -> Option<Entry> {
        if self.entries.is_empty() {
            return None;
        }
        Some(self.entries.remove(0))
    }

    /// Forget every entry and any open gesture (both branches).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.redo.clear();
        self.open = None;
    }

    /// Commit a fresh entry: a new edit invalidates the redo branch, and the
    /// capacity bound drops the oldest when exceeded.
    fn push_entry(&mut self, entry: Entry) {
        self.redo.clear();
        if self.capacity > 0 && self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DragId, Journal};

    const DRAG_A: DragId = DragId(0);
    const DRAG_B: DragId = DragId(1);
    const H: (u64, u64, u64, u64) = (100, 200, 300, 400);

    #[test]
    fn fifty_same_drag_commands_coalesce_into_one_entry() {
        let mut j = Journal::new();
        for i in 0..50 {
            j.push("view", Some(DRAG_A), H.0, 1000 + i);
        }
        assert_eq!(j.len(), 1, "50 drag commands collapse to 1 entry");
        let e = &j.entries()[0];
        assert_eq!(e.token, "view");
        assert_eq!(e.before, H.0, "first before kept");
        assert_eq!(e.after, 1000 + 49, "last after kept");
    }

    #[test]
    fn different_drag_id_opens_a_fresh_entry() {
        let mut j = Journal::new();
        j.push("view", Some(DRAG_A), H.0, H.1);
        j.push("view", Some(DRAG_B), H.1, H.2);
        assert_eq!(j.len(), 2, "a new gesture must not coalesce");
    }

    #[test]
    fn same_drag_id_after_end_drag_starts_a_new_entry() {
        let mut j = Journal::new();
        j.push("view", Some(DRAG_A), H.0, H.1);
        j.end_drag(DRAG_A);
        j.push("view", Some(DRAG_A), H.1, H.2);
        assert_eq!(j.len(), 2, "gesture ended: same id later is a new entry");
    }

    #[test]
    fn discrete_commands_never_coalesce() {
        let mut j = Journal::new();
        for i in 0..5 {
            j.push("point", None, 10 * i as u64, 10 * (i as u64 + 1));
        }
        assert_eq!(j.len(), 5, "five discrete commands are five entries");
    }

    #[test]
    fn undo_and_redo_return_the_exact_hashes() {
        let mut j = Journal::new();
        j.push("view", Some(DRAG_A), H.0, H.1);
        assert_eq!(j.undo(), Some(H.0), "undo returns the prior hash");
        assert_eq!(j.redo(), Some(H.1), "redo returns the same after hash");
        assert!(j.can_undo());
        assert!(!j.can_redo());
    }

    #[test]
    fn new_edit_after_undo_truncates_the_redo_branch() {
        let mut j = Journal::new();
        for i in 0..3 {
            j.push("point", None, i as u64, (i + 1) as u64);
        }
        assert_eq!(j.undo(), Some(2));
        assert!(j.can_redo());
        j.push("extrude", None, 5, 6);
        assert!(!j.can_redo(), "a fresh edit invalidates redo");
    }

    #[test]
    fn bounded_capacity_drops_the_oldest() {
        let mut j = Journal::with_capacity(2);
        for i in 0..=3 {
            j.push("point", None, (10 * i) as u64, (10 * (i + 1)) as u64);
        }
        assert_eq!(j.len(), 2);
        assert_eq!(
            j.entries().iter().map(|e| e.before).collect::<Vec<_>>(),
            vec![20, 30],
            "the two newest survive, the oldest dropped from the front"
        );
    }

    #[test]
    fn boundary_undo_at_bottom_and_redo_at_top_are_noops() {
        let mut j = Journal::new();
        assert!(j.undo().is_none());
        assert!(j.redo().is_none());
        j.push("point", None, 1, 2);
        assert!(j.redo().is_none());
    }
}
