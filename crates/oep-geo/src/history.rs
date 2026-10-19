//! P6-M3-T02: topology undo/redo stack, serializable.
//!
//! A bounded undo/redo history over an arbitrary serializable "state record"
//! ([`HistoryState`]). The model is a single ordered vector of committed states
//! plus a cursor (a linear, branch-capable history):
//!
//! - [`History::commit`] appends a state at the cursor and **truncates the
//!   redo branch** (classic CAD rule: a fresh edit invalidates redo).
//! - [`History::undo`] walks the cursor back one committed state and returns
//!   the newly-current state; [`History::redo`] walks forward. Both return
//!   `None` when no move is possible (already at the oldest / newest).
//! - Capacity is optional (`0` = unbounded). When bounded, a commit that would
//!   exceed capacity drops the OLDEST states from the front, so memory stays
//!   bounded while the most recent history (and the current state) survive.
//!
//! The stack is serializable in our own documented text format: `oep-history`
//! v1 (see [`History::to_text`] / [`History::from_text`]). The whole stack -
//! capacity, cursor, every state on both branches - round-trips byte-identically
//! (locked by the committed golden and independently re-encoded by the Python
//! acceptance test), so an editing session's undo/redo history can be
//! persisted and restored (P6-M3-T02: "Topology undo/redo stack, serializable").

/// The canonical `format` tag of the serialized history.
pub const HISTORY_FORMAT: &str = "oep-history";
/// The serialization schema version (v1).
pub const HISTORY_VERSION: u32 = 1;

/// A single committed state record the history can persist.
///
/// Implementations define a deterministic, strict `encode`/`decode` pair.
/// `decode` must return `None` (never panic) on any input that `encode`
/// cannot produce (tolerant-failure doctrine).
pub trait HistoryState: Clone + PartialEq {
    /// The state's revision label.
    fn revision(&self) -> u32;
    /// A short, human-readable operation label for the edit that produced it.
    fn op(&self) -> &str;
    /// Deterministic canonical text form of this state record.
    fn encode(&self) -> String;
    /// Strictly parse `encode`d text back into a record; `None` on any
    /// malformed, non-finite or structurally invalid input.
    fn decode(text: &str) -> Option<Self>;
}

/// A bounded, serializable topology undo/redo stack.
///
/// See the module docs for the model and the commit/undo/redo semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct History<S: HistoryState> {
    /// Maximum number of retained states; `0` = unbounded.
    capacity: usize,
    /// Index of the current state in `states`.
    cursor: usize,
    /// Committed states, oldest first. `states[cursor]` is the current state.
    states: Vec<S>,
}

/// Iterator over the committed states, oldest first.
#[derive(Clone, Debug)]
pub struct States<'a, S> {
    iter: std::slice::Iter<'a, S>,
}

impl<'a, S> Iterator for States<'a, S> {
    type Item = &'a S;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<S: HistoryState> History<S> {
    /// A new empty history. `capacity == 0` means unbounded; otherwise at most
    /// `capacity` states are retained, with the oldest dropped on overflow.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cursor: 0,
            states: Vec::new(),
        }
    }

    /// The capacity bound (`0` = unbounded).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of committed states currently retained.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// True when the history holds no states.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// The cursor index (`states[cursor]` is the current state).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The current state, if any.
    pub fn current(&self) -> Option<&S> {
        self.states.get(self.cursor)
    }

    /// True when [`History::undo`] would move to an older state.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// True when [`History::redo`] would move to a newer state.
    pub fn can_redo(&self) -> bool {
        !self.states.is_empty() && self.cursor + 1 < self.states.len()
    }

    /// Iterator over the committed states, oldest first.
    pub fn states(&self) -> States<'_, S> {
        States {
            iter: self.states.iter(),
        }
    }

    /// Record a fresh committed state.
    ///
    /// The redo branch (anything newer than the cursor) is discarded, then the
    /// state is appended and becomes current. If bounded and this would exceed
    /// `capacity`, the oldest states are dropped from the front first.
    pub fn commit(&mut self, state: S) {
        // Bound memory first: make room for one more state by dropping the
        // oldest, but never below 0 and never the entire history.
        if self.capacity > 0 {
            let overflow = (self.states.len() + 1).saturating_sub(self.capacity);
            let drop = overflow.min(self.states.len());
            if drop > 0 {
                self.states.drain(..drop);
                self.cursor = self.cursor.saturating_sub(drop);
            }
        }
        // Discard the redo branch, then append and point the cursor at it.
        self.states.truncate(self.cursor + 1);
        self.states.push(state);
        self.cursor = self.states.len() - 1;
    }

    /// Walk the cursor back one state and return the newly-current state, or
    /// `None` when already at the oldest (no move is performed).
    pub fn undo(&mut self) -> Option<&S> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.states.get(self.cursor)
    }

    /// Walk the cursor forward one state and return the newly-current state,
    /// or `None` when already at the newest (no move is performed).
    pub fn redo(&mut self) -> Option<&S> {
        if self.states.is_empty() || self.cursor + 1 >= self.states.len() {
            return None;
        }
        self.cursor += 1;
        self.states.get(self.cursor)
    }

    /// Empty the history (all states dropped, cursor reset).
    pub fn clear(&mut self) {
        self.states.clear();
        self.cursor = 0;
    }

    /// Serialize the whole stack to our own `oep-history` v1 text format.
    ///
    /// Deterministic: an identical history always produces identical bytes
    /// (locked by the committed golden and the Python re-encode cross-check).
    pub fn to_text(&self) -> Option<String> {
        let mut out = String::new();
        out.push_str(&format!("format {HISTORY_FORMAT}\n"));
        out.push_str(&format!("version {HISTORY_VERSION}\n"));
        out.push_str(&format!("capacity {}\n", self.capacity));
        out.push_str(&format!("cursor {}\n", self.cursor));
        out.push_str(&format!("states {}\n", self.states.len()));
        for (i, s) in self.states.iter().enumerate() {
            out.push_str(&format!("state {i}\n"));
            out.push_str(s.encode().as_str());
        }
        Some(out)
    }

    /// Strictly parse `oep-history` v1 text back into a [`History`].
    ///
    /// Returns `None` (never panics) on malformed headers, a state/count
    /// mismatch, out-of-range cursor, or any state record that fails
    /// [`HistoryState::decode`].
    pub fn from_text(text: &str) -> Option<Self> {
        let mut lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let format = lines.first()?.strip_prefix("format ")?;
        if format != HISTORY_FORMAT {
            return None;
        }
        let version: u32 = lines.get(1)?.strip_prefix("version ")?.parse().ok()?;
        if version != HISTORY_VERSION {
            return None;
        }
        let capacity: usize = lines.get(2)?.strip_prefix("capacity ")?.parse().ok()?;
        let cursor: usize = lines.get(3)?.strip_prefix("cursor ")?.parse().ok()?;
        let declared: usize = lines.get(4)?.strip_prefix("states ")?.parse().ok()?;
        lines.drain(..5);

        let mut states: Vec<S> = Vec::new();
        for _ in 0..declared {
            let idx_marker = lines.first()?;
            let idx: usize = idx_marker.strip_prefix("state ")?.parse().ok()?;
            if idx != states.len() {
                return None;
            }
            // Collect this state's body until the next "state " marker.
            let mut body: Vec<&str> = Vec::new();
            lines.remove(0);
            while let Some(line) = lines.first() {
                if line.starts_with("state ") {
                    break;
                }
                body.push(lines.remove(0));
            }
            if body.is_empty() {
                return None;
            }
            let rec = S::decode(&body.join("\n"))?;
            states.push(rec);
        }

        // No trailing junk may remain.
        if !lines.is_empty() {
            return None;
        }
        if cursor > states.len() {
            return None;
        }
        if !states.is_empty() && cursor >= states.len() {
            return None;
        }
        Some(Self {
            capacity,
            cursor,
            states,
        })
    }
}
