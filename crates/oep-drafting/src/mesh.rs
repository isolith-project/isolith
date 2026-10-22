//! The solid mesh value type the drafting workbench replays out of the
//! `oep-geo` feature graph.
//!
//! [`Mesh`] is an index/positions+triangles triangle mesh, the SAME slice
//! representation the `oep-geo` P6 exporters consume (P2-M2-T01 naming). The
//! drafting evaluator ([`crate::solid::SolidEvaluator`]) produces a [`Mesh`]
//! for every graph node instead of the reference bounding box, so view
//! projection has real edge geometry to work from — every projected edge in a
//! drawing is computed from these triangles, never fabricated.
//!
//! f64 everywhere (double-everywhere per P6-M1-T03). Deterministic, typed,
//! never panics on the operations provided here.

/// Number of vertices in a solid box.
const BOX_VERTS: usize = 8;
/// Number of triangles in a solid box (6 faces x 2).
const BOX_TRIS: usize = 12;
/// Number of true edges in a solid box.
const BOX_EDGES: usize = 12;

/// A triangle mesh: positions plus an index list of triangles.
///
/// Carries an explicit `edges` list (the true solid edges of the model). The
/// drafting projection reads THESE edges so a box projects as its 12 true
/// edges (each view: a clean rectangle outline) rather than tripping on the
/// per-face triangulation diagonals, which are tessellation artifacts, not
/// model edges. For a future generic mesh without an explicit edge list the
/// projector falls back to triangle boundaries.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// Vertex positions.
    pub positions: Vec<[f64; 3]>,
    /// Triangles as index triples into `positions` (right-hand order arbitrary,
    /// edge uniqueness does not depend on it).
    pub triangles: Vec<[u32; 3]>,
    /// True solid edges (pairs of position indices).
    pub edges: Vec<[usize; 2]>,
}

impl Mesh {
    /// An empty mesh (the evaluator's `start_value`).
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            triangles: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// True when the mesh has no edges.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// The axis-aligned box solid spanning `[0,0,0]`..`[lx,ly,lz]`.
    ///
    /// Uses 8 vertices, 12 triangles (the standard box tessellation) and an
    /// explicit edge list of the box's 12 TRUE edges, so projecting it yields
    /// exactly the four-edge rectangle outline of each standard view.
    pub fn box_solid(lx: f64, ly: f64, lz: f64) -> Self {
        debug_assert!(lx >= 0.0 && ly >= 0.0 && lz >= 0.0);
        let v: [[f64; 3]; BOX_VERTS] = [
            [0.0, 0.0, 0.0],
            [lx, 0.0, 0.0],
            [lx, ly, 0.0],
            [0.0, ly, 0.0],
            [0.0, 0.0, lz],
            [lx, 0.0, lz],
            [lx, ly, lz],
            [0.0, ly, lz],
        ];
        // 6 faces x 2 triangles; orientation is not sign convention (edge
        // projection only reads connectivity and positions).
        let t: [[u32; 3]; BOX_TRIS] = [
            [0, 3, 1],
            [0, 1, 2], // bottom (-z)
            [4, 5, 6],
            [4, 6, 7], // top (+z)
            [0, 4, 7],
            [0, 7, 3], // -x face
            [1, 2, 6],
            [1, 6, 5], // +x face
            [3, 7, 6],
            [3, 6, 2], // +y face
            [0, 1, 5],
            [0, 5, 4], // -y face
        ];
        // The 12 TRUE edges of the box (no face diagonals).
        let e: [[usize; 2]; BOX_EDGES] = [
            [0, 1],
            [1, 2],
            [2, 3],
            [3, 0], // bottom ring
            [4, 5],
            [5, 6],
            [6, 7],
            [7, 4], // top ring
            [0, 4],
            [1, 5],
            [2, 6],
            [3, 7], // verticals
        ];
        Self {
            positions: v.to_vec(),
            triangles: t.to_vec(),
            edges: e.to_vec(),
        }
    }

    /// Translate every vertex by `(dx, dy, dz)` (the `Offset` feature op).
    pub fn translated(&self, dx: f64, dy: f64, dz: f64) -> Self {
        Self {
            positions: self
                .positions
                .iter()
                .map(|p| [p[0] + dx, p[1] + dy, p[2] + dz])
                .collect(),
            triangles: self.triangles.clone(),
            edges: self.edges.clone(),
        }
    }

    /// Reflect every vertex about the plane `axis` (`x`/`y`/`z`) at `at`
    /// (the `Mirror` feature op).
    pub fn reflected(&self, axis: char, at: f64) -> Self {
        let map = |p: &[f64; 3]| match axis {
            'x' => [2.0 * at - p[0], p[1], p[2]],
            'y' => [p[0], 2.0 * at - p[1], p[2]],
            'z' => [p[0], p[1], 2.0 * at - p[2]],
            _ => *p,
        };
        Self {
            positions: self.positions.iter().map(map).collect(),
            triangles: self.triangles.clone(),
            edges: self.edges.clone(),
        }
    }

    /// Union (concatenate) two meshes into one (the `Union` feature op:
    /// bounding-box envelope at the bbox level extends to a mesh-level concat
    /// over disjoint inputs; a true boolean union is the kernel lane).
    pub fn extended(&self, other: &Mesh) -> Self {
        let n = self.positions.len() as u32;
        let mut positions = self.positions.clone();
        positions.extend_from_slice(&other.positions);
        let mut triangles = self.triangles.clone();
        triangles.extend(
            other
                .triangles
                .iter()
                .map(|t| [t[0] + n, t[1] + n, t[2] + n]),
        );
        let mut edges = self.edges.clone();
        edges.extend(
            other
                .edges
                .iter()
                .map(|e| [e[0] + n as usize, e[1] + n as usize]),
        );
        Self {
            positions,
            triangles,
            edges,
        }
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}
