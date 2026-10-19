//! Tolerant, bounds-checked, depth-limited CGR record walker (P2-M6-T01).
//!
//! The subject under fuzz for campaign round 2 (structure depth, huge files,
//! truncated tails). This is a structural SAFETY probe, not a semantic
//! parser:
//!
//! - It models the CGR payload framing HYPOTHESIS from the P1-M2-T01 hex
//!   notebook — little-endian `u32 tag + u32 len + payload`, depth-first,
//!   with records nested inside record payloads. The notebook explicitly
//!   found NO forward block table and hypothesized an implicit-length,
//!   depth-first serialization; this walker is the `[campaign probe]` that
//!   exercises exactly that shape. Tag VALUES carry no meaning here (no tag
//!   catalog exists yet); the module guarantees ONLY structural safety.
//!   P2-M1-T03 will anchor this framing to measured bytes.
//! - Invariants (the fuzz contract): for ANY byte string the walker returns
//!   a classified `Outcome`, is memory-bounded (slice-only, it never
//!   allocates a record payload), depth-capped (`MAX_DEPTH`), node-capped
//!   (`MAX_BLOCKS`), and length-capped per record (`MAX_BLOCK_LEN`). Record
//!   payloads larger than the cap or past end-of-input are rejected, never
//!   allocated. Deep nesting is walked with an explicit stack, so structure
//!   depth cannot smash the call stack.
//! - Truncated tails: the walker never reads past the buffer; a truncated
//!   field/payload is a typed `Truncated`, and a container ending in fewer
//!   than 8 residue bytes is `Ok` with a `trailing_residue` count (real CGR
//!   files carry 0x1000-aligned zero pad at the tail).
//!
//! Every constant here is defined as a documented default and can be
//! overridden through [`Limits`] so the campaign can probe each boundary.

/// Maximum record nesting depth. Deeper input yields `DepthLimitExceeded`
/// instead of recursion, and the walker is iterative (no call-stack growth).
pub const MAX_DEPTH: usize = 256;

/// Maximum number of records visited per walk. Exceeding it yields
/// `BlockBudgetExceeded`, bounding CPU even for pathological generators.
pub const MAX_BLOCKS: u64 = 1_000_000;

/// Maximum accepted `len` field per record (64 MiB). A larger declared
/// length yields `LengthLimitExceeded` and is never materialized.
pub const MAX_BLOCK_LEN: u32 = 64 * 1024 * 1024;

/// Size in bytes of one record header (`tag: u32 + len: u32`).
pub const RECORD_HEADER: usize = 8;

/// Tunable walker limits. `Default` is the campaign-pinned invariant set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum record nesting depth (see [`MAX_DEPTH`]).
    pub max_depth: usize,
    /// Maximum records visited (see [`MAX_BLOCKS`]).
    pub max_blocks: u64,
    /// Maximum accepted per-record `len` (see [`MAX_BLOCK_LEN`]).
    pub max_block_len: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: MAX_DEPTH,
            max_blocks: MAX_BLOCKS,
            max_block_len: MAX_BLOCK_LEN,
        }
    }
}

/// Classified result of a walk. The tree is total: every byte string maps to
/// exactly one `Outcome` (or a panic — which is exactly what the campaign
/// asserts never happens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The walk consumed the buffer and hit a clean end (possibly with a
    /// sub-header trailing residue reported on the `ProbeReport`).
    Ok,
    /// A field or payload read required bytes past the end of the buffer.
    /// `offset` is the end-of-walk position (one past the last byte that a
    /// complete parse would need).
    Truncated {
        /// One past the last index the parser needed.
        offset: usize,
    },
    /// A record declared a `len` larger than `Limits::max_block_len`, so the
    /// walker refused it instead of materializing the payload.
    LengthLimitExceeded {
        /// Offset of the offending payload (immediately after its 8-byte
        /// header).
        offset: usize,
        /// The declared (and refused) length.
        declared: u64,
    },
    /// A record would occupy nesting depth deeper than `Limits::max_depth`.
    DepthLimitExceeded {
        /// The depth that was refused (one past the cap).
        depth: usize,
    },
    /// The walk visited more than `Limits::max_blocks` records. `nodes` is
    /// the count that triggered the stop.
    BlockBudgetExceeded {
        /// Number of records visited when the budget tripped.
        nodes: u64,
    },
}

impl Outcome {
    /// Short stable label used by the campaign report aggregation.
    pub fn kind(&self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Truncated { .. } => "truncated",
            Outcome::LengthLimitExceeded { .. } => "length_limit",
            Outcome::DepthLimitExceeded { .. } => "depth_limit",
            Outcome::BlockBudgetExceeded { .. } => "block_budget",
        }
    }
}

/// Result of one walk: classified outcome plus the structural counters the
/// campaign reports on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    /// The classified outcome of the walk.
    pub outcome: Outcome,
    /// Number of records successfully entered (header read, budget ok).
    pub nodes: u64,
    /// Deepest record depth entered during the walk.
    pub max_depth_reached: usize,
    /// Bytes left at the very end of the buffer after the last complete
    /// record when the walk is `Ok` and the root container ended short of a
    /// full header. Real CGR tail padding of 0..7 residue bytes is legal.
    pub trailing_residue: usize,
}

/// Walk `bytes` as the depth-first little-endian record framing described at
/// the top of this module, bounded by `limits`. Returns a [`ProbeReport`];
/// never panics; never allocates a record payload; never recurses.
///
/// This is the fuzz-campaign entry point for round 2. Callers that want the
/// pinned invariant limits use `&Limits::default()`.
pub fn walk_probe(bytes: &[u8], limits: &Limits) -> ProbeReport {
    let mut report = ProbeReport {
        outcome: Outcome::Ok,
        nodes: 0,
        max_depth_reached: 0,
        trailing_residue: 0,
    };

    // Explicit stack of containers: (start, end, container_depth) byte ranges
    // to scan for sibling records. Records inside a container sit one level
    // deeper than the container; the root buffer is container depth 0. Depth
    // is stored on the frame (not derived from stack length) so every
    // sibling payload is counted at its true depth.
    let mut stack: Vec<(usize, usize, usize)> = Vec::new();
    stack.push((0, bytes.len(), 0));

    while let Some((start, end, container_depth)) = stack.pop() {
        let mut cursor = start;
        while end - cursor >= RECORD_HEADER {
            // A record header fits: tag (ignored structurally), then len.
            let payload_offset = match cursor.checked_add(RECORD_HEADER) {
                Some(v) => v,
                None => {
                    report.outcome = Outcome::LengthLimitExceeded {
                        offset: cursor,
                        declared: u64::MAX,
                    };
                    return report;
                }
            };
            let declared = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
            let record_depth = container_depth + 1;
            if record_depth > limits.max_depth {
                report.outcome = Outcome::DepthLimitExceeded {
                    depth: record_depth,
                };
                return report;
            }
            if declared > limits.max_block_len {
                report.outcome = Outcome::LengthLimitExceeded {
                    offset: payload_offset,
                    declared: u64::from(declared),
                };
                return report;
            }
            let payload_end =
                match payload_offset.checked_add(usize::try_from(declared).unwrap_or(usize::MAX)) {
                    Some(p) if p <= end => p,
                    _ => {
                        // Payload would run past the container end: the container
                        // is a truncated tail.
                        report.outcome = Outcome::Truncated {
                            offset: end.min(payload_offset),
                        };
                        return report;
                    }
                };
            // Hard node budget: refuse the record that would exceed the cap
            // so `nodes` NEVER exceeds `Limits::max_blocks`. This is the
            // memory-boundedness guarantee the huge-file class asserts.
            if report.nodes >= limits.max_blocks {
                report.outcome = Outcome::BlockBudgetExceeded {
                    nodes: report.nodes,
                };
                return report;
            }
            // Enter the record.
            report.nodes += 1;
            report.max_depth_reached = report.max_depth_reached.max(record_depth);
            if payload_end > payload_offset {
                // Descend: the payload is itself a container of records.
                stack.push((payload_offset, payload_end, record_depth));
            }
            cursor = payload_end;
        }
        // Residual (< 8 bytes) inside a nested container is legal payload
        // slack; only the root container's residue is reported.
        if stack.is_empty() {
            report.trailing_residue = end - cursor;
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_ok() {
        let rep = walk_probe(&[], &Limits::default());
        assert_eq!(rep.outcome, Outcome::Ok);
        assert_eq!(rep.nodes, 0);
    }

    #[test]
    fn short_residue_is_ok() {
        let rep = walk_probe(&[1u8, 2, 3], &Limits::default());
        assert_eq!(rep.outcome, Outcome::Ok);
        assert_eq!(rep.trailing_residue, 3);
    }

    #[test]
    fn default_limits_match_consts() {
        let l = Limits::default();
        assert_eq!(l.max_depth, MAX_DEPTH);
        assert_eq!(l.max_blocks, MAX_BLOCKS);
        assert_eq!(l.max_block_len, MAX_BLOCK_LEN);
    }
}
