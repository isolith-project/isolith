//! Internal mesh model for the CGR reader (P2-M2-T01).
//!
//! A tessellated CGR payload is decoded into an indexed triangle mesh:
//! a `Vec<[f32; 3]>` of vertex positions and a `Vec<[u32; 3]>` of triangle
//! indices into that vertex list (the vertices/indices/triangles triple named
//! by P2-M2-T01). This is the internal type downstream P2 cards consume:
//! exporters (P2-M2-T02/T03 STL + glTF), attribute recovery (P2-M3-T01),
//! and topology reconstruction (P2-M3-T02).
//!
//! The type alone (Part 1 of `tests/golden_mesh.rs`) is format-independent
//! and fully deterministic. `extract` (Part 2/3) is the clean-room bridge to
//! real CGR bytes: it validates the observed header spine, scans the payload
//! for a structurally self-consistent indexed triangle stream, and returns
//! either that mesh or a *classified* skip (typed reason), never a panic —
//! the P2-M1-T04 tolerant-failure doctrine applied at mesh level.
//!
//! Honesty note (clean-room, observable-fact base only): the location of the
//! triangle stream inside the payload is [uncertain] until the P2-M1-T03
//! block walker and P2-M1-T06 diff harness land (see P1-M3-T03 report
//! section 8 and P1-M2-T01 hex notebook section 9.3). `extract` therefore
//! checks concrete structural hypotheses against the bytes and only reports
//! a mesh when one passes; otherwise it reports a classified skip with the
//! hypothesis that failed. It never invents geometry.

use std::fmt;

/// Errors produced by the mesh model and extractor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshError {
    /// A triangle index referenced a vertex position that does not exist.
    IndexOutOfRange,
    /// A vertex position is NaN or infinite.
    NonFiniteVertex(usize),
    /// A triangle has zero area (degenerate/duplicate indices).
    DegenerateTriangle,
    /// The bytes do not carry the observed CGR header spine.
    NotCgrPreamble,
    /// Classified skip: the payload scan found no structurally
    /// self-consistent indexed triangle stream under the tested hypotheses.
    MeshNotFound(String),
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::IndexOutOfRange => write!(f, "triangle index out of range"),
            MeshError::NonFiniteVertex(i) => write!(f, "vertex {i} not finite"),
            MeshError::DegenerateTriangle => write!(f, "degenerate triangle (zero area)"),
            MeshError::NotCgrPreamble => write!(f, "missing observed CGR header spine"),
            MeshError::MeshNotFound(detail) => {
                write!(
                    f,
                    "classified skip: no self-consistent triangle stream found ({detail})"
                )
            }
        }
    }
}

impl std::error::Error for MeshError {}

/// A triangle-area computation used by structural validation.
fn triangle_area(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> f32 {
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cx = u[1] * v[2] - u[2] * v[1];
    let cy = u[2] * v[0] - u[0] * v[2];
    let cz = u[0] * v[1] - u[1] * v[0];
    0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
}

/// Internal indexed-triangle mesh (vertices / indices / triangles), the
/// canonical mesh carrier of the `oep-cgr` crate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// An empty mesh.
    pub fn new() -> Self {
        Self::default()
    }

    /// The vertex position list.
    pub fn vertices(&self) -> &[[f32; 3]] {
        &self.vertices
    }

    /// The triangle index list (each entry is three vertex indices).
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Append a vertex, returning its index. No validation performed here —
    /// call [`Mesh::validate`] to run invariant checks over the whole mesh.
    pub fn push_vertex(&mut self, v: [f32; 3]) -> u32 {
        self.vertices.push(v);
        (self.vertices.len() - 1) as u32
    }

    /// Push a triangle, rejecting indices that fall outside the current
    /// vertex list (bounds-checked at build time).
    pub fn push_triangle(&mut self, a: u32, b: u32, c: u32) -> Result<(), MeshError> {
        let n = self.vertices.len() as u32;
        if a >= n || b >= n || c >= n {
            return Err(MeshError::IndexOutOfRange);
        }
        self.triangles.push([a, b, c]);
        Ok(())
    }

    /// Push a triangle without a bounds check (only used to construct a
    /// deliberately invalid mesh for validation tests).
    #[doc(hidden)]
    pub fn push_triangle_raw(&mut self, a: u32, b: u32, c: u32) {
        self.triangles.push([a, b, c]);
    }

    /// Validate structural invariants:
    ///   * every triangle index is within `vertices`;
    ///   * every referenced vertex position is finite;
    ///   * no triangle is degenerate (zero area).
    pub fn validate(&self) -> Result<(), MeshError> {
        let n = self.vertices.len() as u32;
        for &t in &self.triangles {
            for &i in &t {
                if i >= n {
                    return Err(MeshError::IndexOutOfRange);
                }
            }
            for &i in &t {
                let v = self.vertices[i as usize];
                if !v.iter().all(|x| x.is_finite()) {
                    return Err(MeshError::NonFiniteVertex(i as usize));
                }
            }
            if triangle_area(
                self.vertices[t[0] as usize],
                self.vertices[t[1] as usize],
                self.vertices[t[2] as usize],
            ) <= f32::EPSILON
            {
                return Err(MeshError::DegenerateTriangle);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// clean-room extractor: observed header spine + structural payload scan
// ---------------------------------------------------------------------------

/// CGR magic, u32 little-endian == 1, at file offset 0x00 (hex notebook
/// P1-M2-T01; P2-M1-T01 `header::{MAGIC, PREAMBLE_OFF, PREAMBLE_LEN,
/// TEMPLATE_OFF, TEMPLATE}` mirrors the same observed facts).
const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const PREAMBLE_LEN: usize = 0x60; // bytes 0x04..0x63 are zero
const TEMPLATE_OFF: usize = 0x68;
const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

fn le_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn be_u16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

fn le_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// Validate the observed CGR header spine (magic, zero preamble, invariant
/// template). Returns `Err(MeshError::NotCgrPreamble)` on mismatch. This is
/// intentionally self-contained so `extract` does not depend on the unmerged
/// P2-M1-T01 `header` module (the constants are the same observed facts).
fn check_preamble(b: &[u8]) -> Result<(), MeshError> {
    if b.len() < TEMPLATE_OFF + TEMPLATE.len() {
        return Err(MeshError::NotCgrPreamble);
    }
    if b[0..4] != MAGIC {
        return Err(MeshError::NotCgrPreamble);
    }
    if b[4..4 + PREAMBLE_LEN] != [0u8; PREAMBLE_LEN] {
        return Err(MeshError::NotCgrPreamble);
    }
    if b[TEMPLATE_OFF..TEMPLATE_OFF + TEMPLATE.len()] != TEMPLATE {
        return Err(MeshError::NotCgrPreamble);
    }
    Ok(())
}

/// Candidate (offset, vertex_count) of a contiguous little-endian f32
/// coordinate run that "looks like" vertex positions.
fn scan_vertex_pools(b: &[u8]) -> Vec<(usize, usize)> {
    let mut pools = Vec::new();
    let n = b.len();
    let mut i = 0usize;
    while i + 12 <= n {
        let ok = (0..3).all(|k| {
            let v = le_f32(b, i + 4 * k);
            v.is_finite() && v.abs() < 1e6
        });
        if ok {
            let mut j = i + 12;
            while j + 12 <= n {
                let good = (0..3).all(|k| {
                    let v = le_f32(b, j + 4 * k);
                    v.is_finite() && v.abs() < 1e6
                });
                if !good {
                    break;
                }
                j += 12;
            }
            if (j - i) / 12 >= 20 {
                pools.push((i, (j - i) / 12));
            }
            i = j;
        } else {
            i += 4;
        }
    }
    pools
}

/// Candidate index streams: contiguous u16 values all `< 0x4000`, of a given
/// width flavour (`be` flag), length >= 60 and divisible by 3.
fn scan_index_streams(b: &[u8], be: bool) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let n = b.len();
    let mut i = 0usize;
    while i + 2 <= n {
        let v = if be { be_u16(b, i) } else { le_u16(b, i) };
        if v < 0x4000 {
            let mut j = i + 2;
            while j + 2 <= n {
                let w = if be { be_u16(b, j) } else { le_u16(b, j) };
                if !(w < 0x4000) {
                    break;
                }
                j += 2;
            }
            let len = (j - i) / 2;
            if len >= 60 && len.is_multiple_of(3) {
                runs.push((i, len));
            }
            // Advance 2 bytes even on a qualifier: an aligned run inside a
            // larger low-value region must still be discoverable from its own
            // start, so never skip over it by jumping to j.
            i += 2;
        } else {
            i += 2;
        }
    }
    runs
}

/// Try to assemble a structurally-valid mesh from a vertex pool and index
/// stream hypothesis. Returns `Ok(mesh)` only when every index is in range,
/// at least half the pool's leading indices are used, and the vast majority
/// of triangles are non-degenerate (i.e. the pairing is highly
/// self-consistent — chance coincidence would fail these checks).
fn try_assemble(
    b: &[u8],
    pool_off: usize,
    pool_len: usize,
    idx_off: usize,
    len: usize,
    be: bool,
) -> Option<Mesh> {
    let max_idx = if be {
        (0..len).map(|k| be_u16(b, idx_off + 2 * k) as u32).max()?
    } else {
        (0..len).map(|k| le_u16(b, idx_off + 2 * k) as u32).max()?
    };
    // Every referenced index must be within the pool.
    if (max_idx as usize) >= pool_len {
        return None;
    }
    // Per-vertex coordinates.
    let coords: Vec<[f32; 3]> = (0..=max_idx as usize)
        .map(|k| {
            [
                le_f32(b, pool_off + 12 * k),
                le_f32(b, pool_off + 12 * k + 4),
                le_f32(b, pool_off + 12 * k + 8),
            ]
        })
        .collect();

    let mut mesh = Mesh::new();
    for c in &coords {
        mesh.push_vertex(*c);
    }

    let mut used = std::collections::HashSet::new();
    let mut degenerate = 0u32;
    let tri_count = len / 3;
    for t in 0..tri_count {
        let a = if be {
            be_u16(b, idx_off + 6 * t) as u32
        } else {
            le_u16(b, idx_off + 6 * t) as u32
        };
        let b_ = if be {
            be_u16(b, idx_off + 6 * t + 2) as u32
        } else {
            le_u16(b, idx_off + 6 * t + 2) as u32
        };
        let c_ = if be {
            be_u16(b, idx_off + 6 * t + 4) as u32
        } else {
            le_u16(b, idx_off + 6 * t + 4) as u32
        };
        mesh.push_triangle(a, b_, c_).ok()?;
        used.insert(a);
        used.insert(b_);
        used.insert(c_);
        if triangle_area(coords[a as usize], coords[b_ as usize], coords[c_ as usize])
            <= f32::EPSILON
        {
            degenerate += 1;
        }
    }
    // Self-consistency thresholds: strictly more than half of the pool's
    // leading vertices are actually referenced, and < 10% of triangles are
    // degenerate. False-positive pairings (random floats + random u16s) do
    // not meet both.
    if used.len() <= (pool_len / 2) {
        return None;
    }
    if degenerate * 10 >= tri_count as u32 {
        return None;
    }
    Some(mesh)
}

/// Extract a tessellated triangle mesh from raw CGR bytes.
///
/// Returns `Ok(mesh)` when a structurally self-consistent indexed triangle
/// stream is found in the payload; `Err(MeshError::MeshNotFound(detail))`
/// (a classified skip) when the payload does not contain one under the
/// tested hypotheses; `Err(MeshError::NotCgrPreamble)` when the bytes are
/// not a CGR container. Never panics, never fabricates geometry.
pub fn extract(b: &[u8]) -> Result<Mesh, MeshError> {
    check_preamble(b)?;
    let pools = scan_vertex_pools(b);
    // Try big-endian then little-endian index width hypotheses. A debug
    // transcript of every candidate seen is emitted when CGR_DEBUG_SPEW=1
    // (used by the P2-M2-T01 RE note to record probe output on real material).
    let mut report = String::new();
    if std::env::var("CGR_DEBUG_SPEW").is_ok() {
        report.push_str(&format!("  pools: {:?}\n", pools));
    }
    for be in [true, false] {
        for (idx_off, len) in scan_index_streams(b, be) {
            if std::env::var("CGR_DEBUG_SPEW").is_ok() {
                report.push_str(&format!(
                    "  {} index stream @0x{idx_off:x} len {len}: {:?}\n",
                    if be { "BE" } else { "LE" },
                    (0..len.min(16))
                        .map(|k| if be {
                            be_u16(b, idx_off + 2 * k)
                        } else {
                            le_u16(b, idx_off + 2 * k)
                        })
                        .collect::<Vec<_>>()
                ));
            }
            // The vertex pool must lie entirely before the index stream.
            for &(pool_off, pool_len) in &pools {
                if pool_off + 12 * pool_len <= idx_off {
                    if let Some(mesh) = try_assemble(b, pool_off, pool_len, idx_off, len, be) {
                        return Ok(mesh);
                    }
                }
            }
        }
    }
    if std::env::var("CGR_DEBUG_SPEW").is_ok() {
        eprint!("{report}");
    }
    Err(MeshError::MeshNotFound(format!(
        "scan {} bytes: {} vertex-pool candidates, no self-consistent index pairing",
        b.len(),
        pools.len()
    )))
}
