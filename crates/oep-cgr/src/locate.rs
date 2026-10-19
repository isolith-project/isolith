//! Measured triangle-stream record locator for real CGR material (P2-M7-T01).
//!
//! Fresh pairing attempt over the real corpus with the P2-M1-T03 table-of-
//! blocks walker (`blocks`) and the P1-M2-T06 diff-harness lineage. This
//! module encodes the **observed polygon-soup record grammar** found on the
//! 2007-era `piston.cgr` sample and LOCATES the triangle-stream records with
//! exact, measured numbers.
//!
//! ## The observed record grammar (measured, not guessed)
//!
//! On the piston family the payload carries a sequence of records of the form
//!
//! ```text
//!   u16 BE  N                      face count        (2..=500)
//!   N x     u16 BE per-face counts (3..=16)          vertex count per face
//!   S x     u16 BE handles         (S = sum(counts)) 3-encoded: h % 3 == 0,
//!                                                     true vertex index = h / 3
//! ```
//!
//! Every handle in every located piston record is a multiple of 3 — the
//! writer stores `3 * vertex_index` (a byte-offset form). The per-face fan
//! triangle count for a face with `c` vertices triangulates to `c - 2`, so the
//! record's triangle count is `sum(c - 2)` and its index count is `sum(c)`
//! (the handle stream length). All of these are OBSERVED reference counts,
//! verified by two independent readers (this Rust module and the Python
//! reference reader `python/oep_tooling/cgr_locate_reference.py`).
//!
//! ## Honesty boundary (no fabricated PASS)
//!
//! Locating and counting a triangle-stream record is NOT the same as latching
//! a spatial mesh: the record's coordinate pool (the `h/3` indexed vertices)
//! is **not** contiguous f32 on this corpus — measured: piston has no f32
//! triple lattice covering the required index span, and the R19-era cfysa
//! family does not even carry this counts/handles signature (0 records, their
//! payload uses a different family). The module therefore reports the measured
//! record table plus per-file classification, and never fabricates a spatial
//! mesh. It never panics (bounds-checked), per the P2-M1-T04 tolerant-failure
//! doctrine.
//!
//! Clean-room: every constant is an observed fact from the real corpus read
//! by pure-stdlib probes in this repo; no vendor library read any CGR, no
//! proprietary bytes are committed (samples are dev-only reference material).

use crate::blocks;

/// Minimum handle count for a located record (bounds the scan against
/// accidental short patterns; measured piston records all exceed this).
pub const MIN_HANDLES: usize = 30;

/// Maximum accepted face count per record.
pub const MAX_FACES: u16 = 500;

/// One located triangle-stream record with its measured counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateRecord {
    /// Absolute file offset of the `u16 BE` face-count word.
    pub offset: usize,
    /// Measured per-face vertex counts (`N` values, all in 3..=16).
    pub counts: Vec<u16>,
    /// Number of faces (`counts.len()`).
    pub face_count: usize,
    /// Measured index count `S` — equals the handle-stream length and
    /// `sum(counts)`. This is the record's index-count reference.
    pub index_count: usize,
    /// Number of distinct vertex indices (`h / 3`) referenced.
    pub vertex_count: usize,
    /// Triangle count from fan triangulation `sum(c - 2)`.
    pub triangle_count: usize,
    /// Largest referenced vertex index (`max(h / 3)`).
    pub max_index: usize,
}

/// Classified result of the real-material locate sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateOutcome {
    /// One or more triangle-stream records were located (offsets + counts).
    RecordsLocated(Vec<LocateRecord>),
    /// The bytes are not a CGR container under the observed header spine.
    NotCgrPreamble,
    /// The bytes are a CGR but carry no self-consistent counts/handles
    /// record under the observed grammar (e.g. the cfysa R19-era family).
    NoSelfConsistentRecord,
}

fn be_u16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

/// Scan `bytes` for the observed polygon-soup triangle-stream records.
///
/// Mirrors the Python reference reader byte-for-byte: walk from the measured
/// payload start in 2-byte steps; at each offset try `[N][counts][handles]`
/// with all invariants and `S >= MIN_HANDLES`; on a match skip past the
/// record. Returns a classified outcome; never panics for any input.
pub fn locate_triangle_streams(bytes: &[u8]) -> LocateOutcome {
    let start = match blocks::anchor_start(bytes) {
        Ok(s) => s,
        Err(_) => return LocateOutcome::NotCgrPreamble,
    };
    let n = bytes.len();
    let mut records = Vec::new();
    let mut i = start;
    while i + 4 < n {
        let face_hdr = be_u16(bytes, i);
        if (2..=MAX_FACES).contains(&face_hdr) && i + 2 + 2 * face_hdr as usize + 4 < n {
            let mut counts = Vec::with_capacity(face_hdr as usize);
            let mut ok = true;
            for k in 0..face_hdr as usize {
                let c = be_u16(bytes, i + 2 + 2 * k);
                if !(3..=16).contains(&c) {
                    ok = false;
                    break;
                }
                counts.push(c);
            }
            if ok {
                let s: usize = counts.iter().map(|&c| c as usize).sum();
                let ho = i + 2 + 2 * face_hdr as usize;
                if s >= MIN_HANDLES && ho + 2 * s <= n {
                    let mut handles_ok = true;
                    let mut max_idx = 0usize;
                    for k in 0..s {
                        let h = be_u16(bytes, ho + 2 * k);
                        if !h.is_multiple_of(3) || h / 3 >= 0x8000 {
                            handles_ok = false;
                            break;
                        }
                        if (h / 3) as usize > max_idx {
                            max_idx = (h / 3) as usize;
                        }
                    }
                    if handles_ok {
                        let mut seen = std::collections::HashSet::new();
                        for k in 0..s {
                            let h = be_u16(bytes, ho + 2 * k);
                            seen.insert(h / 3);
                        }
                        let triangle_count: usize =
                            counts.iter().map(|&c| (c as usize).saturating_sub(2)).sum();
                        records.push(LocateRecord {
                            offset: i,
                            counts: counts.clone(),
                            face_count: counts.len(),
                            index_count: s,
                            vertex_count: seen.len(),
                            triangle_count,
                            max_index: max_idx,
                        });
                        i = ho + 2 * s;
                        continue;
                    }
                }
            }
        }
        i += 2;
    }
    if records.is_empty() {
        LocateOutcome::NoSelfConsistentRecord
    } else {
        LocateOutcome::RecordsLocated(records)
    }
}
