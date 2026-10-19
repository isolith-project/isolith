//! Clean-room scanner from raw CGR bytes to an assembly/instance structure
//! (P2-M4-T02).
//!
//! A CGR file may be a *part* (one bag of mesh reps) or an *assembly* — a
//! container whose payload carries N placed instances forming a hierarchy.
//! This module is the honest bridge from real bytes to the
//! [`crate::instance`] model: it scans the payload for a self-consistent
//! *transform stream* (a run of >= 2 structurally-valid rigid 4x4 transforms)
//! and, when found, materializes one [`Instance`] per transform.
//!
//! Doctrine (P2-M1-T04 tolerant-failure + clean-room invariant): the scanner
//! returns a *classified* outcome for every input — [`AssemblyOutcome::Assembly`]
//! when a transform stream is found, [`AssemblyOutcome::NoAssembly`] with the
//! measured evidence when not, [`AssemblyOutcome::NotCgrPreamble`] when the
//! bytes are not a CGR container. It never panics, never reads out of bounds,
//! and never fabricates geometry.
//!
//! Measured facts (2026-10-15, full probe in
//! `docs/re-notes/oep-cgr-instance-transforms.md`): across the verified
//! old-era single-part corpus (11 samples, cfysa DMU set + 2007-era piston)
//! there are ZERO rigid, ZERO near-identity and ZERO rotation-only 4x4/3x4
//! transform records under any tested hypothesis — those part CGRs carry no
//! observable placement data, so real material today classifies as
//! `NoAssembly`. A larger, separately-downloaded VALVE.cgr (697 KB, magic
//! `0x55`) exposes the class strings `CAT3DBagRep` / `CATSurfacicRep` /
//! `CAT4x4Matrix` — the modern-era family that DOES store 4x4 matrices — but
//! its provenance is not in the license-verified registry, so it is recorded
//! as an `[uncertain]` lead, not yet golden material. When a licensed
//! modern-era assembly CGR lands, this same scanner reads it.

/// Observed CGR magic, u32 LE == 1, at offset 0 (P1-M2-T01 hex notebook; the
/// R18/R19-era sample family). The modern-era sample observed with magic
/// `0x55` is a different family — not claimed as CGR here.
const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
/// Bytes 0x04..0x63 are a zero preamble (observed on every verified sample).
const PREAMBLE_LEN: usize = 0x60;
/// Invariant signature observed at 0x68 on the verified family.
const TEMPLATE_OFF: usize = 0x68;
const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

/// Orthonormality tolerance for the upper-left 3x3 of a candidate transform
/// (column norms within this of 1.0, pairwise column dots within this of 0).
/// Real CAD placement matrices are exact rigid motions stored as f32, so the
/// tolerance only absorbs rounding, not slop.
const ORTHO_TOL: f32 = 1e-2;
/// Maximum |entry| accepted in a candidate transform (bounds check).
const MAX_VAL: f32 = 1e6;
/// Tolerated deviation of the last row from `[0, 0, 0, 1]` (row-major).
const LAST_ROW_TOL: f32 = 1e-3;
/// A candidate 4x4 block at offset `o` and the next one at `o2` belong to
/// the same stream when `o2 - o <= WINDOW`. Consecutive rigid transforms in
/// a real instance table are packed within 64 bytes; a loose window keeps
/// the stream test from depending on exact table stride.
const WINDOW: usize = 256;
/// The smallest number of consecutive rigid transforms that constitutes an
/// *assembly* signal (a single isolated block is plausibly byte noise; a run
/// of them is a placement table).
const MIN_STREAM: usize = 2;

use crate::instance::{Instance, Transform};

/// Classified outcome of [`scan_assembly_structure`].
#[derive(Debug, Clone)]

pub enum AssemblyOutcome {
    /// The bytes do not carry the observed CGR header spine.
    NotCgrPreamble,
    /// A self-consistent transform/instance stream was found; `instances`
    /// are the placed nodes (one per transform, in stream order) and
    /// `transforms` the corresponding local transforms.
    Assembly {
        /// Placed instances (ids 0..n in stream order, no names/meshes —
        /// CGR placement records observed to carry geometry not identifiers).
        instances: Vec<Instance>,
        /// The local transforms backing the instances.
        transforms: Vec<Transform>,
    },
    /// Classified skip: no transform stream found. `reason` names the
    /// measured evidence (bytes scanned, candidates, stream verdict).
    NoAssembly {
        /// Human-readable evidence summary.
        reason: String,
    },
}

fn le_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Validate the observed CGR header spine. Self-contained (constants above)
/// so the scanner does not depend on the unmerged P2-M1-T01 `header` module.
fn check_preamble(b: &[u8]) -> bool {
    if b.len() < TEMPLATE_OFF + TEMPLATE.len() {
        return false;
    }
    b[0..4] == MAGIC
        && b[4..4 + PREAMBLE_LEN] == [0u8; PREAMBLE_LEN]
        && b[TEMPLATE_OFF..TEMPLATE_OFF + 16] == TEMPLATE
}

/// Read a 16-f32 window at `o` (bounds-checked) as a row-major 4x4 block.
/// Returns `None` when the window is past the end of `b`.
fn read_transform(b: &[u8], o: usize) -> Option<Transform> {
    if o + 64 > b.len() {
        return None;
    }
    let vals: [[f32; 4]; 4] =
        std::array::from_fn(|r| std::array::from_fn(|c| le_f32(b, o + (r * 4 + c) * 4)));
    Transform::from_rows(vals).ok()
}

/// Is this block a structurally-valid rigid 4x4 transform?
///
/// Checks: every entry finite and bounded; the last row (indices 12..16) is
/// within [`LAST_ROW_TOL`] of `[0, 0, 0, 1]`; the upper-left 3x3 has unit
/// column norms and pairwise-orthogonal columns within [`ORTHO_TOL`].
fn is_rigid(t: &Transform) -> bool {
    let r = t.rows();
    if r.iter()
        .flatten()
        .any(|x| !x.is_finite() || x.abs() > MAX_VAL)
    {
        return false;
    }
    let last = [r[3][0], r[3][1], r[3][2], r[3][3]];
    if last[0].abs() > LAST_ROW_TOL
        || last[1].abs() > LAST_ROW_TOL
        || last[2].abs() > LAST_ROW_TOL
        || (last[3] - 1.0).abs() > LAST_ROW_TOL
    {
        return false;
    }
    // Row-major upper-left 3x3: columns are (r[0][0], r[1][0], r[2][0]) etc.
    let cols = [
        [r[0][0], r[1][0], r[2][0]],
        [r[0][1], r[1][1], r[2][1]],
        [r[0][2], r[1][2], r[2][2]],
    ];
    let norms: Vec<f32> = cols
        .iter()
        .map(|c| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt())
        .collect();
    if norms.iter().any(|n| (n - 1.0).abs() > ORTHO_TOL) {
        return false;
    }
    let dots = [
        cols[0][0] * cols[1][0] + cols[0][1] * cols[1][1] + cols[0][2] * cols[1][2],
        cols[0][0] * cols[2][0] + cols[0][1] * cols[2][1] + cols[0][2] * cols[2][2],
        cols[1][0] * cols[2][0] + cols[1][1] * cols[2][1] + cols[1][2] * cols[2][2],
    ];
    dots.iter().all(|d| d.abs() <= ORTHO_TOL)
}

/// Scan raw CGR-like bytes for a self-consistent transform/instance stream
/// and classify the outcome. Never panics; always returns one of the three
/// [`AssemblyOutcome`] variants.
///
/// Walk: starting after the 0x80 header region, visit every 4-byte-aligned
/// offset; a window that parses and is rigid marks a candidate. Candidates
/// closer than [`WINDOW`] bytes form a stream; the FIRST stream of
/// [`MIN_STREAM`]+ candidates wins and materializes instances. When no
/// stream qualifies, a `NoAssembly` reason summarizes the measured counts.
pub fn scan_assembly_structure(bytes: &[u8]) -> AssemblyOutcome {
    if !check_preamble(bytes) {
        return AssemblyOutcome::NotCgrPreamble;
    }

    let mut candidates: Vec<usize> = Vec::new();
    let mut o = 0;
    while o + 64 <= bytes.len() {
        if let Some(t) = read_transform(bytes, o) {
            if is_rigid(&t) {
                candidates.push(o);
            }
        }
        o += 4;
    }

    // Stream detection: group candidate offsets that are within WINDOW.
    let mut best: Vec<usize> = Vec::new();
    for &c in &candidates {
        if let Some(last) = best.last() {
            if c - *last <= WINDOW {
                best.push(c);
                continue;
            }
        }
        if best.len() >= MIN_STREAM {
            break; // first qualifying stream wins
        }
        best.clear();
        best.push(c);
    }
    if best.len() >= MIN_STREAM {
        let mut instances = Vec::new();
        let mut transforms = Vec::new();
        for (i, &o) in best.iter().enumerate() {
            let t = read_transform(bytes, o).expect("candidate re-reads");
            let mut inst = Instance::new(i as u32, None);
            inst.set_local(t);
            instances.push(inst);
            transforms.push(t);
        }
        return AssemblyOutcome::Assembly {
            instances,
            transforms,
        };
    }

    let reason = format!(
        "scanned {} bytes: {} rigid candidate(s), longest run {} < {MIN_STREAM} (old-era single-part CGRs carry no observable transform records; see RE note)",
        bytes.len(),
        candidates.len(),
        best.len(),
    );
    AssemblyOutcome::NoAssembly { reason }
}
