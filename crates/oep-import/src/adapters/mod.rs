//! Adapter registry for the importer hub (P6-M4-T02).
//!
//! Each adapter maps ONE source format into the canonical [`oep_geo::Document`].
//! The registry is deliberately small and total: every [`SourceKind`] has an
//! adapter today (CGR both eras, 3DXML, STEP via the P4 read lane), so the
//! hub's `import_bytes` dispatch never panics on a missing handler.

pub mod cgr;
pub mod step;
pub mod xml3d;
