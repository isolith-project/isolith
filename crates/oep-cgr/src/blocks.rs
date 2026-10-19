//! Central table-of-blocks walker for the CGR container (P2-M1-T03).
//!
//! This is the *central* structural walker of the `oep-cgr` crate: it takes
//! raw CGR bytes, validates the OBSERVED header spine (the measured anchor
//! locked by P2-M1-T01), and walks the payload as the depth-first record
//! framing documented in the P1-M2-T01 hex notebook section 7. It emits the
//! ordered table of blocks it enters — each block's absolute offset, raw tag,
//! declared length, and nesting depth — so downstream P2 cards (mesh
//! extraction P2-M2-T01, topology P2-M3-T02, material P2-M4-T01) can consume
//! the structure instead of rescanning bytes.
//!
//! ## Observed structure vs hypothesis — honesty note
//!
//! Everything in this module is derived from lawful observation of real
//! material (the eleven-sample P1 corpus, cross-checked on 2026-10-15); no
//! Dassault spec or library is used. The fixed FACTS are:
//!
//! - magic `01 00 00 00` (u32 LE == 1) at 0x00, 96 zero bytes 0x04..0x63, a
//!   16-byte invariant template at 0x68..0x77 — identical in all 11 samples;
//! - file sizes 0x1000-aligned with zero tail padding;
//! - little-endian scalar encoding.
//!
//! The block *framing* is a HYPOTHESIS marked `[uncertain]` by the hex
//! notebook: the payload is a depth-first serialization of the rep graph with
//! implicit lengths — `u32 LE tag + u32 LE len + payload`, records nested
//! inside payloads — and NO forward block table was found in the first ~0x200
//! bytes of any sample (an all-offset forward-table scan for this card found
//! zero clean candidates on VALVE/piston). This walker implements exactly that
//! observed framing, but the contract it guarantees is STRUCTURAL SAFETY, not
//! semantics: for any byte string it returns a classified [`WalkReport`], never
//! panics, never allocates a declared payload, and refuses runaway lengths,
//! depths and node counts. When real material does not decompose cleanly under
//! the framing (as the old-era corpus often does not — see the P2-M1-T03 RE
//! note transcript), the walker reports the classified stop rather than
//! fabricating blocks. Tag VALUES carry no meaning here (no tag catalog exists
//! yet); interpreting tags is P2-M1-T02/T04's lane.

/// CGR magic, u32 little-endian == 1, at file offset 0x00 (observed 11/11).
pub const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// Byte offset of the magic field.
pub const MAGIC_OFF: usize = 0x00;

/// Byte offset where the fixed zero preamble begins.
pub const PREAMBLE_OFF: usize = 0x04;

/// Length in bytes of the zero preamble (0x04..=0x63 all zero).
pub const PREAMBLE_LEN: usize = 0x60;

/// Byte offset of the per-sample landscape u32. Semantics `[uncertain]`.
pub const LANDSCAPE_OFF: usize = 0x64;

/// Byte offset of the 16-byte invariant template.
pub const TEMPLATE_OFF: usize = 0x68;

/// The 16-byte invariant template observed byte-for-byte identical in every
/// real sample.
pub const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

/// Length in bytes of the observed header spine (0x00..0x78), the measured
/// payload start for the walker.
pub const HEADER_LEN: usize = TEMPLATE_OFF + TEMPLATE.len();

/// Size in bytes of one record header (`tag: u32 + len: u32`).
pub const RECORD_HEADER: usize = 8;

/// Maximum record nesting depth. Deeper input yields `DepthLimitExceeded`
/// instead of recursion; the walker is iterative (no call-stack growth).
pub const MAX_DEPTH: usize = 256;

/// Maximum number of records visited per walk. Exceeding it yields
/// `BlockBudgetExceeded`, bounding CPU even for pathological generators.
pub const MAX_BLOCKS: u64 = 1_000_000;

/// Maximum accepted `len` field per record (64 MiB). A larger declared length
/// yields `LengthLimitExceeded` and is never materialized.
pub const MAX_BLOCK_LEN: u32 = 64 * 1024 * 1024;

/// A header-spine failure of the measured CGR anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlocksError {
    /// The buffer is shorter than the observed header spine.
    TooShort {
        /// Minimum bytes required for a complete spine.
        need: usize,
        /// Bytes actually supplied.
        got: usize,
    },
    /// The four magic bytes are not `01 00 00 00`.
    NotCgrPreamble,
}

/// Classified outcome of a walk. The tree is total: every byte string maps to
/// exactly one [`WalkReport`] (never a panic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The walk consumed the container and hit a clean end (possibly with a
    /// sub-header trailing residue reported on the [`WalkReport`]).
    Ok,
    /// A field or payload read required bytes past the end of the buffer.
    /// `offset` is one past the last byte a complete parse would need.
    Truncated {
        /// One past the last index the parser needed.
        offset: usize,
    },
    /// A record declared a `len` larger than [`Limits::max_block_len`], so the
    /// walker refused it instead of materializing the payload.
    LengthLimitExceeded {
        /// Offset of the offending payload (immediately after its header).
        offset: usize,
        /// The declared (and refused) length.
        declared: u64,
    },
    /// A record would occupy nesting depth deeper than [`Limits::max_depth`].
    DepthLimitExceeded {
        /// The depth that was refused (one past the cap).
        depth: usize,
    },
    /// The walk visited more than [`Limits::max_blocks`] records.
    BlockBudgetExceeded {
        /// Number of records visited when the budget tripped.
        nodes: u64,
    },
}

/// One block (record) of the observed framing, as entered by the walker.
///
/// The (offset, tag, len, depth) quad is the "table of blocks" — the ordered
/// map downstream P2 cards consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// Absolute file offset of the record's 8-byte header (`tag + len`).
    pub offset: usize,
    /// The raw u32 LE tag word read at `offset`. Structural only — no meaning
    /// is attached here (tag catalog is P2-M1-T02's lane).
    pub tag: u32,
    /// The raw u32 LE `len` word read from the record header.
    pub len: u32,
    /// Nesting depth of this record (root container records are depth 1).
    pub depth: usize,
}

/// Tunable walker limits. `Default` is the pinned invariant set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum record nesting depth.
    pub max_depth: usize,
    /// Maximum records visited.
    pub max_blocks: u64,
    /// Maximum accepted per-record `len`.
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

/// Result of one walk: the classified outcome plus the emitted block table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkReport {
    /// The classified outcome of the walk.
    pub outcome: Outcome,
    /// The ordered table of blocks entered during the walk.
    pub blocks: Vec<Block>,
    /// Deepest record depth entered during the walk.
    pub max_depth_reached: usize,
    /// Bytes left at the very end of the buffer after the last complete
    /// record when the walk is `Ok` and a container ended short of a full
    /// header. Real CGR tail padding of 0..7 residue bytes is legal.
    pub trailing_residue: usize,
}

/// Validate the observed CGR header spine (magic, zero preamble, invariant
/// template) and return the measured payload start (`HEADER_LEN`).
///
/// Returns a typed [`BlocksError`] for a truncated buffer or a wrong magic;
/// never panics. This is the anchor that ties the walker to measured bytes
/// rather than to an arbitrary offset.
pub fn anchor_start(bytes: &[u8]) -> Result<usize, BlocksError> {
    if bytes.len() < HEADER_LEN {
        return Err(BlocksError::TooShort {
            need: HEADER_LEN,
            got: bytes.len(),
        });
    }
    if bytes[MAGIC_OFF..MAGIC_OFF + 4] != MAGIC {
        return Err(BlocksError::NotCgrPreamble);
    }
    if bytes[PREAMBLE_OFF..PREAMBLE_OFF + PREAMBLE_LEN] != [0u8; PREAMBLE_LEN] {
        return Err(BlocksError::NotCgrPreamble);
    }
    if bytes[TEMPLATE_OFF..TEMPLATE_OFF + TEMPLATE.len()] != TEMPLATE {
        return Err(BlocksError::NotCgrPreamble);
    }
    Ok(HEADER_LEN)
}

/// Walk `bytes` starting at `start` as the depth-first little-endian record
/// framing described at the top of this module, bounded by `limits`.
///
/// Returns a [`WalkReport`]; never panics; never allocates a record payload;
/// never recurses (explicit stack). The emitted [`Block`] table is the central
/// deliverable — an ordered, offset-precise map of what the framing observed.
pub fn walk_blocks(bytes: &[u8], start: usize, limits: &Limits) -> WalkReport {
    let mut report = WalkReport {
        outcome: Outcome::Ok,
        blocks: Vec::new(),
        max_depth_reached: 0,
        trailing_residue: 0,
    };

    // Explicit stack of containers: (start, end, container_depth). Records
    // inside a container sit one level deeper than the container; the root
    // container is the caller's `start..len`. The walker is iterative so
    // structure depth cannot smash the call stack.
    let mut stack: Vec<(usize, usize, usize)> = Vec::new();
    if start < bytes.len() {
        stack.push((start, bytes.len(), 0));
    }

    let mut nodes: u64 = 0;
    while let Some((s, e, container_depth)) = stack.pop() {
        let mut cursor = s;
        while e - cursor >= RECORD_HEADER {
            let tag = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            let declared = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
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
                    Some(p) if p <= e => p,
                    _ => {
                        // Payload would run past the container end: the container
                        // is a truncated tail.
                        report.outcome = Outcome::Truncated {
                            offset: e.min(payload_offset),
                        };
                        return report;
                    }
                };
            // Hard node budget: refuse the record that would exceed the cap so
            // the emitted table NEVER grows past `Limits::max_blocks`.
            if nodes >= limits.max_blocks {
                report.outcome = Outcome::BlockBudgetExceeded { nodes };
                return report;
            }
            nodes += 1;
            report.max_depth_reached = report.max_depth_reached.max(record_depth);
            report.blocks.push(Block {
                offset: cursor,
                tag,
                len: declared,
                depth: record_depth,
            });
            if payload_end > payload_offset {
                // Descend: the payload is itself a container of records.
                stack.push((payload_offset, payload_end, record_depth));
            }
            cursor = payload_end;
        }
        // Residual (< 8 bytes) inside a nested container is legal payload
        // slack; only the root container's residue is reported.
        if stack.is_empty() {
            report.trailing_residue = e - cursor;
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_len_is_measured_payload_start() {
        assert_eq!(HEADER_LEN, 0x78);
        assert_eq!(TEMPLATE_OFF + TEMPLATE.len(), 0x78);
    }

    #[test]
    fn limits_defaults_match_consts() {
        let l = Limits::default();
        assert_eq!(l.max_depth, MAX_DEPTH);
        assert_eq!(l.max_blocks, MAX_BLOCKS);
        assert_eq!(l.max_block_len, MAX_BLOCK_LEN);
    }
}
