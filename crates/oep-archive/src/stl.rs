//! Deterministic binary STL writer (P10-M3-T02 geometry escape hatch).
//!
//! STL is the smallest neutral mesh interchange format; it carries only
//! triangles, so it is cheap to write and open in every CAD/CAE viewer.
//! Output is a standard binary STL: an 80-byte header, a little-endian
//! uint32 triangle count, then per triangle a face normal + 3 vertices +
//! a 0 attribute byte count. For a given (name, mesh) the bytes are always
//! identical (deterministic; no timestamps).

use std::io::Write;

/// Why a binary STL write could not proceed. Deliberately narrow: the only
/// inputs we accept are already indexed triangles, so failures are index
/// errors, non-finite geometry, and IO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StlError {
    /// A triangle referenced an index beyond the positions length.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// Number of positions in the mesh.
        len: usize,
    },
    /// A vertex position is NaN or infinite.
    NonFiniteVertex(usize),
    /// An IO failure while writing.
    Io(String),
}

impl std::fmt::Display for StlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StlError::IndexOutOfRange { index, len } => {
                write!(
                    f,
                    "STL write: triangle index {index} out of range (positions: {len})"
                )
            }
            StlError::NonFiniteVertex(i) => write!(f, "STL write: vertex {i} not finite"),
            StlError::Io(e) => write!(f, "STL write: io error: {e}"),
        }
    }
}

impl std::error::Error for StlError {}

impl From<std::io::Error> for StlError {
    fn from(e: std::io::Error) -> Self {
        StlError::Io(e.to_string())
    }
}

/// Cross-product normal for `(b - a) x (c - a)`, unnormalized magnitude
/// carried so the caller can detect a zero-area (degenerate) triangle.
/// Computed in f64 for precision, returned as f64 components.
fn geometric_normal(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Write an indexed triangle mesh as a binary STL into `w`.
///
/// `name` goes into the 80-byte header (truncated to 72 bytes plus the
/// mandatory trailing `\0` region). Each face normal is computed from the
/// triangle itself (cross product, normalized); a zero-area triangle gets a
/// zero normal (readers treat it as "not a real facet"). Validation runs
/// before any byte is written, so a failed call emits nothing.
pub fn write_stl_binary<W: Write>(
    name: &str,
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> Result<(), StlError> {
    for (i, v) in positions.iter().enumerate() {
        if !v.iter().all(|c| c.is_finite()) {
            return Err(StlError::NonFiniteVertex(i));
        }
    }
    for tri in triangles {
        for &idx in tri {
            if idx as usize >= positions.len() {
                return Err(StlError::IndexOutOfRange {
                    index: idx,
                    len: positions.len(),
                });
            }
        }
    }

    // 80-byte header: the name, space-padded, last 4 bytes left as zeros.
    let mut header = [0u8; 80];
    let name_bytes = name.as_bytes();
    let n = name_bytes.len().min(72);
    header[..n].copy_from_slice(&name_bytes[..n]);
    w.write_all(&header)?;

    w.write_all(&(triangles.len() as u32).to_le_bytes())?;
    for tri in triangles {
        let a = positions[tri[0] as usize].map(|v| v as f64);
        let b = positions[tri[1] as usize].map(|v| v as f64);
        let c = positions[tri[2] as usize].map(|v| v as f64);
        let n = geometric_normal(a, b, c);
        let norm = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let (nx, ny, nz) = if norm > 1e-12 {
            (n[0] / norm, n[1] / norm, n[2] / norm)
        } else {
            (0.0, 0.0, 0.0)
        };
        w.write_all(&(nx as f32).to_le_bytes())?;
        w.write_all(&(ny as f32).to_le_bytes())?;
        w.write_all(&(nz as f32).to_le_bytes())?;
        for idx in tri {
            for c in positions[*idx as usize] {
                w.write_all(&c.to_le_bytes())?;
            }
        }
        w.write_all(&0u16.to_le_bytes())?;
    }
    Ok(())
}
