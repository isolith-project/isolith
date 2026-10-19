//! Face/edge topology reconstruction for tessellated meshes (P2-M3-T02).
//!
//! Given an indexed triangle mesh (`vertices` + `triangles`, the exact
//! contract surface the P2-M2-T01 internal `Mesh` type exposes), this module
//! rebuilds the combinatorial topology of the surface:
//!
//! * **edges** — every unique undirected edge of the triangle soup, with the
//!   list of incident triangles (faces);
//! * **edge classification** — an edge is *boundary* when exactly one face
//!   uses it (the open/unclosed boundary of the surface), *interior* when
//!   exactly two faces share it (a manifold edge), and *non-manifold* when
//!   more than two faces meet at it (flagged, never a crash);
//! * **face shells** — the connected components of triangles joined along
//!   shared edges, each flagged closed (no boundary edges) or open;
//! * **boundary loops** — the open contours traced along boundary edges;
//! * **open/unclosed flags** — per-shell and whole-mesh: a surface with zero
//!   boundary edges is *closed* (watertight), otherwise it is *open*.
//!
//! This is a clean-room, observation-free module: it is pure combinatorial
//! topology over indices and needs no CGR format knowledge, so it compiles
//! and test-flies before mesh extraction (P2-M2-T01) merges. Integration with
//! the internal mesh type is one call once both land:
//! `reconstruct(mesh.vertices(), mesh.triangles())`.

/// Errors produced by topology reconstruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopoError {
    /// The triangle list is empty — there is no surface to analyze.
    NoTriangles,
    /// A triangle referenced a vertex index that does not exist.
    IndexOutOfRange,
    /// A referenced vertex coordinate was NaN or infinite.
    NonFiniteVertex(usize),
}

/// A unique undirected edge of the triangle soup, with its incidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// The smaller vertex index of the edge.
    pub a: u32,
    /// The larger vertex index of the edge.
    pub b: u32,
    /// Indices of the triangles (faces) that use this edge.
    pub faces: Vec<u32>,
}

/// One face shell: a maximal connected set of triangles joined along shared
/// edges, with its open/closed boundary flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// Triangle indices of this shell (sorted, ascending).
    triangle_indices: Vec<u32>,
    /// `true` when this shell has no boundary edges (closed / watertight).
    is_closed: bool,
}

impl Face {
    /// The triangle indices of this shell.
    pub fn triangle_indices(&self) -> &[u32] {
        &self.triangle_indices
    }

    /// `true` when this shell is closed (zero boundary edges).
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }
}

/// The reconstructed topology of an indexed triangle mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    edges: Vec<Edge>,
    faces: Vec<Face>,
    boundary_loops: Vec<Vec<u32>>,
}

impl Topology {
    /// Number of unique undirected edges in the triangle soup.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Number of boundary edges (used by exactly one face).
    pub fn boundary_edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.faces.len() == 1).count()
    }

    /// Number of interior (manifold) edges (used by exactly two faces).
    pub fn interior_edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.faces.len() == 2).count()
    }

    /// Number of non-manifold edges (used by more than two faces).
    pub fn non_manifold_edge_count(&self) -> usize {
        self.edges.iter().filter(|e| e.faces.len() > 2).count()
    }

    /// The unique undirected edges with their face incidence.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The face shells (connected components of triangles).
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// The open contours traced along boundary edges. Empty for a closed mesh.
    pub fn boundary_loops(&self) -> &[Vec<u32>] {
        &self.boundary_loops
    }

    /// `true` when the whole mesh is closed: zero boundary edges AND zero
    /// non-manifold edges (a clean watertight surface).
    pub fn is_closed(&self) -> bool {
        self.boundary_edge_count() == 0 && self.non_manifold_edge_count() == 0
    }

    /// `true` when the mesh has any boundary (open/unclosed) edge.
    pub fn is_open(&self) -> bool {
        self.boundary_edge_count() > 0
    }
}

/// Canonical undirected edge key: the two endpoints sorted ascending.
fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Validate that every triangle index is in range and every referenced vertex
/// coordinate is finite.
fn validate(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<(), TopoError> {
    let n = vertices.len() as u32;
    for &t in triangles {
        for i in t {
            if i >= n {
                return Err(TopoError::IndexOutOfRange);
            }
            let p = vertices[i as usize];
            if !p.iter().all(|x| x.is_finite()) {
                return Err(TopoError::NonFiniteVertex(i as usize));
            }
        }
    }
    Ok(())
}

/// Trace open contours along the given boundary edges.
///
/// Boundary edges form one or more polylines/cycles on an open surface. For
/// a 2-manifold-with-boundary these are closed vertex loops; for a non-
/// manifold boundary the walk is best-effort (it stops when no unvisited
/// continuation exists) and never panics.
fn trace_boundary_loops(boundary: &[(u32, u32)]) -> Vec<Vec<u32>> {
    use std::collections::{BTreeMap, BTreeSet};
    if boundary.is_empty() {
        return Vec::new();
    }
    let mut adj: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(a, b) in boundary {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    let mut used: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut loops: Vec<Vec<u32>> = Vec::new();
    for &(a, b) in boundary {
        let key = edge_key(a, b);
        if !used.insert(key) {
            continue;
        }
        let mut verts = vec![a, b];
        let (mut prev, mut cur) = (a, b);
        let guard = boundary.len() + 2;
        let mut steps = 0usize;
        loop {
            steps += 1;
            if steps > guard {
                break;
            }
            // Candidate continuations from `cur`, excluding backtracking.
            let nexts: Vec<u32> = adj
                .get(&cur)
                .map(|v| v.iter().copied().filter(|&n| n != prev).collect())
                .unwrap_or_default();
            let next = nexts
                .iter()
                .copied()
                .find(|&n| n == a) // prefer closing back to start
                .or_else(|| nexts.first().copied());
            let Some(next) = next else { break };
            let ek = edge_key(cur, next);
            if !used.insert(ek) {
                break;
            }
            if next == a {
                break; // loop closed back at start vertex; do not duplicate it
            }
            verts.push(next);
            prev = cur;
            cur = next;
        }
        loops.push(verts);
    }
    loops
}

/// Reconstruct the face/edge topology of an indexed triangle mesh.
///
/// * `vertices` — vertex positions `[[x, y, z]; N]`.
/// * `triangles` — triangle vertex indices `[[a, b, c]; M]`.
///
/// Returns [`Topology`] on success, or a typed [`TopoError`] (empty triangle
/// list, out-of-range index, non-finite coordinate). Never panics on any
/// input, including degenerate, overlapping, or adversarial triangles.
pub fn reconstruct(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<Topology, TopoError> {
    if triangles.is_empty() {
        return Err(TopoError::NoTriangles);
    }
    validate(vertices, triangles)?;

    // 1. Unique undirected edges with face incidence.
    use std::collections::BTreeMap;
    let mut incidence: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    for (ti, &[a, b, c]) in triangles.iter().enumerate() {
        let t = ti as u32;
        for &(x, y) in &[(a, b), (b, c), (c, a)] {
            if x != y {
                incidence.entry(edge_key(x, y)).or_default().push(t);
            }
        }
    }
    let edges: Vec<Edge> = incidence
        .iter()
        .map(|(&(a, b), faces)| Edge {
            a,
            b,
            faces: faces.clone(),
        })
        .collect();

    // 2. Triangle adjacency via shared edges, then connected components.
    let n = triangles.len();
    let mut tadj: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for tris in incidence.values() {
        for (i, &x) in tris.iter().enumerate() {
            for &y in &tris[i + 1..] {
                tadj.entry(x).or_default().push(y);
                tadj.entry(y).or_default().push(x);
            }
        }
    }
    let mut visited = vec![false; n];
    let mut faces: Vec<Face> = Vec::new();
    for start in 0..n {
        if visited[start] {
            continue;
        }
        let mut comp: Vec<u32> = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(t) = stack.pop() {
            comp.push(t as u32);
            if let Some(nbrs) = tadj.get(&(t as u32)) {
                for &nb in nbrs {
                    if !visited[nb as usize] {
                        visited[nb as usize] = true;
                        stack.push(nb as usize);
                    }
                }
            }
        }
        comp.sort_unstable();
        let in_comp: std::collections::BTreeSet<u32> = comp.iter().copied().collect();
        let has_boundary = incidence
            .iter()
            .any(|(_, tris)| tris.len() == 1 && in_comp.contains(&tris[0]));
        faces.push(Face {
            triangle_indices: comp,
            is_closed: !has_boundary,
        });
    }

    // 3. Boundary loops.
    let boundary: Vec<(u32, u32)> = edges
        .iter()
        .filter(|e| e.faces.len() == 1)
        .map(|e| (e.a, e.b))
        .collect();
    let boundary_loops = trace_boundary_loops(&boundary);

    Ok(Topology {
        edges,
        faces,
        boundary_loops,
    })
}
