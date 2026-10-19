//! P2-M6-T02 — Performance pass: streaming parse for large assemblies
//! (100k+ parts).
//!
//! A TRUE streaming record walker: it reads the depth-first little-endian
//! `u32 tag + u32 len + payload` framing incrementally from any
//! [`std::io::Read`] source with **constant, bounded memory** — the only
//! storage is an explicit depth stack plus one scratch buffer, so a large
//! assembly (100k+ part records, hundreds of megabytes) parses in O(depth)
//! resident memory instead of requiring the whole file in RAM. It never
//! materializes a record payload and never recurses.
//!
//! This is the streaming counterpart of the P2-M6-T01 slice-based walker
//! (`crates/oep-cgr/src/structure.rs`, branch `task/fuzz-round2-1015a`, not
//! yet on main at the time of writing — the board's open-PR pattern). The
//! outcome taxonomy and the node/depth/length/budget semantics mirror that
//! walker exactly so the two are interchangeable on identical bytes; see the
//! RE note `docs/re-notes/oep-cgr-streaming-parse.md`.
//!
//! Clean-room: only the PUBLIC observed framing facts are used (8-byte LE
//! record header `tag u32 + len u32`, depth-first implicit-length payloads,
//! per P1-M2-T01 hex notebook). No proprietary CGR bytes are read, written
//! or committed here; real material is consumed read-only through
//! `OEP_CGR_SAMPLES` in the acceptance test.

/// Maximum record nesting depth, mirroring `structure::MAX_DEPTH`. Deeper
/// input yields `Outcome::DepthLimitExceeded` and the walk is iterative, so
/// structure depth cannot smash the call stack.
pub const MAX_DEPTH: usize = 256;

/// Maximum number of records visited per walk, mirroring
/// `structure::MAX_BLOCKS`. Exceeding it yields `Outcome::BlockBudgetExceeded`,
/// bounding CPU even for pathological generators.
pub const MAX_BLOCKS: u64 = 1_000_000;

/// Maximum accepted `len` field per record, mirroring
/// `structure::MAX_BLOCK_LEN`. A larger declared length yields
/// `Outcome::LengthLimitExceeded` and is never materialized.
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

/// Classified result of a stream walk. The classification is total: every
/// byte stream maps to exactly one `Outcome` (or a panic — which is exactly
/// what the acceptance test asserts never happens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The walk consumed the stream to a clean end (possibly with a root
    /// trailing residue < 8 bytes reported on [`ProbeReport`]; real CGR
    /// files carry 0x1000-aligned zero pad at the tail).
    Ok,
    /// A nested payload or a record header required bytes past the end of
    /// the underlying stream (truncated file/tail). `offset` is the stream
    /// position where the walk ran out of bytes.
    Truncated {
        /// One past the last stream byte the walker could consume.
        offset: u64,
    },
    /// A record declared a `len` larger than `Limits::max_block_len`, so the
    /// walker refused it instead of materializing the payload.
    LengthLimitExceeded {
        /// Stream offset immediately after that record's 8-byte header.
        offset: u64,
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
    /// Short stable label used by campaign/report aggregation.
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

/// Result of one stream walk: classified outcome plus structural counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    /// The classified outcome of the walk.
    pub outcome: Outcome,
    /// Number of records successfully entered during the walk.
    pub nodes: u64,
    /// Deepest record depth entered during the walk.
    pub max_depth_reached: usize,
    /// Bytes of root-level trailing residue (< 8) when the outcome is `Ok`.
    /// Real CGR tail padding of 0..7 residue bytes is legal.
    pub trailing_residue: usize,
}

/// One open container scope on the explicit depth stack. `remaining` is the
/// number of payload bytes still to consume inside this scope (`None` for the
/// unbounded root container, which reaches its end at stream EOF).
#[derive(Debug, Clone, Copy)]
struct Scope {
    /// Bytes of payload remaining in this container, or `None` for root.
    remaining: Option<u64>,
    /// Container depth: records inside this scope sit one level deeper.
    depth: usize,
    /// Node count before the record that opened this scope was entered —
    /// used to roll the count back when a truncated tail is discovered
    /// inside it, keeping node parity with the slice-based walker.
    nodes_before: u64,
    /// `max_depth_reached` before the record that opened this scope was
    /// entered — restored alongside `nodes_before` on truncation so a rolled
    /// back subtree never inflates the reported depth.
    depth_before: usize,
}

/// Read `buf.len()` bytes from `reader`, using `scratch` as the physical read
/// buffer, or as many as are available before physical EOF. Returns the
/// number of bytes actually read (0..=buf.len()).
fn read_into<R: std::io::Read>(
    reader: &mut R,
    buf: &mut [u8],
    scratch: &mut [u8; RECORD_HEADER],
) -> usize {
    let mut filled = 0usize;
    while filled < buf.len() {
        let want = (buf.len() - filled).min(scratch.len());
        let mut got = 0usize;
        while got < want {
            match reader.read(&mut scratch[got..want]) {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(_) => break, // IO error treated as EOF: never panic
            }
        }
        if got == 0 {
            break;
        }
        buf[filled..filled + got].copy_from_slice(&scratch[..got]);
        filled += got;
        if got < want {
            break; // physical EOF
        }
    }
    filled
}

/// Walk `reader` as the depth-first little-endian record framing described at
/// the top of this module, bounded by `limits`, invoking `visit(tag, len,
/// offset)` for every record entered. `visit` returning `false` stops the
/// walk early (outcome [`Outcome::Ok`] with the nodes visited so far).
///
/// Streaming guarantees:
/// - Memory is O(depth): an explicit `Scope` stack (bounded by
///   `Limits::max_depth`) plus one `RECORD_HEADER`-byte scratch buffer. It
///   never requests the whole stream and never materializes a payload.
/// - The walk never panics for any input; classification is total.
///
/// Node-count parity with the slice-based walker on identical bytes: a
/// record is only counted once its payload has been fully consumed without
/// truncation, so a file cut mid-payload reports the same node count the
/// slice walker would have.
pub fn walk_stream<R: std::io::Read>(
    reader: &mut R,
    limits: &Limits,
    mut visit: impl FnMut(u32, u32, u64) -> bool,
) -> ProbeReport {
    let mut report = ProbeReport {
        outcome: Outcome::Ok,
        nodes: 0,
        max_depth_reached: 0,
        trailing_residue: 0,
    };
    let mut stack: Vec<Scope> = vec![Scope {
        remaining: None,
        depth: 0,
        nodes_before: 0,
        depth_before: 0,
    }];
    let mut scratch = [0u8; RECORD_HEADER];
    let mut offset: u64 = 0; // total stream bytes consumed so far

    while let Some(&scope) = stack.last() {
        // If this nested scope has fewer than a header's worth of bytes
        // left, consume the slack and pop back to its parent.
        if let Some(rem) = scope.remaining {
            if rem < RECORD_HEADER as u64 {
                if rem > 0 {
                    let mut small = [0u8; RECORD_HEADER];
                    let got = read_into(reader, &mut small[..rem as usize], &mut scratch);
                    offset += got as u64;
                    if got < rem as usize {
                        // Physical EOF inside a declared payload: truncated.
                        report.nodes = scope.nodes_before;
                        report.max_depth_reached = scope.depth_before;
                        report.outcome = Outcome::Truncated { offset };
                        return report;
                    }
                }
                stack.pop();
                continue;
            }
        }

        // Read one 8-byte record header within this scope.
        let mut hdr = [0u8; RECORD_HEADER];
        let header_offset = offset;
        let got = read_into(reader, &mut hdr, &mut scratch);
        offset += got as u64;

        if got < RECORD_HEADER {
            if scope.remaining.is_none() {
                // Root ended with 0..7 residue bytes: clean end (legal pad).
                report.trailing_residue = got;
                report.outcome = Outcome::Ok;
                return report;
            }
            // Nested scope promised >= 8 bytes but the stream ran out:
            // truncated tail; roll back this scope's records.
            report.nodes = scope.nodes_before;
            report.max_depth_reached = scope.depth_before;
            report.outcome = Outcome::Truncated { offset };
            return report;
        }

        let tag = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
        let record_depth = scope.depth + 1;

        if record_depth > limits.max_depth {
            report.outcome = Outcome::DepthLimitExceeded {
                depth: record_depth,
            };
            return report;
        }
        if len > limits.max_block_len {
            report.outcome = Outcome::LengthLimitExceeded {
                offset: header_offset + RECORD_HEADER as u64,
                declared: u64::from(len),
            };
            return report;
        }

        // Nested-scope length fit: a child payload must not overrun its
        // container's remaining budget.
        let parent_remaining = match scope.remaining {
            Some(r) => {
                let after_header = r - RECORD_HEADER as u64;
                if u64::from(len) > after_header {
                    report.outcome = Outcome::Truncated {
                        offset: header_offset + RECORD_HEADER as u64,
                    };
                    return report;
                }
                Some(after_header - u64::from(len))
            }
            None => None,
        };

        if report.nodes >= limits.max_blocks {
            report.outcome = Outcome::BlockBudgetExceeded {
                nodes: report.nodes,
            };
            return report;
        }

        // Enter the record.
        let depth_before_record = report.max_depth_reached;
        report.nodes += 1;
        report.max_depth_reached = report.max_depth_reached.max(record_depth);
        if !visit(tag, len, header_offset) {
            return report; // early stop, outcome stays Ok
        }

        // Consume the header's 8 bytes from this scope, plus the payload
        // when non-empty, then (for a non-empty payload) descend into it as
        // a nested scope.
        if let Some(after) = parent_remaining {
            stack.last_mut().unwrap().remaining = Some(after);
        }
        if len > 0 {
            stack.push(Scope {
                remaining: Some(u64::from(len)),
                depth: record_depth,
                nodes_before: report.nodes - 1,
                depth_before: depth_before_record,
            });
        }
    }

    // Root scope popped with no bytes left and no EOF yet read: clean end.
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_ok() {
        let mut r = std::io::empty();
        let rep = walk_stream(&mut r, &Limits::default(), |_t, _l, _o| true);
        assert_eq!(rep.outcome, Outcome::Ok);
        assert_eq!(rep.nodes, 0);
        assert_eq!(rep.trailing_residue, 0);
    }

    #[test]
    fn short_residue_is_ok() {
        let mut r: &[u8] = &[1u8, 2, 3];
        let rep = walk_stream(&mut r, &Limits::default(), |_t, _l, _o| true);
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

    #[test]
    fn flat_record_stream_counts() {
        let mut bytes = Vec::new();
        for i in 0..5u32 {
            bytes.extend_from_slice(&i.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        let mut r: &[u8] = &bytes;
        let rep = walk_stream(&mut r, &Limits::default(), |_t, _l, _o| true);
        assert_eq!(rep.outcome, Outcome::Ok);
        assert_eq!(rep.nodes, 5);
    }

    #[test]
    fn visitor_can_stop_early() {
        let bytes = synthetic_flat(100);
        let mut r: &[u8] = &bytes;
        let mut seen = 0u64;
        let rep = walk_stream(&mut r, &Limits::default(), |_t, _l, _o| {
            seen += 1;
            seen < 10
        });
        assert_eq!(rep.nodes, 10);
        assert_eq!(seen, 10);
        assert_eq!(rep.outcome, Outcome::Ok);
    }

    fn synthetic_flat(n: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(n * 8);
        for i in 0..n as u32 {
            bytes.extend_from_slice(&(0x0A00_0000 + i).to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        bytes
    }
}
