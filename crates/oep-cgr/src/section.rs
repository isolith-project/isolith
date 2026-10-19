//! P2-M3-T03 — computed planar section (planar cut) over an indexed triangle
//! mesh.
//!
//! Observation gate (see `docs/re-notes/oep-cgr-section-extraction.md`): the
//! task asked for section/view extraction "if records observed". The real-CGR
//! observation pass on the eleven-sample corpus found NO stored planar-cut /
//! section / clipping records: zero `section/cut/clip/plane/view` string
//! markers, no section type tag in the P2-M1-T02 catalog, and plane-equation
//! quad density indistinguishable from the float-noise floor. CGR stores
//! display tessellation only; a CATIA "section view / planar cut" is a
//! viewer-computed clipping plane applied to that tessellation `[uncertain]`.
//!
//! So this module delivers the honest engineering equivalent: intersect an
//! oriented plane with the mesh (`cut`) to obtain the section line segments,
//! then chain them into 2D contours (`Section::loops`). It is
//! format-independent — it consumes the same data contract the internal
//! `Mesh` type (P2-M2-T01) exposes, `vertices: &[[f32;3]]` /
//! `triangles: &[[u32;3]]` — and wires in with a one-liner once both merge:
//!
//! ```ignore
//! cut(mesh.vertices(), mesh.triangles(), &plane)
//! ```
//!
//! Tolerant failure per P2-M1-T04: typed errors, never panics on any input.

/// The epsilon used to decide when a vertex lies ON the cutting plane and
/// when two section points coincide (relative to the unit-scale meshes this
/// reader targets; documented as a constant convention, not a vendor fact).
pub const ON_PLANE_EPS: f32 = 1e-5;

/// An oriented cutting plane: a point `origin` on the plane and a non-zero
/// surface `normal`. The plane itself is `{ p : dot(p - origin, normal) == 0 }`
/// and is insensitive to reversing `normal` (see `cut`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// A point on the plane (finite coordinates required).
    pub origin: [f32; 3],
    /// A non-zero, finite surface normal. Only its direction matters.
    pub normal: [f32; 3],
}

/// A planar section of a mesh: the set of line segments where the cutting
/// plane crosses triangle interiors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Section {
    segments: Vec<[[f32; 3]; 2]>,
}

/// Why a planar cut could not be performed. Every variant is a typed, non-
/// panicking outcome (P2-M1-T04); a plane that simply misses the mesh is
/// NOT an error — it yields an empty `Section`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionError {
    /// The triangle list is empty.
    NoTriangles,
    /// A triangle references a vertex index outside the vertex list.
    IndexOutOfRange,
    /// A vertex coordinate is non-finite (NaN / infinity).
    NonFiniteVertex,
    /// The plane is invalid (non-finite or zero-length normal, non-finite
    /// origin); the reason is carried as a message.
    InvalidPlane(&'static str),
}

fn finite3(p: [f32; 3]) -> bool {
    p[0].is_finite() && p[1].is_finite() && p[2].is_finite()
}

fn near(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    (a[0] - b[0]).abs() <= eps && (a[1] - b[1]).abs() <= eps && (a[2] - b[2]).abs() <= eps
}

impl Section {
    /// The raw section segments: one per crossing triangle, in triangle
    /// order. Meaningless ordering — use [`Section::loops`] for the contour.
    pub fn segments(&self) -> &[[[f32; 3]; 2]] {
        &self.segments
    }

    /// Chain the raw segments end-to-end into contours (polylines).
    ///
    /// Segments are matched when their endpoints coincide within `eps`. A
    /// closed contour repeats its first point as its last, so callers can
    /// detect closure by comparing `polyline[0]` with `polyline.last()`.
    /// Segments that fail to chain (open surfaces, gaps, non-manifold
    /// arrangements) yield open polylines; the method never panics and never
    /// drops a segment silently.
    pub fn loops(&self, eps: f32) -> Vec<Vec<[f32; 3]>> {
        let mut chains: Vec<Vec<[f32; 3]>> = Vec::new();
        let mut closed: Vec<bool> = Vec::new();

        // Phase 1: extend / close chains greedily as segments arrive.
        for s in &self.segments {
            let a = s[0];
            let b = s[1];
            // Try to close an open chain whose two ends match (a, b).
            let mut placed = false;
            for i in 0..chains.len() {
                if closed[i] {
                    continue;
                }
                let h = chains[i][0];
                let t = *chains[i].last().unwrap();
                if near(t, a, eps) && near(h, b, eps) {
                    chains[i].push(b); // repeat the start as the close marker
                    closed[i] = true;
                    placed = true;
                    break;
                }
                if near(t, b, eps) && near(h, a, eps) {
                    chains[i].push(a);
                    closed[i] = true;
                    placed = true;
                    break;
                }
            }
            if placed {
                continue;
            }
            // Otherwise extend the first open chain whose head or tail matches.
            for i in 0..chains.len() {
                if closed[i] {
                    continue;
                }
                let h = chains[i][0];
                let t = *chains[i].last().unwrap();
                if near(t, a, eps) {
                    chains[i].push(b);
                    placed = true;
                    break;
                }
                if near(h, a, eps) {
                    chains[i].insert(0, b);
                    placed = true;
                    break;
                }
                if near(t, b, eps) {
                    chains[i].push(a);
                    placed = true;
                    break;
                }
                if near(h, b, eps) {
                    chains[i].insert(0, a);
                    placed = true;
                    break;
                }
            }
            if !placed {
                chains.push(vec![a, b]);
                closed.push(false);
            }
        }

        // Phase 2: join chains whose ends touch; close rings.
        loop {
            let mut changed = false;
            'scan: for i in 0..chains.len() {
                if closed[i] {
                    continue;
                }
                let hi = chains[i][0];
                let ti = *chains[i].last().unwrap();
                for j in 0..chains.len() {
                    if i == j || closed[j] {
                        continue;
                    }
                    let hj = chains[j][0];
                    let tj = *chains[j].last().unwrap();
                    // i++j closes a ring (both ends touch).
                    if near(ti, hj, eps) && near(hi, tj, eps) {
                        let mut pts = chains[i].clone();
                        // chain_i tail == chain_j head and chain_i head ==
                        // chain_j tail, so the join already gives head == tail:
                        // a closed loop with no extra vertices.
                        pts.extend_from_slice(&chains[j][1..]);
                        chains[i] = pts;
                        closed[i] = true;
                        chains.remove(j);
                        closed.remove(j);
                        changed = true;
                        break 'scan;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Phase 3: join open chains end-to-end (open joins).
        loop {
            let mut changed = false;
            'scan: for i in 0..chains.len() {
                if closed[i] {
                    continue;
                }
                let hi = chains[i][0];
                let ti = *chains[i].last().unwrap();
                for j in 0..chains.len() {
                    if i == j || closed[j] {
                        continue;
                    }
                    let hj = chains[j][0];
                    let tj = *chains[j].last().unwrap();
                    if near(ti, hj, eps) {
                        let mut rest = chains[j][1..].to_vec();
                        chains[i].append(&mut rest);
                        chains.remove(j);
                        closed.remove(j);
                        changed = true;
                        break 'scan;
                    }
                    if near(tj, hi, eps) {
                        let mut pts = chains[j].clone();
                        pts.extend_from_slice(&chains[i][1..]);
                        chains[i] = pts;
                        chains.remove(j);
                        closed.remove(j);
                        changed = true;
                        break 'scan;
                    }
                    if near(hi, hj, eps) {
                        // reverse j and prepend (minus shared endpoint)
                        let mut rev = chains[j].clone();
                        rev.reverse();
                        let keep = rev[..rev.len() - 1].to_vec();
                        chains[i].splice(0..0, keep);
                        chains.remove(j);
                        closed.remove(j);
                        changed = true;
                        break 'scan;
                    }
                    if near(ti, tj, eps) {
                        let mut rev = chains[j].clone();
                        rev.reverse();
                        chains[i].extend_from_slice(&rev[1..]);
                        chains.remove(j);
                        closed.remove(j);
                        changed = true;
                        break 'scan;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Phase 4: any remaining open chain whose ends now coincide closes.
        for i in 0..chains.len() {
            if chains[i].len() > 1 && !closed[i] {
                let h = chains[i][0];
                let t = *chains[i].last().unwrap();
                if near(h, t, eps) {
                    chains[i].push(h);
                    closed[i] = true;
                }
            }
        }

        chains
    }
}

/// Intersect an oriented `plane` with an indexed triangle mesh.
///
/// Returns the [`Section`] line segments produced by strict interior
/// crossings: a triangle contributes the segment between the two points
/// where the plane crosses its edges. A triangle with any vertex ON the
/// plane is skipped (coincident / boundary cuts are degenerate, not
/// sections — a documented convention, not an asserted vendor behavior). A
/// plane that misses the mesh entirely returns an empty `Ok(Section)`, never
/// an error. The result is independent of the sign of `plane.normal`.
///
/// Validates the mesh (indices in range, finite coordinates) and the plane
/// (finite origin, finite non-zero normal) and returns a typed
/// [`SectionError`]; never panics on adversarial input.
pub fn cut(
    vertices: &[[f32; 3]],
    triangles: &[[u32; 3]],
    plane: &Plane,
) -> Result<Section, SectionError> {
    // Validate the plane first.
    if !finite3(plane.origin) {
        return Err(SectionError::InvalidPlane("non-finite origin"));
    }
    if !finite3(plane.normal) {
        return Err(SectionError::InvalidPlane("non-finite normal"));
    }
    let nl = plane.normal[0].powi(2) + plane.normal[1].powi(2) + plane.normal[2].powi(2);
    if nl <= 1e-12 {
        return Err(SectionError::InvalidPlane("zero length normal"));
    }
    if triangles.is_empty() {
        return Err(SectionError::NoTriangles);
    }

    let nx = plane.normal[0];
    let ny = plane.normal[1];
    let nz = plane.normal[2];

    let mut segments: Vec<[[f32; 3]; 2]> = Vec::new();

    for tri in triangles {
        // Bounds and finiteness checks (P2-M1-T04: typed, never panics).
        let mut p = [[0.0f32; 3]; 3];
        for (k, i) in tri.iter().enumerate() {
            if *i as usize >= vertices.len() {
                return Err(SectionError::IndexOutOfRange);
            }
            p[k] = vertices[*i as usize];
            if !finite3(p[k]) {
                return Err(SectionError::NonFiniteVertex);
            }
        }

        // Signed distances to the plane.
        let mut d = [0.0f32; 3];
        let mut on = 0usize;
        for (k, pv) in p.iter().enumerate() {
            let s = (pv[0] - plane.origin[0]) * nx
                + (pv[1] - plane.origin[1]) * ny
                + (pv[2] - plane.origin[2]) * nz;
            d[k] = s;
            if s.abs() <= ON_PLANE_EPS {
                on += 1;
            }
        }

        // Any vertex on the plane -> degenerate, skip.
        if on > 0 {
            continue;
        }

        // Count sides; no strict crossing if all on one side.
        let mut pos = 0usize;
        let mut neg = 0usize;
        let mut lone = usize::MAX; // index of the single-opposite vertex
        for s in &d {
            if *s > 0.0 {
                pos += 1;
            } else {
                neg += 1;
            }
        }
        if pos == 0 || neg == 0 {
            continue;
        }
        // Exactly one vertex sits on its own side (no on-plane vertices).
        if pos == 1 {
            for (k, &s) in d.iter().enumerate() {
                if s > 0.0 {
                    lone = k;
                    break;
                }
            }
        } else {
            for (k, &s) in d.iter().enumerate() {
                if s < 0.0 {
                    lone = k;
                    break;
                }
            }
        }

        // Intersect the two edges from the lone vertex to the others.
        let a_idx = lone;
        let others: Vec<usize> = (0..3).filter(|&k| k != a_idx).collect();
        let mut pts = [[0.0f32; 3]; 2];
        for (n_out, &other) in others.iter().enumerate() {
            // edge (a_idx -> other)
            let da = d[a_idx];
            let dk = d[other];
            let t = da / (da - dk);
            pts[n_out] = [
                p[a_idx][0] + t * (p[other][0] - p[a_idx][0]),
                p[a_idx][1] + t * (p[other][1] - p[a_idx][1]),
                p[a_idx][2] + t * (p[other][2] - p[a_idx][2]),
            ];
        }

        // Drop zero-length segments (would be NaNs/duplicates otherwise).
        if near(pts[0], pts[1], ON_PLANE_EPS) {
            continue;
        }
        segments.push(pts);
    }

    Ok(Section { segments })
}
