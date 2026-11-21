//! UI-B5 UI-SNAP-INFERENCE: pointer snap inference while drawing (wave B).
//!
//! The pure, egui-free half of the snap-inference card. While a draw tool is
//! active (UI-B4) the pointer is free to hover anywhere on the sketch plane,
//! but it *snaps* to the nearest inference candidate and the viewport shows a
//! glyph plus a label naming which inference fired. This module computes that
//! candidate from the committed sketch document ([`tools::SketchDoc`]); the
//! interactive half (pointer wiring, glyph/label painting, the toggle) lives
//! in `panels/sketch.rs`, which `#[path]`-includes this file exactly the way
//! it includes `tools.rs`.
//!
//! The inference kinds the card names:
//!
//! - [`Inference::Endpoint`] - an existing committed point,
//! - [`Inference::Midpoint`] - the midpoint of a committed line,
//! - [`Inference::Centre`] - the centre of a committed circle,
//! - [`Inference::OnEdge`] - the closest point on a committed line segment,
//! - [`Inference::Intersection`] - the crossing of two committed line
//!   segments,
//! - [`Inference::Horizontal`] / [`Inference::Vertical`] - the pointer
//!   aligned with a committed point's row / column (the card's `h/v`).
//!
//! Selection is explicit and **tested**: priority first, then distance, then
//! document order. Every candidate within the snap radius of the pointer
//! competes; the lowest priority rank wins (endpoint beats on-edge,
//! intersection beats on-edge, midpoint beats on-edge, centre beats on-edge,
//! and the ambient h/v alignment guides rank last), ties within a rank go to
//! the nearest candidate, and equal candidates fall back to document order
//! for determinism. [`Inference::Endpoint`] means the end of a sketch edge
//! (a point that terminates a committed line); a point that is only a circle
//! centre infers as [`Inference::Centre`], not Endpoint.
//!
//! The toggle is a command token like any other: turning inference off is the
//! `sk:`-family command line [`SNAP_TOKEN`] (`sk:snap`), whose dispatch
//! family head is the same `sk` token the UI-B1 router and this wave's commit
//! seam validate through `oep_shell::commands::validate_token`. The engine
//! side command record for `sk:snap` lands with a later wiring card; this
//! card drives the family-validated token as a plain sketch-mode key, the way
//! the action tools drive `sk:point` / `sk:line`.

use super::tools::{distance, SketchDoc};

/// The command token of the inference toggle, in the same `sk:` family as the
/// draw tools' `sk:point` / `sk:line` commit tokens.
pub const SNAP_TOKEN: &str = "sk:snap";
/// The dispatch family head of [`SNAP_TOKEN`]: the token the CLI (and the
/// UI-B1 router's table) validates through `validate_token` before dispatch.
/// (`dead_code` allowed - the router's table documents the family; the
/// acceptance asserts it through the registry's own validate step.)
#[allow(dead_code)]
pub const SNAP_FAMILY: &str = "sk";
/// The default snap radius in plane units: half a sketch-grid cell
/// (`SKETCH_GRID_SPACING` = 10), so inference fires when the pointer is
/// within half a grid cell of a candidate.
pub const SNAP_RADIUS: f64 = 5.0;

/// The inference kinds this card adds. [`Inference::ALL`] enumerates them in
/// the card's order; [`Inference::priority`] carries the explicit, tested
/// selection rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inference {
    /// The end of a committed line (a point that terminates a sketch edge).
    Endpoint,
    /// The crossing of two committed line segments.
    Intersection,
    /// The midpoint of a committed line.
    Midpoint,
    /// The centre of a committed circle.
    Centre,
    /// The closest point on a committed line segment.
    OnEdge,
    /// The pointer aligned with a committed point's row (same `v`).
    Horizontal,
    /// The pointer aligned with a committed point's column (same `u`).
    Vertical,
}

impl Inference {
    /// Every inference kind, in the card's enumeration order. (`dead_code`
    /// allowed - the keymap/legend cards and the module tests enumerate; the
    /// bin reads kinds through the returned values.)
    #[allow(dead_code)]
    pub const ALL: [Inference; 7] = [
        Inference::Endpoint,
        Inference::Midpoint,
        Inference::Centre,
        Inference::OnEdge,
        Inference::Intersection,
        Inference::Horizontal,
        Inference::Vertical,
    ];
    /// Stable machine id used in the oracle/report (`infer=`) and the tests.
    pub fn id(self) -> &'static str {
        match self {
            Inference::Endpoint => "endpoint",
            Inference::Intersection => "intersection",
            Inference::Midpoint => "midpoint",
            Inference::Centre => "centre",
            Inference::OnEdge => "onedge",
            Inference::Horizontal => "horizontal",
            Inference::Vertical => "vertical",
        }
    }

    /// Human label painted next to the glyph - "a label naming which
    /// inference fired".
    pub fn label(self) -> &'static str {
        match self {
            Inference::Endpoint => "Endpoint",
            Inference::Intersection => "Intersection",
            Inference::Midpoint => "Midpoint",
            Inference::Centre => "Centre",
            Inference::OnEdge => "On edge",
            Inference::Horizontal => "Horizontal",
            Inference::Vertical => "Vertical",
        }
    }

    /// The explicit, tested selection rank: lower fires first. Geometry that
    /// pins a precise point outranks the in-place on-edge snap, and the
    /// ambient h/v alignment guides rank last. The card pins "endpoint beats
    /// on-edge" (rank 0 < rank 4) and the tests pin intersection beats
    /// on-edge, midpoint beats on-edge and on-edge beats h/v.
    pub fn priority(self) -> u8 {
        match self {
            Inference::Endpoint => 0,
            Inference::Intersection => 1,
            Inference::Midpoint => 2,
            Inference::Centre => 3,
            Inference::OnEdge => 4,
            Inference::Horizontal | Inference::Vertical => 5,
        }
    }
}

/// One inference hit: which inference fired, the exact snapped plane
/// coordinate the pointer is drawn to, and the pointer-to-snap distance in
/// plane units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    /// The inference that fired.
    pub kind: Inference,
    /// The exact snapped plane coordinate `(u, v)` the pointer is drawn to.
    pub point: (f64, f64),
    /// Pointer-to-snap distance in plane units at selection time.
    pub distance: f64,
}

/// The closest point on segment `[a, b]` to `p` (the clamped perpendicular
/// projection).
fn closest_on_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> (f64, f64) {
    let (ax, ay, bx, by, px, py) = (a.0, a.1, b.0, b.1, p.0, p.1);
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        return a; // degenerate segment: the endpoint
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    (ax + t * dx, ay + t * dy)
}

/// 2-D cross product (the z component of `a x b`).
fn cross2(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// The crossing point of segments `[a, b]` and `[c, d]`, when the segments
/// actually cross (including at an endpoint). Parallel or collinear segments
/// yield `None` - a collinear overlap is not ambiguous enough to be an
/// inference candidate.
fn segment_intersection(
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
    d: (f64, f64),
) -> Option<(f64, f64)> {
    let r = (b.0 - a.0, b.1 - a.1);
    let s = (d.0 - c.0, d.1 - c.1);
    let denom = cross2(r, s);
    if denom.abs() < 1e-12 {
        return None;
    }
    let qp = (c.0 - a.0, c.1 - a.1);
    let t = cross2(qp, s) / denom;
    let u = cross2(qp, r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((a.0 + t * r.0, a.1 + t * r.1))
    } else {
        None
    }
}

/// One candidate in the inference pool: `(priority, distance, sequence,
/// kind, point)`. `sequence` is the push order (kind blocks in a fixed order,
/// entity order within a kind), so an exact tie falls back to the earliest
/// candidate deterministically. Clippy wants this factored out of the big
/// inline tuple.
type Candidate = (u8, f64, usize, Inference, (f64, f64));

/// Compute the inference candidate the pointer snaps to inside a committed
/// sketch document, or `None` when the pointer is not within `radius` (plane
/// units) of any candidate - or `radius` is non-positive (the disabled
/// lane).
///
/// Selection: collect every inference candidate within `radius`, then pick
/// lowest [`Inference::priority`] first, nearest within a rank second, and
/// earliest in document order to break exact ties - precise, deterministic,
/// and asserted per type and per priority pair by the acceptance.
pub fn infer(doc: &SketchDoc, pointer: (f64, f64), radius: f64) -> Option<Snap> {
    if radius <= 0.0 {
        return None;
    }
    let mut cands: Vec<Candidate> = Vec::new();
    let mut seq: usize = 0;
    let mut push = |kind: Inference, point: (f64, f64), dist: f64| {
        cands.push((kind.priority(), dist, seq, kind, point));
        seq += 1;
    };

    // Endpoints: the points that terminate a committed line - the ends of
    // sketch edges, the card's "endpoint". A point that is only a circle
    // centre infers as Centre (there is no edge end there), so the two kinds
    // stay distinct at one coordinate.
    let mut line_end = vec![false; doc.points.len()];
    for line in &doc.lines {
        for want in [&line.start, &line.end] {
            if let Some(i) = doc.points.iter().position(|p| &p.id == want) {
                line_end[i] = true;
            }
        }
    }
    for (i, p) in doc.points.iter().enumerate() {
        if !line_end[i] {
            continue;
        }
        let d = distance(pointer, (p.x, p.y));
        if d <= radius {
            push(Inference::Endpoint, (p.x, p.y), d);
        }
    }
    // Intersections: every pair of committed line segments that cross.
    for i in 0..doc.lines.len() {
        let (Some(a0), Some(a1)) = (
            doc.point_coord(&doc.lines[i].start),
            doc.point_coord(&doc.lines[i].end),
        ) else {
            continue;
        };
        for j in (i + 1)..doc.lines.len() {
            let (Some(b0), Some(b1)) = (
                doc.point_coord(&doc.lines[j].start),
                doc.point_coord(&doc.lines[j].end),
            ) else {
                continue;
            };
            if let Some(pt) = segment_intersection(a0, a1, b0, b1) {
                let d = distance(pointer, pt);
                if d <= radius {
                    push(Inference::Intersection, pt, d);
                }
            }
        }
    }
    // Midpoints of committed lines.
    for line in &doc.lines {
        let (Some(p0), Some(p1)) = (doc.point_coord(&line.start), doc.point_coord(&line.end))
        else {
            continue;
        };
        let mid = ((p0.0 + p1.0) * 0.5, (p0.1 + p1.1) * 0.5);
        let d = distance(pointer, mid);
        if d <= radius {
            push(Inference::Midpoint, mid, d);
        }
    }
    // Circle centres.
    for circle in &doc.circles {
        if let Some(c) = doc.point_coord(&circle.centre) {
            let d = distance(pointer, c);
            if d <= radius {
                push(Inference::Centre, c, d);
            }
        }
    }
    // On-edge: the closest point on each committed segment.
    for line in &doc.lines {
        let (Some(p0), Some(p1)) = (doc.point_coord(&line.start), doc.point_coord(&line.end))
        else {
            continue;
        };
        let pt = closest_on_segment(p0, p1, pointer);
        let d = distance(pointer, pt);
        if d <= radius {
            push(Inference::OnEdge, pt, d);
        }
    }
    // h/v alignment guides: same row (Horizontal) / same column (Vertical)
    // as a committed point, snapping the pointer into alignment.
    for p in &doc.points {
        let (dx, dy) = (pointer.0 - p.x, pointer.1 - p.y);
        if dx.abs() <= radius {
            push(Inference::Vertical, (p.x, pointer.1), dx.abs());
        }
        if dy.abs() <= radius {
            push(Inference::Horizontal, (pointer.0, p.y), dy.abs());
        }
    }

    // Priority, then distance, then document order.
    cands.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(x.2.cmp(&y.2)));
    cands.into_iter().next().map(|(_, d, _, kind, point)| Snap {
        kind,
        point,
        distance: d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The toggle rides the `sk` dispatch family exactly like every other
    /// token this wave's sketch commands use - the router's and CLI's own
    /// validate step accepts it, and the token line carries the family name.
    #[test]
    fn toggle_token_is_a_sk_family_command_token() {
        assert!(SNAP_TOKEN.starts_with("sk:"), "token is a sk: family line");
        assert_eq!(
            oep_shell::commands::validate_token(SNAP_FAMILY),
            Ok(SNAP_FAMILY),
            "the dispatch family head validates like every router binding"
        );
    }

    /// The explicit priority map the card's "endpoint beats on-edge, etc."
    /// pins: endpoint/intersection/midpoint/centre outrank on-edge, and the
    /// h/v guides rank last. Each same-rank pair is distinct.
    #[test]
    fn priority_order_is_explicit_and_ranked() {
        // Named pair the card calls out: endpoint beats on-edge.
        assert!(Inference::Endpoint.priority() < Inference::OnEdge.priority());
        // Precise geometry outranks the in-place edge snap.
        assert!(Inference::Intersection.priority() < Inference::OnEdge.priority());
        assert!(Inference::Midpoint.priority() < Inference::OnEdge.priority());
        assert!(Inference::Centre.priority() < Inference::OnEdge.priority());
        // h/v guides rank last (ambient alignment guides, not a pick).
        for k in [
            Inference::Endpoint,
            Inference::Intersection,
            Inference::Midpoint,
            Inference::Centre,
            Inference::OnEdge,
        ] {
            assert!(k.priority() < Inference::Horizontal.priority(), "{k:?}");
            assert!(k.priority() < Inference::Vertical.priority(), "{k:?}");
        }
        assert_eq!(
            Inference::Horizontal.priority(),
            Inference::Vertical.priority()
        );
    }

    /// Every inference kind has a stable machine id and a distinct human
    /// label (the oracle `infer=` and the painted label both read them).
    #[test]
    fn every_kind_has_a_distinct_id_and_label() {
        let mut ids = std::collections::BTreeSet::new();
        let mut labels = std::collections::BTreeSet::new();
        for k in Inference::ALL {
            assert!(ids.insert(k.id()), "ids distinct");
            assert!(labels.insert(k.label()), "labels distinct");
            assert!(!k.id().is_empty() && !k.label().is_empty());
        }
        // The card's spelling in the label surface.
        assert_eq!(Inference::Endpoint.label(), "Endpoint");
        assert_eq!(Inference::Midpoint.label(), "Midpoint");
        assert_eq!(Inference::Centre.label(), "Centre");
        assert_eq!(Inference::OnEdge.label(), "On edge");
        assert_eq!(Inference::Intersection.label(), "Intersection");
        assert_eq!(Inference::Horizontal.label(), "Horizontal");
        assert_eq!(Inference::Vertical.label(), "Vertical");
    }

    /// Clamped perpendicular projection onto a segment, including the
    /// clamping at both ends and a degenerate segment.
    #[test]
    fn closest_point_on_a_segment_is_clamped() {
        assert_eq!(
            closest_on_segment((0.0, 0.0), (40.0, 0.0), (14.0, 0.4)),
            (14.0, 0.0),
            "interior projection clamps the perpendicular"
        );
        // Past the far end: clamped onto the endpoint, not the line.
        assert_eq!(
            closest_on_segment((0.0, 0.0), (40.0, 0.0), (50.0, 3.0)),
            (40.0, 0.0)
        );
        // Degenerate segment: the endpoint.
        assert_eq!(
            closest_on_segment((5.0, 5.0), (5.0, 5.0), (9.0, 9.0)),
            (5.0, 5.0)
        );
    }

    /// Segment crossings: an interior crossing is found at the exact point;
    /// parallel and non-interior pairs are not.
    #[test]
    fn segment_crossings_are_found_exactly() {
        let hit = segment_intersection((0.0, 0.0), (40.0, 0.0), (25.0, -10.0), (25.0, 40.0))
            .expect("perpendicular crossing");
        assert!((hit.0 - 25.0).abs() < 1e-12 && hit.1.abs() < 1e-12);
        // Parallel (same slope, never touch).
        assert!(segment_intersection((0.0, 0.0), (10.0, 0.0), (0.0, 1.0), (10.0, 1.0)).is_none());
        // The lines meet, but outside both segments.
        assert!(
            segment_intersection((0.0, 0.0), (10.0, 0.0), (50.0, 10.0), (50.0, -10.0)).is_none()
        );
    }
}
