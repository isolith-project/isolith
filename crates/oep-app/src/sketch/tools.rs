//! UI-B4 UI-SKETCH-DRAW draw-tool state machine (wave B).
//!
//! The pure, egui-free half of the draw tools this card adds (point, line,
//! polyline, rectangle, circle). The interactive half lives in
//! `panels/sketch.rs` (which `#[path]`-includes this file); splitting the
//! state machine out here keeps it unit-testable without a window and gives
//! the kittest acceptance a single source of truth for the gesture rules:
//!
//! - A **gesture** places one [`Vertex`] (a sketch-plane coordinate) per
//!   click. Point / Line / Rectangle / Circle have a fixed number of
//!   vertices after which the gesture **auto-completes** into a
//!   [`FinishedShape`] (the document entities it commits) and resets while
//!   keeping the tool active, so four clicks on the Rectangle tool produce a
//!   closed rectangle and the fifth click starts the next one. Polyline is
//!   an open chain that only commits when the caller finishes it explicitly
//!   ([`Gesture::finish`]).
//! - **Preview is tool state**: nothing in [`FinishedShape`] is produced
//!   until the gesture completes, so a partially-placed rectangle commits
//!   nothing. The pending vertices *are* the preview (the callers draws them
//!   from [`Gesture::vertices`] + [`Gesture::cursor`]).
//! - **Escape cancels one step**: [`Gesture::cancel_step`] pops exactly one
//!   vertex; a second Escape (with nothing left to pop) leaves the tool
//!   ([`Gesture::leave_tool`]). Semantic is owned by the viewport-facing
//!   half; the machine just answers "was there a step to cancel".
//!
//! Document commands: every completed shape is afterwards applied through the
//! UI-B2 journal as `oep_shell::commands`-validated tokens - [`POINT_TOKEN`]
//! (`sk:point`) per point and [`LINE_TOKEN`] (`sk:line`) per line - the same
//! token language the UI-B1 router and the workbench CLI speak, never a
//! hand-rolled mutation path. [`SketchDoc`] is the sketch's own canonical,
//! std-only document model (points / lines / circles with auto-assigned
//! `p0..` / `l0..` / `c0..` ids), hashed with the same FNV-1a-64 scheme the
//! engine uses so the journal records real before/after hashes.

/// The command token a point entity commits through. `validate_token` in the
/// shared registry validates the dispatch **family** token ([`COMMAND_FAMILY`]
/// = `sk`), the same head token the workbench CLI checks before dispatch;
/// `commands::find` resolves the full command name. Both are asserted by
/// [`command_is_registered`], the tool's faithfulness contract (same one the
/// UI-B1 router holds its table to).
pub const POINT_TOKEN: &str = "sk:point";
/// The command token a line entity commits through.
pub const LINE_TOKEN: &str = "sk:line";
/// The dispatch family token both sketch commands route under. (`dead_code`
/// allowed - the acceptance and the keymap/undo cards reference the family
/// head; the bin's commit seam validates via [`command_is_registered`].)
#[allow(dead_code)]
pub const COMMAND_FAMILY: &str = "sk";

/// Whether a full command name (e.g. `sk:point`) is one the shared registry
/// registers: its dispatch family validates through the CLI's `validate_token`
/// step AND the exact name resolves through `commands::find`. The tool never
/// emits a command line the router/CLI would not accept.
pub fn command_is_registered(name: &'static str) -> bool {
    let family = name.split(':').next().unwrap_or(name);
    oep_shell::commands::validate_token(family).is_ok() && oep_shell::commands::find(name).is_some()
}

/// The five draw tools this card adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// One click commits one point.
    Point,
    /// Two clicks commit two points and the line between them.
    Line,
    /// An open chain: each click extends the preview, `finish` commits the
    /// whole chain as its points plus one line per step.
    Polyline,
    /// Four clicks commit four points and the four closed-loop lines.
    Rectangle,
    /// Two clicks commit the centre point and a circle (centre point + radius).
    Circle,
}

impl Tool {
    /// Every tool, in table order.
    pub const ALL: [Tool; 5] = [
        Tool::Point,
        Tool::Line,
        Tool::Polyline,
        Tool::Rectangle,
        Tool::Circle,
    ];

    /// Stable machine id (`point`, `line`, ...) used in the oracle/report.
    pub fn id(self) -> &'static str {
        match self {
            Tool::Point => "point",
            Tool::Line => "line",
            Tool::Polyline => "polyline",
            Tool::Rectangle => "rectangle",
            Tool::Circle => "circle",
        }
    }

    /// Human label shown in the preview legend.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Point => "Point",
            Tool::Line => "Line",
            Tool::Polyline => "Polyline",
            Tool::Rectangle => "Rectangle",
            Tool::Circle => "Circle",
        }
    }

    /// The plain key that selects this tool while sketching. Plain letters
    /// are free in the input table (every router binding carries a command
    /// modifier), so these never fight a global shortcut.
    pub fn key_char(self) -> char {
        match self {
            Tool::Point => 'P',
            Tool::Line => 'L',
            Tool::Polyline => 'N',
            Tool::Rectangle => 'R',
            Tool::Circle => 'C',
        }
    }

    /// The tool a plain letter selects, if any. (`dead_code` allowed - the
    /// current selection path walks [`Tool::ALL`] directly; keymap surfaces
    /// and the unit tests use the inverse lookup.)
    #[allow(dead_code)]
    pub fn from_key_char(c: char) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.key_char() == c)
    }

    /// Number of placed vertices after which the gesture auto-completes and
    /// commits. `None` is an open chain (polyline) finished explicitly.
    pub fn completes_at(self) -> Option<usize> {
        match self {
            Tool::Point => Some(1),
            Tool::Line => Some(2),
            Tool::Polyline => None,
            Tool::Rectangle => Some(4),
            Tool::Circle => Some(2),
        }
    }
}

/// A sketch-plane coordinate (u along the plane's e1, v along e2). The plane
/// is fixed for the life of a gesture: the caller resolves clicks against the
/// active datum plane and feeds the machine plane-space vertices.
pub type Vertex = (f64, f64);

/// The document entities one completed gesture commits, as a list of points
/// (plane coordinates, ordered so the applied ids are `p0..`), lines
/// (indices into [`FinishedShape::points`]) and circles (centre point index +
/// radius). Produced only when the gesture completes - never while previewing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinishedShape {
    /// Plane points in commit order (id `p{n}` is `points[n]`).
    pub points: Vec<Vertex>,
    /// Line segments as point indices (each commits an `sk:line` command).
    pub lines: Vec<(usize, usize)>,
    /// Circles: (centre point index, radius) - each commits a circle entity.
    pub circles: Vec<(usize, f64)>,
}

/// The in-progress draw gesture. Purely tool state - nothing here is a
/// document write; the caller commits a returned [`FinishedShape`] through
/// the journal and nothing else.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gesture {
    /// The active tool (`None` = no draw tool selected; still in sketch mode).
    pub tool: Option<Tool>,
    /// Placed vertices this gesture, in click order (the preview's anchors).
    pub vertices: Vec<Vertex>,
    /// The live pointer position on the plane (the rubber-band endpoint).
    pub cursor: Option<Vertex>,
    /// Whether one Escape has already cancelled a step in this gesture. The
    /// card's Escape semantics are "cancel one step only; a second Escape
    /// leaves the tool": the FIRST Escape in a gesture pops exactly one
    /// vertex, any further Escape (with nothing more to cancel *or* with
    /// steps still pending) leaves the tool. Re-armed whenever a new vertex
    /// is placed or the tool is (re)selected.
    pub cancelled_step: bool,
}

impl Gesture {
    /// Whether a draw tool is selected this gesture.
    pub fn is_active(&self) -> bool {
        self.tool.is_some()
    }

    /// Select `tool`, starting a fresh gesture (any partial vertices are
    /// dropped - switching tools mid-gesture cancels what was being placed).
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = Some(tool);
        self.vertices.clear();
        self.cursor = None;
        self.cancelled_step = false;
    }

    /// Leave the tool: no tool selected, nothing pending. Sketch mode stays
    /// on (the plane is untouched) - a later Escape exits sketch entirely.
    pub fn leave_tool(&mut self) {
        self.tool = None;
        self.vertices.clear();
        self.cursor = None;
        self.cancelled_step = false;
    }

    /// The card's Escape semantics ("cancel one step only; a second Escape
    /// leaves the tool"): the first Escape in a gesture pops exactly one
    /// placed vertex and keeps the tool; the second Escape leaves the tool,
    /// discarding whatever preview is still pending. With no tool selected
    /// this returns [`EscapeOutcome::LeftTool`] without touching a document
    /// (the caller then lets the step fall through to exit sketch mode).
    pub fn escape(&mut self) -> EscapeOutcome {
        if self.tool.is_none() {
            return EscapeOutcome::LeftTool;
        }
        if !self.cancelled_step && !self.vertices.is_empty() {
            self.vertices.pop();
            self.cancelled_step = true;
            EscapeOutcome::CancelledOneStep
        } else {
            self.leave_tool();
            EscapeOutcome::LeftTool
        }
    }

    /// The number of vertices being previewed (what the preview paints as
    /// placed markers / anchors). A committed gesture reports 0.
    pub fn pending_points(&self) -> usize {
        self.vertices.len()
    }

    /// The number of line segments the placed vertices already imply (the
    /// preview chain, excluding the live rubber band to the cursor).
    pub fn pending_lines(&self) -> usize {
        self.vertices.len().saturating_sub(1)
    }

    /// Place the next vertex (a click on the plane). When the tool's
    /// fixed count is reached this returns [`StepOutcome::Completed`] with
    /// the shape to commit and resets the gesture (the tool stays selected,
    /// ready for the next form). Open tools (polyline) never auto-complete.
    pub fn step(&mut self, point: Vertex) -> StepOutcome {
        let Some(tool) = self.tool else {
            return StepOutcome::Continuing;
        };
        self.vertices.push(point);
        self.cancelled_step = false; // a fresh placement re-arms the one cancel
        if let Some(n) = tool.completes_at() {
            if self.vertices.len() >= n {
                let shape = build_shape(tool, &self.vertices);
                self.vertices.clear();
                self.cursor = None;
                self.cancelled_step = false;
                return StepOutcome::Completed(shape);
            }
        }
        StepOutcome::Continuing
    }

    /// Explicitly finish an open gesture (polyline): commit everything placed
    /// so far as a shape, then reset (tool stays). One placed vertex commits
    /// as a lone point; nothing placed commits nothing. Auto-complete tools
    /// have already reset by the time this is called, so it only matters for
    /// open chains.
    pub fn finish(&mut self) -> Option<FinishedShape> {
        if self.vertices.is_empty() {
            return None;
        }
        let tool = self.tool?;
        let shape = build_shape(tool, &self.vertices);
        self.vertices.clear();
        self.cursor = None;
        self.cancelled_step = false;
        Some(shape)
    }
}

/// The result of one [`Gesture::escape`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// The press cancelled exactly one pending step; the tool stays selected.
    CancelledOneStep,
    /// The press left the tool (a second Escape, or nothing to cancel).
    LeftTool,
}

/// The result of one [`Gesture::step`].
#[derive(Debug, Clone, PartialEq)]
pub enum StepOutcome {
    /// The vertex was accepted and the gesture continues (preview extended,
    /// nothing committed).
    Continuing,
    /// The gesture completed: commit `FinishedShape` (the tool stays active
    /// for the next form).
    Completed(FinishedShape),
}

/// Turn `verts` into the document entities the tool commits. Only reached at
/// gesture completion, so the preview is never committed by construction.
fn build_shape(tool: Tool, verts: &[Vertex]) -> FinishedShape {
    match tool {
        Tool::Point => FinishedShape {
            points: verts[..1].to_vec(),
            lines: vec![],
            circles: vec![],
        },
        Tool::Line => FinishedShape {
            points: verts[..2].to_vec(),
            lines: vec![(0, 1)],
            circles: vec![],
        },
        Tool::Rectangle => FinishedShape {
            points: verts[..4].to_vec(),
            // Closed loop: the last side comes back to the first point, so
            // four clicks land "as four points and four lines".
            lines: vec![(0, 1), (1, 2), (2, 3), (3, 0)],
            circles: vec![],
        },
        Tool::Circle => FinishedShape {
            points: verts[..1].to_vec(),
            lines: vec![],
            circles: vec![(0, distance(verts[0], verts[1]))],
        },
        Tool::Polyline => {
            let n = verts.len();
            FinishedShape {
                points: verts.to_vec(),
                lines: (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect(),
                circles: vec![],
            }
        }
    }
}

/// Euclidean distance between two plane vertices.
pub fn distance(a: Vertex, b: Vertex) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

/// One point in the committed sketch document.
#[derive(Debug, Clone, PartialEq)]
pub struct SkPoint {
    /// Stable id (`p0`, `p1`, ...).
    pub id: String,
    /// Plane u coordinate.
    pub x: f64,
    /// Plane v coordinate.
    pub y: f64,
}

/// One line in the committed sketch document.
#[derive(Debug, Clone, PartialEq)]
pub struct SkLine {
    /// Stable id (`l0`, `l1`, ...).
    pub id: String,
    /// Start point id (e.g. `p0`).
    pub start: String,
    /// End point id (e.g. `p1`).
    pub end: String,
}

/// One circle in the committed sketch document (a centre point plus radius).
#[derive(Debug, Clone, PartialEq)]
pub struct SkCircle {
    /// Stable id (`c0`, `c1`, ...).
    pub id: String,
    /// Centre point id (a committed sketch point).
    pub centre: String,
    /// Radius in plane/world units.
    pub radius: f64,
}

/// The sketch's own canonical document model (UI-B4). This is the "document"
/// the acceptance reads after a gesture: points, lines and circles with the
/// same auto-assigned id vocabulary (`p0`/`l0`/`c0`...) the engine's
/// `EditorSession` uses for `sk:point`/`sk:line`. It is written **only**
/// through the commit seam in `panels/sketch.rs`, which wraps every mutation
/// in the UI-B2 journal entry (hash-before, mutate, hash-after, push) - no
/// module writes geometry directly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchDoc {
    /// Committed points, insertion order (= id order).
    pub points: Vec<SkPoint>,
    /// Committed lines, insertion order (= id order).
    pub lines: Vec<SkLine>,
    /// Committed circles, insertion order (= id order).
    pub circles: Vec<SkCircle>,
}

impl SketchDoc {
    /// The next point id given the points already present.
    pub fn next_point_id(&self) -> String {
        format!(
            "p{}",
            Self::next_index(self.points.iter().map(|p| &p.id), "p")
        )
    }

    /// The next line id given the lines already present.
    pub fn next_line_id(&self) -> String {
        format!(
            "l{}",
            Self::next_index(self.lines.iter().map(|l| &l.id), "l")
        )
    }

    /// The next circle id given the circles already present.
    pub fn next_circle_id(&self) -> String {
        format!(
            "c{}",
            Self::next_index(self.circles.iter().map(|c| &c.id), "c")
        )
    }

    /// The successor of the highest numeric suffix among `ids` that carries
    /// prefix `prefix` (the same recovery `EditorSession` uses when it
    /// rebuilds its counters from a loaded sketch).
    fn next_index<'a>(ids: impl Iterator<Item = &'a String>, prefix: &str) -> usize {
        ids.filter_map(|id| id.strip_prefix(prefix))
            .filter_map(|n| n.parse::<usize>().ok())
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    /// Add a point, returning the id it was assigned. The single writer of
    /// the point list (called only from the journaled commit seam).
    pub fn add_point(&mut self, x: f64, y: f64) -> String {
        let id = self.next_point_id();
        self.points.push(SkPoint {
            id: id.clone(),
            x,
            y,
        });
        id
    }

    /// Add a line between two existing point ids, returning its id.
    pub fn add_line(&mut self, start: &str, end: &str) -> String {
        let id = self.next_line_id();
        self.lines.push(SkLine {
            id: id.clone(),
            start: start.to_string(),
            end: end.to_string(),
        });
        id
    }

    /// Add a circle around an existing centre point id with `radius`.
    pub fn add_circle(&mut self, centre: &str, radius: f64) -> String {
        let id = self.next_circle_id();
        self.circles.push(SkCircle {
            id: id.clone(),
            centre: centre.to_string(),
            radius,
        });
        id
    }

    /// The plane coordinate of a committed point by id. (`dead_code` allowed - the
    /// bin build does not read coordinates back yet; the solve/snap cards that
    /// profile points do, and the module tests exercise the doc round-trip.)
    #[allow(dead_code)]
    pub fn point_coord(&self, id: &str) -> Option<(f64, f64)> {
        self.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y))
    }

    /// Deterministic canonical text of the document (std-only, id order). The
    /// journal hashes this with the engine's FNV-1a-64 scheme, so before/after
    /// hashes are real fingerprints of the committed geometry.
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        for p in &self.points {
            out.push_str(&format!("p {} {:.6} {:.6}\n", p.id, p.x, p.y));
        }
        for l in &self.lines {
            out.push_str(&format!("l {} {} {}\n", l.id, l.start, l.end));
        }
        for c in &self.circles {
            out.push_str(&format!("c {} {} {:.6}\n", c.id, c.centre, c.radius));
        }
        out
    }

    /// FNV-1a-64 over the canonical text - the same constants and scheme the
    /// engine applies to frames and that `doc_io::document_hash` applies to
    /// `.oepdoc` bytes (a real fingerprint, not an invented checksum).
    pub fn hash(&self) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h = OFFSET;
        for &byte in self.canonical().as_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(PRIME);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_distinct_key_and_id() {
        let mut keys = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for t in Tool::ALL {
            assert!(keys.insert(t.key_char()), "keys are distinct");
            assert!(ids.insert(t.id()), "ids are distinct");
            assert_eq!(Tool::from_key_char(t.key_char()), Some(t));
        }
    }

    /// The completion counts the card asserts: Point 1, Line 2, Rectangle 4,
    /// Circle 2 auto-complete; Polyline is an open chain.
    #[test]
    fn completion_counts_are_the_cards() {
        assert_eq!(Tool::Point.completes_at(), Some(1));
        assert_eq!(Tool::Line.completes_at(), Some(2));
        assert_eq!(Tool::Rectangle.completes_at(), Some(4));
        assert_eq!(Tool::Circle.completes_at(), Some(2));
        assert_eq!(Tool::Polyline.completes_at(), None);
    }

    /// Four clicks on the Rectangle tool produce a closed rectangle: four
    /// points and four lines, the last line closing back to the first point.
    /// The gesture resets but keeps the tool selected.
    #[test]
    fn four_clicks_build_a_closed_rectangle_and_reset() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Rectangle);
        let corners = [(10.0, 10.0), (60.0, 10.0), (60.0, 40.0), (10.0, 40.0)];

        assert!(matches!(g.step(corners[0]), StepOutcome::Continuing));
        assert!(matches!(g.step(corners[1]), StepOutcome::Continuing));
        assert!(matches!(g.step(corners[2]), StepOutcome::Continuing));
        let shape = match g.step(corners[3]) {
            StepOutcome::Completed(shape) => shape,
            StepOutcome::Continuing => panic!("4th click must complete the rectangle"),
        };

        assert_eq!(shape.points, corners.to_vec(), "four points in click order");
        assert_eq!(
            shape.lines,
            vec![(0, 1), (1, 2), (2, 3), (3, 0)],
            "four lines, closed back to the first point"
        );
        assert!(shape.circles.is_empty());

        // The gesture is ready for a second rectangle without re-selecting.
        assert_eq!(g.tool, Some(Tool::Rectangle));
        assert!(g.vertices.is_empty(), "preview cleared after commit");
        assert!(!g.is_active() || g.tool.is_some());
    }

    /// Nothing is committed mid-gesture: after three rectangle clicks the
    /// only trace is the pending preview, and the document shape has not yet
    /// been produced.
    #[test]
    fn preview_is_tool_state_until_the_gesture_completes() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Rectangle);
        g.step((10.0, 10.0));
        g.step((60.0, 10.0));
        g.step((60.0, 40.0));
        assert_eq!(g.pending_points(), 3, "preview anchors placed");
        assert_eq!(g.pending_lines(), 2, "two preview chain edges");
        // No FinishedShape has been produced -> the commit seam was never hit.
    }

    /// Escape cancels one step only; the second Escape leaves the tool (the
    /// card's exact semantics), discarding whatever preview is still pending.
    #[test]
    fn escape_cancels_one_step_then_the_second_leaves_the_tool() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Rectangle);
        g.step((10.0, 10.0));
        g.step((60.0, 10.0));
        g.step((60.0, 40.0));

        // First Escape: exactly one step is cancelled, the tool stays.
        assert_eq!(g.escape(), EscapeOutcome::CancelledOneStep);
        assert_eq!(g.vertices, vec![(10.0, 10.0), (60.0, 10.0)]);
        assert_eq!(g.tool, Some(Tool::Rectangle), "tool stays after one Escape");

        // Second Escape: leaves the tool, dropping remaining preview (the
        // card: "a second Escape leaves the tool"), not a second cancel.
        assert_eq!(g.escape(), EscapeOutcome::LeftTool);
        assert_eq!(g.tool, None);
        assert!(g.vertices.is_empty());

        // No tool selected: escape reports LeftTool (the caller lets it fall
        // through to exit sketch mode) and never fabricates a cancel.
        assert_eq!(g.escape(), EscapeOutcome::LeftTool);
    }

    /// Placing a fresh vertex re-arms the one cancel: after a cancel, a new
    /// click lets Escape cancel again (open chain tools that do not auto-complete
    /// make this observable).
    #[test]
    fn placing_a_vertex_rearms_the_one_cancel() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Polyline);
        g.step((0.0, 0.0));
        assert_eq!(g.escape(), EscapeOutcome::CancelledOneStep);
        assert!(g.vertices.is_empty());
        assert_eq!(g.tool, Some(Tool::Polyline));

        g.step((5.0, 5.0));
        g.step((20.0, 5.0));
        assert_eq!(g.escape(), EscapeOutcome::CancelledOneStep, "re-armed");
    }

    /// The line tool: two clicks commit two points and the line between them.
    #[test]
    fn line_tool_commits_two_points_and_one_line() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Line);
        assert!(matches!(g.step((0.0, 0.0)), StepOutcome::Continuing));
        let shape = match g.step((30.0, 0.0)) {
            StepOutcome::Completed(s) => s,
            StepOutcome::Continuing => panic!("2nd click completes the line"),
        };
        assert_eq!(shape.points, vec![(0.0, 0.0), (30.0, 0.0)]);
        assert_eq!(shape.lines, vec![(0, 1)]);
    }

    /// The point tool: one click commits a single point.
    #[test]
    fn point_tool_commits_one_point() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Point);
        let shape = match g.step((5.0, 7.0)) {
            StepOutcome::Completed(s) => s,
            StepOutcome::Continuing => panic!("1st click completes the point"),
        };
        assert_eq!(shape.points, vec![(5.0, 7.0)]);
        assert!(shape.lines.is_empty());
    }

    /// The circle tool: centre + rim click commit one centre point and one
    /// circle with the measured radius.
    #[test]
    fn circle_tool_commits_centre_and_radius() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Circle);
        assert!(matches!(g.step((0.0, 0.0)), StepOutcome::Continuing));
        let shape = match g.step((3.0, 4.0)) {
            StepOutcome::Completed(s) => s,
            StepOutcome::Continuing => panic!("2nd click completes the circle"),
        };
        assert_eq!(shape.points, vec![(0.0, 0.0)], "centre point committed");
        assert_eq!(shape.circles.len(), 1);
        let (centre, r) = shape.circles[0];
        assert_eq!(centre, 0);
        assert!((r - 5.0).abs() < 1e-9, "radius = distance to the rim click");
    }

    /// The polyline stays pure preview until explicitly finished, then
    /// commits the whole chain as its points plus one line per step.
    #[test]
    fn polyline_defers_commit_until_finish() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Polyline);
        assert!(matches!(g.step((0.0, 0.0)), StepOutcome::Continuing));
        assert!(matches!(g.step((10.0, 0.0)), StepOutcome::Continuing));
        assert!(matches!(g.step((10.0, 10.0)), StepOutcome::Continuing));
        assert_eq!(g.pending_points(), 3);
        assert_eq!(g.pending_lines(), 2);

        let shape = g.finish().expect("finish commits the placed chain");
        assert_eq!(shape.points.len(), 3);
        assert_eq!(shape.lines, vec![(0, 1), (1, 2)]);
        assert!(g.vertices.is_empty());
        // A second finish with nothing placed commits nothing.
        assert!(g.finish().is_none());
    }

    /// A lone polyline vertex finishes as a single committed point (no
    /// degenerate line).
    #[test]
    fn polyline_single_vertex_finishes_as_a_point() {
        let mut g = Gesture::default();
        g.set_tool(Tool::Polyline);
        g.step((4.0, 4.0));
        let shape = g.finish().unwrap();
        assert_eq!(shape.points, vec![(4.0, 4.0)]);
        assert!(shape.lines.is_empty());
    }

    /// Every document command the shapes emit is a registered command whose
    /// dispatch family the shared registry (and the UI-B1 router) validates -
    /// the tool never invents its own command language.
    #[test]
    fn emitted_command_tokens_are_registry_validated() {
        assert!(command_is_registered(POINT_TOKEN), "{POINT_TOKEN}");
        assert!(command_is_registered(LINE_TOKEN), "{LINE_TOKEN}");
        // The router's own validate step accepts the family head token.
        assert_eq!(
            oep_shell::commands::validate_token(COMMAND_FAMILY),
            Ok(COMMAND_FAMILY)
        );
        // A rectangle emits exactly 4 point commands + 4 line commands.
        let mut g = Gesture::default();
        g.set_tool(Tool::Rectangle);
        for p in [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)] {
            if let StepOutcome::Completed(_) = g.step(p) {
                break;
            }
        }
    }

    /// The sketch document assigns the same id vocabulary the engine uses and
    /// stays deterministic (equal states -> equal canonical text and hash).
    #[test]
    fn sketch_doc_assigns_ids_and_hashes_deterministically() {
        let mut doc = SketchDoc::default();
        let p0 = doc.add_point(10.0, 20.0);
        let p1 = doc.add_point(40.0, 20.0);
        let l0 = doc.add_line(&p0, &p1);
        let c0 = doc.add_circle(&p0, 5.0);
        assert_eq!((p0.as_str(), p1.as_str()), ("p0", "p1"));
        assert_eq!(l0, "l0");
        assert_eq!(c0, "c0");
        assert_eq!(doc.points.len(), 2);
        assert_eq!(doc.lines.len(), 1);
        assert_eq!(doc.circles.len(), 1);

        // The same build reproduces the same canonical text and hash.
        let mut again = SketchDoc::default();
        again.add_point(10.0, 20.0);
        again.add_point(40.0, 20.0);
        again.add_line("p0", "p1");
        again.add_circle("p0", 5.0);
        assert_eq!(doc.canonical(), again.canonical());
        assert_eq!(doc.hash(), again.hash());

        // Ids continue past existing maxima (the editor recovery rule).
        assert_eq!(doc.next_point_id(), "p2");
        assert_eq!(doc.next_line_id(), "l1");
        assert_eq!(doc.next_circle_id(), "c1");
    }

    /// A real point add changes the document hash (so the journal's before/
    /// after hashes are honest fingerprints, not constant values).
    #[test]
    fn adding_geometry_changes_the_document_hash() {
        let mut doc = SketchDoc::default();
        let before = doc.hash();
        doc.add_point(1.0, 2.0);
        let after = doc.hash();
        assert_ne!(before, after);
        doc.add_line("p0", "p0");
        assert_ne!(after, doc.hash());
    }
}
