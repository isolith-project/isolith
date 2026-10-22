//! Deterministic tessellation of sweep solids for the interactive editor
//! (INTERACTIVE-SHELL).
//!
//! The editor shows real solids: every on-screen and exported state is
//! derived from the actual oep-geo model (a constrained sketch, a variable
//! set, and an `Extrude` / `Revolve` feature) by the deterministic evaluator
//! in this module. A closed polygon profile sweeps into a triangle mesh whose
//! boundary edges are also listed, so the viewport can draw both filled
//! faces and edge wires, and the STEP/STL exporter can emit the same mesh.
//!
//! The same profile + parameters the kernel seam consumes
//! ([`solid_extrude`](crate::kernel::solid_extrude),
//! [`solid_revolve`](crate::kernel::solid_revolve)) produce closed-form
//! solids, so a mesh built here cross-checks against the kernel goldens:
//! an `extrude_profile` of a 80x40 rectangle by 10 has volume exactly
//! 32000, matching what the OCCT-backed kernel measures on the same input.
//! This is the "canonical model is real, never a fake" contract: nothing is
//! invented, everything is re-derived from the model.
//!
//! Numerics discipline (P6-M1-T03, double-everywhere): coordinates are `f64`
//! throughout this module; the exporters ([`crate::export`]) own the
//! single-precision boundary and convert when they consume a [`TriMesh`]. All
//! summation for [`mesh_volume`] / [`mesh_bbox`] is `f64`. Output order is
//! deterministic: the same profile and parameters always produce byte-identical
//! positions/triangles/edges.

use crate::kernel::Point2D;

/// A triangle mesh with explicit boundary edges.
///
/// Positions are `f64` (the oep-geo double-everywhere discipline, P6-M1-T03);
/// exporters that need single precision (STEP, STL, glTF, OBJ) convert at
/// their boundary.
/// `triangles` and `edges` index into `positions`. `edges` lists the solid's
/// sharp/dihedral boundary as index pairs (used to draw edge wires in the
/// viewport and to prove the solid silhouette).
#[derive(Debug, Clone, PartialEq)]
pub struct TriMesh {
    /// Vertex positions `[x, y, z]`.
    pub positions: Vec<[f64; 3]>,
    /// Indexed triangles `[a, b, c]` into [`TriMesh::positions`].
    pub triangles: Vec<[u32; 3]>,
    /// Boundary edge polylines `[start, end]` (indices into positions).
    pub edges: Vec<[usize; 2]>,
}

impl TriMesh {
    /// An empty mesh (no vertices).
    pub fn empty() -> Self {
        Self {
            positions: Vec::new(),
            triangles: Vec::new(),
            edges: Vec::new(),
        }
    }
}

/// Typed failure of a tessellation. Never a panic: a non-finite or degenerate
/// profile is a typed error, matching the kernel seam's contract.
#[derive(Debug, Clone, PartialEq)]
pub enum TessellateError {
    /// The profile has fewer than 3 distinct points or is a degenerate loop.
    DegenerateProfile(String),
    /// A profile coordinate or sweep parameter is not finite.
    NonFinite(String),
    /// The profile is not a simple polygon (self-intersecting) or the
    /// ear-clipping triangulation could not close a region.
    NotSimple(String),
    /// Extrude height must be finite and positive.
    BadHeight(f64),
    /// Revolve angle must be finite and positive.
    BadAngle(f64),
    /// Revolve needs at least 3 meridian segments.
    BadSegments(u32),
    /// A revolve profile crosses the negative radial side (`x < 0`).
    NegativeRadius(String),
}

impl std::fmt::Display for TessellateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TessellateError::DegenerateProfile(m) => write!(f, "degenerate profile: {m}"),
            TessellateError::NonFinite(m) => write!(f, "non-finite input: {m}"),
            TessellateError::NotSimple(m) => write!(f, "profile is not a simple polygon: {m}"),
            TessellateError::BadHeight(h) => write!(f, "extrude height must be positive, got {h}"),
            TessellateError::BadAngle(a) => write!(f, "revolve angle must be positive, got {a}"),
            TessellateError::BadSegments(n) => {
                write!(f, "revolve needs >= 3 meridian segments, got {n}")
            }
            TessellateError::NegativeRadius(m) => write!(f, "negative revolve radius: {m}"),
        }
    }
}

impl std::error::Error for TessellateError {}

/// The axis-aligned bounding box of a mesh as `[minx, miny, minz, maxx,
/// maxy, maxz]`. An empty mesh yields `None`; otherwise all six bounds are
/// finite (positions are validated at construction).
pub fn mesh_bbox(mesh: &TriMesh) -> Option<[f64; 6]> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in &mesh.positions {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if lo[0].is_infinite() {
        return None;
    }
    Some([lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]])
}

/// The enclosed volume of a closed triangle mesh (divergence theorem: one
/// sixth of the signed tetrahedron sum under each triangle, exact for a
/// consistently outward-oriented watertight mesh). An open or empty mesh
/// returns a number whose sign is not meaningful; callers should treat a
/// non-positive result on a closed mesh as a defect.
pub fn mesh_volume(mesh: &TriMesh) -> f64 {
    let mut vol = 0.0f64;
    for t in &mesh.triangles {
        let a = mesh.positions[t[0] as usize];
        let b = mesh.positions[t[1] as usize];
        let c = mesh.positions[t[2] as usize];
        vol += signed_tetra(a, b, c);
    }
    vol / 6.0
}

/// Signed tetrahedron volume `(a x b) dot c / 6` times 6 (the un-divided dot
/// of cross products), summed directly.
#[inline]
fn signed_tetra(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    // (a x b) . c, the determinant of the 3x3 matrix built from a,b,c.
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
}

/// Validate a profile loop, returning the canonicalized counter-clockwise
/// polygon (2D `(x, y)` pairs) ready for ear clipping.
///
/// Rejects non-finite coordinates, fewer than 3 distinct points, and a loop
/// with zero (or non-finite) signed area.
fn canonicalize_ccw(profile: &[Point2D], label: &str) -> Result<Vec<[f64; 2]>, TessellateError> {
    if profile.len() < 3 {
        return Err(TessellateError::DegenerateProfile(format!(
            "{label} needs at least 3 points, got {}",
            profile.len()
        )));
    }
    for (i, p) in profile.iter().enumerate() {
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(TessellateError::NonFinite(format!(
                "{label} point {i} is not finite"
            )));
        }
    }
    let mut pts: Vec<[f64; 2]> = profile.iter().map(|p| [p.x, p.y]).collect();
    // Drop consecutive duplicates (a repeated vertex is legal in a profile but
    // must not become a degenerate triangle edge).
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12);
    if pts.len() < 3 {
        return Err(TessellateError::DegenerateProfile(format!(
            "{label} has fewer than 3 distinct points"
        )));
    }
    let mut twice = 0.0f64;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        twice += a[0] * b[1] - b[0] * a[1];
    }
    if !twice.is_finite() || twice.abs() <= f64::EPSILON {
        return Err(TessellateError::DegenerateProfile(format!(
            "{label} has zero swept area"
        )));
    }
    if twice < 0.0 {
        pts.reverse();
    }
    Ok(pts)
}

/// Ear-clipping triangulation of a simple polygon given in counter-clockwise
/// order. Returns the CCW triangle list (indices into `poly`). Deterministic:
/// ears are clipped left-to-right along the loop. Typed error when the input
/// is not a simple polygon (an ear could not be found: self-intersection).
fn ear_clip(poly: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, TessellateError> {
    let n = poly.len();
    if n < 3 {
        return Err(TessellateError::DegenerateProfile(
            "ear clip needs >= 3".to_string(),
        ));
    }
    // Working index ring; `removed[i]` says vertex i was already clipped.
    let mut removed = vec![false; n];
    let mut tris: Vec<[usize; 3]> = Vec::with_capacity(n - 2);
    let mut remaining = n;

    // Guard against pathological loops: cap work per vertex to keep an
    // infinite loop impossible (a self-intersecting polygon never clips, so
    // after n^2 ear candidates we declare the loop not simple).
    let mut cursor = 0usize;
    let mut progress = true;
    while remaining > 3 {
        if !progress {
            return Err(TessellateError::NotSimple(
                "ear clipping stalled; profile is self-intersecting or degenerate".to_string(),
            ));
        }
        progress = false;
        let max_scans = remaining * remaining + 1;
        let mut scans = 0;
        // Walk the ring starting at `cursor`.
        let mut idx = cursor;
        while scans < max_scans {
            scans += 1;
            if removed[idx] {
                idx = (idx + 1) % n;
                continue;
            }
            let prev = prev_active(idx, &removed, n);
            let next = next_active(idx, &removed, n);
            if prev == next {
                // Only one vertex left in a degenerate configuration.
                break;
            }
            if is_ear(idx, prev, next, poly, &removed) {
                tris.push([prev, idx, next]);
                removed[idx] = true;
                remaining -= 1;
                progress = true;
                cursor = prev;
                break;
            }
            idx = (idx + 1) % n;
        }
    }
    // The final triangle: the three remaining active vertices.
    let mut left: Vec<usize> = (0..n).filter(|&i| !removed[i]).collect();
    if left.len() != 3 {
        return Err(TessellateError::NotSimple(
            "ear clipping ended with more than 3 vertices; profile not simple".to_string(),
        ));
    }
    // Keep CCW winding for the final triangle.
    let (a, b, c) = (left[0], left[1], left[2]);
    if cross2(poly[a], poly[b], poly[c]) <= 0.0 {
        left.swap(1, 2);
    }
    tris.push([left[0], left[1], left[2]]);
    Ok(tris)
}

fn prev_active(idx: usize, removed: &[bool], n: usize) -> usize {
    let mut i = (idx + n - 1) % n;
    while removed[i] {
        i = (i + n - 1) % n;
    }
    i
}

fn next_active(idx: usize, removed: &[bool], n: usize) -> usize {
    let mut i = (idx + 1) % n;
    while removed[i] {
        i = (i + 1) % n;
    }
    i
}

/// Twice the signed area of triangle `(a, b, c)` (positive when CCW).
fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Whether vertex `v` (between `prev` and `next`, counter-clockwise ring) is a
/// valid ear: the triangle is convex (CCW) and contains no other active
/// vertex strictly inside.
fn is_ear(v: usize, prev: usize, next: usize, poly: &[[f64; 2]], removed: &[bool]) -> bool {
    let (a, b, c) = (poly[prev], poly[v], poly[next]);
    if cross2(a, b, c) <= 0.0 {
        return false; // reflex vertex: not an ear
    }
    for (i, p) in poly.iter().enumerate() {
        if removed[i] || i == prev || i == v || i == next {
            continue;
        }
        if point_in_triangle(*p, a, b, c) {
            return false; // another vertex inside -> not an ear
        }
    }
    true
}

/// Strict point-in-triangle (CCW triangle, edges inclusive on the boundary
/// which is fine for ear testing: a vertex exactly on the boundary is still a
/// blocker for a clean ear).
fn point_in_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let d1 = cross2(a, b, p);
    let d2 = cross2(b, c, p);
    let d3 = cross2(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// Build a mesh with dedicated bottom, top, then side triangles. Accounts for
/// winding so the mesh is consistently outward-oriented: `bottom` faces -Z,
/// `top` faces +Z, sides radiate outward.
///
/// `poly`: CCW 2D polygon (Z=0 plane). `z`: the sweep coordinate for `top`.
struct PrismBuilder {
    positions: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    edges: Vec<[usize; 2]>,
}

impl PrismBuilder {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            triangles: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn push(&mut self, p: [f64; 3]) -> u32 {
        self.positions.push(p);
        (self.positions.len() - 1) as u32
    }

    fn finish(self) -> TriMesh {
        TriMesh {
            positions: self.positions,
            triangles: self.triangles,
            edges: self.edges,
        }
    }
}

/// Extrude a closed planar profile (in the Z = 0 plane) by `height` along +Z
/// into a triangle mesh with listed boundary edges.
///
/// The profile is a closed loop (the last edge closes back to the first
/// point). The sweep is in the model's XY plane -> +Z, matching
/// [`crate::kernel::solid_extrude`] semantics, so the mesh cross-checks the
/// kernel solid's measures.
pub fn extrude_profile(profile: &[Point2D], height: f64) -> Result<TriMesh, TessellateError> {
    let poly = canonicalize_ccw(profile, "extrude profile")?;
    if !height.is_finite() || height <= 0.0 {
        return Err(TessellateError::BadHeight(height));
    }
    let tris = ear_clip(&poly)?;
    let n = poly.len();

    let mut b = PrismBuilder::new();
    // Bottom cap: index 0..n at z=0; top cap: n..2n at z=height.
    let top_offset = n as u32;
    for p in &poly {
        b.push([p[0], p[1], 0.0]);
    }
    for p in &poly {
        b.push([p[0], p[1], height]);
    }
    // Bottom cap triangles: CCW polygon at z=0 has outward normal +Z, so
    // reverse each ear triangle to point -Z.
    for t in &tris {
        b.triangles.push([t[0] as u32, t[2] as u32, t[1] as u32]);
    }
    // Top cap triangles: CCW at z=height faces +Z, keep winding.
    for t in &tris {
        b.triangles.push([
            t[0] as u32 + top_offset,
            t[1] as u32 + top_offset,
            t[2] as u32 + top_offset,
        ]);
    }
    // Side quads between consecutive boundary vertices (a->b), a at z=0 and
    // b at z=0, sweeping up: quad (ai, bi, bj, aj) with outward normal.
    for i in 0..n {
        let j = (i + 1) % n;
        let (ai, aj) = (i as u32, i as u32 + top_offset);
        let (bi, bj) = (j as u32, j as u32 + top_offset);
        b.triangles.push([ai, bi, bj]);
        b.triangles.push([ai, bj, aj]);
    }
    // Boundary edges: bottom loop, top loop, and vertical studs.
    for i in 0..n {
        let j = (i + 1) % n;
        b.edges.push([i, j]);
        b.edges.push([i + n, j + n]);
        b.edges.push([i, i + n]);
    }
    Ok(b.finish())
}

/// Revolve a closed meridian profile about the Z axis by `angle` radians into
/// a triangle mesh.
///
/// Profile semantics match [`crate::kernel::solid_revolve`]: each [`Point2D`]
/// carries `(radial distance from the Z axis, axial height)`, `x >= 0`, and
/// the loop closes implicitly. The sweep uses `segments` meridian steps
/// (>= 3). The returned mesh is closed and consistently outward-oriented, so
/// [`mesh_volume`] of a full-turn revolved rectangle equals the analytic
/// annular volume.
pub fn revolve_profile(
    profile: &[Point2D],
    angle: f64,
    segments: u32,
) -> Result<TriMesh, TessellateError> {
    let poly = canonicalize_ccw(profile, "revolve profile")?;
    if !angle.is_finite() || angle <= 0.0 {
        return Err(TessellateError::BadAngle(angle));
    }
    if segments < 3 {
        return Err(TessellateError::BadSegments(segments));
    }
    for (i, p) in poly.iter().enumerate() {
        if p[0] < 0.0 {
            return Err(TessellateError::NegativeRadius(format!(
                "profile point {i} has radial x = {} (must be >= 0)",
                p[0]
            )));
        }
    }
    let tris = ear_clip(&poly)?;
    let n = poly.len();

    // Vertex grid: vertex (meridian k, profile index i) at index
    // k * n + i, for k in 0..=segments.
    let cols = (segments + 1) as usize;
    let mut build = PrismBuilder::new();
    let mut grid: Vec<Vec<u32>> = Vec::with_capacity(cols);
    for k in 0..=segments as usize {
        let theta = angle * k as f64 / segments as f64;
        let (cs, sn) = (theta.cos(), theta.sin());
        let mut row = Vec::with_capacity(n);
        for p in &poly {
            let rad = p[0];
            row.push(build.push([rad * cs, rad * sn, p[1]]));
        }
        grid.push(row);
    }

    // Annular band between meridian k and k+1: for each profile edge
    // (i, i+1), quad (vk_i, vk_i1, vk1_i1, vk1_i) wound so the outward normal
    // points away from the axis (verified by the full-turn volume golden).
    for k in 0..segments as usize {
        let (row_a, row_b) = (&grid[k], &grid[k + 1]);
        for i in 0..n {
            let j = (i + 1) % n;
            let a = row_a[i];
            let b = row_a[j];
            let c = row_b[j];
            let d = row_b[i];
            build.triangles.push([a, c, b]);
            build.triangles.push([a, d, c]);
        }
    }
    // End caps: profile polygon at meridian 0 mapped to the plane theta=0 has
    // outward normal -Y (the interior is on the +theta side), so reverse each
    // ear triangle; the cap at theta=angle faces +theta (keep winding).
    for t in &tris {
        let (a, b, c) = (grid[0][t[0]], grid[0][t[1]], grid[0][t[2]]);
        build.triangles.push([a, c, b]);
    }
    for t in &tris {
        let (a, b, c) = (
            grid[segments as usize][t[0]],
            grid[segments as usize][t[1]],
            grid[segments as usize][t[2]],
        );
        build.triangles.push([a, b, c]);
    }
    // Boundary edges: the profile loops at both end meridians, plus axial
    // studs along each profile vertex ring (meridian k to k+1 at vertex i).
    // `i` indexes two grid rows at once, so an index loop is the honest form.
    #[allow(clippy::needless_range_loop)]
    for i in 0..n {
        build
            .edges
            .push([grid[0][i] as usize, grid[segments as usize][i] as usize]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        build.edges.push([grid[0][i] as usize, grid[0][j] as usize]);
        build.edges.push([
            grid[segments as usize][i] as usize,
            grid[segments as usize][j] as usize,
        ]);
    }
    Ok(build.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<Point2D> {
        vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(w, 0.0),
            Point2D::new(w, h),
            Point2D::new(0.0, h),
        ]
    }

    #[test]
    fn extrude_rectangle_volume_and_bbox() {
        let mesh = extrude_profile(&rect(80.0, 40.0), 10.0).expect("extrudes");
        assert_eq!(mesh.positions.len(), 8);
        assert_eq!(mesh.triangles.len(), 12);
        // bottom 2 + top 2 + 4 sides * 2
        let vol = mesh_volume(&mesh);
        assert!((vol - 32000.0).abs() < 1e-6, "volume {vol}");
        let bb = mesh_bbox(&mesh).expect("bbox");
        assert!(
            (bb[0] - 0.0).abs() < 1e-6 && (bb[1] - 0.0).abs() < 1e-6 && (bb[2] - 0.0).abs() < 1e-6
        );
        assert!(
            (bb[3] - 80.0).abs() < 1e-4
                && (bb[4] - 40.0).abs() < 1e-4
                && (bb[5] - 10.0).abs() < 1e-4
        );
    }

    #[test]
    fn extrude_is_deterministic() {
        let a = extrude_profile(&rect(80.0, 40.0), 10.0).unwrap();
        let b = extrude_profile(&rect(80.0, 40.0), 10.0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn extrude_winding_counter_clockwise_input_reversed() {
        // Clockwise (CW) input must canonicalize the same as CCW.
        let ccw = rect(30.0, 20.0);
        let mut cw = ccw.clone();
        cw.reverse();
        let a = extrude_profile(&ccw, 5.0).unwrap();
        let b = extrude_profile(&cw, 5.0).unwrap();
        assert_eq!(a, b);
        assert!((mesh_volume(&a) - 3000.0).abs() < 1e-6);
    }

    #[test]
    fn extrude_rejects_degenerate_and_bad_height() {
        assert!(matches!(
            extrude_profile(&[Point2D::new(0.0, 0.0), Point2D::new(1.0, 0.0)], 1.0),
            Err(TessellateError::DegenerateProfile(_))
        ));
        assert!(matches!(
            extrude_profile(&rect(1.0, 1.0), 0.0),
            Err(TessellateError::BadHeight(_))
        ));
    }

    #[test]
    fn concave_profile_clips_without_error() {
        // L-shape (concave): ear clipping must still produce a valid closed
        // solid whose volume equals the analytic L area * depth.
        let l: Vec<Point2D> = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(60.0, 0.0),
            Point2D::new(60.0, 20.0),
            Point2D::new(20.0, 20.0),
            Point2D::new(20.0, 60.0),
            Point2D::new(0.0, 60.0),
        ];
        let mesh = extrude_profile(&l, 5.0).expect("L extrudes");
        // L area = 60*60 - 40*40 = 2000; * depth 5 = 10000.
        let vol = mesh_volume(&mesh);
        assert!((vol - 10000.0).abs() < 1e-6, "L volume {vol}");
    }

    #[test]
    fn revolve_rectangle_full_turn_annular_volume() {
        let ring: Vec<Point2D> = vec![
            Point2D::new(0.4, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(0.4, 1.0),
        ];
        let segments: u32 = 64;
        let mesh = revolve_profile(&ring, 2.0 * std::f64::consts::PI, segments).expect("revolves");
        // The mesh is the EXACT solid swept by the polygonal (faceted)
        // meridian, not the smooth annulus: its volume is the polygonized ring
        // area times the axial extent. The smooth annulus pi*(R^2-r^2) is the
        // OCCT kernel value this faceted mesh converges to as segments grow.
        let area_outer = 0.5
            * 1.0f64.powi(2)
            * segments as f64
            * (2.0 * std::f64::consts::PI / segments as f64).sin();
        let area_inner = 0.5
            * 0.4f64.powi(2)
            * segments as f64
            * (2.0 * std::f64::consts::PI / segments as f64).sin();
        let expected = (area_outer - area_inner) * 1.0;
        let vol = mesh_volume(&mesh);
        assert!(
            (vol - expected).abs() < 1e-6,
            "full-turn volume {vol} vs faceted {expected}"
        );
        // Sanity: it is within the expected tolerance of the smooth annulus
        // (polygonization error ~ (1/2)*theta^2 each segment).
        let smooth = std::f64::consts::PI * (1.0 - 0.16) * 1.0;
        assert!(
            ((vol - smooth) / smooth).abs() < 0.01,
            "too far from smooth {smooth}"
        );
    }

    #[test]
    fn revolve_half_turn_quarter_volume() {
        let ring: Vec<Point2D> = vec![
            Point2D::new(0.4, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(0.4, 1.0),
        ];
        let segments: u32 = 48;
        // Half turn sweeps a 180-degree sector; the faceted sector area is a
        // polygon fan of `segments` trapezoids.
        let mesh = revolve_profile(&ring, std::f64::consts::PI, segments).expect("revolves");
        let area_outer =
            0.5 * 1.0f64.powi(2) * segments as f64 * (std::f64::consts::PI / segments as f64).sin();
        let area_inner =
            0.5 * 0.4f64.powi(2) * segments as f64 * (std::f64::consts::PI / segments as f64).sin();
        let expected = (area_outer - area_inner) * 1.0;
        let vol = mesh_volume(&mesh);
        assert!(
            (vol - expected).abs() < 1e-6,
            "half-turn volume {vol} vs faceted {expected}"
        );
    }

    #[test]
    fn revolve_rejects_negative_radius() {
        let bad: Vec<Point2D> = vec![
            Point2D::new(-0.1, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(-0.1, 1.0),
        ];
        assert!(matches!(
            revolve_profile(&bad, 2.0 * std::f64::consts::PI, 8),
            Err(TessellateError::NegativeRadius(_))
        ));
    }

    #[test]
    fn revolve_is_deterministic() {
        let ring: Vec<Point2D> = vec![
            Point2D::new(0.4, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(0.4, 1.0),
        ];
        let a = revolve_profile(&ring, std::f64::consts::PI, 16).unwrap();
        let b = revolve_profile(&ring, std::f64::consts::PI, 16).unwrap();
        assert_eq!(a, b);
    }
}
