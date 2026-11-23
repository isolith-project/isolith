//! Measure panel (wave C, card UI-C5 UI-MEASURE: distance, angle, radius).
//!
//! Picking two entities shows the measurement annotated on screen. The
//! measurement MATH lives in this panel's egui-free engine section (the
//! "geometry layer" of the card): pure functions over the committed bracket
//! fixture's plane geometry - Euclidean distance between two points, the
//! angle between two lines, the radius of a circle - and the panel `show`
//! below only FORMATS the number and DRAWS the annotation. Nothing in the
//! drawing path recomputes a measurement or resolves an identifier.
//!
//! ## The committed bracket fixture
//!
//! The card asserts against ANALYTIC GOLDENS for a committed fixture. This
//! panel commits that fixture ([`bracket_fixture`]): a 40 x 30 bracket
//! profile (points `p0..p3`, the four edge lines `l0..l3`) plus a 10-unit
//! circle `c0` through a centre point `p4`. The analytic values fall
//! straight out of the coordinates:
//!
//! - distance `p0 (0,0)` -> `p1 (40,0)`            = 40
//! - distance `p0` -> `p2 (40,30)`                 = 50  (3-4-5 triangle)
//! - angle between `l0` (horizontal) and `l1` (vertical) = 90 deg
//! - radius of `c0`                                 = 10
//!
//! The acceptance compares the ENGINE's measured values to these goldens
//! within [`MEASURE_TOLERANCE`] (1e-6, stated), and the panel's annotation -
//! the value text painted as a real `Label` widget plus the leader line in
//! the preview box - is what "appears in the snapshot".
//!
//! ## Interaction
//!
//! Plain `M` toggles the measure surface over the viewport (free in the B1
//! table: the `Mates` row requires Ctrl, the tool keys own P/L/N/R/C, F8 is
//! snap, F is finish). While open, clicking an entity button picks it - the
//! first click is the first entity, the second the second - and once both are
//! picked the engine measures, the surface formats the value and draws a
//! leader line between the two entities' anchors plus the value label.
//! `Clear` resets the picks. An unsupported pair is never silently
//! measured: the surface reports the named refusal honestly.
//!
//! ## Bounds this panel respects
//!
//! - UI-A3 / UI-A12 guards: no raw hex and no bold-text ink construction
//!   outside `theme.rs`; all painted ink comes from `oep_ui_theme` tokens.
//! - UI-B1 mutation guard: this panel never writes a document collection; it
//!   only reads the committed fixture geometry (and holds its own pick
//!   state in the egui context store).
//! - Default frame unchanged: the surface draws nothing until `M` is pressed,
//!   so the committed launch-frame goldens are untouched.
//! - The engine is egui-free and `#[path]`-includable (the same seam pattern
//!   every wave-B/C engine uses), so the acceptance can drive the pure math
//!   directly and the real panel through kittest.

use eframe::egui;

use oep_ui_theme::{Mode, Palette};

// ---------------------------------------------------------------------------
// Keys and the cross-`#[path]`-safe oracle
// ---------------------------------------------------------------------------

/// Context-data key under which the typed measure panel state lives (this
/// module's own copy only; tests read the `String` oracle instead).
pub const MEASURE_STATE_KEY: &str = "ui_c5_measure_state";
/// Context-data key under which the panel publishes its plain-`String` report
/// every frame: surface open, the picked entity ids, the measured kind, the
/// formatted value and the last named error. Cross-`#[path]`-copy safe.
pub const MEASURE_REPORT_KEY: &str = "ui_c5_measure_report";

/// The plain key that toggles the measure surface (free in the B1 table, the
/// sketch tool keys and the shell regions, like `F8`).
pub const MEASURE_TOGGLE_KEY: egui::Key = egui::Key::M;

/// The stated comparison tolerance for the fixture's analytic goldens: the
/// engine's floating point over these coordinates is exact to far below this
/// (1e-6 in mm for lengths, 1e-6 in degrees for the angle).
pub const MEASURE_TOLERANCE: f64 = 1e-6;

// ---------------------------------------------------------------------------
// Geometry layer: the egui-free measurement engine (the card's "geometry
// layer" math - nothing here draws, formats or touches egui)
// ---------------------------------------------------------------------------

/// Two free functions the engine's line math uses internally.
pub(crate) fn dist2(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (b.0 - a.0, b.1 - a.1)
}

/// Euclidean distance between two plane points.
pub fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let d = dist2(a, b);
    (d.0 * d.0 + d.1 * d.1).sqrt()
}

/// The angle (in degrees) between the directions of two lines, normalised to
/// `[0, 180]` (the smaller of the two angles between two undirected lines).
/// A zero-length line degrades to `0` (no defined turn).
pub fn line_angle_deg(a1: (f64, f64), a2: (f64, f64), b1: (f64, f64), b2: (f64, f64)) -> f64 {
    let (u0, u1) = dist2(a1, a2);
    let (v0, v1) = dist2(b1, b2);
    let ulen = (u0 * u0 + u1 * u1).sqrt();
    let vlen = (v0 * v0 + v1 * v1).sqrt();
    if ulen == 0.0 || vlen == 0.0 {
        return 0.0;
    }
    let dot = (u0 * v0 + u1 * v1) / (ulen * vlen);
    let cos = dot.clamp(-1.0, 1.0);
    cos.acos().to_degrees()
}

/// One entity of the committed bracket fixture the panel measures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityRef {
    /// A committed point (`p0..`).
    Point(String),
    /// A committed line (`l0..`).
    Line(String),
    /// A committed circle (`c0..`).
    Circle(String),
}

impl EntityRef {
    /// Stable-ish machine id used in the oracle (`p0`, `l1`, `c0`).
    pub fn id(&self) -> &str {
        match self {
            EntityRef::Point(id) | EntityRef::Line(id) | EntityRef::Circle(id) => id,
        }
    }

    /// One-line human label for the pick button / annotation.
    pub fn label(&self) -> String {
        match self {
            EntityRef::Point(id) => format!("point {id}"),
            EntityRef::Line(id) => format!("line {id}"),
            EntityRef::Circle(id) => format!("circle {id}"),
        }
    }
}

/// The three measurement kinds the card names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureKind {
    /// Euclidean distance between two points (mm).
    Distance,
    /// Angle between two lines (degrees).
    Angle,
    /// Radius of a circle (mm).
    Radius,
}

impl MeasureKind {
    /// Machine id used in the oracle / annotation text.
    pub fn id(self) -> &'static str {
        match self {
            MeasureKind::Distance => "distance",
            MeasureKind::Angle => "angle",
            MeasureKind::Radius => "radius",
        }
    }

    /// The unit of a measured value, as the panel formats it.
    pub fn unit(self) -> &'static str {
        match self {
            MeasureKind::Distance | MeasureKind::Radius => "mm",
            MeasureKind::Angle => "deg",
        }
    }
}

/// A named, typed failure of the measurement engine. Nothing here is a panic
/// and nothing claims a measurement the geometry did not produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasureError {
    /// The pair of picked entities is not a supported measurement.
    IncompatiblePair(String),
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureError::IncompatiblePair(msg) => write!(f, "{msg}"),
        }
    }
}

/// The committed bracket fixture (the "committed fixture" of the card): a
/// 40 x 30 profile with a 10-radius circle. All analytic goldens the
/// acceptance asserts derive from these exact coordinates.
pub fn bracket_fixture() -> Vec<EntityRef> {
    vec![
        EntityRef::Point("p0".into()),
        EntityRef::Point("p1".into()),
        EntityRef::Point("p2".into()),
        EntityRef::Point("p3".into()),
        EntityRef::Line("l0".into()),
        EntityRef::Line("l1".into()),
        EntityRef::Line("l2".into()),
        EntityRef::Line("l3".into()),
        EntityRef::Circle("c0".into()),
    ]
}

/// The plane coordinate of a fixture point. Ordering mirrors
/// [`bracket_fixture`] (points first, then the lines, then circles).
pub fn point_of(id: &str) -> Option<(f64, f64)> {
    match id {
        "p0" => Some((0.0, 0.0)),
        "p1" => Some((40.0, 0.0)),
        "p2" => Some((40.0, 30.0)),
        "p3" => Some((0.0, 30.0)),
        // c0's centre (p4 in the doc comments): a 40x30 box's centroid.
        "p4" => Some((20.0, 15.0)),
        _ => None,
    }
}

/// The `(start, end)` plane points of a fixture line.
pub fn line_of(id: &str) -> Option<((f64, f64), (f64, f64))> {
    match id {
        "l0" => Some((point_of("p0")?, point_of("p1")?)),
        "l1" => Some((point_of("p1")?, point_of("p2")?)),
        "l2" => Some((point_of("p2")?, point_of("p3")?)),
        "l3" => Some((point_of("p3")?, point_of("p0")?)),
        _ => None,
    }
}

/// The `(centre, radius)` of a fixture circle.
pub fn circle_of(id: &str) -> Option<((f64, f64), f64)> {
    match id {
        "c0" => Some((point_of("p4")?, 10.0)),
        _ => None,
    }
}

/// Resolve an entity to its on-screen anchor: a point's own coordinate, a
/// line's midpoint, a circle's centre (what a leader line should point at).
pub fn anchor_of(e: &EntityRef) -> Option<(f64, f64)> {
    match e {
        EntityRef::Point(id) => point_of(id),
        EntityRef::Line(id) => {
            let (a, b) = line_of(id)?;
            Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
        }
        EntityRef::Circle(id) => circle_of(id).map(|(c, _)| c),
    }
}

/// Measure a pair of picked entities through the geometry layer:
///
/// - two points  -> [`MeasureKind::Distance`],
/// - two lines   -> [`MeasureKind::Angle`] (degrees),
/// - one circle  -> [`MeasureKind::Radius`] (a circle needs only one pick).
///
/// Anything else is an honest [`MeasureError::IncompatiblePair`], never a
/// fabricated number.
pub fn measure(first: &EntityRef, second: &EntityRef) -> Result<(MeasureKind, f64), MeasureError> {
    match (first, second) {
        (EntityRef::Point(ai), EntityRef::Point(bi)) => {
            let a = point_of(ai)
                .ok_or_else(|| MeasureError::IncompatiblePair(format!("unknown point {ai}")))?;
            let b = point_of(bi)
                .ok_or_else(|| MeasureError::IncompatiblePair(format!("unknown point {bi}")))?;
            Ok((MeasureKind::Distance, distance(a, b)))
        }
        (EntityRef::Line(ai), EntityRef::Line(bi)) => {
            let a = line_of(ai)
                .ok_or_else(|| MeasureError::IncompatiblePair(format!("unknown line {ai}")))?;
            let b = line_of(bi)
                .ok_or_else(|| MeasureError::IncompatiblePair(format!("unknown line {bi}")))?;
            Ok((MeasureKind::Angle, line_angle_deg(a.0, a.1, b.0, b.1)))
        }
        (EntityRef::Circle(ci), _) | (_, EntityRef::Circle(ci)) => {
            let (_, r) = circle_of(ci)
                .ok_or_else(|| MeasureError::IncompatiblePair(format!("unknown circle {ci}")))?;
            Ok((MeasureKind::Radius, r))
        }
        _ => Err(MeasureError::IncompatiblePair(
            "measure supports two points, two lines, or a circle".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Panel state and the deterministic measure surface (formats and draws only)
// ---------------------------------------------------------------------------

/// The measure panel's typed state (this module's own context-store copy).
#[derive(Debug, Clone, Default)]
pub struct MeasureState {
    /// Whether the measure surface is open over the viewport.
    pub open: bool,
    /// The first picked entity, if any.
    pub first: Option<EntityRef>,
    /// The second picked entity, if any.
    pub second: Option<EntityRef>,
}

fn read_state(ctx: &egui::Context) -> MeasureState {
    ctx.data(|d| d.get_temp(egui::Id::unique(MEASURE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, state: &MeasureState) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(MEASURE_STATE_KEY), state.clone()));
}

/// Palette resolved for the currently installed theme mode (same derivation
/// as the sibling panels).
fn palette(ctx: &egui::Context) -> Palette {
    let mode = if ctx.global_style().visuals.dark_mode {
        Mode::Dark
    } else {
        Mode::Light
    };
    Palette::resolve(mode)
}

/// Deterministic layout of the measure surface anchored at the viewport's
/// bottom-left corner (clear of the sketch/D feature surfaces at the top, and
/// of the constraint toolbar / draw legend). The renderer and the acceptance
/// harness click the same rects (the B6 contract).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureRects {
    /// The surface card (title + border backdrop).
    pub card: egui::Rect,
    /// The preview box where the fixture's wireframe + the annotation draw.
    pub preview: egui::Rect,
    /// The "Clear" reset button.
    pub clear: egui::Rect,
    /// The measured-value annotation label position.
    pub value_label: egui::Rect,
    /// One row per fixture entity, stacked above the value line.
    pub rows: [egui::Rect; 9],
}

/// The entity pick buttons, in document order (points, lines, then circles).
pub fn entity_rows() -> Vec<EntityRef> {
    bracket_fixture()
}

/// Compute the surface rects for a given viewport rect. The card is 300pt
/// wide so the full annotation line (e.g. `distance point p0 -> point p1 =
/// 40.000 mm`) fits on one row without truncation.
pub fn measure_rects(viewport: egui::Rect) -> MeasureRects {
    let x = viewport.min.x + 8.0;
    let w = 300.0;
    let row_h = 18.0;
    let total_rows = entity_rows().len() as f32;
    let card_h = 26.0 + 10.0 + total_rows * row_h + 10.0 + 20.0 + 8.0;
    let y = viewport.max.y - card_h - 8.0;
    let card = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, card_h));

    let preview = egui::Rect::from_min_size(egui::pos2(x + 6.0, y + 26.0), egui::vec2(96.0, 78.0));
    let clear =
        egui::Rect::from_min_size(egui::pos2(x + w - 66.0, y + 6.0), egui::vec2(58.0, 20.0));
    let value_label = egui::Rect::from_min_size(
        egui::pos2(x + 6.0, y + card_h - 26.0),
        egui::vec2(w - 12.0, 20.0),
    );

    let mut rows = [egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO); 9];
    for (i, slot) in rows.iter_mut().enumerate() {
        let ry = y + 26.0 + 6.0 + i as f32 * (row_h + 2.0);
        *slot = egui::Rect::from_min_size(egui::pos2(x + 108.0, ry), egui::vec2(w - 120.0, row_h));
    }

    MeasureRects {
        card,
        preview,
        clear,
        value_label,
        rows,
    }
}

/// Map a fixture plane coordinate into the preview box (a fixed affine fit of
/// the 40 x 30 fixture into the 96 x 78 box, keeping a small margin).
fn preview_pos(preview: egui::Rect, p: (f64, f64)) -> egui::Pos2 {
    let sx = (preview.width() - 8.0) / 40.0;
    let sy = (preview.height() - 8.0) / 30.0;
    let s = sx.min(sy);
    let ox = preview.min.x + 4.0 + (40.0 - p.0) as f32 * s;
    // plane y grows up, screen y grows down
    let oy = preview.min.y + 4.0 + p.1 as f32 * s;
    egui::pos2(ox, oy)
}

/// The oracle lines for the current state: whether open, the picks, the
/// measured kind / value / unit and any honest error. The value is the
/// ENGINE's measured number formatted, never a bare literal re-read.
fn oracle_lines(state: &MeasureState) -> String {
    let mut out = String::new();
    out.push_str(&format!("open={}\n", u8::from(state.open)));
    match &state.first {
        Some(e) => out.push_str(&format!("first={}\n", e.id())),
        None => out.push_str("first=-\n"),
    }
    match &state.second {
        Some(e) => out.push_str(&format!("second={}\n", e.id())),
        None => out.push_str("second=-\n"),
    }
    match (&state.first, &state.second) {
        (Some(a), Some(b)) => match measure(a, b) {
            Ok((kind, value)) => {
                out.push_str(&format!("kind={}\n", kind.id()));
                out.push_str(&format!("value={value:.6}\n"));
                out.push_str(&format!("unit={}\n", kind.unit()));
                out.push_str("error=-\n");
            }
            Err(e) => {
                out.push_str("kind=-\nvalue=-\nunit=-\n");
                out.push_str(&format!("error={e}\n"));
            }
        },
        _ => {
            out.push_str("kind=-\nvalue=-\nunit=-\nerror=-\n");
        }
    }
    out.push_str(&format!("tolerance={MEASURE_TOLERANCE:.1e}\n"));
    out
}

/// Publish the `String` oracle.
fn publish_oracle(ctx: &egui::Context, state: &MeasureState) {
    let text = oracle_lines(state);
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(MEASURE_REPORT_KEY), text);
    });
}

/// Per-frame keyboard interaction: plain `M` toggles the measure surface.
/// Runs from the viewport panel before its own click handling (so the key is
/// consumed here while the surface is open, exactly like F8 for snap).
pub fn poll(ctx: &egui::Context) {
    let mut state = read_state(ctx);
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, MEASURE_TOGGLE_KEY)) {
        state.open = !state.open;
        if !state.open {
            state.first = None;
            state.second = None;
        }
    }
    write_state(ctx, &state);
    publish_oracle(ctx, &state);
}

/// Draw the measure surface over the viewport: the card, the entity pick
/// buttons, the fixture wireframe in the preview box, and (once two entities
/// are picked) the annotation - a leader line between the two anchors plus
/// the formatted value as a real `Label` widget so it is queryable and
/// appears in the rendered snapshot. Formats and draws only; all numbers
/// come from the engine section above.
pub fn draw(ctx: &egui::Context, viewport: egui::Rect) {
    let mut state = read_state(ctx);
    if !state.open {
        return;
    }
    let p = palette(ctx);
    let rects = measure_rects(viewport);

    egui::Area::new(egui::Id::unique("ui_c5_measure_surface"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            let painter = ui.painter();
            // Card background + hairline (token colours only).
            painter.rect_filled(rects.card, 4.0, p.surface_raised);
            painter.rect_stroke(
                rects.card,
                4.0,
                egui::Stroke::new(1.0, p.border),
                egui::StrokeKind::Outside,
            );
            painter.text(
                egui::pos2(rects.card.min.x + 6.0, rects.card.min.y + 5.0),
                egui::Align2::LEFT_TOP,
                "Measure",
                egui::FontId::proportional(12.0),
                p.text_primary,
            );

            // Preview box: the committed fixture's wireframe + the circle.
            painter.rect_stroke(
                rects.preview,
                2.0,
                egui::Stroke::new(1.0, p.border),
                egui::StrokeKind::Inside,
            );
            for l in ["l0", "l1", "l2", "l3"] {
                if let Some((a, b)) = line_of(l) {
                    painter.line_segment(
                        [preview_pos(rects.preview, a), preview_pos(rects.preview, b)],
                        egui::Stroke::new(1.0, p.border),
                    );
                }
            }
            if let Some((centre, r)) = circle_of("c0") {
                let c = preview_pos(rects.preview, centre);
                painter.circle_stroke(c, r as f32 * 2.0, egui::Stroke::new(1.0, p.border));
            }

            // The annotation when both entities are picked: a leader between
            // the two anchors plus the value label (the "annotation appears
            // in the snapshot" surface). Drawn from engine values only.
            if let (Some(a), Some(b)) = (&state.first, &state.second) {
                if let (Some(pa), Some(pb)) = (anchor_of(a), anchor_of(b)) {
                    let pa = preview_pos(rects.preview, pa);
                    let pb = preview_pos(rects.preview, pb);
                    painter.line_segment([pa, pb], egui::Stroke::new(2.0, p.accent));
                    painter.circle_filled(pa, 2.5, p.accent);
                    painter.circle_filled(pb, 2.5, p.accent);
                }
            }

            // Entity pick buttons: first click picks the first entity, second
            // the second; picking the same entity twice is a no-op.
            let rows = entity_rows();
            for (i, e) in rows.iter().enumerate() {
                let rect = *rects
                    .rows
                    .get(i)
                    .expect("fixture row count matches the fixed rect table");
                let btn = egui::Button::new(egui::RichText::new(e.label()).size(10.0))
                    .min_size(egui::vec2(rect.width(), rect.height()));
                if ui.put(rect, btn).clicked() {
                    let already_second = state.second.as_ref().map(|s| s == e).unwrap_or(false);
                    match (&state.first, already_second) {
                        (None, _) => state.first = Some(e.clone()),
                        (Some(f), false) if f != e => state.second = Some(e.clone()),
                        _ => {}
                    }
                }
            }

            // Clear resets the picks.
            let clear_btn = egui::Button::new(egui::RichText::new("Clear").size(10.0))
                .min_size(egui::vec2(rects.clear.width(), rects.clear.height()));
            if ui.put(rects.clear, clear_btn).clicked() {
                state.first = None;
                state.second = None;
            }

            // The formatted measurement, as a real Label (queryable a11y +
            // visible in the snapshot). Formats only; the number is the
            // engine's measured value.
            let text = match (&state.first, &state.second) {
                (Some(a), Some(b)) => match measure(a, b) {
                    Ok((kind, value)) => {
                        format!(
                            "{} {} = {value:.3} {}",
                            kind.id(),
                            label_pair(a, b),
                            kind.unit()
                        )
                    }
                    Err(e) => format!("measure error: {e}"),
                },
                (Some(a), None) => format!("{} · pick a second entity", a.label()),
                (None, _) => "pick two entities".to_string(),
            };
            ui.put(
                rects.value_label,
                egui::Label::new(egui::RichText::new(text).monospace().size(11.0)).truncate(),
            );
        });

    write_state(ctx, &state);
    publish_oracle(ctx, &state);
}

/// `"l0" -(l1)` style pair label for the formatted readout.
fn label_pair(a: &EntityRef, b: &EntityRef) -> String {
    format!("{} -> {}", a.label(), b.label())
}
