//! Vendor-neutral facet types for the oep-step reader (P4 read lane).
//!
//! These are the reader's coined, vendor-neutral carriers: they hold the
//! parsed STEP content in shapes the rest of the platform understands and
//! do NOT depend on the wrapped OSS STEP library. The library's own types
//! never cross this surface (vendor-neutral seam, workstreams/P4-M1/step-library-adr.md);
//! a future swap of the underlying library behind an unchanged API replaces
//! only the private walk in `lib.rs`.

/// Facts about one STEP shell (a set of faces forming a boundary).
#[derive(Debug, Clone, PartialEq)]
pub struct ShellFacts {
    /// True when the shell is a watertight `CLOSED_SHELL`.
    pub closed: bool,
    /// Number of faces in the shell.
    pub faces: usize,
}

/// Axis-aligned bounding box over parsed vertices (min/max corners).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacetBbox {
    /// Minimum corner `[x, y, z]`.
    pub min: [f64; 3],
    /// Maximum corner `[x, y, z]`.
    pub max: [f64; 3],
}

/// The parsed facet content of a STEP part, in a vendor-neutral shape.
///
/// Points are f64 triples; triangles index into [`Self::vertices`]. The
/// values are real observables from the file (coordinates, loop membership),
/// never estimates or reconstructed metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct FacetedModel {
    /// Product name (from the STEP `PRODUCT.name`; falls back to the
    /// `FILE_NAME` file stem when the product cluster is not parseable by the
    /// underlying library  -  recorded honestly, never fabricated).
    pub product_name: String,
    /// One entry per shell found in the part.
    pub shells: Vec<ShellFacts>,
    /// All distinct vertices in insertion order.
    pub vertices: Vec<[f64; 3]>,
    /// Triangle faces; each entry is three indices into [`Self::vertices`].
    pub triangles: Vec<[u32; 3]>,
    /// Bounding box over every vertex; `None` when there are no vertices.
    pub bbox: Option<FacetBbox>,
}

impl FacetedModel {
    /// Total face count across all shells.
    pub fn face_count(&self) -> usize {
        self.shells.iter().map(|s| s.faces).sum()
    }
}

/// What the reader learned about the source file while parsing it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadReport {
    /// The `FILE_NAME` file name from the STEP header, when present.
    pub file_name: Option<String>,
    /// The identified schema (AP family / edition / stage) as text, when
    /// present.
    pub schema: Option<String>,
    /// Non-fatal notes: content the underlying library dropped or could not
    /// model. Recorded, never hidden.
    pub warnings: Vec<String>,
    /// How many items the underlying library dropped (the report size).
    pub dropped: usize,
}

/// The result of a successful [`crate::parse`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReadOutcome {
    /// The parsed, vendor-neutral facet content.
    pub facets: FacetedModel,
    /// Source identity and honesty notes.
    pub report: ReadReport,
}

/// Compute the axis-aligned box over a vertex stream (insertion order).
///
/// Returns `None` when there are no vertices.
pub(crate) fn bbox_over(vertices: &[[f64; 3]]) -> Option<FacetBbox> {
    let mut iter = vertices.iter();
    let first = *iter.next()?;
    let mut min = first;
    let mut max = first;
    for v in iter {
        for i in 0..3 {
            if v[i] < min[i] {
                min[i] = v[i];
            }
            if v[i] > max[i] {
                max[i] = v[i];
            }
        }
    }
    Some(FacetBbox { min, max })
}

/// Shortest-round-trip decimal for a coordinate, deterministic across runs.
pub(crate) fn key_point(p: [f64; 3]) -> [u64; 3] {
    [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
}

/// Reason string used when the part contains no shell-based geometry.
pub(crate) const NO_FACETS_REASON: &str =
    "part contains no shell-based surface model (faceted geometry) this reader maps";
