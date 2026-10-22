//! Standard-view projection: turn a solid mesh into 2D projected-edge sets.
//!
//! Each [`ViewKind`] maps to a fixed orthonormal in-plane frame `(u, v)` of
//! the model, and a projected edge stores its two endpoints' 2D coordinates
//! `(dot(p,u), dot(p,v))`. This is the SAME projected-edge discipline as the
//! `oep-geo` exporters (deterministic first-seen dedupe of undirected,
//! canonical-order edges, zero-length drops), generalised from the hard-coded
//! XY plane to any standard view plane — every edge a drawing emits is
//! computed from the mesh here, never fabricated.
//!
//! View frames (documented in `docs/geometry-drawing-schema.md`):
//!
//! - Front: `u = X`, `v = Z` (length × height).
//! - Top: `u = X`, `v = Y` (length × width).
//! - Right: `u = Y`, `v = Z` (width × height).
//!
//! The first/third-angle convention does not change the view CONTENT (the
//! projected geometry is identical); it changes only how the views are
//! ARRANGED on the sheet, which is the layout lane's job ([`crate::layout`]).

use oep_geo::ViewKind;

use crate::mesh::Mesh;

/// A 2D projected line segment (two endpoints in view coordinates).
pub type Edge2 = [[f64; 2]; 2];

/// The orthonormal in-plane frame `(u, v)` for a standard view.
pub fn view_basis(kind: ViewKind) -> ([f64; 3], [f64; 3]) {
    match kind {
        ViewKind::Front => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ViewKind::Top => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ViewKind::Right => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    }
}

/// Compare two 2D endpoints — total order over the finite floats we emit.
fn le2(a: [f64; 2], b: [f64; 2]) -> bool {
    a[0].total_cmp(&b[0])
        .then_with(|| a[1].total_cmp(&b[1]))
        .is_le()
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Project the mesh's TRUE EDGES onto a standard view plane.
///
/// Uses the mesh's explicit edge list when present (a box then projects to
/// its clean 4-edge rectangle outline) and falls back to triangle boundaries
/// for a generic mesh with no edge list. Contract (same as the P6 exporters'
/// projected-edge discipline): edges are keyed by their two endpoints in
/// canonical order (smaller first); duplicates collapse to one; edges whose
/// projected endpoints coincide (zero-length in this projection) are dropped;
/// order is FIRST-SEEN. Tolerant on junk input: out-of-range indices and
/// non-finite positions are skipped, never panicked on.
pub fn project_edges(mesh: &Mesh, kind: ViewKind) -> Vec<Edge2> {
    let (u, v) = view_basis(kind);
    let mut out: Vec<Edge2> = Vec::new();

    let visit = |ai: usize, bi: usize, out: &mut Vec<Edge2>| {
        let (pa, pb) = match (mesh.positions.get(ai), mesh.positions.get(bi)) {
            (Some(a), Some(b)) => (*a, *b),
            _ => return,
        };
        let p2a = [dot(pa, u), dot(pa, v)];
        let p2b = [dot(pb, u), dot(pb, v)];
        if !(p2a.iter().all(|c| c.is_finite()) && p2b.iter().all(|c| c.is_finite())) {
            return;
        }
        if p2a == p2b {
            return; // zero-length in this projection
        }
        let e: Edge2 = if le2(p2a, p2b) {
            [p2a, p2b]
        } else {
            [p2b, p2a]
        };
        if !out.contains(&e) {
            out.push(e);
        }
    };

    if !mesh.edges.is_empty() {
        for e in &mesh.edges {
            visit(e[0], e[1], &mut out);
        }
    } else {
        for tri in &mesh.triangles {
            for (ai, bi) in [(tri[0] as usize, tri[1] as usize), (1, 2), (2, 0)] {
                visit(ai, bi, &mut out);
            }
        }
    }
    out
}

/// `(min, max)` corner bounds over a set of projected edges.
pub fn bounds2(edges: &[Edge2]) -> ([f64; 2], [f64; 2]) {
    let mut mn = [f64::INFINITY; 2];
    let mut mx = [f64::NEG_INFINITY; 2];
    for e in edges {
        for p in e {
            for a in 0..2 {
                mn[a] = mn[a].min(p[a]);
                mx[a] = mx[a].max(p[a]);
            }
        }
    }
    (mn, mx)
}

/// The `(width, height)` of the projected edge set (max - min per axis).
pub fn extent(edges: &[Edge2]) -> (f64, f64) {
    let (mn, mx) = bounds2(edges);
    (mx[0] - mn[0], mx[1] - mn[1])
}
