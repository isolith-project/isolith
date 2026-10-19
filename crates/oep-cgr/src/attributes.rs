//! Optional per-vertex attribute recovery for the CGR reader (P2-M3-T01).
//!
//! The internal mesh from [`crate::mesh`] carries positions + triangles.
//! CGR payloads MAY also carry per-vertex attributes (normals first, colors
//! and UVs as later cards observe them). This module recovers those
//! attributes "**where present**": a payload with an identifiable per-vertex
//! normal table whose length matches the mesh vertex list yields `Some`,
//! a payload without one yields the honest absent `None`, and structurally
//! unreadable input yields a typed [`AttributeError`] — never a panic, never
//! a fabricated normal (the P2-M1-T04 tolerant-failure doctrine applied to
//! attributes).
//!
//! Byte-level rationale and negative evidence (raw unit-normal f32 runs are
//! NOT found in either observed CGR family — absence is a real outcome) are
//! recorded in `docs/re-notes/oep-cgr-vertex-attributes.md` (P2-M3-T01).

use crate::mesh::Mesh;

/// Per-vertex attributes recovered from a CGR payload, keyed by count to the
/// mesh vertex list they annotate.
#[derive(Debug, Clone, PartialEq)]
pub struct VertexAttributes {
    /// Number of vertices the attribute set annotates (== `mesh.vertices().len()`).
    vertex_count: usize,
    /// Optional per-vertex unit normals, one `[nz, ny, nz]` per vertex.
    normals: Option<Vec<[f32; 3]>>,
}

/// Errors produced during attribute recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeError {
    /// The bytes do not carry the observed CGR header spine, so no honest
    /// record walk is possible.
    NotCgrPreamble,
    /// A per-vertex normal table was found whose length does not match the
    /// mesh vertex list (count drift = structural inconsistency).
    NormalCountMismatch {
        /// Normals actually decoded.
        found: usize,
        /// Vertices in the mesh.
        expected: usize,
    },
}

impl std::fmt::Display for AttributeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeError::NotCgrPreamble => write!(f, "missing observed CGR header spine"),
            AttributeError::NormalCountMismatch { found, expected } => {
                write!(
                    f,
                    "normal table length {found} != vertex list length {expected}"
                )
            }
        }
    }
}

impl std::error::Error for AttributeError {}

impl VertexAttributes {
    /// An empty attribute set annotating `vertex_count` vertices.
    pub fn none(vertex_count: usize) -> Self {
        Self {
            vertex_count,
            normals: None,
        }
    }

    /// Attributes carrying `normals`, one per vertex. Returns
    /// [`AttributeError::NormalCountMismatch`] when the table length differs
    /// from `vertex_count` (a count mismatch is a structural inconsistency,
    /// not something to silently tolerate).
    pub fn with_normals(
        normals: Vec<[f32; 3]>,
        vertex_count: usize,
    ) -> Result<Self, AttributeError> {
        if normals.len() != vertex_count {
            return Err(AttributeError::NormalCountMismatch {
                found: normals.len(),
                expected: vertex_count,
            });
        }
        Ok(Self {
            vertex_count,
            normals: Some(normals),
        })
    }

    /// The number of vertices this attribute set annotates.
    pub fn count(&self) -> usize {
        self.vertex_count
    }

    /// The recovered per-vertex normals, or `None` when the payload carries none.
    pub fn normals(&self) -> Option<&[[f32; 3]]> {
        self.normals.as_deref()
    }
}

// Observed CGR header spine facts (hex notebook P1-M2-T01; mirrors
// `crate::mesh`'s own clean-room re-derivation of the same observed bytes).
const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const PREAMBLE_LEN: usize = 0x60;
const TEMPLATE_OFF: usize = 0x68;
const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

/// True when `b` carries the observed CGR header spine.
fn has_cgr_spine(b: &[u8]) -> bool {
    b.len() >= TEMPLATE_OFF + TEMPLATE.len()
        && b[0..4] == MAGIC
        && b[4..4 + PREAMBLE_LEN] == [0u8; PREAMBLE_LEN]
        && b[TEMPLATE_OFF..TEMPLATE_OFF + TEMPLATE.len()] == TEMPLATE
}

/// The maximal run of contiguous little-endian f32 unit-length triples found
/// in `b` starting at or after `start`, returned as `(offset, length)` of
/// triples. Returns `None` when no such run of at least `min_len` triples
/// exists. A triple is "unit" when its Euclidean norm is within [`UNIT_TOL`]
/// of 1.0 (raw f32 normals, per the observation notebook).
fn longest_unit_normal_run(b: &[u8], start: usize, min_len: usize) -> Option<(usize, usize)> {
    const UNIT_TOL: f32 = 2e-3;
    let n = b.len();
    let mut best: Option<(usize, usize)> = None;
    let mut i = start;
    while i + 12 <= n {
        let mut tri_unit = [0.0f32; 3];
        for (k, v) in tri_unit.iter_mut().enumerate() {
            let off = i + 4 * k;
            *v = f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
        }
        let norm =
            (tri_unit[0] * tri_unit[0] + tri_unit[1] * tri_unit[1] + tri_unit[2] * tri_unit[2])
                .sqrt();
        if (norm - 1.0).abs() <= UNIT_TOL {
            let mut j = i + 12;
            while j + 12 <= n {
                let mut t2 = [0.0f32; 3];
                for (k, v) in t2.iter_mut().enumerate() {
                    let off = j + 4 * k;
                    *v = f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
                }
                let n2 = (t2[0] * t2[0] + t2[1] * t2[1] + t2[2] * t2[2]).sqrt();
                if (n2 - 1.0).abs() > UNIT_TOL {
                    break;
                }
                j += 12;
            }
            let len = (j - i) / 12;
            if len >= min_len && best.is_none_or(|(_, bl)| len > bl) {
                best = Some((i, len));
            }
            i = j;
        } else {
            i += 4;
        }
    }
    best
}

/// Recover optional per-vertex attributes from `bytes` for `mesh`.
///
/// Tolerant, "where present" behavior:
///   * bytes without the observed CGR spine → [`AttributeError::NotCgrPreamble`];
///   * payload with no identifiable normal table (no unit-normal f32 run of
///     at least [`MIN_NORMAL_RUN`] triples) → `Ok` with `normals == None`;
///   * payload whose only unit-normal run length mismatches the vertex list →
///     [`AttributeError::NormalCountMismatch`] (structural inconsistency);
///   * payload with a unit-normal run matching the vertex count → those
///     normals, decoded in order.
///
/// It never panics and never fabricates attributes.
///
/// The [`MIN_NORMAL_RUN`] floor is deliberate: on real material, raw
/// unit-f32 runs of 8+ triples are NOT found in either observed family
/// (negative evidence, doc `oep-cgr-vertex-attributes.md`), so a short stray
/// run in packed float noise is read as "absent", not as a skewed table.
pub fn recover_attributes(bytes: &[u8], mesh: &Mesh) -> Result<VertexAttributes, AttributeError> {
    if !has_cgr_spine(bytes) {
        return Err(AttributeError::NotCgrPreamble);
    }
    let vertex_count = mesh.vertices().len();
    match longest_unit_normal_run(bytes, 0x78, MIN_NORMAL_RUN) {
        Some((run_off, len)) if len == vertex_count => {
            // Decode the matched table (run is aligned to 12-byte triples).
            let normals = (0..len)
                .map(|k| {
                    let off = run_off + 12 * k;
                    [
                        f32::from_le_bytes([
                            bytes[off],
                            bytes[off + 1],
                            bytes[off + 2],
                            bytes[off + 3],
                        ]),
                        f32::from_le_bytes([
                            bytes[off + 4],
                            bytes[off + 5],
                            bytes[off + 6],
                            bytes[off + 7],
                        ]),
                        f32::from_le_bytes([
                            bytes[off + 8],
                            bytes[off + 9],
                            bytes[off + 10],
                            bytes[off + 11],
                        ]),
                    ]
                })
                .collect();
            VertexAttributes::with_normals(normals, vertex_count)
        }
        Some((_off, len)) => Err(AttributeError::NormalCountMismatch {
            found: len,
            expected: vertex_count,
        }),
        None => Ok(VertexAttributes::none(vertex_count)),
    }
}

/// Minimum length (in triples) for a unit-normal run to be treated as a real
/// normal table rather than floating-point noise.
pub const MIN_NORMAL_RUN: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;

    fn spine() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&MAGIC);
        b.extend_from_slice(&[0u8; PREAMBLE_LEN]);
        b.extend_from_slice(&0x10CD_3600u32.to_le_bytes());
        b.extend_from_slice(&TEMPLATE);
        b
    }

    #[test]
    fn absent_normals_on_empty_payload() {
        let m = Mesh::new();
        let a = recover_attributes(&spine(), &m).expect("absence not an error");
        assert!(a.normals().is_none());
        assert_eq!(a.count(), 0);
    }

    #[test]
    fn count_mismatch_is_typed_error() {
        let mut m = Mesh::new();
        let _ = m.push_vertex([0.0, 0.0, 0.0]);
        // A 12-long normal table vs a 1-vertex mesh -> mismatch (>= MIN run).
        let mut b = spine();
        for _ in 0..12 {
            b.extend_from_slice(&[0.0f32, 0.0, 1.0].map(|v| v.to_le_bytes()).concat());
        }
        match recover_attributes(&b, &m) {
            Err(AttributeError::NormalCountMismatch {
                found: 12,
                expected: 1,
            }) => {}
            other => panic!("expected mismatch, got {:?}", other),
        }
    }

    #[test]
    fn short_stray_unit_run_reads_as_absent() {
        // A single unit triple in packed noise is NOT a normal table
        // (below MIN_NORMAL_RUN); recovery must say "absent", never error.
        let mut b = spine();
        for _ in 0..3 {
            b.extend_from_slice(&[0.0f32, 0.0, 1.0].map(|v| v.to_le_bytes()).concat());
        }
        let m = Mesh::new();
        let a = recover_attributes(&b, &m).expect("stray short run is absence, not error");
        assert!(a.normals().is_none());
    }
}
