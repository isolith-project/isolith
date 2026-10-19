//! Shared error type and geometry helpers for the P6 exporters.
//!
//! Every exporter produces typed errors and never panics. The error variants
//! are the same across formats (index bounds, non-finite vertices,
//! degenerate facets, empty mesh, I/O), so a single [`ExportError`] is shared
//! and each format module re-exports it under its familiar name
//! (`StlError`, `GltfError`, ...) via a type alias.

use std::fmt;

/// A triangle with zero area under the right-hand rule (duplicate or
/// co-linear vertices) -- produces no meaningful facet.
pub const DEGENERATE_EPS: f32 = 1e-12;

/// Typed error produced by any of the P6 exporters.
#[derive(Debug)]
pub enum ExportError {
    /// A triangle index referenced a vertex position that does not exist.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of positions in the mesh.
        len: usize,
    },
    /// A vertex position is NaN or infinite.
    NonFiniteVertex(usize),
    /// A triangle is degenerate (zero area / co-linear / duplicated).
    DegenerateTriangle,
    /// The mesh has no usable (non-degenerate) primitives to write.
    EmptyMesh,
    /// Underlying I/O failure while writing.
    Io(std::io::Error),
}

impl PartialEq for ExportError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::IndexOutOfRange { index: a, len: la },
                Self::IndexOutOfRange { index: b, len: lb },
            ) => a == b && la == lb,
            (Self::NonFiniteVertex(a), Self::NonFiniteVertex(b)) => a == b,
            (Self::DegenerateTriangle, Self::DegenerateTriangle) => true,
            (Self::EmptyMesh, Self::EmptyMesh) => true,
            (Self::Io(_), Self::Io(_)) => true,
            _ => false,
        }
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfRange { index, len } => {
                write!(f, "mesh index {index} out of range (positions: {len})")
            }
            Self::NonFiniteVertex(i) => write!(f, "vertex {i} is non-finite"),
            Self::DegenerateTriangle => write!(f, "degenerate triangle (zero area)"),
            Self::EmptyMesh => write!(f, "mesh has no usable primitives"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Cross product `u x v` as an f32 triple.
pub(crate) fn cross(u: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Unit vector along `v`, or `None` when `v` is ~zero (degenerate facet).
pub(crate) fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if mag.is_finite() && mag > DEGENERATE_EPS {
        Some([v[0] / mag, v[1] / mag, v[2] / mag])
    } else {
        None
    }
}

/// Right-hand-rule unit normal of a triangle (zero-area -> `None`).
pub(crate) fn facet_normal(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> Option<[f32; 3]> {
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    unit(cross(u, v))
}

/// Resolve a triangle's three positions, bounds-checked and finite-checked.
pub(crate) fn triangle_positions(
    positions: &[[f32; 3]],
    tri: &[u32; 3],
) -> Result<[[f32; 3]; 3], ExportError> {
    let mut out = [[0.0f32; 3]; 3];
    for (k, &idx) in tri.iter().enumerate() {
        let i = idx as usize;
        if i >= positions.len() {
            return Err(ExportError::IndexOutOfRange {
                index: idx,
                len: positions.len(),
            });
        }
        let v = positions[i];
        if !v.iter().all(|c| c.is_finite()) {
            return Err(ExportError::NonFiniteVertex(i));
        }
        out[k] = v;
    }
    Ok(out)
}

/// Canonical 2D endpoint used to dedupe undirected projected edges.
pub(crate) type Edge2 = [[f32; 2]; 2];

/// Compare two 2D endpoints lexicographically; returns `true` when
/// `a <= b` (total order over the finite floats we emit).
fn le2(a: [f32; 2], b: [f32; 2]) -> bool {
    a[0].total_cmp(&b[0])
        .then_with(|| a[1].total_cmp(&b[1]))
        .is_le()
}

/// Unique undirected edges of the mesh projected onto the XY plane.
///
/// Contract (matches `tools/gen_golden_box_{svg,pdf}.py`): an undirected
/// projected edge is keyed by its two endpoints normalized to canonical order
/// (lexicographically smaller endpoint first); duplicates collapse to one;
/// edges whose projected endpoints coincide (zero-length in this projection)
/// are dropped; the returned order is FIRST-SEEN over the triangle list
/// (per triangle: edges (a,b), (b,c), (c,a)).
pub(crate) fn unique_projected_edges(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
) -> Result<Vec<Edge2>, ExportError> {
    let mut out: Vec<Edge2> = Vec::new();
    for tri in triangles {
        for (i, j) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let pa = triangle_positions(positions, tri)?[i];
            let pb = triangle_positions(positions, tri)?[j];
            let p2a = [pa[0], pa[1]];
            let p2b = [pb[0], pb[1]];
            if p2a == p2b {
                continue; // zero-length in this projection
            }
            let e: Edge2 = if le2(p2a, p2b) {
                [p2a, p2b]
            } else {
                [p2b, p2a]
            };
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    Ok(out)
}

/// XY bounds over a list of projected endpoints (min/max per axis).
pub(crate) fn bounds2(edges: &[Edge2]) -> ([f32; 2], [f32; 2]) {
    let mut mn = [f32::INFINITY; 2];
    let mut mx = [f32::NEG_INFINITY; 2];
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
